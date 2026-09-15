//! Replace native dialog tools without displaying windows during CLI tests.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct DialogTools {
    pub directory: PathBuf,
}

impl DialogTools {
    pub fn new(home: &Path) -> Self {
        let directory = home.join("dialog-tools");
        fs::create_dir(&directory).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let script = r#"#!/bin/sh
case "$*" in
  *KE_DLG_FILE*|"-t "*)
    printf '%s' "$$" > "$HY_TEST_DIALOG_DIR/pid"
    printf '%s' "${KE_DLG_FILE:-$4}" > "$HY_TEST_DIALOG_DIR/progress"
    exec /bin/sleep 600
    ;;
  *"with icon stop"*|*'"Error")'*|"-u "*)
    printf '%s' "${KE_DLG_MSG:-$4}" > "$HY_TEST_DIALOG_DIR/error"
    exit 0
    ;;
  *)
    printf '%s' "${KE_DLG_MSG:-$3}" > "$HY_TEST_DIALOG_DIR/confirm"
    printf '%s\n' "$@" > "$HY_TEST_DIALOG_DIR/confirm-args"
    exit "${HY_TEST_DIALOG_APPROVE:-1}"
    ;;
esac
"#;
            for tool in ["osascript", "powershell", "zenity", "kdialog", "notify-send"] {
                let path = directory.join(tool);
                fs::write(&path, script).unwrap();
                fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
            }
        }
        #[cfg(windows)]
        {
            // Existing transfer tests exercise optional dialog-spawn failure on
            // Windows; they must not discover and display the system PowerShell UI.
            fs::write(directory.join("powershell.exe"), b"invalid fixture executable").unwrap();
        }
        Self {
            directory,
        }
    }

    pub fn configure(&self, command: &mut Command) {
        command
            .env("PATH", &self.directory)
            .env("HY_TEST_DIALOG_DIR", &self.directory)
            .env_remove("HY_TEST_DIALOG_APPROVE");
    }

    pub fn text(&self, name: &str) -> String {
        fs::read_to_string(self.directory.join(name)).unwrap()
    }
}
