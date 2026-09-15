//! CPython cookie expiry dates, including its rolling century and overflow rules.

use std::sync::LazyLock;

use chrono::NaiveDate;
use regex::Regex;

use crate::util::http_headers::whitespace;

static STRICT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        r"(?-u)^[SMTWF][a-z][a-z], ([0-9]{2}) ([JFMASOND][a-z][a-z]) ",
        r"([0-9]{4}) ([0-9]{2}):([0-9]{2}):([0-9]{2}) GMT\n?$",
    ))
    .unwrap()
});
static WEEKDAY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i-u)^(?:Sun|Mon|Tue|Wed|Thu|Fri|Sat)[a-z]*,?\s*").unwrap());
static LOOSE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?x-u)^([0-9]{1,2})(?:\s+|[-/])(\w+)(?:\s+|[-/])([0-9]+)
        (?:(?:\s+|:)([0-9]{1,2}):([0-9]{2})(?::([0-9]{2}))?)?\s*
        (?:([-+]?[0-9]{2,4}|[A-Za-z]+)\s*)?(?:\(\w+\)\s*)?\n?$",
    )
    .unwrap()
});
static OFFSET: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^([-+])?([0-9]{1,2}):?([0-9]{2})?$").unwrap());

const MONTHS: [&str; 12] =
    ["jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec"];

/// An error aborts the whole Set-Cookie batch, as CookieJar.make_cookies does.
pub(super) fn parse(text: &str, current_year: i32) -> Result<Option<i64>, ()> {
    if let Some(parts) = STRICT.captures(text) {
        let month = month(&parts[2]).ok_or(())?;
        return Ok(timestamp(
            parts[3].parse().unwrap(),
            month,
            parts[1].parse().unwrap(),
            parts[4].parse().unwrap(),
            parts[5].parse().unwrap(),
            parts[6].parse().unwrap(),
        ));
    }
    let text = text.trim_start_matches(whitespace);
    let text = WEEKDAY.replace(text, "");
    let Some(parts) = LOOSE.captures(&text) else {
        return Ok(None);
    };
    // CPython's default decimal conversion limit applies before its year bound.
    if parts[3].len() > 4300 {
        return Err(());
    }
    let Some(mut year) = parts[3].parse::<i32>().ok().filter(|year| *year <= 9999) else {
        return Ok(None);
    };
    let Some(month) = month(&parts[2]) else {
        return Ok(None);
    };
    if year < 1000 {
        let difference = current_year % 100 - year;
        year += current_year - current_year % 100;
        if difference.abs() > 50 {
            year += if difference > 0 {
                100
            } else {
                -100
            };
        }
    }
    let component = |index| parts.get(index).map_or(0, |value| value.as_str().parse().unwrap());
    let day = component(1);
    let hour = component(4);
    let minute = component(5);
    let second = component(6);
    let Some(seconds) = timestamp(year, month, day, hour, minute, second) else {
        return Ok(None);
    };
    let timezone = parts.get(7).map_or("UTC", |value| value.as_str());
    Ok(offset(timezone).map(|offset| seconds - offset))
}

fn month(text: &str) -> Option<u32> {
    MONTHS
        .iter()
        .position(|month| text.eq_ignore_ascii_case(month))
        .map(|index| index as u32 + 1)
        .or_else(|| {
            if text.bytes().filter(u8::is_ascii_digit).count() > 4300
                || !text.split('_').all(|part| !part.is_empty())
            {
                return None;
            }
            text.replace('_', "").parse().ok().filter(|month| (1..=12).contains(month))
        })
}

fn timestamp(year: i32, month: u32, day: u32, hour: u32, minute: u32, second: u32) -> Option<i64> {
    if year < 1970 || !(1..=31).contains(&day) || hour > 24 || minute > 59 || second > 61 {
        return None;
    }
    // calendar.timegm starts at the first of the month and adds components.
    // Thus February 31, hour 24 and leap-second values roll forward.
    let first = NaiveDate::from_ymd_opt(year, month, 1)?.and_hms_opt(0, 0, 0)?.and_utc();
    Some(
        first.timestamp()
            + i64::from(day - 1) * 86400
            + i64::from(hour) * 3600
            + i64::from(minute) * 60
            + i64::from(second),
    )
}

fn offset(text: &str) -> Option<i64> {
    if ["GMT", "UTC", "UT", "Z"].iter().any(|zone| text.eq_ignore_ascii_case(zone)) {
        return Some(0);
    }
    let parts = OFFSET.captures(text)?;
    let hours = parts[2].parse::<i64>().ok()?;
    let minutes = parts.get(3).map_or(Some(0), |value| value.as_str().parse::<i64>().ok())?;
    let sign = if parts.get(1).is_some_and(|value| value.as_str() == "-") {
        -1
    } else {
        1
    };
    Some(sign * (hours * 3600 + minutes * 60))
}

#[cfg(test)]
mod tests;
