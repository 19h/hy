//! Desktop-entry publication and MIME association on Linux.

use std::path::Path;
use std::process::Command;

use crate::error::Result;

use super::PYTHON_ENVIRONMENT_RESET;

const DESKTOP_FILE: &str = "hcli-idb-handler.desktop";
const MIME_TYPE: &str = "x-scheme-handler/ida";

type RunCommand<'a> = dyn FnMut(&mut Command, bool) -> Result<()> + 'a;

#[cfg(target_os = "linux")]
pub(super) fn register(binary_path: &str) -> Result<()> {
    register_at(binary_path, &super::home()?.join(".local/share/applications"), &mut run_command)
}

#[cfg(target_os = "linux")]
pub(super) fn unregister() -> Result<()> {
    unregister_at(&super::home()?.join(".local/share/applications"), &mut run_command)
}

#[cfg(target_os = "linux")]
fn run_command(command: &mut Command, required: bool) -> Result<()> {
    if required {
        super::checked(command)
    } else {
        command.output()?;
        Ok(())
    }
}

fn register_at(binary_path: &str, directory: &Path, run: &mut RunCommand<'_>) -> Result<()> {
    std::fs::create_dir_all(directory)?;
    let path = directory.join(DESKTOP_FILE);
    std::fs::write(&path, desktop_entry(binary_path))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))?;
    }
    run(Command::new("xdg-mime").args(["default", DESKTOP_FILE, MIME_TYPE]), true)?;
    run(Command::new("update-desktop-database").arg(directory), false)
}

fn unregister_at(directory: &Path, run: &mut RunCommand<'_>) -> Result<()> {
    let path = directory.join(DESKTOP_FILE);
    if path.exists() {
        std::fs::remove_file(path)?;
        run(Command::new("xdg-mime").args(["default", "", MIME_TYPE]), false)?;
        run(Command::new("update-desktop-database").arg(directory), false)?;
    }
    Ok(())
}

fn desktop_entry(binary_path: &str) -> String {
    let binary_path = desktop_quote(binary_path);
    format!(
        "[Desktop Entry]\nName=HCLI IDB Link Handler\nExec={PYTHON_ENVIRONMENT_RESET} {binary_path} ida open -- %u\nType=Application\nNoDisplay=true\nMimeType={MIME_TYPE};\n"
    )
}

fn desktop_quote(path: &str) -> String {
    // Apply Exec argument quoting before the desktop-entry string escape layer.
    let mut quoted = String::from("\"");
    for character in path.chars() {
        if ['"', '\\', '$', '`'].contains(&character) {
            quoted.push('\\');
        }
        if character == '%' {
            quoted.push('%');
        }
        quoted.push(character);
    }
    quoted.push('"');
    quoted.replace('\\', "\\\\")
}

#[cfg(test)]
mod tests;
