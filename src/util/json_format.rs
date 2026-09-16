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
    write_codepoints(value.chars().map(u32::from), output);
}

/// ASCII JSON escaping also represents Python's unpaired surrogate code points.
pub(crate) fn write_codepoints(points: impl IntoIterator<Item = u32>, output: &mut String) {
    output.push('"');
    for point in points {
        match point {
            0x22 => output.push_str("\\\""),
            0x5c => output.push_str("\\\\"),
            0x08 => output.push_str("\\b"),
            0x0c => output.push_str("\\f"),
            0x0a => output.push_str("\\n"),
            0x0d => output.push_str("\\r"),
            0x09 => output.push_str("\\t"),
            0x20..=0x7e => output.push(char::from_u32(point).unwrap()),
            0..=0xffff => write!(output, "\\u{point:04x}").unwrap(),
            _ => {
                let high = 0xd800 + ((point - 0x10000) >> 10);
                let low = 0xdc00 + ((point - 0x10000) & 0x3ff);
                write!(output, "\\u{high:04x}\\u{low:04x}").unwrap();
            }
        }
    }
    output.push('"');
}
