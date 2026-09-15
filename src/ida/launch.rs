//! Select and detach the IDA launcher used by ordinary and KE links.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::config::ConfigStore;
use crate::error::{Error, Result};

pub(crate) struct Installation {
    binary: PathBuf,
    version: Option<String>,
}

impl Installation {
    pub fn resolve() -> Result<Self> {
        let configured = configured_binary(&ConfigStore::global());
        let binary = configured
            .or_else(|| {
                super::current_install_dir()
                    .and_then(|directory| super::ida_binary_path(&directory))
            })
            .ok_or(Error::IdaNotFound)?;
        let directory = directory_from_binary(&binary);
        // The upstream launcher reads SDK/directory versions, independently of
        // the global current-version override and binary/registry diagnostics.
        let version = super::version::sdk_version(&directory).or_else(|| {
            directory
                .file_name()
                .and_then(|name| name.to_str())
                .and_then(super::version::version_in_name)
        });
        Ok(Self {
            binary,
            version,
        })
    }

    pub fn supports_ipc(&self) -> bool {
        let parsed = self.version.as_deref().and_then(|version| {
            let (major, minor) = version.split_once('.')?;
            Some((major.parse::<u64>().ok()?, minor.parse::<u64>().ok()?))
        });
        parsed.is_none_or(|version| version >= (9, 4))
    }

    pub fn launch(&self, database: &Path) -> Result<()> {
        if !database.is_file() {
            return Err(Error::Other(format!("IDB path is not a file: {}", database.display())));
        }
        let mut command = command(&self.binary, database);
        command.stdout(Stdio::null()).stderr(Stdio::null());
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            // SAFETY: setsid is async-signal-safe and touches no Rust state in
            // the post-fork child. Only its return value and errno are read.
            unsafe {
                command.pre_exec(|| {
                    if libc::setsid() == -1 {
                        Err(std::io::Error::last_os_error())
                    } else {
                        Ok(())
                    }
                });
            }
        }
        // Tokio reaps a detached child while this CLI remains alive. Dropping
        // the handle does not terminate IDA when navigation or this runtime ends.
        tokio::process::Command::from(command).spawn()?;
        Ok(())
    }
}

fn configured_binary(store: &ConfigStore) -> Option<PathBuf> {
    let default = store.get_str("ida.default").filter(|name| !name.is_empty())?;
    let instances = store.get_string_map("ida.instances");
    super::ida_binary_path(Path::new(instances.get(default)?))
}

fn directory_from_binary(binary: &Path) -> PathBuf {
    if cfg!(target_os = "macos")
        && let Some((directory, _)) = binary.to_string_lossy().split_once("/Contents/MacOS/")
    {
        return PathBuf::from(directory);
    }
    binary.parent().unwrap_or(Path::new("")).to_owned()
}

fn command(binary: &Path, database: &Path) -> Command {
    if cfg!(target_os = "macos")
        && let Some((bundle, _)) = binary.to_string_lossy().split_once("/Contents/MacOS/")
        && bundle.ends_with(".app")
    {
        let mut command = Command::new("open");
        command.args(["-n", "-a", bundle, "--args"]).arg(database);
        return command;
    }
    let mut command = Command::new(binary);
    command.arg(database);
    command
}
