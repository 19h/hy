//! Native path joining for Python strings with filesystem surrogate code points.

use std::path::{Path, PathBuf};

use crate::util::python_json::Text;

pub(crate) fn join_text(base: &Path, name: &Text) -> Option<PathBuf> {
    if let Ok(name) = name.to_utf8() {
        return Some(super::join(base, &name));
    }
    join_surrogates(base, name)
}

#[cfg(unix)]
fn join_surrogates(base: &Path, name: &Text) -> Option<PathBuf> {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    let bytes = name.to_utf8_surrogateescape().ok()?;
    // pathlib removes empty and '.' components but retains '..' and exactly
    // two leading slashes. Surrogateescape bytes cannot contain separators.
    let mut normalized = Vec::new();
    if bytes.starts_with(b"/") {
        normalized.extend_from_slice(if bytes.starts_with(b"//") && !bytes.starts_with(b"///") {
            b"//"
        } else {
            b"/"
        });
    }
    for part in bytes.split(|&byte| byte == b'/').filter(|part| !part.is_empty() && *part != b".") {
        if !normalized.is_empty() && !normalized.ends_with(b"/") {
            normalized.push(b'/');
        }
        normalized.extend_from_slice(part);
    }
    Some(base.join(OsString::from_vec(normalized)))
}

#[cfg(all(test, unix))]
mod tests;

#[cfg(windows)]
fn join_surrogates(base: &Path, name: &Text) -> Option<PathBuf> {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;

    let mut units = Vec::new();
    for point in name.codepoints() {
        if point <= 0xffff {
            units.push(point as u16);
        } else {
            units.extend_from_slice(char::from_u32(point)?.encode_utf16(&mut [0; 2]));
        }
    }
    Some(base.join(OsString::from_wide(&units)))
}
