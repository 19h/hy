//! Compare reference grammar, lexical joins and real directory lookups with HCLI.

use std::io::Write;
use std::process::{Command, Stdio};

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use super::*;
use crate::util::python_path::path::{Flavor, ParsedPath};

#[path = "parity/directory.rs"]
#[cfg(unix)]
mod directory;

fn compare_source(input: Value, expected: &Value) {
    let Some(python) = std::env::var_os("HY_TEST_BUNDLE_ORACLE_PYTHON") else {
        return;
    };
    let source =
        std::env::var_os("HY_TEST_HCLI_SOURCE").map(std::path::PathBuf::from).unwrap_or_else(
            || Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("ida-hcli"),
        );
    let mut child = Command::new(python)
        .args(["-I", "-B", "-c", include_str!("parity/reference.py")])
        .arg(source)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(&serde_json::to_vec(&input).unwrap()).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success(), "source oracle failed");
    let actual: Value = serde_json::from_slice(&output.stdout).unwrap();
    compare_values(&actual, expected, "$");
}

fn compare_values(actual: &Value, expected: &Value, location: &str) {
    match (actual, expected) {
        (Value::Array(actual), Value::Array(expected)) => {
            assert_eq!(actual.len(), expected.len(), "{location}");
            for (index, (actual, expected)) in actual.iter().zip(expected).enumerate() {
                compare_values(actual, expected, &format!("{location}[{index}]"));
            }
        }
        (Value::Object(actual), Value::Object(expected)) => {
            assert_eq!(actual.len(), expected.len(), "{location}");
            for (key, expected) in expected {
                compare_values(&actual[key], expected, &format!("{location}.{key}"));
            }
        }
        _ => assert_eq!(actual, expected, "{location}"),
    }
}

fn paths() -> Vec<String> {
    let mut paths = vec![String::new()];
    let mut level = vec![String::new()];
    for _ in 0..4 {
        level = level
            .iter()
            .flat_map(|prefix| {
                ['a', '.', '/', '\\', ':', '\0', 'é'].map(|c| format!("{prefix}{c}"))
            })
            .collect();
        paths.extend(level.iter().cloned());
    }
    for byte in 0..128 {
        let character = char::from(byte);
        paths.extend([
            character.to_string(),
            format!("{character}.py"),
            format!("a/{character}/b.py"),
            format!("a{character}b.py"),
        ]);
    }
    paths.extend(
        [
            "C:",
            "c:foo",
            "C:/foo",
            "D:foo",
            "./C:foo",
            "\\root",
            "\\\\server\\share",
            "\\\\?\\C:\\root",
            "\\\\?\\UNC\\server\\share",
            "../foo",
            "a/../foo",
            "a//./foo/",
            "日本語.py",
            "λ.py",
            "\u{80}.py",
            "\u{a0}.py",
        ]
        .map(str::to_owned),
    );
    paths
}

#[test]
fn reference_grammar_and_joins_match_posix_and_windows_pathlib() {
    let paths = paths();
    let bases = [
        "",
        ".",
        "/",
        "//",
        "root",
        "root/",
        "./root",
        "C:/root",
        "c:root",
        "\\root",
        "\\\\server\\share\\root",
        "\\\\?\\C:\\root",
        "file/.",
    ];
    let valid: Vec<_> = paths.iter().map(|path| validate_path(path, "fixture").is_ok()).collect();
    let joins = |flavor| {
        bases
            .iter()
            .map(|base| {
                paths
                    .iter()
                    .map(|relative| ParsedPath::joined(base, relative, flavor).render(flavor))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>()
    };
    let expected =
        json!({"valid": valid, "posix": joins(Flavor::Posix), "windows": joins(Flavor::Windows)});
    compare_source(json!({"paths": paths, "bases": bases}), &expected);
    assert_eq!(paths.len(), 3329);
    assert_eq!(2 * bases.len() * paths.len(), 86554);
    assert_eq!(
        format!("{:x}", Sha256::digest(serde_json::to_vec(&expected).unwrap())),
        "7cf47470e992622349fff314a2e1f61e15ff9466fe1915e7b8f9c7d30c6be89e",
    );
}
