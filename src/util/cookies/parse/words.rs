//! CookieJar.split_header_words grammar for legacy Set-Cookie2 headers.

use crate::util::http_headers::whitespace;

use super::Attribute;

pub(crate) fn split(mut text: &str) -> Vec<Vec<Attribute>> {
    let mut cookies = Vec::new();
    let mut attributes = Vec::new();
    while !text.is_empty() {
        let start = text.trim_start_matches(whitespace);
        let end =
            start.find(|c| whitespace(c) || matches!(c, '=' | ';' | ',')).unwrap_or(start.len());
        if end != 0 {
            let key = start[..end].to_owned();
            text = &start[end..];
            let value = if let Some(value) = text.trim_start_matches(whitespace).strip_prefix('=') {
                let value = value.trim_start_matches(whitespace);
                if let Some((decoded, remaining)) = quoted(value) {
                    text = remaining;
                    Some(decoded)
                } else {
                    let end = value
                        .find(|c| whitespace(c) || matches!(c, ';' | ','))
                        .unwrap_or(value.len());
                    text = &value[end..];
                    Some(value[..end].to_owned())
                }
            } else {
                None
            };
            attributes.push(Attribute {
                key,
                value,
                expiry: None,
            });
        } else if let Some(remaining) = start.strip_prefix(',') {
            text = remaining;
            if !attributes.is_empty() {
                cookies.push(std::mem::take(&mut attributes));
            }
        } else {
            // A failed token starts with whitespace, '=' or ';'. Consume that
            // junk exactly as Python does, leaving the next token or comma.
            text = text.trim_start_matches(|c| whitespace(c) || matches!(c, '=' | ';'));
        }
    }
    if !attributes.is_empty() {
        cookies.push(attributes);
    }
    cookies
}

fn quoted(text: &str) -> Option<(String, &str)> {
    let mut characters = text.strip_prefix('"')?.chars();
    let mut value = String::new();
    while let Some(character) = characters.next() {
        match character {
            '"' => return Some((value, characters.as_str())),
            '\\' => {
                let escaped = characters.next()?;
                if escaped == '\n' {
                    return None;
                }
                value.push(escaped);
            }
            _ => value.push(character),
        }
    }
    None
}

#[cfg(test)]
mod tests;
