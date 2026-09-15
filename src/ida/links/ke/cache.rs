//! Best-effort KE retention cleanup, after confirmation and host validation.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use num_bigint::BigInt;
use num_traits::ToPrimitive;

use crate::error::{Error, Result};

pub(super) fn cleanup(directory: &Path, retention_days: &BigInt) -> Result<()> {
    if !directory.exists() {
        return Ok(());
    }
    let cutoff = cutoff(seconds(SystemTime::now()), retention_days)?;
    let _ = cleanup_before(directory, cutoff);
    Ok(())
}

fn cutoff(now: f64, retention_days: &BigInt) -> Result<f64> {
    // Python multiplies arbitrary-precision days before converting for float
    // subtraction. Its conversion overflow is outside the best-effort walk.
    let interval =
        (retention_days * 86_400_u32).to_f64().filter(|value| value.is_finite()).ok_or_else(
            || Error::Other("KE retention period: int too large to convert to float".into()),
        )?;
    Ok(now - interval)
}

fn cleanup_before(directory: &Path, cutoff: f64) -> std::io::Result<()> {
    if !directory.exists() {
        return Ok(());
    }
    let mut entries = collect(directory)?;
    entries.sort_unstable_by(|left, right| right.cmp(left));
    for path in &entries {
        if path.is_file() && seconds(fs::metadata(path)?.modified()?) < cutoff {
            fs::remove_file(path)?;
        }
    }
    // Reverse lexical order visits descendants before their parent directory.
    for path in entries {
        if path.is_dir() && fs::read_dir(&path)?.next().transpose()?.is_none() {
            fs::remove_dir(path)?;
        }
    }
    Ok(())
}

fn collect(directory: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut pending = vec![directory.to_owned()];
    let mut entries = Vec::new();
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(directory)? {
            let entry = entry?;
            // Like pathlib.rglob, do not recurse through directory symlinks.
            if entry.file_type()?.is_dir() {
                pending.push(entry.path());
            }
            entries.push(entry.path());
        }
    }
    Ok(entries)
}

fn seconds(time: SystemTime) -> f64 {
    match time.duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_secs_f64(),
        Err(error) => -error.duration().as_secs_f64(),
    }
}

#[cfg(test)]
mod tests;
