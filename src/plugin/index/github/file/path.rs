//! url2pathname conversion without pathlib normalization.
//!
//! The CPython adaptation and license notice are in util/python_path/LICENSE.

use std::path::PathBuf;

use crate::error::{Error, Result};
use crate::util::python_json::Text;

pub(super) fn native(selector: &Text) -> Result<PathBuf> {
    let points: Vec<_> = selector.codepoints().collect();
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;
        Ok(std::ffi::OsString::from_vec(posix(&points)?).into())
    }
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStringExt;
        let points = windows(&points)?;
        let mut units = Vec::new();
        for point in points {
            if point <= 0xffff {
                units.push(point as u16);
            } else {
                units.extend_from_slice(char::from_u32(point).unwrap().encode_utf16(&mut [0; 2]));
            }
        }
        Ok(std::ffi::OsString::from_wide(&units).into())
    }
}

fn without_authority(path: &[u32]) -> &[u32] {
    if starts_with(path, "///") {
        &path[2..]
    } else if starts_with(path, "//localhost/") {
        &path[11..]
    } else {
        path
    }
}

#[cfg(any(unix, test))]
pub(super) fn posix(path: &[u32]) -> Result<Vec<u8>> {
    let text = Text::from_codepoints(without_authority(path).iter().copied())?;
    let bytes =
        text.to_utf8_surrogateescape().map_err(|error| Error::GitHubValue(error.to_string()))?;
    Ok(percent_encoding::percent_decode(&bytes).collect())
}

#[cfg(any(windows, test))]
pub(super) fn windows(path: &[u32]) -> Result<Vec<u32>> {
    let mut path = without_authority(path);
    if starts_with(path, "///") {
        path = &path[1..];
    }
    let path: Vec<_> = path
        .iter()
        .map(|&point| {
            if point == u32::from(':') {
                u32::from('|')
            } else {
                point
            }
        })
        .collect();
    let drive = path.iter().position(|&point| point == u32::from('|'));
    let slashes = |points: &[u32]| {
        points
            .iter()
            .map(|&point| {
                if point == u32::from('/') {
                    u32::from('\\')
                } else {
                    point
                }
            })
            .collect::<Vec<_>>()
    };
    let Some(index) = drive else {
        return Ok(unquote(&slashes(&path)));
    };
    let Some(letter) = index.checked_sub(1).and_then(|index| char::from_u32(path[index])) else {
        return Err(Error::Other("invalid drive in file URL".into()));
    };
    if !letter.is_ascii_alphabetic() || path[index + 1..].contains(&u32::from('|')) {
        // nturl2path raises OSError before FileHandler's OSError wrapper.
        return Err(Error::Other("invalid drive in file URL".into()));
    }
    let mut result = vec![u32::from(letter.to_ascii_uppercase()), u32::from(':')];
    result.extend(unquote(&slashes(&path[index + 1..])));
    Ok(result)
}

/// urllib.unquote decodes each ASCII run separately and retains non-ASCII points.
pub(super) fn unquote(points: &[u32]) -> Vec<u32> {
    let mut result = Vec::new();
    let mut remaining = points;
    while !remaining.is_empty() {
        let end = remaining.iter().position(|&point| point > 0x7f).unwrap_or(remaining.len());
        let bytes: Vec<_> = remaining[..end].iter().map(|&point| point as u8).collect();
        let bytes: Vec<_> = percent_encoding::percent_decode(&bytes).collect();
        result.extend(String::from_utf8_lossy(&bytes).chars().map(u32::from));
        if end == remaining.len() {
            break;
        }
        result.push(remaining[end]);
        remaining = &remaining[end + 1..];
    }
    result
}

fn starts_with(points: &[u32], prefix: &str) -> bool {
    points.len() >= prefix.len()
        && points.iter().copied().take(prefix.len()).eq(prefix.bytes().map(u32::from))
}
