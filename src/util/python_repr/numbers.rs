use serde_json::Number;

pub(super) fn repr(value: &Number) -> String {
    let text = value.as_str();
    if !text.contains(['.', 'e', 'E']) {
        return if text == "-0" {
            "0"
        } else {
            text
        }
        .to_owned();
    }
    float_repr(text.parse().expect("JSON number is valid binary64 input"))
}

pub(crate) fn float_repr(value: f64) -> String {
    if value.is_nan() {
        return "nan".into();
    }
    let sign = if value.is_sign_negative() {
        "-"
    } else {
        ""
    };
    if value.is_infinite() {
        return format!("{sign}inf");
    }
    if value == 0.0 {
        return format!("{sign}0.0");
    }

    // Rust's Debug formatter rounds some shortest-decimal ties away from zero.
    // Ryu uses ties-to-even, matching the pinned CPython formatting oracle.
    let mut buffer = ryu::Buffer::new();
    let shortest = buffer.format_finite(value.abs());
    let (digits, exponent) = decimal_parts(shortest);
    if !(-4..16).contains(&exponent) {
        let fraction = if digits.len() > 1 {
            format!(".{}", &digits[1..])
        } else {
            String::new()
        };
        return format!("{sign}{}{fraction}e{exponent:+03}", &digits[..1]);
    }

    let point = exponent + 1;
    if point <= 0 {
        format!("{sign}0.{}{digits}", "0".repeat((-point) as usize))
    } else if point as usize >= digits.len() {
        format!("{sign}{digits}{}.0", "0".repeat(point as usize - digits.len()))
    } else {
        let (integer, fraction) = digits.split_at(point as usize);
        format!("{sign}{integer}.{fraction}")
    }
}

/// Normalize Ryu's positive finite decimal into significant digits and a
/// scientific exponent, independently of Ryu's choice of fixed/scientific form.
fn decimal_parts(text: &str) -> (String, i32) {
    let (mantissa, exponent) = text
        .split_once('e')
        .map_or((text, 0), |(mantissa, exponent)| (mantissa, exponent.parse::<i32>().unwrap()));
    let point = mantissa.find('.').unwrap_or(mantissa.len());
    let digits: String = mantissa.chars().filter(|&character| character != '.').collect();
    let leading = digits.bytes().take_while(|&byte| byte == b'0').count();
    let exponent = exponent + point as i32 - leading as i32 - 1;
    (digits[leading..].trim_end_matches('0').to_owned(), exponent)
}
