use std::fmt::Write;

pub(super) fn write_repr(value: &str, output: &mut String) {
    let quote = if value.contains('\'') && !value.contains('"') {
        '"'
    } else {
        '\''
    };
    write_codepoints(value.chars().map(u32::from), quote, output);
}

pub(super) fn write_python(value: &crate::util::python_json::Text, output: &mut String) {
    let quote = if value.contains('\'') && !value.contains('"') {
        '"'
    } else {
        '\''
    };
    write_codepoints(value.codepoints(), quote, output);
}

fn write_codepoints(points: impl Iterator<Item = u32>, quote: char, output: &mut String) {
    output.push(quote);
    for code in points {
        let Some(character) = char::from_u32(code) else {
            write!(output, "\\u{code:04x}").unwrap();
            continue;
        };
        match character {
            '\\' => output.push_str("\\\\"),
            '\t' => output.push_str("\\t"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            character if character == quote => {
                output.push('\\');
                output.push(character);
            }
            character if super::printable::contains(u32::from(character)) => output.push(character),
            character => {
                let code = u32::from(character);
                if code <= 0xff {
                    write!(output, "\\x{code:02x}").unwrap();
                } else if code <= 0xffff {
                    write!(output, "\\u{code:04x}").unwrap();
                } else {
                    write!(output, "\\U{code:08x}").unwrap();
                }
            }
        }
    }
    output.push(quote);
}
