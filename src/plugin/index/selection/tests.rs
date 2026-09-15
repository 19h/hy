use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use super::{Reference, Snapshot, select_for_platform};

const HOST: &str = "https://github.com/example/original";

#[test]
fn identity_lookup_matches_source_unicode_host_and_ambiguity_rules() {
    let mut cases = Vec::new();
    for (name, host) in [
        ("example", HOST),
        ("EXAMPLE", HOST),
        ("İ", HOST),
        ("i\u{307}", HOST),
        ("ΟΣ", HOST),
        ("ος", HOST),
        ("example", ""),
        ("example", "invalid"),
    ] {
        let mut plugin = catalogue(&["1"], "all", false);
        plugin["name"] = json!(name);
        plugin["host"] = json!(host);
        for count in 0..=2 {
            let snapshot = json!({"plugins": vec![plugin.clone(); count]});
            for wanted in ["example", "ExAmPlE", "İ", "i\u{307}", "ΟΣ", "ος", "missing"] {
                for host in [
                    None,
                    Some(""),
                    Some(HOST),
                    Some("https://GitHub.com/Example/Original/"),
                    Some("https://github.com/foreign/repo"),
                    Some("invalid"),
                ] {
                    for spec in ["", "==1"] {
                        cases.push(case(&snapshot, wanted, spec, host, "linux-x86_64", None));
                    }
                }
            }
        }
    }
    assert_eq!(cases.len(), 2016);
    verify(&cases, "835e385bbc9614989d36ad74508179daf941ba8711524e9bfdad06c12fb73543");
}

#[test]
fn version_and_location_selection_match_the_source_repository() {
    let mut cases = Vec::new();
    for versions in [
        vec![],
        vec!["1"],
        vec!["2", "1"],
        vec!["1", "1.0"],
        vec!["0.0.0-alpha"],
        vec!["1", "bad"],
        vec!["bad", "1"],
    ] {
        for skew in ["none", "name", "version", "host", "all"] {
            for reverse in [false, true] {
                let snapshot = json!({"plugins": [catalogue(&versions, skew, reverse)]});
                for spec in ["", "==1", ">=0", "==2", "!=1", "==1,<=2", "==bad"] {
                    for platform in ["linux-x86_64", "windows-x86_64", "unknown"] {
                        for ida in [None, Some("9.0"), Some("9.4"), Some("9.9")] {
                            cases.push(case(&snapshot, "example", spec, None, platform, ida));
                        }
                    }
                }
            }
        }
    }
    assert_eq!(cases.len(), 5880);
    verify(&cases, "af30225339de7e11134ecd1a56edf9c3c47082c806df4d58517bb31b9acbd08e");
}

fn catalogue(versions: &[&str], skew: &str, reverse: bool) -> Value {
    let name = if matches!(skew, "name" | "all") {
        "other"
    } else {
        "example"
    };
    let descriptor_version = if matches!(skew, "version" | "all") {
        "7"
    } else {
        "1"
    };
    let host = if matches!(skew, "host" | "all") {
        "https://github.com/foreign/repo"
    } else {
        HOST
    };
    let mut entries = serde_json::Map::new();
    for version in versions {
        let mut locations = Vec::new();
        for (index, (platforms, ida)) in [
            (vec!["linux-x86_64"], vec!["9.0"]),
            (vec!["windows-x86_64"], vec!["9.4"]),
            (vec!["linux-x86_64", "windows-x86_64"], vec!["9.1", "9.4"]),
        ]
        .into_iter()
        .enumerate()
        {
            let metadata = json!({
                "name": name,
                "version": descriptor_version,
                "urls": {"repository": host},
                "entryPoint": "plugin.py",
                "authors": [{"email": "author@example.test"}],
                "platforms": platforms,
                "idaVersions": ida,
            });
            locations.push(json!({
                "url": format!("fixture:{version}/{index}"), "sha256": "fixture",
                "metadata": {"IDAMetadataDescriptorVersion": 1, "plugin": metadata},
            }));
        }
        if reverse {
            locations.reverse();
        }
        entries.insert((*version).into(), json!(locations));
    }
    json!({"name": "example", "host": HOST, "versions": entries})
}

fn case(
    snapshot: &Value,
    name: &str,
    spec: &str,
    host: Option<&str>,
    platform: &str,
    ida: Option<&str>,
) -> Value {
    let parsed: Snapshot = serde_json::from_value(snapshot.clone()).unwrap();
    let reference = Reference {
        name: name.into(),
        spec: spec.into(),
        host: host.map(str::to_owned),
        repo: None,
    };
    let expected = match select_for_platform(&parsed, &reference, platform, ida) {
        Ok(location) => json!({"url": location.url}),
        Err(_) => json!({"error": true}),
    };
    json!({
        "snapshot": snapshot,
        "name": name,
        "spec": spec,
        "host": host,
        "platform": platform,
        "ida": ida,
        "expected": expected,
    })
}

fn verify(cases: &[Value], digest: &str) {
    if let Some(python) = std::env::var_os("HY_TEST_BUNDLE_ORACLE_PYTHON") {
        let source =
            std::env::var_os("HY_TEST_HCLI_SOURCE").map(std::path::PathBuf::from).unwrap_or_else(
                || Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("ida-hcli"),
            );
        let mut child = Command::new(python)
            .args(["-I", "-B", "-c", include_str!("reference.py")])
            .arg(source)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(&serde_json::to_vec(cases).unwrap()).unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(output.status.success());
        let actual: Vec<Value> = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(actual.len(), cases.len());
        for (actual, case) in actual.iter().zip(cases) {
            assert_eq!(actual, &case["expected"], "{case}");
        }
    }
    let expected: Vec<_> = cases.iter().map(|case| &case["expected"]).collect();
    assert_eq!(format!("{:x}", Sha256::digest(serde_json::to_vec(&expected).unwrap())), digest);
}
