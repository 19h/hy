use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use super::{Location, Plugin, Snapshot};

#[test]
fn snapshot_envelope_validation_matches_the_source_models() {
    let base = fixture();
    let mut cases = Vec::new();
    for (parent, field) in [
        ("", "version"),
        ("", "plugins"),
        ("/plugins/0", "name"),
        ("/plugins/0", "host"),
        ("/plugins/0", "versions"),
        ("/plugins/0/versions/1/0", "url"),
        ("/plugins/0/versions/1/0", "sha256"),
        ("/plugins/0/versions/1/0", "metadata"),
        ("/plugins/0/versions/1/0/metadata", "IDAMetadataDescriptorVersion"),
        ("/plugins/0/versions/1/0/metadata", "$schema"),
        ("/plugins/0/versions/1/0/metadata", "plugin"),
    ] {
        for value in [
            None,
            Some(Value::Null),
            Some(json!(true)),
            Some(json!(false)),
            Some(json!(0)),
            Some(json!(1)),
            Some(json!(1.0)),
            Some(json!(2)),
            Some(json!("")),
            Some(json!("1")),
            Some(json!("x")),
            Some(json!([])),
            Some(json!([{}])),
            Some(json!({})),
            Some(json!({"dummy": "value"})),
        ] {
            let mut input = base.clone();
            let target = input.pointer_mut(parent).unwrap().as_object_mut().unwrap();
            match value {
                Some(value) => {
                    target.insert(field.into(), value);
                }
                None => {
                    target.remove(field);
                }
            }
            cases.push(case(serde_json::to_string(&input).unwrap()));
        }
    }
    for value in [
        Value::Null,
        json!(false),
        json!(true),
        json!(0),
        json!(1),
        json!(1.0),
        json!(""),
        json!("x"),
        json!([]),
        json!([{}]),
        json!({}),
    ] {
        cases.push(case(serde_json::to_string(&value).unwrap()));
    }
    for parent in ["", "/plugins/0", "/plugins/0/versions/1/0", "/plugins/0/versions/1/0/metadata"]
    {
        let mut input = base.clone();
        input
            .pointer_mut(parent)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert("unknown".into(), json!([1, true]));
        cases.push(case(serde_json::to_string(&input).unwrap()));
    }
    assert_eq!(cases.len(), 180);
    verify(&cases, "ae52a2c3496604c7af632d670a5cae79f12b1e42623d34078e1a678e196aaa22");
}

#[test]
fn version_maps_preserve_document_order() {
    let location = serde_json::to_string(&fixture()["plugins"][0]["versions"]["1"]).unwrap();
    let mut cases = Vec::new();
    for keys in [["1.0", "1", "1.0.0", "01"], ["01", "1.0.0", "1", "1.0"]] {
        let versions =
            keys.iter().map(|key| format!("{key:?}:{location}")).collect::<Vec<_>>().join(",");
        let document = format!(
            "{{\"plugins\":[{{\"name\":\"example\",\"host\":\"https://github.com/a/b\",\"versions\":{{{versions}}}}}]}}",
        );
        let snapshot: Snapshot = serde_json::from_str(&document).unwrap();
        assert_eq!(
            snapshot.plugins[0].versions.keys().map(String::as_str).collect::<Vec<_>>(),
            keys
        );
        cases.push(case(document));
    }
    verify(&cases, "fc6d1bd4aff6e469f4bc2f81dbab4cf144ae3866312d4a0fd2f4c6b87a9558b5");
}

fn fixture() -> Value {
    let descriptor = json!({
        "IDAMetadataDescriptorVersion": 1,
        "plugin": {
            "name": "example", "version": "1", "entryPoint": "plugin.py",
            "urls": {"repository": "https://github.com/a/b"},
            "authors": [{"email": "author@example.test"}],
        },
    });
    let location = json!({"url": "fixture:archive", "sha256": "abc", "metadata": descriptor});
    json!({
        "version": 1,
        "plugins": [{"name": "example", "host": "https://github.com/a/b", "versions": {"1": [location]}}],
    })
}

fn case(document: String) -> Value {
    let expected = match serde_json::from_str::<Snapshot>(&document) {
        Ok(snapshot) => {
            let plugins: Vec<_> = snapshot.plugins.iter().map(project_plugin).collect();
            json!({"version": snapshot.version, "plugins": plugins})
        }
        Err(_) => json!({"invalid": true}),
    };
    json!({"document": document, "expected": expected})
}

fn project_plugin(plugin: &Plugin) -> Value {
    let versions: Vec<_> = plugin
        .versions
        .iter()
        .map(|(version, locations)| {
            json!({
                "version": version,
                "locations": locations.iter().map(project_location).collect::<Vec<_>>(),
            })
        })
        .collect();
    json!({"name": plugin.name, "host": plugin.host, "versions": versions})
}

fn project_location(location: &Location) -> Value {
    let descriptor = serde_json::to_value(&location.descriptor).unwrap();
    json!({
        "url": location.url,
        "sha256": location.sha256,
        "descriptor_version": descriptor["IDAMetadataDescriptorVersion"],
        "has_schema": descriptor.get("$schema").is_some(),
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
            assert_eq!(actual, &case["expected"], "{}", case["document"]);
        }
    }
    let expected: Vec<_> = cases.iter().map(|case| &case["expected"]).collect();
    assert_eq!(format!("{:x}", Sha256::digest(serde_json::to_vec(&expected).unwrap())), digest);
}
