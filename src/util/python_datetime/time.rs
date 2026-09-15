//! Basic/extended times and offsets with microsecond precision.

use chrono::NaiveTime;

use super::decimal;

pub(super) fn parse(value: &str) -> Option<(NaiveTime, Option<i64>)> {
    let split = value.find(['+', '-', 'Z']).unwrap_or(value.len());
    let parts = components(value.get(..split)?.as_bytes())?;
    let time = NaiveTime::from_hms_micro_opt(parts[0], parts[1], parts[2], parts[3])?;
    let offset = match value.get(split..)? {
        "" => None,
        "Z" => Some(0),
        suffix if suffix.starts_with(['+', '-']) => {
            let parts = components(suffix.get(1..)?.as_bytes())?;
            let seconds =
                i64::from(parts[0]) * 3600 + i64::from(parts[1]) * 60 + i64::from(parts[2]);
            let microseconds = seconds * 1_000_000 + i64::from(parts[3]);
            if microseconds >= 86_400_000_000 {
                return None;
            }
            Some(if suffix.starts_with('-') {
                -microseconds
            } else {
                microseconds
            })
        }
        _ => return None,
    };
    Some((time, offset))
}

fn components(bytes: &[u8]) -> Option<[u32; 4]> {
    let mut parts = [0; 4];
    let mut position = 0;
    let separated = bytes.get(2) == Some(&b':');
    for (index, part) in parts[..3].iter_mut().enumerate() {
        if index > 0 && separated {
            if bytes.get(position) != Some(&b':') {
                return None;
            }
            position += 1;
        }
        *part = decimal(bytes.get(position..position + 2)?)?;
        position += 2;
        if position == bytes.len() || matches!(bytes.get(position), Some(b'.' | b',')) {
            break;
        }
    }
    if position == bytes.len() {
        return Some(parts);
    }
    let fraction = if matches!(bytes.get(position), Some(b'.' | b',')) {
        bytes.get(position + 1..)?
    } else {
        // In basic (colon-free) times, CPython's C parser also accepts at least
        // two fractional digits following seconds without a decimal separator.
        let fraction = bytes.get(position..)?;
        if separated || fraction.len() < 2 {
            return None;
        }
        fraction
    };
    if fraction.is_empty() || !fraction.iter().all(u8::is_ascii_digit) {
        return None;
    }
    let digits = fraction.len().min(6);
    parts[3] = decimal(&fraction[..digits])? * 10_u32.pow(6 - digits as u32);
    Some(parts)
}
