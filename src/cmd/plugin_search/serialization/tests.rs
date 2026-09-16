use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde_json::{Value as Json, json};

use super::super::ExactVersionReport;
use super::*;

#[test]
fn exact_search_json_preserves_surrogates_with_source_escaping_and_field_order() {
    let mut documents: Vec<_> =
        (0xd800..=0xdfff).map(|point| format!(r#""fixture:\u{point:04x}""#)).collect();
    documents.extend([r#""a\n\"\\'""#.into(), r#""https://example.test/Δ🧠.zip""#.into()]);
    assert_eq!(documents.len(), 2050);
    let plugin = json!({"name":"Δ", "version":"1", "description":"'\"\n\u{7f}"});
    let expected: Vec<_> = documents
        .iter()
        .map(|document| {
            let Value::String(ref url) = python_json::parse(document).unwrap() else {
                unreachable!()
            };
            let report = PluginQueryReport::Exact(ExactVersionReport {
                plugin: plugin.clone(),
                download_locations: vec![DownloadLocation {
                    ida_versions: "9.0".into(),
                    platforms: "linux-x86_64".into(),
                    url: url.clone(),
                }],
            });
            json!({
                "json": format!("{}\n", report.to_json().unwrap()),
                "stdout": url.to_utf8_surrogateescape().ok(),
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
        .args(["-I", "-B", "-c", include_str!("reference.py")])
        .arg(source)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(&serde_json::to_vec(&json!({"plugin":plugin,"urls":documents})).unwrap())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    let actual: Vec<Json> = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(actual.len(), expected.len());
    for (index, (actual, expected)) in actual.iter().zip(expected).enumerate() {
        assert_eq!(actual, &expected, "case {index}");
    }
    eprintln!("matched {} source search JSON and stdout encoding cases", documents.len());
}
