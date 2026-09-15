//! Shared setting parsing and interactive configuration for install and setup.

use serde_json::{Map, Value};

use crate::error::{Error, Result};
use crate::plugin::{self, PluginMetadata};

mod prompt;

pub use prompt::prompt;

pub fn for_install(metadata: &PluginMetadata, arguments: &[String]) -> Result<Map<String, Value>> {
    let settings = &metadata.settings;
    if settings.is_empty() {
        return Ok(Map::new());
    }
    let existing = plugin::get_all_plugin_settings(&metadata.name)?;
    if !arguments.is_empty() {
        let mut values = Map::new();
        for argument in arguments {
            let (key, raw) = argument.split_once('=').ok_or_else(|| {
                Error::Other(format!("invalid setting '{argument}'; expected key=value"))
            })?;
            let value = metadata.setting(key)?.parse_value(key, raw)?;
            values.insert(key.into(), value);
        }
        // Upstream resolves repeated --config keys before validating constraints.
        for (key, value) in &values {
            settings[key].validate_value(key, value)?;
        }
        values.retain(|key, value| settings[key].default.as_ref() != Some(value));
        return Ok(values);
    }
    if crate::util::tui::is_interactive() {
        return prompt(metadata);
    }
    let missing: Vec<_> = settings
        .iter()
        .filter(|(key, descriptor)| {
            descriptor.required && descriptor.default.is_none() && !existing.contains_key(*key)
        })
        .map(|(key, _)| format!("--config {key}=VALUE"))
        .collect();
    if !missing.is_empty() {
        return Err(Error::Other(format!("required settings missing: {}", missing.join(", "))));
    }
    Ok(Map::new())
}
