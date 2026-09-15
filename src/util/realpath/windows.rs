//! Resolve existing Windows prefixes before appending a missing download tail.

use std::os::windows::fs::MetadataExt;
use std::path::{Path, PathBuf};

use windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT;

pub fn resolve(path: &Path) -> std::io::Result<PathBuf> {
    let absolute = std::path::absolute(path)?;
    for ancestor in absolute.ancestors() {
        match ancestor.canonicalize() {
            Ok(resolved) => {
                let suffix = absolute.strip_prefix(ancestor).expect("ancestor is a path prefix");
                return Ok(resolved.join(suffix));
            }
            Err(error) => {
                // An unresolved reparse point cannot establish containment by
                // falling back to its lexical parent.
                if std::fs::symlink_metadata(ancestor).is_ok_and(|metadata| {
                    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
                }) {
                    return Err(error);
                }
            }
        }
    }
    Err(std::io::Error::new(std::io::ErrorKind::NotFound, "no resolvable Windows path prefix"))
}
