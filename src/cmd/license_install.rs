//! Local license destination selection and metadata-preserving installation.

use std::path::PathBuf;

use super::license::LicenseInstallArgs;
use crate::error::{Error, Result};
use crate::util::files::absolute_path;
use crate::util::{fmt, tui};

pub(super) async fn run(args: LicenseInstallArgs) -> Result<()> {
    if !args.file.is_file() {
        return Err(Error::Other(format!("not a file: {}", args.file.display())));
    }
    let target = match args.ida_dir {
        Some(path) => path,
        None => {
            let Some(path) = select_target().await else {
                return Ok(());
            };
            path
        }
    };
    let target = absolute_path(&target)?;
    if !target.exists() {
        let choices = vec!["y. Yes".into(), "n. No".into()];
        if tui::select("Create directory?", &choices, 0) != Some(0) {
            return Ok(());
        }
        std::fs::create_dir_all(&target)?;
    }
    let filename =
        args.file.file_name().ok_or_else(|| Error::Other("missing license filename".into()))?;
    crate::util::files::copy_metadata(&args.file, &target.join(filename))?;
    fmt::success(&format!("License installed successfully in {}", target.display()));
    Ok(())
}

async fn select_target() -> Option<PathBuf> {
    let mut paths = vec![crate::ida::ida_user_dir()];
    paths.extend(crate::ida::find_standard_installations().await);
    let mut choices: Vec<String> = paths
        .iter()
        .enumerate()
        .map(|(index, path)| {
            format!(
                "{}. {}{}",
                index + 1,
                path.display(),
                if index == 0 {
                    " (user directory)"
                } else {
                    ""
                }
            )
        })
        .collect();
    choices.push(format!("{}. Other (specify custom path)", choices.len() + 1));
    let selected = tui::select("Select installation", &choices, 0)?;
    if selected < paths.len() {
        return Some(paths.swap_remove(selected));
    }
    // Cancellation must not silently accept the default destination.
    dialoguer::Input::<String>::with_theme(&tui::theme())
        .with_prompt("Enter the target directory path")
        .default(".".into())
        .interact_text()
        .ok()
        .map(PathBuf::from)
}
