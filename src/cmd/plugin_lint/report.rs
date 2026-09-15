//! Plain-text lint findings in upstream order, written to standard output.

use crate::plugin::PluginMetadata;

pub(super) fn metadata(metadata: &PluginMetadata, source: &str) -> usize {
    let mut findings = 0;
    for key in metadata.extra.keys() {
        println!("Warning ({source}): unexpected key in plugin metadata: '{key}'");
        println!("  This key is not part of the ida-plugin.json schema and will be ignored");
        findings += 1;
    }
    for (missing, field, explanation) in [
        (
            metadata.ida_versions.is_empty(),
            "idaVersions",
            "Specify which IDA versions your plugin supports (e.g., ['9.0', '9.1'])",
        ),
        (
            metadata.description.as_deref().is_none_or(str::is_empty),
            "description",
            "A one-line description improves discoverability in the plugin repository",
        ),
        (
            metadata.categories.is_empty(),
            "categories",
            "Categories help users find your plugin (e.g., 'malware-analysis', 'decompilation')",
        ),
        (
            metadata.logo_path.as_deref().is_none_or(str::is_empty),
            "logoPath",
            "A logo image (16:9 aspect ratio) makes your plugin more visually appealing",
        ),
        (
            metadata.keywords.is_empty(),
            "keywords",
            "Keywords improve search discoverability in the plugin repository",
        ),
        (
            metadata.license.as_deref().is_none_or(str::is_empty),
            "license",
            "Specify the license (e.g., 'MIT', 'Apache 2.0') to clarify usage rights",
        ),
    ] {
        if missing {
            recommendation(source, &format!("ida-plugin.json: provide plugin.{field}"));
            println!("  {explanation}");
            findings += 1;
        }
    }
    // Version and required-contact invariants are enforced by metadata decoding.
    for (kind, role, contacts) in [
        ("authors", "an author", &metadata.authors),
        ("maintainers", "a maintainer", &metadata.maintainers),
    ] {
        for (index, contact) in contacts.iter().enumerate() {
            if contact.name.as_deref().is_none_or(str::is_empty) {
                recommendation(source, &format!("plugin.{kind}[{index}]: provide {role} name"));
                findings += 1;
            }
        }
    }
    findings
}

pub(super) fn readme<'a>(filenames: impl Iterator<Item = &'a str>, source: &str) -> usize {
    let mut alternative = None;
    for name in filenames {
        if name == "README.md" {
            return 0;
        }
        if alternative.is_none() && name.to_lowercase().starts_with("readme") {
            alternative = Some(name);
        }
    }
    if let Some(name) = alternative {
        recommendation(source, &format!("rename {name} to README.md (exact casing)"));
        println!("  Use 'README.md' with exact casing for consistency and discoverability");
    } else {
        recommendation(source, "add a README.md file");
        println!("  A README helps users understand what your plugin does and how to use it");
    }
    1
}

fn recommendation(source: &str, message: &str) {
    println!("Recommendation ({source}): {message}");
}
