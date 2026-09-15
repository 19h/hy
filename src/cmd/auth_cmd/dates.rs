//! Credential and API-key date presentation, matching their separate commands.

use chrono::{Local, NaiveDateTime, Timelike, Utc};

use crate::util::python_datetime::{self, Parsed};

fn parse(value: &str) -> Option<Parsed> {
    python_datetime::parse(&value.replace('Z', "+00:00"))
}

pub(super) fn credentials(created: &str, last_used: &str) -> (String, String) {
    match (parse(created), parse(last_used)) {
        (Some(created), Some(last_used)) => (
            created.local.format("%Y-%m-%d %H:%M").to_string(),
            last_used.local.format("%Y-%m-%d %H:%M").to_string(),
        ),
        // Upstream parses both fields in one try block: either failure makes
        // both columns use their original first sixteen characters.
        _ => (fallback(created), fallback(last_used)),
    }
}

fn fallback(value: &str) -> String {
    if value.is_empty() {
        "N/A".into()
    } else {
        value.chars().take(16).collect()
    }
}

pub(super) fn key_created(value: &str) -> String {
    parse(value)
        .map(|date| date.local.format("%b %d %Y").to_string())
        .unwrap_or_else(|| "Unknown".into())
}

pub(super) fn key_last_used(value: Option<&str>) -> String {
    relative_at(value, Utc::now().naive_utc(), Local::now().naive_local())
}

fn relative_at(value: Option<&str>, utc_now: NaiveDateTime, local_now: NaiveDateTime) -> String {
    let Some(value) = value.filter(|value| !value.is_empty()) else {
        return "never".into();
    };
    let Some(date) = parse(value) else {
        return "unknown".into();
    };
    let (now, then) = date.utc().map(|date| (utc_now, date)).unwrap_or((local_now, date.local));
    let now = now.with_nanosecond(now.nanosecond() / 1000 * 1000).expect("microsecond precision");
    let microseconds =
        now.signed_duration_since(then).num_microseconds().expect("Python year range");
    let days = microseconds.div_euclid(86_400_000_000);
    let seconds = microseconds.rem_euclid(86_400_000_000) / 1_000_000;
    let (count, unit) = if days > 0 {
        (days, "day")
    } else if seconds > 3600 {
        (seconds / 3600, "hour")
    } else if seconds > 60 {
        (seconds / 60, "minute")
    } else {
        return "just now".into();
    };
    format!(
        "{count} {unit}{} ago",
        if count == 1 {
            ""
        } else {
            "s"
        }
    )
}

#[cfg(test)]
mod tests;
