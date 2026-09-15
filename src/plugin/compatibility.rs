//! IDA compatibility declarations use exact versions from the published schema.

use once_cell::sync::Lazy;
use semver::Version;
use serde::{Deserialize, Deserializer};

use super::PluginMetadata;

static IDA_VERSIONS: Lazy<Vec<String>> = Lazy::new(|| {
    let mut versions = schema_values("idaVersions");
    versions.sort_by_key(|version| parse_ida_version(version));
    versions
});
static PLATFORMS: Lazy<Vec<String>> = Lazy::new(|| {
    let mut platforms = schema_values("platforms");
    platforms.sort();
    platforms
});

fn schema_values(field: &str) -> Vec<String> {
    serde_json::from_value(
        super::ida_plugin_json_schema()["$defs"]["PluginMetadata"]["properties"][field]["items"]
            ["enum"]
            .clone(),
    )
    .expect("compatibility values in the pinned plugin schema")
}

pub fn all_ida_versions() -> Vec<String> {
    IDA_VERSIONS.clone()
}

pub fn all_platforms() -> Vec<String> {
    PLATFORMS.clone()
}

pub fn parse_ida_version(raw: &str) -> Option<Version> {
    let normalized = raw.replace("sp", ".");
    let normalized = match normalized.matches('.').count() {
        0 => format!("{normalized}.0.0"),
        1 => format!("{normalized}.0"),
        _ => normalized,
    };
    Version::parse(&normalized).ok()
}

pub(super) fn deserialize_ida_versions<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Vec<String>, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Declaration {
        Versions(Vec<String>),
        Specification(String),
    }

    let mut versions = match Declaration::deserialize(deserializer)? {
        Declaration::Versions(versions) => {
            validate_members::<D::Error>(&versions, &IDA_VERSIONS, "IDA version")?;
            versions
        }
        Declaration::Specification(specification) => {
            let specification = specification.replace("sp", ".");
            if !super::version::valid_specification(&specification) {
                return Err(serde::de::Error::custom("invalid IDA version specification"));
            }
            IDA_VERSIONS
                .iter()
                .filter(|version| {
                    super::version_matches(&version.replace("sp", "."), &specification)
                })
                .cloned()
                .collect()
        }
    };
    versions.sort_by_key(|version| parse_ida_version(version));
    Ok(versions)
}

pub(super) fn deserialize_platforms<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Vec<String>, D::Error> {
    let mut platforms = Vec::<String>::deserialize(deserializer)?;
    validate_members::<D::Error>(&platforms, &PLATFORMS, "platform")?;
    platforms.sort();
    Ok(platforms)
}

fn validate_members<E: serde::de::Error>(
    values: &[String],
    allowed: &[String],
    kind: &str,
) -> Result<(), E> {
    for value in values {
        if !allowed.contains(value) {
            return Err(E::custom(format!("unknown {kind}: {value}")));
        }
    }
    Ok(())
}

pub fn is_ida_version_compatible(plugin: &PluginMetadata, ida_version: &str) -> bool {
    plugin.ida_versions.iter().any(|version| version == ida_version)
}

pub fn is_platform_compatible(plugin: &PluginMetadata, current: &str) -> bool {
    plugin.platforms.iter().any(|platform| platform == current)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn input() -> serde_json::Value {
        json!({"name": "example", "version": "1", "entryPoint": "plugin.py",
            "urls": {"repository": "https://github.com/example/plugin"},
            "authors": [{"email": "author@example.test"}]})
    }

    fn metadata(versions: serde_json::Value) -> PluginMetadata {
        let mut input = input();
        input["idaVersions"] = versions;
        serde_json::from_value(input).unwrap()
    }

    #[test]
    fn service_pack_ranges_expand_to_known_ida_versions() {
        let plugin = metadata(json!(">=9.0sp1,<9.3"));
        assert_eq!(plugin.ida_versions, ["9.0sp1", "9.1", "9.2"]);
        assert!(is_ida_version_compatible(&plugin, "9.0sp1"));
        assert!(!is_ida_version_compatible(&plugin, "9.0"));
        assert!(!is_ida_version_compatible(&plugin, "9.1sp1"));
        assert!(!is_ida_version_compatible(&metadata(json!([])), "9.4"));
        assert_eq!(
            metadata(json!(["10.0", "9.4", "9.0sp1", "9.0"])).ida_versions,
            ["9.0", "9.0sp1", "9.4", "10.0"]
        );
    }

    #[test]
    fn invalid_compatibility_declarations_are_rejected() {
        for (field, values) in [
            (
                "idaVersions",
                vec![json!(null), json!(["9.1sp1"]), json!([">=9"]), json!("bad"), json!("")],
            ),
            ("platforms", vec![json!(null), json!(["all"]), json!(["macarm"])]),
        ] {
            for value in values {
                let mut input = input();
                input[field] = value.clone();
                assert!(
                    serde_json::from_value::<PluginMetadata>(input).is_err(),
                    "{field}: {value}"
                );
            }
        }
    }

    #[test]
    fn omitted_lists_default_to_the_pinned_catalogue() {
        let plugin: PluginMetadata = serde_json::from_value(input()).unwrap();
        assert_eq!(plugin.ida_versions.len(), 63);
        assert_eq!(plugin.platforms.len(), 6);
        assert!(is_ida_version_compatible(&plugin, "10.0"));
        assert!(!is_ida_version_compatible(&plugin, "11.0"));
        assert_eq!(parse_ida_version("9.0sp1"), Version::parse("9.0.1").ok());
        assert_eq!(parse_ida_version("09.0"), None);
    }
}
