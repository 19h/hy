//! Read and update IDA plugin configuration without discarding malformed user data.

use std::io::Write;

use serde_json::{Map, Value, json};

use crate::error::{Error, Result};
use crate::ida::ida_user_dir;

pub fn read_ida_config() -> Result<Value> {
    let path = ida_user_dir().join("ida-config.json");
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(json!({})),
        Err(error) => return Err(error.into()),
    };
    let config: Value = serde_json::from_slice(&bytes)?;
    validate_config(&config)?;
    Ok(config)
}

/// Change idalib's installation independently of the CLI's registered default.
pub fn set_ida_installation_directory(directory: &std::path::Path) -> Result<()> {
    let mut config = read_ida_config()?;
    if config.get("Paths").is_none() {
        config["Paths"] = json!({});
    }
    config["Paths"]["ida-install-dir"] = json!(directory);
    write_ida_config(&config)
}

fn require_object<'a>(value: &'a Value, location: &str) -> Result<&'a Map<String, Value>> {
    value
        .as_object()
        .ok_or_else(|| Error::Other(format!("ida-config.json: {location} must be a JSON object")))
}

fn validate_config(config: &Value) -> Result<()> {
    let root = require_object(config, "root")?;
    for section in ["Paths", "Settings", "Plugins"] {
        if let Some(value) = root.get(section) {
            require_object(value, section)?;
        }
    }
    if let Some(plugins) = root.get("Plugins").and_then(Value::as_object) {
        for (name, plugin) in plugins {
            let location = format!("Plugins.{name}");
            let plugin = require_object(plugin, &location)?;
            if let Some(settings) = plugin.get("settings") {
                require_object(settings, &format!("{location}.settings"))?;
            }
        }
    }
    Ok(())
}

/// Replace the file atomically after serialization and validation succeed.
pub fn write_ida_config(config: &Value) -> Result<()> {
    validate_config(config)?;
    let directory = ida_user_dir();
    std::fs::create_dir_all(&directory)?;
    let path = directory.join("ida-config.json");
    let mut temporary = tempfile::NamedTempFile::new_in(&directory)?;
    if let Ok(metadata) = std::fs::metadata(&path) {
        temporary.as_file().set_permissions(metadata.permissions())?;
    }
    serde_json::to_writer_pretty(&mut temporary, config)?;
    temporary.write_all(b"\n")?;
    temporary.as_file().sync_all()?;
    temporary.persist(path).map_err(|error| error.error)?;
    Ok(())
}

pub fn get_plugin_setting(plugin_name: &str, key: &str) -> Result<Option<Value>> {
    let metadata = super::read_installed_metadata(plugin_name)?;
    let descriptor = metadata.setting(key)?;
    let value = get_all_plugin_settings(&metadata.name)?
        .get(key)
        .cloned()
        .or_else(|| descriptor.default.clone());
    if let Some(value) = &value {
        descriptor.validate_value(key, value)?;
    }
    Ok(value)
}

pub fn set_plugin_setting(plugin_name: &str, key: &str, value: Value) -> Result<()> {
    set_plugin_settings(plugin_name, &Map::from_iter([(key.into(), value)]))
}

/// Validate the complete update before replacing any configuration bytes.
pub fn set_plugin_settings(plugin_name: &str, values: &Map<String, Value>) -> Result<()> {
    let metadata = super::read_installed_metadata(plugin_name)?;
    for (key, value) in values {
        metadata.setting(key)?.validate_value(key, value)?;
    }
    let mut config = read_ida_config()?;
    if values
        .iter()
        .all(|(key, value)| config["Plugins"][&metadata.name]["settings"].get(key) == Some(value))
    {
        return Ok(());
    }
    if config["Plugins"].is_null() {
        config["Plugins"] = json!({});
    }
    let plugin = &mut config["Plugins"][&metadata.name];
    if plugin.is_null() {
        *plugin = json!({});
    }
    if plugin["settings"].is_null() {
        plugin["settings"] = json!({});
    }
    for (key, value) in values {
        plugin["settings"][key] = value.clone();
    }
    write_ida_config(&config)
}

pub fn del_plugin_setting(plugin_name: &str, key: &str) -> Result<()> {
    let metadata = super::read_installed_metadata(plugin_name)?;
    let descriptor = metadata.setting(key)?;
    // Match the upstream deletion contract, which treats false and the empty
    // string as absent defaults for required settings.
    let has_default = match descriptor.default.as_ref() {
        Some(Value::Bool(value)) => *value,
        Some(Value::String(value)) => !value.is_empty(),
        _ => false,
    };
    if descriptor.required && !has_default {
        return Err(Error::Other(format!(
            "cannot delete required setting without default: {}.{key}",
            metadata.name
        )));
    }
    let mut config = read_ida_config()?;
    if let Some(settings) = config
        .get_mut("Plugins")
        .and_then(|plugins| plugins.get_mut(&metadata.name))
        .and_then(|plugin| plugin.get_mut("settings"))
        .and_then(Value::as_object_mut)
        && settings.remove(key).is_some()
    {
        write_ida_config(&config)?;
    } else {
        return Err(Error::NotFound(format!("plugin setting: {}.{key}", metadata.name)));
    }
    Ok(())
}

pub fn get_all_plugin_settings(plugin_name: &str) -> Result<Map<String, Value>> {
    let config = read_ida_config()?;
    Ok(config["Plugins"][plugin_name]["settings"].as_object().cloned().unwrap_or_default())
}
