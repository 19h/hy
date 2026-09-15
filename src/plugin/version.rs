//! Match upstream semantic_version 2.10 coercion and SimpleSpec range semantics.

use std::cmp::Ordering;

use once_cell::sync::Lazy;
use regex::Regex;
use semver::{BuildMetadata, Prerelease, Version};

static VERSION_PREFIX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^[0-9]+(?:\.[0-9]+(?:\.[0-9]+)?)?").unwrap());
static SPEC: Lazy<Regex> = Lazy::new(|| {
    Regex::new(concat!(
        r"^(<=|>=|==|!=|~=|<|>|=|\^|~)?",
        r"(\*|0|[1-9][0-9]*)(?:\.(\*|0|[1-9][0-9]*)(?:\.(\*|0|[1-9][0-9]*))?)?",
        r"(?:-([a-zA-Z0-9.-]*))?(?:\+([a-zA-Z0-9.-]*))?$"
    ))
    .unwrap()
});

pub fn parse_version(raw: &str) -> Option<Version> {
    let prefix = VERSION_PREFIX.find(raw)?;
    let mut numbers = prefix.as_str().split('.').map(|number| number.parse::<u64>());
    let mut version = Version::new(
        numbers.next()?.ok()?,
        numbers.next().transpose().ok()?.unwrap_or(0),
        numbers.next().transpose().ok()?.unwrap_or(0),
    );
    let remainder: String = raw[prefix.end()..]
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || "+.-".contains(character) {
                character
            } else {
                '-'
            }
        })
        .collect();
    let (prerelease, build) = if let Some(build) = remainder.strip_prefix(['+', '.']) {
        ("", build)
    } else {
        let rest = remainder.strip_prefix('-').unwrap_or(&remainder);
        rest.split_once('+').unwrap_or((rest, ""))
    };
    version.pre = Prerelease::new(prerelease).ok()?;
    version.build = BuildMetadata::new(&build.replace('+', ".")).ok()?;
    Some(version)
}

/// Build metadata identifies a release but does not participate in precedence.
pub fn version_precedence(raw: &str) -> Option<Version> {
    let mut version = parse_version(raw)?;
    version.build = BuildMetadata::EMPTY;
    Some(version)
}

pub fn version_matches(raw: &str, specification: &str) -> bool {
    let Some(version) = parse_version(raw) else {
        return false;
    };
    specification.is_empty()
        || specification
            .split(',')
            .all(|expression| Block::parse(expression).is_some_and(|block| block.matches(&version)))
}

pub(crate) fn valid_specification(specification: &str) -> bool {
    specification.split(',').all(|expression| Block::parse(expression).is_some())
}

struct Block<'a> {
    operator: &'a str,
    target: Version,
    precision: usize,
    include_same_patch_prerelease: bool,
    strict_build: bool,
}

impl<'a> Block<'a> {
    fn parse(expression: &'a str) -> Option<Self> {
        let captures = SPEC.captures(expression)?;
        let operator = match captures.get(1).map(|value| value.as_str()).unwrap_or("") {
            "" | "=" => "==",
            operator => operator,
        };
        let numbers: Vec<_> = (2..=4)
            .map(|index| {
                captures
                    .get(index)
                    .filter(|value| value.as_str() != "*")
                    .map(|value| value.as_str().parse::<u64>())
                    .transpose()
            })
            .collect::<std::result::Result<_, _>>()
            .ok()?;
        let precision = numbers.iter().take_while(|number| number.is_some()).count();
        let prerelease = captures.get(5).map(|value| value.as_str());
        let build = captures.get(6).map(|value| value.as_str());
        if precision == 0 && !matches!(operator, "==" | ">=")
            || precision < 3
                && (prerelease.is_some_and(|value| !value.is_empty())
                    || build.is_some_and(|value| !value.is_empty()))
            || build.is_some() && !matches!(operator, "==" | "!=")
        {
            return None;
        }
        let mut target =
            Version::new(numbers[0].unwrap_or(0), numbers[1].unwrap_or(0), numbers[2].unwrap_or(0));
        if precision < 2 {
            target.minor = 0;
        }
        if precision < 3 {
            target.patch = 0;
        }
        target.pre = Prerelease::new(prerelease.unwrap_or_default()).ok()?;
        target.build = BuildMetadata::new(build.unwrap_or_default()).ok()?;
        Some(Self {
            operator,
            target,
            precision,
            include_same_patch_prerelease: prerelease == Some(""),
            strict_build: build.is_some(),
        })
    }

