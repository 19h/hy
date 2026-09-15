//! Collect a complete setting form before the caller persists any answers.

use inquire::validator::{StringValidator, Validation};
use inquire::{Confirm, Password, Select, Text};
use serde_json::{Map, Value};

use crate::error::{Error, Result};
use crate::plugin::{self, PluginMetadata, PluginSetting, SettingType};
use crate::util::tui;

struct Question<'a> {
    key: &'a str,
    descriptor: &'a PluginSetting,
    existing: Option<&'a Value>,
    choice: Option<Choice<'a>>,
}

struct Choice<'a> {
    options: &'a [String],
    selected: usize,
}

pub fn prompt(metadata: &PluginMetadata) -> Result<Map<String, Value>> {
    let settings: Vec<_> =
        metadata.settings.iter().filter(|(_, descriptor)| descriptor.is_promptable()).collect();
    if settings.is_empty() {
        return Ok(Map::new());
    }
    if !tui::is_interactive() {
        return Err(Error::Other("setup requires an interactive terminal".into()));
    }
    let existing = plugin::get_all_plugin_settings(&metadata.name)?;
    let plural = if settings.len() == 1 {
        ""
    } else {
        "s"
    };
    eprintln!("configure {} setting{plural}:", settings.len());
    // Questionary constructs the entire form before asking its first question.
    // An obsolete stored choice therefore fails before collecting any answers.
    let questions = settings
        .into_iter()
        .map(|(key, descriptor)| prepare_question(key, descriptor, existing.get(key)))
        .collect::<Result<Vec<_>>>()?;
    let mut answers = Map::new();
    for question in questions {
        if let Some(value) = prompt_one(&question)?
            && question.descriptor.default.as_ref() != Some(&value)
        {
            answers.insert(question.key.into(), value);
        }
    }
    Ok(answers)
}

fn prepare_question<'a>(
    key: &'a str,
    descriptor: &'a PluginSetting,
    existing: Option<&'a Value>,
) -> Result<Question<'a>> {
    let existing = existing.filter(|value| !value.is_null());
    let choice = descriptor
        .choices
        .as_deref()
        .filter(|options| descriptor.setting_type == SettingType::String && !options.is_empty())
        .map(|options| -> Result<Choice<'_>> {
            let selected = match existing.or(descriptor.default.as_ref()) {
                Some(value) => {
                    let value = prompt_text(value);
                    options.iter().position(|option| option == &value).ok_or_else(|| {
                        Error::Other(format!("current value for {key} is not an available choice"))
                    })?
                }
                None => 0,
            };
            Ok(Choice {
                options,
                selected,
            })
        })
        .transpose()?;
    Ok(Question {
        key,
        descriptor,
        existing,
        choice,
    })
}

fn prompt_one(question: &Question<'_>) -> Result<Option<Value>> {
    let Question {
        key,
        descriptor,
        existing,
        choice,
    } = question;
    let value = match descriptor.setting_type {
        SettingType::Boolean => Value::Bool(prompt_boolean(descriptor, *existing)?),
        SettingType::String => {
            let text = match choice {
                Some(choice) => prompt_choice(descriptor, choice)?,
                None => {
                    let text = prompt_string(key, descriptor, *existing)?;
                    if descriptor.secret && existing.is_some() && text.is_empty() {
                        return Ok(None);
                    }
                    text
                }
            };
            Value::String(text)
        }
    };
    // The caller validates the completed form before persistence. In particular,
    // an optional blank answer must not stop later questions from being asked.
    Ok(Some(value))
}

fn prompt_boolean(descriptor: &PluginSetting, existing: Option<&Value>) -> Result<bool> {
    let default = existing
        .and_then(Value::as_bool)
        .or_else(|| descriptor.default.as_ref().and_then(Value::as_bool))
        .unwrap_or(false);
    Confirm::new(&descriptor.name)
        .with_render_config(tui::setting_theme())
        .with_default(default)
        .prompt()
        .map_err(prompt_error)
}

fn prompt_choice(descriptor: &PluginSetting, choice: &Choice<'_>) -> Result<String> {
    Select::new(&descriptor.name, choice.options.to_vec())
        .with_render_config(tui::setting_theme())
        .with_starting_cursor(choice.selected)
        .without_filtering()
        .without_help_message()
        .prompt()
        .map_err(prompt_error)
}

fn prompt_string(
    key: &str,
    descriptor: &PluginSetting,
    existing: Option<&Value>,
) -> Result<String> {
    let keep_existing_secret = descriptor.secret && existing.is_some();
    let validate = string_validator(key.to_owned(), descriptor.clone(), keep_existing_secret);
    if descriptor.secret {
        let label = if keep_existing_secret {
            format!("{} (leave blank to keep current)", descriptor.name)
        } else {
            descriptor.name.clone()
        };
        Password::new(&label)
            .with_render_config(tui::setting_theme())
            .without_confirmation()
            .with_validator(validate)
            .prompt()
            .map_err(prompt_error)
    } else {
        let initial = existing.or(descriptor.default.as_ref()).map(prompt_text).unwrap_or_default();
        Text::new(&descriptor.name)
            .with_render_config(tui::setting_theme())
            .with_initial_value(&initial)
            .with_validator(validate)
            .prompt()
            .map_err(prompt_error)
    }
}

fn string_validator(
    key: String,
    descriptor: PluginSetting,
    keep_existing_secret: bool,
) -> impl StringValidator {
    move |value: &str| -> std::result::Result<Validation, inquire::CustomUserError> {
        if value.is_empty() {
            return Ok(if keep_existing_secret || !descriptor.required {
                Validation::Valid
            } else {
                Validation::Invalid("This field is required".into())
            });
        }
        Ok(match descriptor.validate_value(&key, &Value::String(value.into())) {
            Ok(()) => Validation::Valid,
            Err(error) => Validation::Invalid(error.to_string().into()),
        })
    }
}

fn prompt_error(error: inquire::InquireError) -> Error {
    match error {
        inquire::InquireError::OperationCanceled | inquire::InquireError::OperationInterrupted => {
            Error::Other(format!("configuration cancelled: {error}"))
        }
        error => Error::Other(format!("configuration prompt failed: {error}")),
    }
}

fn prompt_text(value: &Value) -> String {
    match value {
        Value::String(value) => value.clone(),
        Value::Bool(true) => "True".into(),
        Value::Bool(false) => "False".into(),
        value => value.to_string(),
    }
}
