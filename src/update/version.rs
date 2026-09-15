//! Release tag parsing and development indicators.

use semver::Version;

/// Parse a strict semantic version after upstream's tag-prefix cleanup.
pub fn parse_version(tag: &str) -> Option<Version> {
    let clean = crate::util::strings::python_trim(tag.trim_start_matches('v'));
    Version::parse(clean).ok()
}

/// True if a version string contains an upstream development indicator.
pub fn is_dev_tag(tag: &str) -> bool {
    let tag = tag.to_ascii_lowercase();
    ["dev", "alpha", "beta", "rc", "pre", "snapshot", "nightly"]
        .iter()
        .any(|indicator| tag.contains(indicator))
}
