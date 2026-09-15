//! Python regex and decimal-integer rules used by bundle target validation.

use crate::error::{Error, Result};
use crate::util::{python_integer, python_repr::string_repr};

pub(super) fn validate_version(version: &str) -> Result<()> {
    // Python's `$` also matches immediately before one final newline.
    let matched = version.strip_suffix('\n').unwrap_or(version);
    let (major, minor) =
        matched.split_once('.').filter(|(a, b)| decimal(a) && decimal(b)).ok_or_else(|| {
            Error::Other(format!(
                "invalid python version: {} (expected 'major.minor')",
                string_repr(version)
            ))
        })?;
    let major = integer(major)?;
    let minor = integer(minor)?;
    let minimum = super::MINIMUM_PYTHON_VERSION;
    if (major, minor) < (minimum.0.into(), minimum.1.into()) {
        return Err(Error::Other(format!(
            "python {version} is below minimum {}.{}",
            minimum.0, minimum.1
        )));
    }
    Ok(())
}

pub(super) fn target_id(value: &str) -> Result<(&str, String)> {
    let matched = value.strip_suffix('\n').unwrap_or(value);
    if let Some((platform, digits)) = matched.rsplit_once("-cp")
        && !platform.is_empty()
        && !platform.contains('\n')
        && decimal(digits)
        && let Some((boundary, _)) = digits.char_indices().nth(1)
    {
        return Ok((platform, format!("{}.{}", &digits[..boundary], &digits[boundary..])));
    }
    Err(Error::Other(format!(
        "invalid target ID: {}\n\
         expected format: <platform>-cp<ver> (e.g. 'linux-x86_64-cp312')\n\
         hint: prefer --platform and --python instead of --target",
        string_repr(value)
    )))
}

fn decimal(value: &str) -> bool {
    !value.is_empty() && value.chars().all(python_integer::is_decimal_digit)
}

fn integer(value: &str) -> Result<num_bigint::BigInt> {
    python_integer::parse(value).ok_or_else(|| {
        Error::Other(format!(
            "Exceeds the limit (4300 digits) for integer string conversion: value has {} digits; \
         use sys.set_int_max_str_digits() to increase the limit",
            value.chars().count()
        ))
    })
}
