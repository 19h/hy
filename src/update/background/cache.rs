//! Upstream update_check.json location, timestamp policy and advisory writes.

use std::path::PathBuf;

use chrono::{NaiveDateTime, SecondsFormat, TimeDelta, Timelike, Utc};
use serde_json::Value;

pub(super) struct Cache {
    pub(super) path: PathBuf,
}

impl Cache {
    pub(super) fn for_binary(binary_name: &str) -> Self {
        // Unlike download caches, platformdirs.user_cache_dir does not read
        // HCLI_CACHE_DIR and appends the application name to XDG_CACHE_HOME.
        let base = dirs::cache_dir().unwrap_or_else(|| PathBuf::from(".cache"));
        let directory = if cfg!(windows) {
            base.join("hex-rays").join(binary_name).join("Cache")
        } else {
            base.join(binary_name)
        };
        Self {
            path: directory.join("update_check.json"),
        }
    }

    pub(super) fn should_check(&self, now: NaiveDateTime) -> bool {
        let recent = std::fs::read(&self.path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
            .is_some_and(|data| is_recent(&data, now));
        !recent
    }

    pub(super) fn save(&self, latest: Option<&str>) {
        let Some(parent) = self.path.parent() else {
            return;
        };
        if std::fs::create_dir_all(parent).is_err() {
            return;
        }
        let data = serde_json::json!({
            "last_check": Utc::now().to_rfc3339_opts(SecondsFormat::Micros, false),
            "latest_version": latest,
        });
        let _ = std::fs::write(&self.path, data.to_string());
    }
}

fn is_recent(data: &Value, now: NaiveDateTime) -> bool {
    let Some(last_check) = data
        .get("last_check")
        .and_then(Value::as_str)
        .and_then(crate::util::python_datetime::parse)
        .and_then(|parsed| parsed.utc())
    else {
        return false;
    };
    let now = now.with_nanosecond(now.nanosecond() / 1000 * 1000).unwrap();
    now - last_check <= TimeDelta::days(1)
}

#[cfg(test)]
mod tests;
