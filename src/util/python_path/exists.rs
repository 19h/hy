//! Follow links with CPython 3.13 pathlib's missing-path and error policy.

use std::io;
use std::path::Path;

pub(crate) fn exists(path: &Path) -> io::Result<bool> {
    // Python reports embedded NUL paths as absent, without calling stat.
    if path.as_os_str().as_encoded_bytes().contains(&0) {
        return Ok(false);
    }
    match std::fs::metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if ignored_error(&error) => Ok(false),
        Err(error) => Err(error),
    }
}

#[cfg(unix)]
fn ignored_error(error: &io::Error) -> bool {
    matches!(error.raw_os_error(), Some(libc::ENOENT | libc::ENOTDIR | libc::EBADF | libc::ELOOP))
}

#[cfg(windows)]
fn ignored_error(error: &io::Error) -> bool {
    let Some(mut code) = error.raw_os_error().map(|code| code as u32) else {
        return false;
    };
    // pathlib tests winerror separately from the mapped errno.
    if matches!(code, 21 | 123 | 1921) {
        return true;
    }
    // CPython PC/errmap.h unwraps FACILITY_WIN32 HRESULT values.
    if code & 0xffff_0000 == 0x8007_0000 {
        code &= 0xffff;
    }
    matches!(
        code,
        // ENOENT, EBADF and ENOTDIR mappings from CPython 3.13.15 PC/errmap.h.
        2 | 3 | 15 | 18 | 53 | 67 | 161 | 206 | 6 | 114 | 130 | 267 | 10009
    )
}
