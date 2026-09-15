//! Native KE dialogs, with message text passed separately from script source.

use std::process::{Command, Stdio};
use std::time::Duration;

mod commands;

use commands::{Platform, confirm_commands, error_command, progress_command};

pub(super) async fn confirm(filename: &str, host: &str) -> bool {
    let prompt =
        format!("Download and open IDB '{filename}' from {host} (or where it redirects) in IDA?");
    for command in confirm_commands(Platform::current(), &prompt) {
        if !available(&command) {
            continue;
        }
        return quiet(command).status().await.is_ok_and(|status| status.success());
    }
    crate::util::fmt::warning(
        "No confirmation prompt available (install zenity/kdialog, or set HCLI_KE_SKIP_CONFIRM=1).",
    );
    false
}

pub(super) async fn error(message: &str) {
    let command = error_command(Platform::current(), message);
    let _ = quiet(command).status().await;
}

pub(super) struct Progress(Option<tokio::process::Child>);

impl Progress {
    pub fn show(filename: &str) -> Self {
        let command = progress_command(Platform::current(), filename);
        Self(quiet(command).kill_on_drop(true).spawn().ok())
    }

    pub async fn dismiss(mut self) {
        if let Some(child) = &mut self.0 {
            let _ = child.start_kill();
            let _ = tokio::time::timeout(Duration::from_secs(2), child.wait()).await;
        }
    }
}

fn quiet(mut command: Command) -> tokio::process::Command {
    command.stdout(Stdio::null()).stderr(Stdio::null());
    tokio::process::Command::from(command)
}

fn available(command: &Command) -> bool {
    // shutil.which skips non-executable candidates before choosing zenity/kdialog.
    // macOS and Windows each have one fixed command; spawn errors mean cancellation.
    if !cfg!(target_os = "linux") {
        return true;
    }
    std::env::var_os("PATH").is_some_and(|paths| {
        std::env::split_paths(&paths).any(|directory| {
            let path = directory.join(command.get_program());
            #[cfg(unix)]
            {
                use std::os::unix::ffi::OsStrExt;
                if path.is_dir() {
                    return false;
                }
                let Ok(path) = std::ffi::CString::new(path.as_os_str().as_bytes()) else {
                    return false;
                };
                // SAFETY: path is a live NUL-terminated string; access only
                // checks permissions using the process's real credentials.
                unsafe { libc::access(path.as_ptr(), libc::X_OK) == 0 }
            }
            #[cfg(not(unix))]
            {
                path.is_file()
            }
        })
    })
}
