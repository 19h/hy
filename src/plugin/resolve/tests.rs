use super::*;
use serde_json::json;

fn metadata(name: &str, version: &str, dependencies: serde_json::Value) -> PluginMetadata {
    serde_json::from_value(json!({
        "name": name, "version": version, "entryPoint": "plugin.py",
        "authors": [{"email": "author@example.test"}],
        "urls": {"repository": format!("https://github.com/example/{name}")},
        "dependencies": dependencies,
    }))
    .unwrap()
}

fn snapshot(entries: Vec<PluginMetadata>) -> Snapshot {
    let mut plugins: IndexMap<String, super::super::index::Plugin> = IndexMap::new();
    for metadata in entries {
        let plugin =
            plugins.entry(metadata.name.clone()).or_insert_with(|| super::super::index::Plugin {
                name: metadata.name.clone(),
                host: metadata.normalized_host().unwrap(),
                versions: IndexMap::new(),
            });
        plugin.versions.entry(metadata.version.clone()).or_default().push(Location {
            url: format!("file:///{}.zip", metadata.name).into(),
            sha256: String::new(),
            descriptor: super::super::PluginManifest {
                _schema: None,
                descriptor_version: 1,
                metadata,
            },
        });
    }
    Snapshot {
        version: 1,
        plugins: plugins.into_values().collect(),
    }
}

fn cell() -> Cell {
    Cell {
        platform: "linux-x86_64".into(),
        ida_version: Some("9.4".into()),
        python_version: Some("3.12.2".into()),
        check_python: true,
    }
}

#[test]
fn backtracks_when_newest_transitive_versions_conflict() {
    let source = snapshot(vec![
        metadata("root", "2", json!(["left", "right"])),
        metadata("left", "2", json!(["shared==2"])),
        metadata("left", "1", json!(["shared==1"])),
        metadata("right", "1", json!(["shared==1"])),
        metadata("shared", "2", json!([])),
        metadata("shared", "1", json!([])),
    ]);
    let result =
        resolve(&source, &[Requirement::parse("root").unwrap()], &cell(), &[], &[]).unwrap();
    assert_eq!(result.selected["left"].descriptor.metadata.version, "1");
    assert_eq!(result.selected["shared"].descriptor.metadata.version, "1");
    assert!(
        result.order.iter().position(|name| name == "shared")
            < result.order.iter().position(|name| name == "root")
    );
}

#[test]
fn optional_unsatisfied_subtree_does_not_change_required_choices() {
    let source = snapshot(vec![
        metadata("root", "1", json!(["shared==1", {"plugin":"extra", "required":false}])),
        metadata("shared", "1", json!([])),
        metadata("extra", "1", json!(["shared==2"])),
        metadata("shared", "2", json!([])),
    ]);
    let result =
        resolve(&source, &[Requirement::parse("root").unwrap()], &cell(), &[], &[]).unwrap();
    assert!(!result.selected.contains_key("extra"));
    assert_eq!(result.skipped.len(), 1);
}

#[test]
fn filters_full_python_versions_and_retains_unrestricted_candidates_when_unknown() {
    let mut newer = metadata("root", "2", json!([]));
    newer.requires_python = Some(">=3.12.3".into());
    let source = snapshot(vec![newer, metadata("root", "1", json!([]))]);
    for version in [Some("3.12.2".into()), None] {
        let mut cell = cell();
        cell.python_version = version;
        let result =
            resolve(&source, &[Requirement::parse("root").unwrap()], &cell, &[], &[]).unwrap();
        assert_eq!(result.selected["root"].descriptor.metadata.version, "1");
    }
}

#[test]
fn preserves_newer_installed_pin_and_does_not_reopen_its_dependencies() {
    let root = metadata("root", "1", json!(["shared==1"]));
    let installed = metadata("shared", "2", json!(["absent"]));
    let result = resolve(
        &snapshot(vec![root]),
        &[Requirement::parse("root").unwrap()],
        &cell(),
        &[],
        &[installed],
    )
    .unwrap();
    assert_eq!(result.selected.len(), 1);
    assert_eq!(result.warnings.len(), 1);
}

#[test]
fn cycle_terminates_and_unsatisfied_required_dependency_fails() {
    let source = snapshot(vec![
        metadata("root", "1", json!(["child"])),
        metadata("child", "1", json!(["root"])),
    ]);
    assert_eq!(
        resolve(&source, &[Requirement::parse("root").unwrap()], &cell(), &[], &[])
            .unwrap()
            .order
            .len(),
        2
    );
    let source = snapshot(vec![metadata("root", "1", json!(["missing"]))]);
    assert!(resolve(&source, &[Requirement::parse("root").unwrap()], &cell(), &[], &[]).is_err());
}

#[test]
fn resolution_decisions_match_upstream_for_conflicts_optional_edges_and_installed_pins() {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let mut cases = Vec::new();
    for left_pin in ["shared", "shared==1", "shared==2", "missing"] {
        for right_pin in ["shared", "shared==1", "shared==2", "missing"] {
            for optional in [false, true] {
                for installed_version in [None, Some("1"), Some("2"), Some("3")] {
                    let source = snapshot(vec![
                        metadata(
                            "root",
                            "2",
                            json!(["left", {"plugin":"right", "required":!optional}]),
                        ),
                        metadata("root", "1", json!(["shared==1"])),
                        metadata("left", "2", json!([left_pin])),
                        metadata("left", "1", json!(["shared==1"])),
                        metadata("right", "1", json!([right_pin])),
                        metadata("shared", "1", json!([])),
                        metadata("shared", "2", json!([])),
                    ]);
                    let installed: Vec<_> = installed_version
                        .map(|version| metadata("shared", version, json!([])))
                        .into_iter()
                        .collect();
                    let cell = cell();
                    let expected = match resolve(
                        &source,
                        &[Requirement::parse("root").unwrap()],
                        &cell,
                        &[],
                        &installed,
                    ) {
                        Ok(result) => {
                            let selected: std::collections::BTreeMap<_, _> = result
                                .selected
                                .iter()
                                .map(|(name, location)| {
                                    (name, &location.descriptor.metadata.version)
                                })
                                .collect();
                            json!({
                                "selected": selected,
                                "skipped": result.skipped.len(),
                                "warnings": result.warnings.len(),
                            })
                        }
                        Err(_) => json!({"error":true}),
                    };
                    cases.push(json!({
                        "snapshot": source,
                        "roots": ["root"],
                        "fixed": [],
                        "installed": installed,
                        "cell": {
                            "platform": cell.platform,
                            "ida_version": cell.ida_version,
                            "python_version": cell.python_version,
                            "check_python": true,
                        },
                        "expected": expected,
                    }));
                }
            }
        }
    }
    assert_eq!(cases.len(), 128);
    let Some(python) = std::env::var_os("HY_TEST_RESOLVER_ORACLE_PYTHON") else {
        return;
    };
    let source =
        std::env::var_os("HY_TEST_HCLI_SOURCE").map(std::path::PathBuf::from).unwrap_or_else(
            || std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("ida-hcli"),
        );
    let mut child = Command::new(python)
        .args(["-I", "-B", "-c", include_str!("reference.py")])
        .arg(source)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(&serde_json::to_vec(&cases).unwrap()).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success(), "resolver oracle failed");
    let actual: Vec<serde_json::Value> = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(actual.len(), cases.len());
    for (actual, case) in actual.iter().zip(&cases) {
        assert_eq!(actual, &case["expected"], "{case}");
    }
}
