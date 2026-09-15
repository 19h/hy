//! CPython 3.13 ISO datetime syntax, retaining naive versus aware values.

use chrono::{NaiveDateTime, TimeDelta};

mod date;
mod time;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parsed {
    pub local: NaiveDateTime,
    pub offset_microseconds: Option<i64>,
}

impl Parsed {
    pub fn utc(&self) -> Option<NaiveDateTime> {
        self.local.checked_sub_signed(TimeDelta::microseconds(self.offset_microseconds?))
    }
}

pub fn parse(value: &str) -> Option<Parsed> {
    let end = date::separator(value.as_bytes())?;
    let date = date::parse(value.get(..end)?)?;
    let tail = value.get(end..)?;
    if tail.is_empty() {
        return Some(Parsed {
            local: date.and_hms_opt(0, 0, 0)?,
            offset_microseconds: None,
        });
    }
    // Python accepts any single Unicode character between date and time.
    let time_start = tail.chars().next()?.len_utf8();
    let (time, offset_microseconds) = time::parse(tail.get(time_start..)?)?;
    Some(Parsed {
        local: date.and_time(time),
        offset_microseconds,
    })
}

fn decimal(bytes: &[u8]) -> Option<u32> {
    if bytes.is_empty() || !bytes.iter().all(u8::is_ascii_digit) {
        return None;
    }
    Some(bytes.iter().fold(0, |value, digit| value * 10 + u32::from(digit - b'0')))
}

#[cfg(test)]
mod tests;
