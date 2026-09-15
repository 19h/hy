//! Compile and publish the macOS URL application and its diagnostic launcher.

use std::path::Path;
use std::process::Command;

use crate::error::{Error, Result};

use super::{PYTHON_ENVIRONMENT_RESET, checked};

const LSREGISTER: &str = "/System/Library/Frameworks/CoreServices.framework/Versions/A/Frameworks/LaunchServices.framework/Versions/A/Support/lsregister";

pub(super) fn register(binary_path: &str) -> Result<()> {
    let home = super::home()?;
    let applications = home.join("Applications");
    let log_directory = home.join("Library/Logs");
    // The handler retries mkdir when invoked, as upstream does.
    let _ = std::fs::create_dir_all(&log_directory);
    std::fs::create_dir_all(&applications)?;
    let app_path = applications.join("HCLIHandler.app");
    let staging = tempfile::tempdir_in(applications)?;
    let compiled = staging.path().join("HCLIHandler.app");
    compile(binary_path, &log_directory, &compiled)?;

    let previous = staging.path().join("previous.app");
    let existed = app_path.exists();
    if existed {
        std::fs::rename(&app_path, &previous)?;
    }
    if let Err(error) = std::fs::rename(&compiled, &app_path) {
        if existed && let Err(rollback) = std::fs::rename(&previous, &app_path) {
            let retained = staging.keep();
            return Err(Error::Other(format!(
                "handler publication failed: {error}; rollback failed: {rollback}; backup at {}",
                retained.join("previous.app").display()
            )));
        }
        return Err(error.into());
    }
    checked(Command::new(LSREGISTER).arg("-f").arg(app_path))
}

pub(super) fn unregister() -> Result<()> {
    let app = super::home()?.join("Applications/HCLIHandler.app");
    if app.exists() {
        std::fs::remove_dir_all(&app)?;
        // Upstream tolerates a nonzero status after the application is removed.
        Command::new(LSREGISTER).arg("-u").arg(app).output()?;
    }
    Ok(())
}

fn compile(binary_path: &str, log_directory: &Path, app_path: &Path) -> Result<()> {
    let temporary = tempfile::tempdir()?;
    let script = temporary.path().join("handler.applescript");
    std::fs::write(&script, applescript(binary_path, log_directory))?;
    checked(Command::new("osacompile").arg("-o").arg(app_path).arg(script))?;
    let plist = app_path.join("Contents/Info.plist");
    let schemes = r#"[{"CFBundleURLName":"IDB URL Handler","CFBundleURLSchemes":["ida"]}]"#;
    checked(
        Command::new("plutil").args(["-insert", "CFBundleURLTypes", "-json", schemes]).arg(&plist),
    )?;
    checked(Command::new("plutil").args(["-replace", "LSUIElement", "-bool", "true"]).arg(plist))
}

fn applescript(binary_path: &str, log_directory: &Path) -> String {
    let shell_path = format!("'{}'", binary_path.replace('\'', "'\\''"));
    let invocation = literal(&format!("{PYTHON_ENVIRONMENT_RESET} {shell_path} ida open -- "));
    let log_directory = literal(&log_directory.to_string_lossy());
    format!(
        r#"on open location theURL
    set logDir to "{log_directory}"
    set logFile to logDir & "/idb_handler.log"
    do shell script "/bin/mkdir -p " & quoted form of logDir & " ; /bin/zsh -l -c " & quoted form of ("{invocation}" & quoted form of theURL) & " >> " & quoted form of logFile & " 2>&1"
end open location
on run
end run
"#
    )
}

fn literal(text: &str) -> String {
    text.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests;
