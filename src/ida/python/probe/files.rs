//! Preserve the minimal startup and licensing files needed by an isolated probe.

use std::path::Path;

use crate::error::Result;
use crate::util::files::copy_metadata;

pub(super) fn prepare(source: &Path, target: &Path) -> Result<()> {
    std::fs::create_dir_all(target)?;
    for relative in ["cfg/idapython.cfg", "ida.reg", "idapythonrc.py"] {
        let path = source.join(relative);
        if path.is_file() {
            let destination = target.join(relative);
            std::fs::create_dir_all(destination.parent().unwrap())?;
            copy_metadata(&path, &destination)?;
        }
    }
    for entry in std::fs::read_dir(source)? {
        let path = entry?.path();
        let is_license = path.file_name().is_some_and(|name| {
            let name = name.to_string_lossy();
            if cfg!(windows) {
                name.to_ascii_lowercase().ends_with(".hexlic")
            } else {
                name.ends_with(".hexlic")
            }
        });
        if is_license && path.is_file() {
            copy_metadata(&path, &target.join(path.file_name().unwrap()))?;
        }
    }
    Ok(())
}
