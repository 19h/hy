use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use super::ArchiveCatalogue;

const HOST: &str = "https://github.com/example/original";

fn record(name: &str, version: &str, url: &str, host: &str, mask: usize) -> Value {
    let ida: Vec<_> = ["9.0", "9.1", "9.2", "9.3"]
        .into_iter()
        .enumerate()
        .filter(|(index, _)| mask & (1 << index) != 0)
        .map(|(_, version)| version)
        .collect();
    let platforms: Vec<_> = ["linux-x86_64", "windows-x86_64", "macos-aarch64"]
        .into_iter()
        .enumerate()
        .filter(|(index, _)| mask & (1 << (index + 4)) != 0)
        .map(|(_, platform)| platform)
        .collect();
    json!({
        "url": url,
        "sha256": "fixture",
        "metadata": {
            "IDAMetadataDescriptorVersion": 1,
            "plugin": {
                "name": name, "version": version, "urls": {"repository": host},
                "entryPoint": "plugin.py", "authors": [{"email": "author@example.test"}],
                "idaVersions": ida, "platforms": platforms,
            },
        },
    })
}

#[test]
fn catalogue_identity_version_and_location_order_match_source() {
    let base = [
        record("EXAMPLE", "1", "fixture:z", HOST, 17),
        record("Example", "1.0", "fixture:b", HOST, 17),
        record("eXample", "2", "fixture:c", HOST, 17),
        record("examplE", "10", "fixture:a", HOST, 17),
        record("EXAMPLE", "10", "fixture:z", HOST, 17),
        record("example", "1", "fixture:a", "https://github.com/foreign/repo", 17),
    ];
    let mut cases = Vec::new();
    let mut order: Vec<_> = (0..base.len()).collect();
    permutations(&mut order, 0, &mut |order| {
        cases.push(order.iter().map(|index| base[*index].clone()).collect());
    });
    for names in [["Other", "OTHER"], ["ABcd", "abCD"], ["EXAMPLE", "example"]] {
        cases.push(vec![
            record(names[0], "1", "fixture:z", HOST, 17),
            record(names[1], "1", "fixture:a", HOST, 17),
        ]);
    }
    let one = record("example", "1", "fixture:a", HOST, 17);
    let mut different = one.clone();
    different["metadata"]["plugin"]["name"] = json!("EXAMPLE");
    cases.extend([vec![one.clone(), one.clone()], vec![one, different]]);
    assert_eq!(cases.len(), 725);
    verify(&cases, "134b49820c6a57886d37391baaa4549256a6bc791af268f32332f9f1f871513c");
}

#[test]
fn catalogue_compatibility_order_matches_source_for_large_variant_sets() {
    let masks: Vec<_> = (0..128).filter(|mask| mask & 15 != 0 && mask & 112 != 0).collect();
    let names = ["EXAMPLE", "Example", "eXample", "examplE"];
    let mut cases = Vec::new();
    for length in [2, 3, 31, 32, 63, 64, 65, masks.len()] {
        for seed in 1..=32u64 {
            let mut state = seed;
            let mut order = masks.clone();
            for index in (1..order.len()).rev() {
                state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
                order.swap(index, (state as usize) % (index + 1));
            }
            cases.push(
                order[..length]
                    .iter()
                    .enumerate()
                    .map(|(index, mask)| {
                        record(
                            names[index % names.len()],
                            "1",
                            &format!("fixture:{index}"),
                            HOST,
                            *mask,
                        )
                    })
                    .collect(),
            );
        }
    }
    assert_eq!(cases.len(), 256);
    verify(&cases, "298eb2d6980b4eaecba78e3dbcb26ade34f8da0e0835e049a538cb38cd6fca21");
}

fn permutations(order: &mut [usize], index: usize, visit: &mut impl FnMut(&[usize])) {
    if index == order.len() {
        visit(order);
        return;
    }
    for next in index..order.len() {
        order.swap(index, next);
        permutations(order, index + 1, visit);
        order.swap(index, next);
    }
}

#[test]
fn tied_locations_use_python_model_equality() {
    let values: Vec<Value> = [
        "null",
        "true",
        "false",
        "0",
        "1",
        "0.0",
        "-0.0",
        "1.0",
        "1e0",
        "1.5",
        "9007199254740992",
        "9007199254740993",
        "9007199254740993.0",
        "18446744073709551616",
        "18446744073709551616.0",
        "1e999",
    ]
    .into_iter()
    .map(|text| serde_json::from_str(text).unwrap())
    .collect();
    let base = record("example", "1", "fixture:a", HOST, 17);
    let mut cases = Vec::new();
    for left in &values {
        for right in &values {
            for layout in 0..4 {
                let wrap = |value: &Value| match layout {
                    0 => value.clone(),
                    1 => json!([value]),
                    2 => json!({"field": value}),
                    _ => json!({"field": [value]}),
                };
                let mut first = base.clone();
                first["metadata"]["plugin"]["fixture"] = wrap(left);
                let mut second = base.clone();
                second["metadata"]["plugin"]["fixture"] = wrap(right);
                cases.push(vec![first, second]);
            }
        }
    }
    for left in [None, Some("schema:a"), Some("schema:b")] {
        for right in [None, Some("schema:a"), Some("schema:b")] {
            let mut first = base.clone();
            first["metadata"]["$schema"] = json!(left);
            let mut second = base.clone();
            second["metadata"]["$schema"] = json!(right);
            cases.push(vec![first, second]);
        }
    }
    assert_eq!(cases.len(), 1033);
    verify(&cases, "d7457536503023a6dc1bdc2ee1b26f00c1f0757db65fe7d09fd96cca355374f8");
}

fn projection(records: &[Value]) -> Value {
    let mut catalogue = ArchiveCatalogue::default();
    for record in records {
        let location: super::Location =
            serde_json::from_slice(&serde_json::to_vec(record).unwrap()).unwrap();
        catalogue.add(&location.url, &location.sha256, location.descriptor).unwrap();
    }
    let Ok(plugins) = catalogue.into_plugins() else {
        return json!({"error": true});
    };
    json!(
        plugins
            .into_iter()
            .map(|plugin| {
                let versions: Vec<_> = plugin
                    .versions
                    .into_iter()
                    .map(|(version, locations)| {
                        json!([
                            version,
                            locations
                                .into_iter()
                                .map(|location| {
                                    json!([
                                        location.url,
                                        location.sha256,
                                        location.descriptor.metadata.name
                                    ])
                                })
                                .collect::<Vec<_>>()
                        ])
                    })
                    .collect();
                json!({"name": plugin.name, "host": plugin.host, "versions": versions})
            })
            .collect::<Vec<_>>()
    )
}

fn verify(cases: &[Vec<Value>], digest: &str) {
    let expected: Vec<_> = cases.iter().map(|case| projection(case)).collect();
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
        assert_eq!(actual.len(), expected.len());
        for (index, (actual, expected)) in actual.iter().zip(&expected).enumerate() {
            assert_eq!(actual, expected, "case {index}: {:?}", cases[index]);
        }
    }
    assert_eq!(format!("{:x}", Sha256::digest(serde_json::to_vec(&expected).unwrap())), digest);
}
