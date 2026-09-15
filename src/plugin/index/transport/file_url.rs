//! Decode local archive URLs using CPython 3.13 path conversion semantics.
//!
//! The adaptation and license notice are in util/python_path/LICENSE.

use std::path::PathBuf;

#[cfg(any(windows, test))]
use crate::error::Error;
use crate::error::Result;
#[cfg(any(windows, test))]
use crate::util::python_path::path::{Flavor, ParsedPath};

mod authority;

/// Return a native path only for the file scheme, before WHATWG URL parsing.
pub(in crate::plugin::index) fn path(value: &str) -> Result<Option<PathBuf>> {
    let Some(path) = raw_path(value)? else {
        return Ok(None);
    };
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;
        Ok(Some(std::ffi::OsString::from_vec(posix_path(&path)).into()))
    }
    #[cfg(windows)]
    {
        Ok(Some(windows_path(&path)?.into()))
    }
}

fn raw_path(value: &str) -> Result<Option<String>> {
    // urllib strips leading C0/space and removes embedded tab, CR and LF.
    let cleaned: String = value
        .trim_start_matches(|character: char| character <= '\u{20}')
        .chars()
        .filter(|character| !matches!(character, '\t' | '\r' | '\n'))
        .collect();
    let Some((scheme, mut remainder)) = cleaned.split_once(':') else {
        return Ok(None);
    };
    if !scheme.eq_ignore_ascii_case("file") {
        return Ok(None);
    }
    if let Some(netloc) = remainder.strip_prefix("//") {
        let end = netloc.find(['/', '?', '#']).unwrap_or(netloc.len());
        authority::validate(&netloc[..end])?;
        remainder = &netloc[end..];
    }
    // file is absent from urllib's uses_params list: semicolons remain in paths.
    let end = remainder.find(['?', '#']).unwrap_or(remainder.len());
    Ok(Some(remainder[..end].to_owned()))
}

fn without_empty_or_local_authority(path: &str) -> &str {
    if path.starts_with("///") {
        &path[2..]
    } else if path.starts_with("//localhost/") {
        &path[11..]
    } else {
        path
    }
}

#[cfg(any(unix, test))]
fn posix_path(path: &str) -> Vec<u8> {
    let path = without_empty_or_local_authority(path);
    // UTF-8 plus surrogateescape round-trips undecodable percent-encoded bytes.
    let decoded: Vec<u8> = percent_encoding::percent_decode_str(path).collect();
    let root: &[u8] = if decoded.starts_with(b"//") && !decoded.starts_with(b"///") {
        b"//"
    } else if decoded.starts_with(b"/") {
        b"/"
    } else {
        b""
    };
    let mut result = root.to_vec();
    for part in decoded.split(|byte| *byte == b'/').filter(|part| !part.is_empty() && *part != b".")
    {
        if !result.is_empty() && !result.ends_with(b"/") {
            result.push(b'/');
        }
        result.extend_from_slice(part);
    }
    if result.is_empty() {
        b".".to_vec()
    } else {
        result
    }
}

#[cfg(any(windows, test))]
fn windows_path(path: &str) -> Result<String> {
    let path = without_empty_or_local_authority(path);
    let path = if path.starts_with("///") {
        &path[1..]
    } else {
        path
    };
    let path = path.replace(':', "|");
    let decoded = if let Some((head, tail)) = path.split_once('|') {
        let drive = head
            .chars()
            .last()
            .filter(char::is_ascii_alphabetic)
            .ok_or_else(|| Error::Other("invalid drive in file URL".into()))?;
        if tail.contains('|') {
            return Err(Error::Other("multiple drives in file URL".into()));
        }
        let tail = tail.replace('/', "\\");
        format!(
            "{}:{}",
            drive.to_ascii_uppercase(),
            percent_encoding::percent_decode_str(&tail).decode_utf8_lossy()
        )
    } else {
        let path = path.replace('/', "\\");
        percent_encoding::percent_decode_str(&path).decode_utf8_lossy().into_owned()
    };
    Ok(ParsedPath::parse(&decoded, Flavor::Windows).render(Flavor::Windows))
}

#[cfg(test)]
mod tests;
