//! Search JSON preserves Python strings; direct text uses the native stdout codec.

use std::io::Write;

use crate::util::{json_format, python_json};
use python_json::Value;

use super::{DownloadLocation, PluginQueryReport, Result};

impl PluginQueryReport {
    pub(super) fn to_json(&self) -> Result<String> {
        let Self::Exact(report) = self else {
            return Ok(serde_json::to_string_pretty(self)?);
        };
        let plugin = python_json::parse(&serde_json::to_string(&report.plugin)?)?;
        let locations = report
            .download_locations
            .iter()
            .map(|location| {
                Value::object([
                    ("ida_versions", Value::String(location.ida_versions.as_str().into())),
                    ("platforms", Value::String(location.platforms.as_str().into())),
                    ("url", Value::String(location.url.clone())),
                ])
            })
            .collect();
        let value =
            Value::object([("plugin", plugin), ("download_locations", Value::Array(locations))]);
        Ok(json_format::python_ascii(&value, "  "))
    }
}

impl DownloadLocation {
    pub(super) fn write_text(&self) -> Result<()> {
        let mut bytes =
            format!("IDA: {}\tplatforms: {}\t", self.ida_versions, self.platforms).into_bytes();
        #[cfg(unix)]
        bytes.extend(self.url.to_utf8_surrogateescape()?);
        #[cfg(not(unix))]
        bytes.extend(self.url.to_utf8()?.into_bytes());
        bytes.push(b'\n');
        std::io::stdout().lock().write_all(&bytes)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
