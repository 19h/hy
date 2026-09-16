//! Catalogue file acquisition follows urllib FileHandler, without pathlib cleanup.

use std::fs::File;
use std::io::Read;

use crate::error::{Error, Result};

mod host;
mod path;
mod request;

pub(super) use request::Request;

pub(super) async fn download(request: Request) -> Result<Vec<u8>> {
    let mut file = super::retry::open_file(|| open(&request), tokio::time::sleep).await?;
    // Upstream reads the returned response outside its retry decorators.
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    Ok(bytes)
}

fn open(request: &Request) -> Result<File> {
    if request.has_remote_double_slash() {
        if !host::is_local_address(&request.host)? {
            return Err(url_error("file:// scheme is supported only on localhost"));
        }
        // The source handler falls through for a local address in this branch;
        // the opener's UnknownHandler then raises this URLError.
        return Err(url_error("unknown url type: file"));
    }
    let path = path::native(&request.selector)?;
    if native_contains_nul(&path) {
        return Err(Error::GitHubValue("embedded null byte".into()));
    }
    std::fs::metadata(&path).map_err(url_error)?;
    if !request.host.is_empty() && !host::resolves_locally(&request.host)? {
        return Err(url_error("file not on local host"));
    }
    let file = File::open(path).map_err(url_error)?;
    // Rust opens directories on Unix and fails at read; Python checks the opened
    // descriptor and rejects directories inside the retry boundary.
    #[cfg(unix)]
    if file.metadata().map_err(url_error)?.is_dir() {
        return Err(url_error(std::io::Error::from_raw_os_error(libc::EISDIR)));
    }
    Ok(file)
}

fn url_error(error: impl std::fmt::Display) -> Error {
    Error::GitHubUrl(format!("<urlopen error {error}>"))
}

fn native_contains_nul(path: &std::path::Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        path.as_os_str().as_bytes().contains(&0)
    }
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        path.as_os_str().encode_wide().any(|unit| unit == 0)
    }
}

#[cfg(test)]
mod tests;
