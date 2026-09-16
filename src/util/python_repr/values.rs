//! Iterative repr rendering for Python JSON, including nonfinite numbers and surrogates.

use crate::util::python_json::{Text, Value};

pub(crate) fn python_str(value: &Value) -> String {
    if let Value::String(text) = value {
        return text.diagnostic();
    }
    let mut output = String::new();
    let mut pending = vec![Item::Value(value)];
    while let Some(item) = pending.pop() {
        match item {
            Item::Punctuation(text) => output.push_str(text),
            Item::Key(text) => super::strings::write_python(text, &mut output),
            Item::Value(value) => match value {
                Value::Null => output.push_str("None"),
                Value::Bool(value) => output.push_str(if *value {
                    "True"
                } else {
                    "False"
                }),
                Value::Integer(value) => output.push_str(&value.to_string()),
                Value::Float(value) => output.push_str(&super::float_repr(*value)),
                Value::String(text) => super::strings::write_python(text, &mut output),
                Value::Array(values) => {
                    output.push('[');
                    pending.push(Item::Punctuation("]"));
                    for (index, value) in values.iter().enumerate().rev() {
                        pending.push(Item::Value(value));
                        if index != 0 {
                            pending.push(Item::Punctuation(", "));
                        }
                    }
                }
                Value::Object(fields) => {
                    output.push('{');
                    pending.push(Item::Punctuation("}"));
                    for (index, (key, value)) in fields.iter().enumerate().rev() {
                        pending.push(Item::Value(value));
                        pending.push(Item::Punctuation(": "));
                        pending.push(Item::Key(key));
                        if index != 0 {
                            pending.push(Item::Punctuation(", "));
                        }
                    }
                }
            },
        }
    }
    output
}

enum Item<'a> {
    Value(&'a Value),
    Key(&'a Text),
    Punctuation(&'static str),
}