    fn matches(&self, version: &Version) -> bool {
        let compare = |operator, target: &Version| {
            range(version, operator, target, self.strict_build, self.include_same_patch_prerelease)
        };
        let upper = |component| next(&self.target, component);
        let below = |bound: Option<Version>| {
            bound.is_some_and(|bound| range(version, "<", &bound, false, false))
        };
        let at_least = |bound: Option<Version>| {
            bound.is_some_and(|bound| range(version, ">=", &bound, false, false))
        };
        match self.operator {
            "==" if self.precision == 0 => compare(">=", &self.target),
            "==" if self.precision < 3 => {
                compare(">=", &self.target) && below(upper(self.precision - 1))
            }
            "!=" if self.precision < 3 => {
                compare("<", &self.target) || at_least(upper(self.precision - 1))
            }
            ">" if self.precision < 3 => at_least(upper(self.precision - 1)),
            "<=" if self.precision < 3 => below(upper(self.precision - 1)),
            "^" => {
                let component = if self.target.major != 0 {
                    0
                } else if self.target.minor != 0 {
                    1
                } else {
                    2
                };
                compare(">=", &self.target) && below(upper(component))
            }
            "~" => {
                compare(">=", &self.target)
                    && below(upper(if self.precision == 1 {
                        0
                    } else {
                        1
                    }))
            }
            "~=" => {
                compare(">=", &self.target)
                    && below(upper(if self.precision < 3 {
                        0
                    } else {
                        1
                    }))
            }
            operator => compare(operator, &self.target),
        }
    }
}

fn next(version: &Version, component: usize) -> Option<Version> {
    let mut parts = [version.major, version.minor, version.patch];
    if version.pre.is_empty() || parts[component + 1..].iter().any(|part| *part != 0) {
        parts[component] = parts[component].checked_add(1)?;
    }
    parts[component + 1..].fill(0);
    Some(Version::new(parts[0], parts[1], parts[2]))
}

fn range(
    version: &Version,
    operator: &str,
    target: &Version,
    strict_build: bool,
    include_prerelease: bool,
) -> bool {
    let same_patch =
        (version.major, version.minor, version.patch) == (target.major, target.minor, target.patch);
    if matches!(operator, "<" | "!=")
        && !(operator == "!=" && strict_build)
        && !include_prerelease
        && !version.pre.is_empty()
        && target.pre.is_empty()
        && same_patch
    {
        return false;
    }
    let ordering = version.cmp_precedence(target);
    let equal = ordering == Ordering::Equal && (!strict_build || version.build == target.build);
    match operator {
        "==" => equal,
        "!=" => !equal,
        ">" => ordering == Ordering::Greater,
        ">=" => ordering != Ordering::Less,
        "<" => ordering == Ordering::Less,
        "<=" => ordering != Ordering::Greater,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Deserialize)]
    struct Fixture {
        versions: Vec<VersionCase>,
        specifications: Vec<SpecificationCase>,
    }

    #[derive(Deserialize)]
    struct VersionCase {
        input: String,
        normalized: Option<String>,
    }

    #[derive(Deserialize)]
    struct SpecificationCase {
        expression: String,
        valid: bool,
        matches: Vec<usize>,
    }

    #[test]
    fn matches_upstream_coercion_and_simple_spec_oracle() {
        let fixture: Fixture =
            serde_json::from_str(include_str!("../../tests/fixtures/semantic-version.json"))
                .unwrap();
        for version in &fixture.versions {
            assert_eq!(
                parse_version(&version.input).map(|version| version.to_string()),
                version.normalized,
                "coercion: {}",
                version.input
            );
        }
        for specification in fixture.specifications {
            assert_eq!(
                specification
                    .expression
                    .split(',')
                    .all(|expression| Block::parse(expression).is_some()),
                specification.valid,
                "specifier: {}",
                specification.expression
            );
            for (index, version) in fixture.versions.iter().enumerate() {
                assert_eq!(
                    version_matches(&version.input, &specification.expression),
                    specification.matches.contains(&index),
                    "{} matches {}",
                    version.input,
                    specification.expression
                );
            }
        }
    }
}
