//! Response-header failures occur after stat and before host validation or open.

use std::fs::Metadata;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::Result;

use super::Request;

mod timestamp;

pub(super) fn validate(request: &Request, modified: f64) -> Result<()> {
    timestamp::datetime(modified)?;
    request.validate_mime_selector()
}

pub(super) fn modified(metadata: &Metadata) -> std::io::Result<f64> {
    Ok(epoch_seconds(metadata.modified()?))
}

fn epoch_seconds(time: SystemTime) -> f64 {
    let (seconds, nanoseconds) = match time.duration_since(UNIX_EPOCH) {
        Ok(duration) => (i128::from(duration.as_secs()), duration.subsec_nanos()),
        Err(error) => {
            let duration = error.duration();
            let seconds = -i128::from(duration.as_secs());
            let nanoseconds = duration.subsec_nanos();
            if nanoseconds == 0 {
                (seconds, 0)
            } else {
                (seconds - 1, 1_000_000_000 - nanoseconds)
            }
        }
    };
    // os.stat constructs its float from floor seconds and fractional nanoseconds.
    // Dividing the absolute Duration then negating can round differently.
    let seconds = seconds as f64;
    let nanoseconds = f64::from(nanoseconds);
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    {
        // The pinned macOS ARM64 CPython build contracts fill_time's expression
        // into a fused multiply-add. Native stat-bit comparisons verify this.
        1e-9_f64.mul_add(nanoseconds, seconds)
    }
    #[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
    {
        seconds + 1e-9 * nanoseconds
    }
}

#[cfg(test)]
mod tests;
