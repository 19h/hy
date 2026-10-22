//! CPython uses distinct binary64 conversions for time.time() and os.stat().

use std::fs::Metadata;
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(not(windows))]
use crate::error::Error;
use crate::error::Result;

const NANOS_PER_SECOND: i64 = 1_000_000_000;

pub fn now() -> Result<f64> {
    clock(SystemTime::now())
}

pub fn clock(time: SystemTime) -> Result<f64> {
    let (seconds, nanoseconds) = parts(time);
    #[cfg(not(windows))]
    let nanoseconds = i64::try_from(seconds)
        .ok()
        .and_then(|seconds| seconds.checked_mul(NANOS_PER_SECOND))
        .and_then(|seconds| seconds.checked_add(i64::from(nanoseconds)))
        .ok_or_else(|| Error::Other("timestamp out of range for C PyTime_t".into()))?;
    #[cfg(windows)]
    let nanoseconds = {
        // CPython's FILETIME conversion uses unsigned arithmetic then assigns
        // the result to signed PyTime_t, retaining the low 64 bits.
        (seconds * i128::from(NANOS_PER_SECOND) + i128::from(nanoseconds)) as i64
    };
    Ok(clock_seconds(nanoseconds))
}

fn clock_seconds(nanoseconds: i64) -> f64 {
    // PyTime_AsSecondsDouble preserves exact integral seconds before converting.
    if nanoseconds % NANOS_PER_SECOND == 0 {
        (nanoseconds / NANOS_PER_SECOND) as f64
    } else {
        nanoseconds as f64 / NANOS_PER_SECOND as f64
    }
}

pub fn modified(metadata: &Metadata) -> std::io::Result<f64> {
    let (seconds, nanoseconds) = parts(metadata.modified()?);
    let seconds = seconds as f64;
    let nanoseconds = f64::from(nanoseconds);
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    {
        // The pinned macOS ARM64 CPython build contracts fill_time's expression
        // into a fused multiply-add. Native stat-bit comparisons verify this.
        Ok(1e-9_f64.mul_add(nanoseconds, seconds))
    }
    #[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
    {
        Ok(seconds + 1e-9 * nanoseconds)
    }
}

/// Floor seconds and nonnegative fractional nanoseconds, including before 1970.
fn parts(time: SystemTime) -> (i128, u32) {
    match time.duration_since(UNIX_EPOCH) {
        Ok(duration) => (i128::from(duration.as_secs()), duration.subsec_nanos()),
        Err(error) => {
            let duration = error.duration();
            let seconds = -i128::from(duration.as_secs());
            let nanoseconds = duration.subsec_nanos();
            if nanoseconds == 0 {
                (seconds, 0)
            } else {
                (seconds - 1, NANOS_PER_SECOND as u32 - nanoseconds)
            }
        }
    }
}

#[cfg(test)]
mod tests;
