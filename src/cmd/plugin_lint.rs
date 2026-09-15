//! Inspect plugin sources; validation findings do not fail the lint command.

use std::path::Path;

use super::plugin_cmd::PluginLintArgs;
use crate::error::{Error, Result};
use crate::plugin;
use crate::util::files;

mod archive;
mod report;

pub async fn run(args: PluginLintArgs) -> Result<()> {
    let findings = if args.path.starts_with("https://") || args.path.starts_with("http://") {
        let bytes = plugin::index::fetch(&args.path).await?;
        archive::lint(std::io::Cursor::new(bytes), &args.path)?
    } else {
        let path = files::absolute_path(Path::new(&args.path))?.canonicalize()?;
        if path.is_dir() {
            lint_directory(&path)?
        } else if path.is_file() {
            if !path.extension().is_some_and(|extension| extension.eq_ignore_ascii_case("zip")) {
                return Err(Error::Other("plugin archive must have a .zip extension".into()));
            }
            archive::lint(std::fs::File::open(&path)?, &path.to_string_lossy())?
        } else {
            return Err(Error::Other(format!(
                "path must be a directory or .zip file: {}",
                path.display()
            )));
        }
    };
    if findings == 0 {
        println!("no recommendations");
    }
    Ok(())
}

fn lint_directory(path: &Path) -> Result<usize> {
    let metadata = match plugin::read_metadata_from_directory(path) {
        Ok(metadata) => metadata,
        Err(error @ (Error::Json(_) | Error::PluginInstall(_))) => {
            println!("Error: ida-plugin.json validation failed: {error}");
            return Ok(1);
        }
        Err(error) => return Err(error),
    };
    let source = path.to_string_lossy();
    let mut findings = report::metadata(&metadata, &source);
    let mut filenames = Vec::new();
    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        // Path::is_file follows file symlinks, as upstream's README check does.
        if entry.path().is_file() {
            filenames.push(entry.file_name().to_string_lossy().into_owned());
        }
    }
    findings += report::readme(filenames.iter().map(String::as_str), &source);
    Ok(findings)
}
