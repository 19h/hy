//! Source header precedence, Python integers and wall-clock wait calculations.

use std::time::Duration;

use num_bigint::BigInt;
use num_traits::ToPrimitive;
use reqwest::header::HeaderMap;

use crate::error::{Error, Result};
use crate::util::python_integer;

pub(super) fn reactive(
    headers: &HeaderMap,
    attempt: u32,
    now: impl Fn() -> f64,
) -> Result<Duration> {
    if let Some(value) = header(headers, "retry-after") {
        let seconds = integer(&value, "retry-after")?.clamp(60.into(), 3600.into());
        return Ok(Duration::from_secs(seconds.to_u64().expect("bounded retry delay")));
    }
    // The implementation uses a present reset header regardless of remaining.
    if let Some(value) = header(headers, "x-ratelimit-reset") {
        let seconds = reset_delta(&value, now)?.clamp(60.0, 3600.0);
        return Ok(Duration::from_secs_f64(seconds));
    }
    Ok(Duration::from_secs((60 * 2_u64.pow(attempt - 1)).min(3600)))
}

pub(super) fn proactive(headers: &HeaderMap, now: impl Fn() -> f64) -> Result<Option<Duration>> {
    let (Some(remaining), Some(reset)) =
        (header(headers, "x-ratelimit-remaining"), header(headers, "x-ratelimit-reset"))
    else {
        return Ok(None);
    };
    if integer(&remaining, "x-ratelimit-remaining")? > BigInt::from(2) {
        return Ok(None);
    }
    let seconds = reset_delta(&reset, now)?.max(30.0);
    Ok((seconds < 3600.0).then(|| Duration::from_secs_f64(seconds)))
}

fn reset_delta(value: &str, now: impl Fn() -> f64) -> Result<f64> {
    let reset = integer(value, "x-ratelimit-reset")?
        .to_f64()
        .filter(|value| value.is_finite())
        .ok_or_else(|| Error::Other("GitHub rate-limit timestamp exceeds binary64 range".into()))?;
    Ok(reset - now())
}

fn integer(value: &str, name: &str) -> Result<BigInt> {
    python_integer::parse(value)
        .ok_or_else(|| Error::GitHubValue(format!("invalid GitHub rate-limit header: {name}")))
}

fn header(headers: &HeaderMap, name: &str) -> Option<String> {
    // urllib's HTTPMessage returns the first field and decodes HTTP bytes as
    // ISO-8859-1. HTTPX's response-wide UTF-8 fallback does not apply here.
    let bytes = headers.get(name)?.as_bytes();
    (!bytes.is_empty()).then(|| bytes.iter().map(|byte| char::from(*byte)).collect())
}
