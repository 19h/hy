//! Shared-file byte units, shared by listing, download and deletion reports.

use crate::api::Asset;
use crate::error::{Error, Result};

pub(super) fn size(file: &Asset) -> Result<String> {
    let bytes = file
        .size
        .to_u64()
        .ok_or_else(|| Error::Other("shared-file size cannot be formatted".into()))?;
    if bytes == 0 {
        return Ok("0 B".into());
    }
    let unit = ((bytes as f64).ln() / 1024_f64.ln()).floor() as usize;
    let name = ["B", "KB", "MB", "GB", "TB"]
        .get(unit)
        .ok_or_else(|| Error::Other("shared-file size exceeds the display units".into()))?;
    Ok(format!("{:.1} {name}", bytes as f64 / 1024_f64.powi(unit as i32)))
}
