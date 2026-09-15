//! Interpret agent listings without depending on a particular JSON schema.

use serde_json::Value;

use super::agent::Scope;
use crate::util::strings::{python_trim, python_whitespace};

pub(super) fn has_named(value: &Value, name: &str) -> bool {
    match value {
        Value::Object(map) => map.iter().any(|(key, value)| {
            (["id", "name", "pluginId"].contains(&key.as_str())
                && value.as_str().is_some_and(|value| folded(value) == folded(name)))
                || has_named(value, name)
        }),
        Value::Array(items) => items.iter().any(|value| has_named(value, name)),
        _ => false,
    }
}

pub(super) fn has_line(text: &str, name: &str) -> bool {
    lines(text).any(|line| {
        python_trim(line)
            .trim_start_matches(['•', '◆', '*', '-'])
            .split(python_whitespace)
            .find(|word| !word.is_empty())
            .is_some_and(|word| folded(word) == folded(name))
    })
}

pub(super) fn pi_installed(text: &str, scope: Scope) -> bool {
    let mut in_scope = false;
    for line in lines(text) {
        let heading = folded(python_trim(line));
        match heading.as_str() {
            "user packages:" => in_scope = matches!(scope, Scope::Global),
            "project packages:" | "local packages:" => in_scope = matches!(scope, Scope::Local),
            _ if in_scope && heading.contains("hexrayssa/ida-mcp") => return true,
            _ => (),
        }
    }
    false
}

pub(super) fn same_name(value: &str, expected: &str) -> bool {
    folded(value) == folded(expected)
}

fn lines(text: &str) -> impl Iterator<Item = &str> {
    // Python str.splitlines includes these separators in addition to CR/LF.
    text.split([
        '\n', '\r', '\u{b}', '\u{c}', '\u{1c}', '\u{1d}', '\u{1e}', '\u{85}', '\u{2028}',
        '\u{2029}',
    ])
}

fn folded(value: &str) -> String {
    // All recognized names/headings are ASCII. These are the eleven non-ASCII
    // scalars whose CPython 3.13 casefold is entirely ASCII; other scalars cannot
    // produce an exact match. This is not a general Unicode casefold function.
    let mut result = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            'ß' | 'ẞ' => result.push_str("ss"),
            'ſ' => result.push('s'),
            'K' => result.push('k'),
            'ﬀ' => result.push_str("ff"),
            'ﬁ' => result.push_str("fi"),
            'ﬂ' => result.push_str("fl"),
            'ﬃ' => result.push_str("ffi"),
            'ﬄ' => result.push_str("ffl"),
            'ﬅ' | 'ﬆ' => result.push_str("st"),
            _ => result.push(character.to_ascii_lowercase()),
        }
    }
    result
}

#[cfg(test)]
mod tests;
