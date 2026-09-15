//! Calendar/week dates and Python's ambiguous compact-week separator rule.

use chrono::{Datelike, NaiveDate, Weekday};

use super::decimal;

pub(super) fn separator(value: &[u8]) -> Option<usize> {
    if value.len() < 7 {
        return None;
    }
    if value.len() == 7 {
        return Some(7);
    }
    if value[4] == b'-' {
        if value[5] != b'W' {
            return Some(10);
        }
        if value.get(8) == Some(&b'-') {
            if value.len() == 9 {
                return None;
            }
            return Some(if value.get(10).is_some_and(u8::is_ascii_digit) {
                8
            } else {
                10
            });
        }
        return Some(8);
    }
    if value[4] != b'W' {
        return Some(8);
    }
    let end = (7..value.len()).find(|&index| !value[index].is_ascii_digit()).unwrap_or(value.len());
    Some(if end < 9 {
        end
    } else if end % 2 == 0 {
        7
    } else {
        8
    })
}

pub(super) fn parse(value: &str) -> Option<NaiveDate> {
    let bytes = value.as_bytes();
    if !matches!(bytes.len(), 7 | 8 | 10) {
        return None;
    }
    let year = decimal(&bytes[..4])? as i32;
    if !(1..=9999).contains(&year) {
        return None;
    }
    let separated = bytes[4] == b'-';
    let start = 4 + usize::from(separated);
    let date = if bytes[start] == b'W' {
        let week = decimal(bytes.get(start + 1..start + 3)?)?;
        let suffix = bytes.get(start + 3..)?;
        let day = if suffix.is_empty() {
            1
        } else {
            let digits = if separated {
                suffix.strip_prefix(b"-")?
            } else {
                suffix
            };
            if digits.len() != 1 {
                return None;
            }
            decimal(digits)?
        };
        let weekday = *[
            Weekday::Mon,
            Weekday::Tue,
            Weekday::Wed,
            Weekday::Thu,
            Weekday::Fri,
            Weekday::Sat,
            Weekday::Sun,
        ]
        .get(day.checked_sub(1)? as usize)?;
        NaiveDate::from_isoywd_opt(year, week, weekday)?
    } else {
        let month = decimal(bytes.get(start..start + 2)?)?;
        let suffix = bytes.get(start + 2..)?;
        let digits = if separated {
            suffix.strip_prefix(b"-")?
        } else {
            suffix
        };
        if digits.len() != 2 {
            return None;
        }
        NaiveDate::from_ymd_opt(year, month, decimal(digits)?)?
    };
    (1..=9999).contains(&date.year()).then_some(date)
}
