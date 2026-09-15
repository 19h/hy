//! Python str()/repr() rendering for values decoded from standard JSON.

use serde_json::Value;

mod numbers;
mod printable;
mod strings;

pub(crate) use numbers::float_repr;

pub(crate) fn string_repr(value: &str) -> String {
    let mut output = String::new();
    strings::write_repr(value, &mut output);
    output
}

pub(crate) fn number_repr(value: &serde_json::Number) -> String {
    numbers::repr(value)
}

/// Top-level strings retain their contents; containers use repr() for children.
pub(crate) fn json_str(value: &Value) -> String {
    if let Value::String(value) = value {
        return value.clone();
    }
    let mut output = String::new();
    write_repr(value, &mut output);
    output
}

fn write_repr(value: &Value, output: &mut String) {
    match value {
        Value::Null => output.push_str("None"),
        Value::Bool(value) => output.push_str(if *value {
            "True"
        } else {
            "False"
        }),
        Value::Number(value) => output.push_str(&numbers::repr(value)),
        Value::String(value) => strings::write_repr(value, output),
        Value::Array(values) => {
            output.push('[');
            for (index, value) in values.iter().enumerate() {
                if index != 0 {
                    output.push_str(", ");
                }
                write_repr(value, output);
            }
            output.push(']');
        }
        Value::Object(values) => {
            output.push('{');
            for (index, (key, value)) in values.iter().enumerate() {
                if index != 0 {
                    output.push_str(", ");
                }
                strings::write_repr(key, output);
                output.push_str(": ");
                write_repr(value, output);
            }
            output.push('}');
        }
    }
}

#[cfg(test)]
mod tests;
