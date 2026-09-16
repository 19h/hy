//! datetime.fromtimestamp(..., UTC), as used by email.utils.formatdate.
//!
//! The CPython adaptation and license notice are in util/python_path/LICENSE.

use chrono::{NaiveDate, NaiveDateTime};

use crate::error::{Error, Result};

pub(super) fn datetime(value: f64) -> Result<NaiveDateTime> {
    let (seconds, microseconds) = timeval(value)?;
    let calendar = calendar(seconds).map_err(super::super::url_error)?;
    // CPython's tm_year + 1900 is a C int operation, including its extreme range.
    let year = calendar.tm_year.wrapping_add(1900);
    if !(1..=9999).contains(&year) {
        return Err(Error::GitHubValue(format!("year {year} is out of range")));
    }
    NaiveDate::from_ymd_opt(year, (calendar.tm_mon + 1) as u32, calendar.tm_mday as u32)
        .and_then(|date| {
            date.and_hms_micro_opt(
                calendar.tm_hour as u32,
                calendar.tm_min as u32,
                calendar.tm_sec.min(59) as u32,
                microseconds,
            )
        })
        .ok_or_else(|| {
            Error::GitHubValue("invalid calendar fields in file modification time".into())
        })
}

fn timeval(value: f64) -> Result<(libc::time_t, u32)> {
    if value.is_nan() {
        return Err(Error::GitHubValue("Invalid value NaN (not a number)".into()));
    }
    let mut seconds = value.trunc();
    let mut microseconds = (value.fract() * 1_000_000.0).round_ties_even();
    if microseconds >= 1_000_000.0 {
        seconds += 1.0;
        microseconds -= 1_000_000.0;
    } else if microseconds < 0.0 {
        seconds -= 1.0;
        microseconds += 1_000_000.0;
    }
    // The exclusive upper bound avoids rounding time_t::MAX up before casting.
    let minimum = libc::time_t::MIN as f64;
    if !(minimum <= seconds && seconds < -minimum) {
        return Err(Error::Other("timestamp out of range for platform time_t".into()));
    }
    Ok((seconds as libc::time_t, microseconds as u32))
}

#[cfg(unix)]
fn calendar(seconds: libc::time_t) -> std::io::Result<libc::tm> {
    let mut result = std::mem::MaybeUninit::<libc::tm>::uninit();
    // gmtime_r initializes the caller-owned tm on success; neither pointer
    // escapes this call, and the uninitialized result is never read on failure.
    if unsafe { libc::gmtime_r(&seconds, result.as_mut_ptr()) }.is_null() {
        let error = std::io::Error::last_os_error();
        return Err(if error.raw_os_error() == Some(0) {
            std::io::Error::from_raw_os_error(libc::EINVAL)
        } else {
            error
        });
    }
    Ok(unsafe { result.assume_init() })
}

#[cfg(windows)]
fn calendar(seconds: libc::time_t) -> std::io::Result<libc::tm> {
    let mut result = std::mem::MaybeUninit::<libc::tm>::uninit();
    // The CRT writes tm on success and returns the error number otherwise.
    let error = unsafe { libc::gmtime_s(result.as_mut_ptr(), &seconds) };
    if error != 0 {
        return Err(std::io::Error::from_raw_os_error(error));
    }
    Ok(unsafe { result.assume_init() })
}
