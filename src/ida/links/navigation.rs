//! Find local databases and navigate existing or newly launched IDA instances.

use std::path::{Path, PathBuf};

use crate::error::{Error, Result};
use crate::ida::ipc;

fn base_name(path: &Path) -> String {
    let name = path.file_name().unwrap_or_default().to_string_lossy().to_lowercase();
    name.strip_suffix(".i64").or_else(|| name.strip_suffix(".idb")).unwrap_or(&name).to_owned()
}

pub(super) struct Target {
    pub uri: Option<String>,
    pub name: String,
    pub path: Option<PathBuf>,
    pub exact_path_match: bool,
}

pub(super) struct LaunchOptions {
    pub no_launch: bool,
    pub timeout: f64,
    pub skip_analysis: bool,
}

impl Target {
    async fn navigate(&self, instance: &ipc::Instance) -> Result<()> {
        if let Some(uri) = &self.uri {
            ipc::navigate(instance, uri).await?;
        } else {
            let database = self
                .path
                .as_ref()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| self.name.clone());
            println!("Opening {database} with IDA");
        }
        Ok(())
    }
}

pub(super) async fn navigate_relative(uri: &str, instances: Vec<ipc::Instance>) -> Result<()> {
    let mut loaded = instances.iter().filter(|instance| instance.idb_path.is_some());
    let Some(instance) = loaded.next() else {
        return Err(Error::NotFound("No running IDA instances with an IDB loaded".into()));
    };
    if loaded.next().is_some() {
        return Err(Error::Other(
            "Multiple IDA instances running; specify an IDB name in the URL".into(),
        ));
    }
    ipc::navigate(instance, uri).await
}

pub(super) async fn navigate_to_database(target: Target, options: LaunchOptions) -> Result<()> {
    let matches = |instance: &ipc::Instance, name: &str| {
        instance.idb_path.as_ref().is_some_and(|p| {
            if target.exact_path_match {
                target.path.as_deref().is_some_and(|expected| same_database(expected, p))
            } else {
                base_name(p) == base_name(Path::new(name))
            }
        })
    };
    if let Some(instance) = ipc::find(|instance| matches(instance, &target.name)).await {
        return target.navigate(&instance).await;
    }
    if options.no_launch {
        return Err(Error::NotFound(format!("running IDA instance for {}", target.name)));
    }
    let launch_path =
        target.path.as_ref().ok_or_else(|| Error::NotFound(format!("IDB {}", target.name)))?;
    let instance = launch(launch_path, &options, matches)
        .await
        .map_err(|error| Error::IdaLaunch(Box::new(error)))?;
    if let Some(instance) = instance {
        target.navigate(&instance).await?;
    }
    Ok(())
}

async fn launch(
    launch_path: &Path,
    options: &LaunchOptions,
    matches: impl Fn(&ipc::Instance, &str) -> bool,
) -> Result<Option<ipc::Instance>> {
    let installation = crate::ida::launch::Installation::resolve()?;
    installation.launch(launch_path)?;
    if !installation.supports_ipc() {
        return Ok(None);
    }
    // A glob can resolve to a different literal name. Startup waits for the
    // selected file, while the original request remains the navigation payload.
    let launched_name = launch_path.file_name().unwrap_or_default().to_string_lossy();
    let instance = super::wait::database(options.timeout, || {
        ipc::find(|instance| matches(instance, &launched_name))
    })
    .await?;
    if !options.skip_analysis {
        super::wait::analysis(&instance.socket).await?;
    }
    Ok(Some(instance))
}

fn same_database(expected: &Path, actual: &Path) -> bool {
    match (expected.canonicalize(), actual.canonicalize()) {
        (Ok(expected), Ok(actual)) => expected == actual,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_paths_do_not_identify_the_same_database() {
        let directory = tempfile::tempdir().unwrap();
        assert!(!same_database(&directory.path().join("a.i64"), &directory.path().join("b.i64")));
        let database = directory.path().join("present.i64");
        std::fs::write(&database, b"fixture").unwrap();
        assert!(same_database(&database, &database));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn relative_navigation_uses_the_only_loaded_database_and_preserves_the_uri() {
        use serde_json::json;
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let temporary = tempfile::tempdir().unwrap();
        let socket = temporary.path().join("ipc");
        let listener = tokio::net::UnixListener::bind(&socket).unwrap();
        let uri = "ida:///?rva=0x10&view=disassembly";
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut bytes = Vec::new();
            let mut chunk = [0; 1024];
            let request: serde_json::Value = loop {
                let count = stream.read(&mut chunk).await.unwrap();
                assert!(count > 0 && bytes.len() + count <= 8192);
                bytes.extend_from_slice(&chunk[..count]);
                match serde_json::from_slice(&bytes) {
                    Ok(request) => break request,
                    Err(error) if error.is_eof() => (),
                    Err(error) => panic!("invalid IPC request: {error}"),
                }
            };
            assert_eq!(request, json!({"cmd":"open_ida_link","uri":uri}));
            stream.write_all(br#"{"status":"ok"}"#).await.unwrap();
        });
        navigate_relative(
            uri,
            vec![
                ipc::Instance {
                    pid: 1,
                    socket: temporary.path().join("unused"),
                    idb_path: None,
                },
                ipc::Instance {
                    pid: 2,
                    socket,
                    idb_path: Some(PathBuf::from("/fixture/sample.i64")),
                },
            ],
        )
        .await
        .unwrap();
        server.await.unwrap();
    }

    #[tokio::test]
    async fn relative_navigation_rejects_zero_or_multiple_loaded_databases_before_connecting() {
        let unloaded = ipc::Instance {
            pid: 1,
            socket: PathBuf::from("/nonexistent/socket"),
            idb_path: None,
        };
        let first = ipc::Instance {
            pid: 2,
            idb_path: Some(PathBuf::from("first.i64")),
            ..unloaded.clone()
        };
        let second = ipc::Instance {
            pid: 3,
            idb_path: Some(PathBuf::from("second.i64")),
            ..unloaded.clone()
        };
        let missing = navigate_relative("ida:///?rva=0", vec![unloaded]).await.unwrap_err();
        assert!(missing.to_string().contains("No running IDA instances"));
        let ambiguous = navigate_relative("ida:///?rva=0", vec![first, second]).await.unwrap_err();
        assert!(ambiguous.to_string().contains("Multiple IDA instances"));
    }
}
