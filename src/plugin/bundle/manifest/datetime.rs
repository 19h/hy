//! Pydantic 2.12.5 JSON datetime conversion and Python `datetime.isoformat()` output.

use std::fmt;

use serde::Deserialize;
use serde_json::Value;
use speedate::{
    Date, DateTime, DateTimeConfig, MicrosecondsPrecisionOverflowBehavior, Time, TimeConfig,
};
use strum::EnumMessage;

pub(super) fn deserialize<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<String, D::Error> {
    parse(&Value::deserialize(deserializer)?).map_err(serde::de::Error::custom)
}

fn parse(value: &Value) -> Result<String, ValidationError> {
    let config = DateTimeConfig {
        time_config: TimeConfig {
            microseconds_precision_overflow_behavior:
                MicrosecondsPrecisionOverflowBehavior::Truncate,
            unix_timestamp_offset: Some(0),
        },
        ..Default::default()
    };
    let datetime = match value {
        Value::String(text) => parse_text(text, &config)?,
        Value::Number(number) => {
            let raw = number.to_string();
            if raw.contains(['.', 'e', 'E']) {
                let timestamp = raw.parse::<f64>().map_err(|_| ValidationError::Type)?;
                DateTime::from_float_with_config(timestamp, &config)
            } else {
                let timestamp = number.as_i64().ok_or(ValidationError::Type)?;
                // Jiter promotes JSON integers with more than 18 digits to its
                // BigInt variant, which Pydantic's datetime validator rejects.
                if timestamp.unsigned_abs() >= 1_000_000_000_000_000_000 {
                    return Err(ValidationError::Type);
                }
                DateTime::from_timestamp_with_config(timestamp, 0, &config)
            }
            .map_err(|error| ValidationError::parsing(error, false))?
        }
        _ => return Err(ValidationError::Type),
    };
    if datetime.date.year == 0 {
        return Err(ValidationError::Parsing {
            from_date: false,
            reason: "year 0 is out of range",
        });
    }
    let mut formatted = datetime.to_string();
    if datetime.time.tz_offset == Some(0) {
        // Speedate renders UTC as Z; Python isoformat renders +00:00.
        formatted.pop();
        formatted.push_str("+00:00");
    }
    Ok(formatted)
}

fn parse_text(text: &str, config: &DateTimeConfig) -> Result<DateTime, ValidationError> {
    if let Ok(datetime) = DateTime::parse_bytes_with_config(text.as_bytes(), config) {
        return Ok(datetime);
    }
    // Pydantic's lax datetime validator retries strings as dates. The retry's
    // error replaces the initial datetime error; accepted dates become naive midnight.
    let date = Date::parse_bytes(text.as_bytes())
        .map_err(|error| ValidationError::parsing(error, true))?;
    Ok(DateTime {
        date,
        time: Time {
            hour: 0,
            minute: 0,
            second: 0,
            microsecond: 0,
            tz_offset: None,
        },
    })
}

#[derive(Debug)]
enum ValidationError {
    Type,
    Parsing {
        from_date: bool,
        reason: &'static str,
    },
}

impl ValidationError {
    fn parsing(error: speedate::ParseError, from_date: bool) -> Self {
        Self::Parsing {
            from_date,
            reason: error.get_documentation().unwrap_or_default(),
        }
    }
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Type => f.write_str("Input should be a valid datetime"),
            Self::Parsing {
                from_date,
                reason,
            } => {
                let kind = if *from_date {
                    "datetime or date"
                } else {
                    "datetime"
                };
                write!(f, "Input should be a valid {kind}, {reason}")
            }
        }
    }
}

#[cfg(test)]
mod tests;
