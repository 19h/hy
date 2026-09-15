//! CPython decimal integer syntax using its Unicode 15.1 decimal-digit set.

use num_bigint::BigInt;

mod digits;

const MAX_DIGITS: usize = 4300;

pub(crate) fn is_decimal_digit(character: char) -> bool {
    digits::decimal(character).is_some()
}

pub fn parse(text: &str) -> Option<BigInt> {
    let text = text.trim();
    let (negative, digits) = match text.as_bytes().first()? {
        b'-' => (true, &text[1..]),
        b'+' => (false, &text[1..]),
        _ => (false, text),
    };
    let mut normalized = Vec::new();
    let mut previous_digit = false;
    for character in digits.chars() {
        if character == '_' && previous_digit {
            previous_digit = false;
            continue;
        }
        let digit = digits::decimal(character)?;
        if normalized.len() == MAX_DIGITS {
            return None;
        }
        normalized.push(b'0' + digit);
        previous_digit = true;
    }
    if !previous_digit {
        return None;
    }
    let number = BigInt::parse_bytes(&normalized, 10)?;
    Some(if negative {
        -number
    } else {
        number
    })
}

#[cfg(test)]
mod tests;
