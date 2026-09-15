use std::fmt::Write;

pub(super) fn write_repr(value: &str, output: &mut String) {
    let quote = if value.contains('\'') && !value.contains('"') {
        '"'
    } else {
        '\''
    };
    output.push(quote);
    for character in value.chars() {
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
