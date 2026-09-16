//! URLs retain Python code points through ordering; snapshot serialization is later.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde_json::{Value, json};

use super::*;
use crate::plugin::index::Snapshot;
use crate::util::python_json;

#[test]
fn url_codepoints_ordering_and_snapshot_boundary_match_source() {
    let mut urls: Vec<String> = (0xd800..=0xdfff)
        .map(|point| format!(r#""https://example.test/\u{point:04x}.zip""#))
        .collect();
    for url in ["", "a", "\n'\"\\", "https://example.test/Δ.zip", "https://example.test/🧠.zip"]
    {
        urls.push(serde_json::to_string(url).unwrap());
    }
    let cases: Vec<_> = urls
        .into_iter()
        .map(|url| {
            let records = [
                ("EXAMPLE", "https://example.test/a.zip"),
                ("Example", "placeholder"),
                ("examplE", "https://example.test/🧠.zip"),
            ]
            .map(|(name, url)| {
                super::tests::record(name, "1", url, "https://github.com/example/original", 17)
            });
            json!({"url": url, "records": records})
        })
        .collect();
    assert_eq!(cases.len(), 2053);
    let expected: Vec<_> = cases
        .iter()
        .map(|case| {
            let document = case["records"][1]
                .to_string()
                .replace("\"placeholder\"", case["url"].as_str().unwrap());
            let json_input = serde_json::from_str::<super::Location>(&document).is_ok();
            let mut catalogue = ArchiveCatalogue::default();
            for (index, record) in case["records"].as_array().unwrap().iter().enumerate() {
                let mut location: super::Location = serde_json::from_value(record.clone()).unwrap();
                if index == 1 {
                    let python_json::Value::String(ref text) =
                        python_json::parse(case["url"].as_str().unwrap()).unwrap()
                    else {
                        unreachable!()
                    };
                    location.url = text.clone();
                }
                catalogue.add(&location.url, &location.sha256, location.descriptor).unwrap();
            }
            let plugins = match catalogue.into_plugins() {
                Ok(plugins) => plugins,
                Err(_) => return json!({"error":true}),
            };
            let points: Vec<Vec<_>> = plugins[0].versions["1"]
                .iter()
                .map(|location| location.url.codepoints().collect())
                .collect();
            let name = plugins[0].name.clone();
            let snapshot = Snapshot {
                version: 1,
                plugins,
            }
            .to_json();
            json!({
                "points": points,
                "name": name,
                "snapshot": snapshot.ok(),
                "json_input": json_input,
            })
        })
        .collect();
    let Some(python) = std::env::var_os("HY_TEST_BUNDLE_ORACLE_PYTHON") else {
        return;
    };
    let source = std::env::var_os("HY_TEST_HCLI_SOURCE").map(PathBuf::from).unwrap_or_else(|| {
        Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("ida-hcli")
    });
    let mut child = Command::new(python)
        .args(["-I", "-B", "-c", include_str!("url_reference.py")])
        .arg(source)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(&serde_json::to_vec(&cases).unwrap()).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    let actual: Vec<Value> = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(actual.len(), expected.len());
    for (index, (actual, expected)) in actual.iter().zip(expected).enumerate() {
        assert_eq!(actual, &expected, "case {index}");
    }
    eprintln!("matched {} indexed URL and snapshot cases", cases.len());
}
