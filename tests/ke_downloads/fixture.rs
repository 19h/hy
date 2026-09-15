//! Shared KE HTTP, cache and dialog fixtures.

use super::support;

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};

use sha2::{Digest, Sha256};
use support::Sandbox;
use support::http::{Request, Response, Server};
use support::ke_dialogs::DialogTools;

pub(super) const CONTENT: &[u8] = b"fixture IDB content";
pub(super) const FILENAME: &str = "sample.i64";

pub(super) fn response(status: u16, body: &[u8]) -> Response {
    Response {
        status,
        content_type: "application/octet-stream",
        body: body.to_vec(),
    }
}

pub(super) fn assert_error(output: &Output, expected: &str) {
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "unexpected success: {stderr}");
    assert!(stderr.contains(expected), "expected {expected:?}: {stderr}");
}

pub(super) struct Fixture {
    pub(super) sandbox: Sandbox,
    pub(super) dialogs: DialogTools,
    pub(super) server: Server,
    pub(super) hash: String,
}

impl Fixture {
    pub(super) fn new(metadata: Response, content: Response) -> Self {
        Self::start(metadata, content, None)
    }

    pub(super) fn encoded(metadata: Response, content: Response, encoding: &str) -> Self {
        Self::start(metadata, content, Some(encoding.to_owned()))
    }

    fn start(metadata: Response, content: Response, encoding: Option<String>) -> Self {
        Self::with_handler(move |request, _| {
            let response = if request.path.split('?').next().unwrap().ends_with("/content") {
                content.clone()
            } else {
                metadata.clone()
            };
            let headers =
                encoding.iter().map(|value| ("Content-Encoding".into(), value.clone())).collect();
            (response, headers)
        })
    }

    pub(super) fn with_handler(
        handler: impl Fn(&Request, &str) -> (Response, Vec<(String, String)>) + Send + 'static,
    ) -> Self {
        let sandbox = Sandbox::new();
        let dialogs = DialogTools::new(sandbox.path());
        #[cfg(unix)]
        let progress = dialogs.directory.join("progress");
        let server = Server::start_with_headers(move |request, base| {
            #[cfg(unix)]
            {
                let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
                while !progress.exists() {
                    assert!(std::time::Instant::now() < deadline, "progress fixture did not start");
                    std::thread::sleep(std::time::Duration::from_millis(5));
                }
            }
            handler(request, base)
        });
        Self {
            sandbox,
            dialogs,
            server,
            hash: format!("{:x}", Sha256::digest(CONTENT)),
        }
    }

    pub(super) fn path(&self) -> PathBuf {
        self.sandbox.path().join("downloads").join(&self.hash).join(FILENAME)
    }

    pub(super) fn sidecar(&self) -> PathBuf {
        self.path().with_file_name(format!("{FILENAME}.ke.json"))
    }

    pub(super) fn command(&self) -> Command {
        self.command_for(true)
    }

    pub(super) fn command_for(&self, no_launch: bool) -> Command {
        self.command_for_uri(self.uri().as_str(), no_launch)
    }

    pub(super) fn uri(&self) -> url::Url {
        let content =
            format!("{}/api/objects/{}/content?token=fixture", self.server.url, self.hash);
        let mut uri = url::Url::parse(&format!("ida://ke/{FILENAME}")).unwrap();
        uri.query_pairs_mut().append_pair("url", &content);
        uri
    }

    pub(super) fn command_for_uri(&self, uri: &str, no_launch: bool) -> Command {
        let mut command = self.sandbox.command(&["ida", "open", uri]);
        if no_launch {
            command.arg("--no-launch");
        }
        self.dialogs.configure(&mut command);
        command
            .env("HCLI_KE_DOWNLOADS_DIR", self.sandbox.path().join("downloads"))
            .env("HCLI_KE_ALLOW_PRIVATE_HOSTS", "1");
        command
    }

    pub(super) fn run(&self) -> Output {
        self.command().output().unwrap()
    }

    pub(super) fn seed(&self) {
        fs::create_dir_all(self.path().parent().unwrap()).unwrap();
        fs::write(self.path(), b"old content").unwrap();
        fs::write(self.sidecar(), br#"{"old":true}"#).unwrap();
    }

    pub(super) fn assert_request_order(&self) {
        let paths: Vec<_> =
            self.server.requests().into_iter().map(|request| request.path).collect();
        assert_eq!(
            paths,
            [
                format!("/api/objects/{}?token=fixture", self.hash),
                format!("/api/objects/{}/content?token=fixture", self.hash),
            ]
        );
    }

    pub(super) fn assert_no_staging_files(&self) {
        let entries: Vec<_> = fs::read_dir(self.path().parent().unwrap())
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        assert_eq!(entries.len(), 2, "unexpected cache entries: {entries:?}");
    }

    #[cfg(unix)]
    pub(super) fn installation(&self) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let directory = self.sandbox.path().join("ida-9.3");
        fs::create_dir_all(directory.join("python")).unwrap();
        fs::write(directory.join("python/ida_pro.py"), "# IDA SDK v9.3\n").unwrap();
        let script = r#"#!/bin/sh
printf '%s' "$1" > "$0.database"
"#;
        let binary = directory.join("ida");
        fs::write(&binary, script).unwrap();
        fs::set_permissions(binary, fs::Permissions::from_mode(0o755)).unwrap();
        directory
    }

    #[cfg(unix)]
    pub(super) fn assert_dialog_dismissed(&self) {
        let pid: i32 = self.dialogs.text("pid").parse().unwrap();
        assert!(pid > 0);
        // SAFETY: signal zero probes the positive PID recorded by this fixture.
        assert_eq!(unsafe { libc::kill(pid, 0) }, -1, "progress process still exists");
        assert_eq!(std::io::Error::last_os_error().raw_os_error(), Some(libc::ESRCH));
    }
}
