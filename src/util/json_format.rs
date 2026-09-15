//! Sorted ASCII-escaped JSON for represented Pydantic-serialized values.

use std::fmt::Write;

use serde_json::Value;

use crate::util::python_repr;

pub(crate) fn sorted_ascii(value: &Value, indent: &str) -> String {
    let mut output = String::new();
    write_value(value, 0, indent, &mut output);
    output
}

fn write_value(value: &Value, depth: usize, indent: &str, output: &mut String) {
    match value {
        Value::Null => output.push_str("null"),
        Value::Bool(value) => output.push_str(if *value {
            "true"
        } else {
            "false"
        }),
        Value::Number(value) => {
            let text = python_repr::number_repr(value);
            // Pydantic's default JSON serializer converts non-finite floats to null.
            output.push_str(match text.as_str() {
                "inf" | "-inf" | "nan" => "null",
                _ => &text,
            });
        }
        Value::String(value) => write_string(value, output),
        Value::Array(values) => {
            output.push('[');
            for (index, value) in values.iter().enumerate() {
                separator(index, depth + 1, indent, output);
                write_value(value, depth + 1, indent, output);
            }
            close(']', values.is_empty(), depth, indent, output);
        }
        Value::Object(values) => {
            let mut fields: Vec<_> = values.iter().collect();
            fields.sort_unstable_by_key(|(key, _)| *key);
            output.push('{');
            for (index, (key, value)) in fields.into_iter().enumerate() {
                separator(index, depth + 1, indent, output);
                write_string(key, output);
                output.push_str(": ");
                write_value(value, depth + 1, indent, output);
            }
            close('}', values.is_empty(), depth, indent, output);
        }
    }
}

fn separator(index: usize, depth: usize, indent: &str, output: &mut String) {
    if index != 0 {
        output.push(',');
    }
    newline(depth, indent, output);
}

fn close(delimiter: char, empty: bool, depth: usize, indent: &str, output: &mut String) {
    if !empty {
        newline(depth, indent, output);
    }
    output.push(delimiter);
}

fn newline(depth: usize, indent: &str, output: &mut String) {
    output.push('\n');
    for _ in 0..depth {
        output.push_str(indent);
    }
}

fn write_string(value: &str, output: &mut String) {
    output.push('"');
    for character in value.chars() {
        match character {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\u{8}' => output.push_str("\\b"),
            '\u{c}' => output.push_str("\\f"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            ' '..='~' => output.push(character),
            other => {
                for unit in other.encode_utf16(&mut [0; 2]) {
                    write!(output, "\\u{unit:04x}").unwrap();
                }
            }
        }
    }
    output.push('"');
}
