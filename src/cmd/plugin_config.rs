//! Read, validate, and update installed plugin settings.

use std::io::Read;

use serde_json::{Map, Value};

use super::plugin_cmd::ConfigCommands;
use crate::error::{Error, Result};
use crate::plugin;
use crate::util::fmt;

pub fn run(name: &str, command: ConfigCommands) -> Result<()> {
    let metadata = plugin::read_installed_metadata(name)?;
    let name = &metadata.name;
    match command {
        ConfigCommands::Get(args) => {
            let value = plugin::get_plugin_setting(name, &args.key)?
                .ok_or_else(|| Error::NotFound(format!("plugin setting: {name}.{}", args.key)))?;
            println!("{}", display_value(&value));
        }
        ConfigCommands::Set(args) => {
            let value = metadata.setting(&args.key)?.parse_value(&args.key, &args.value)?;
            plugin::set_plugin_setting(name, &args.key, value)?;
            fmt::success(&format!("Set {name}.{}", args.key));
        }
        ConfigCommands::Del(args) => {
            plugin::del_plugin_setting(name, &args.key)?;
            fmt::success(&format!("Deleted {name}.{}", args.key));
        }
        ConfigCommands::List => list(&metadata)?,
        ConfigCommands::Setup => {
            if metadata.settings.is_empty() {
                fmt::info(&format!("No settings defined for {name}"));
                return Ok(());
            }
            if !metadata.settings.values().any(plugin::PluginSetting::is_promptable) {
                fmt::info(&format!("No interactive settings for {name}"));
                return Ok(());
            }
            let values = super::plugin_settings::prompt(&metadata)?;
            plugin::set_plugin_settings(name, &values)?;
            fmt::success(&format!("Configured {name}"));
        }
        ConfigCommands::Export => {
            let settings = plugin::get_all_plugin_settings(name)?;
            println!("{}", serde_json::to_string_pretty(&settings)?);
        }
        ConfigCommands::Import(args) => {
            let input = match args.json {
                Some(input) if !input.is_empty() => input,
                _ => {
                    let mut input = String::new();
                    std::io::stdin().read_to_string(&mut input)?;
                    input
                }
            };
            let settings: Map<String, Value> = serde_json::from_str(&input)?;
            plugin::set_plugin_settings(name, &settings)?;
            fmt::success(&format!("Imported {} settings for {name}", settings.len()));
        }
    }
    Ok(())
}

fn display_value(value: &Value) -> String {
    match value {
        Value::String(value) => value.clone(),
        value => value.to_string(),
    }
}

fn list(metadata: &plugin::PluginMetadata) -> Result<()> {
    let settings = &metadata.settings;
    if settings.is_empty() {
        fmt::info(&format!("No settings defined for {}", metadata.name));
        return Ok(());
    }
    let mut table = crate::util::tui::Table::new(&["Key", "Value", "Description"]);
    for (key, descriptor) in settings {
        let value = match plugin::get_plugin_setting(&metadata.name, key)? {
            None => "<not set>".into(),
            Some(_) if descriptor.secret => "********".into(),
            Some(value) => {
                let display = display_value(&value);
                if descriptor.default.as_ref() == Some(&value) {
                    format!("{display} (default)")
                } else {
                    display
                }
            }
        };
        let mut description = descriptor.description.clone().unwrap_or_default();
        if let Some(choices) = &descriptor.choices {
            if !description.is_empty() {
                description.push(' ');
            }
            description.push_str(&format!("Choices: {}", choices.join(", ")));
        }
        table.add_row(vec![key.clone(), value, description]);
    }
    table.print();
    Ok(())
}
