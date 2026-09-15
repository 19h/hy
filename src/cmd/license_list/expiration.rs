//! Relative expiration text with an explicit clock for boundary verification.

use chrono::{NaiveDateTime, Timelike, Utc};

pub(super) fn format(value: Option<&str>, verbose: bool) -> String {
    let now = Utc::now().naive_utc();
    let now = now.with_nanosecond(now.nanosecond() / 1000 * 1000).expect("microsecond precision");
    at(value, verbose, now)
}

fn at(value: Option<&str>, verbose: bool, now: NaiveDateTime) -> String {
    let Some(value) = value.filter(|value| !value.is_empty()) else {
        return if verbose {
            "does not expire"
        } else {
            "Never"
        }
        .into();
    };
    let Some(date) = crate::util::python_datetime::parse(&value.replace('Z', "+00:00"))
        .and_then(|date| date.utc())
    else {
        return if verbose {
            format!("expires {value}")
        } else {
            value.into()
        };
    };
    let expired = date < now;
    let days = if expired {
        now.signed_duration_since(date)
    } else {
        date.signed_duration_since(now)
    }
    .num_days();
    let (count, short, long) = if days > 365 {
        (days / 365, "y", "year")
    } else if days > 30 {
        (days / 30, "mo", "month")
    } else {
        (days, "d", "day")
    };
    if !verbose {
        return format!(
            "{count}{short}{}",
            if expired {
                " ago"
            } else {
                ""
            }
        );
    }
    if days == 0 {
        return if expired {
            "expired today"
        } else {
            "expires today"
        }
        .into();
    }
    let duration = format!(
        "{count} {long}{}",
        if count == 1 {
            ""
        } else {
            "s"
        }
    );
    if expired {
        format!("expired {duration} ago")
    } else {
        format!("expires in {duration}")
    }
}

#[cfg(test)]
mod tests;
