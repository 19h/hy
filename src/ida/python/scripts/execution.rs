//! Subprocess environment and terminal attachment shared by script lookup and execution.

use std::path::Path;
use std::process::ExitStatus;

use tokio::process::Command;

use crate::error::{Error, Result};

pub(super) fn command(python: &Path, program: &Path) -> Command {
    let mut command = Command::new(program);
    super::super::configure_environment(&mut command, python);
    command.kill_on_drop(true);
    command
}

pub(super) fn is_executable(path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        let Ok(path) = std::ffi::CString::new(path.as_os_str().as_bytes()) else {
            return false;
        };
        // access(2) matches os.access(..., X_OK), including real-user permissions.
        unsafe { libc::access(path.as_ptr(), libc::X_OK) == 0 }
    }
    #[cfg(windows)]
    {
        // CPython's Windows os.access treats X_OK as an existence check.
        path.exists()
    }
}

pub(super) fn exit_code(status: ExitStatus) -> i32 {
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        status.code().unwrap_or_else(|| -status.signal().unwrap_or(1))
    }
    #[cfg(not(unix))]
    {
        status.code().unwrap_or(1)
    }
}

pub(super) async fn run(command: &mut Command) -> Result<()> {
    let status = command.status().await?;
    if status.success() {
        Ok(())
    } else {
        Err(Error::ChildExit(exit_code(status)))
    }
}
