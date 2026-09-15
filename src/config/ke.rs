//! KE environment settings, captured with the rest of the startup configuration.

use std::path::PathBuf;

use num_bigint::{BigInt, Sign};
use num_traits::ToPrimitive;

use crate::util::{python_integer, strings::python_trim};

#[derive(Debug, Clone)]
pub struct KeSettings {
    pub downloads_dir: Option<PathBuf>,
    pub skip_confirm: bool,
    pub allow_private_hosts: bool,
    pub max_download_bytes: u64,
    pub retention_days: BigInt,
}

impl KeSettings {
    pub(super) fn from_environment() -> Self {
        let limit = std::env::var("HCLI_KE_MAX_DOWNLOAD_MB").ok();
        Self {
            downloads_dir: std::env::var_os("HCLI_KE_DOWNLOADS_DIR")
                .filter(|value| !value.is_empty())
                .map(PathBuf::from),
            skip_confirm: flag("HCLI_KE_SKIP_CONFIRM"),
            allow_private_hosts: flag("HCLI_KE_ALLOW_PRIVATE_HOSTS"),
            retention_days: std::env::var("HCLI_KE_DOWNLOADS_RETENTION_DAYS")
                .ok()
                .and_then(|value| integer(&value))
                .unwrap_or_else(|| BigInt::from(3)),
            max_download_bytes: download_limit(limit.as_deref()),
        }
    }
}

fn flag(name: &str) -> bool {
    std::env::var(name).ok().is_some_and(|value| {
        matches!(python_trim(&value).to_lowercase().as_str(), "true" | "yes" | "on" | "1")
    })
}

fn integer(value: &str) -> Option<BigInt> {
    // ENV applies str.strip before int; int alone rejects U+001C..U+001F.
    python_integer::parse(python_trim(value))
}

fn download_limit(value: Option<&str>) -> u64 {
    value
        .and_then(integer)
        .filter(|value| value.sign() == Sign::Plus)
        .and_then(|value| value.to_u64())
        .and_then(|value| value.checked_mul(1024 * 1024))
        .unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests;
