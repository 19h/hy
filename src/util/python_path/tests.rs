use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use super::{Flavor, expand_with, home, unless_url};

fn normalize(
    value: &str,
    flavor: Flavor,
    home: impl FnOnce(&str) -> crate::error::Result<Option<String>>,
) -> crate::error::Result<String> {
    unless_url(value, || expand_with(value, flavor, home))
}

#[test]
fn local_link_spelling_matches_pathlib_for_both_platforms() {
    let cases = lexical_cases(true);
    compare(&cases);
    digest(&cases, "046cf8c48e7dd6aa6fefc6557762d26a2addc22ed6456201dd2bf28ade24f995");
}

#[test]
fn path_spelling_expands_tildes_without_a_url_exception() {
    let cases = lexical_cases(false);
    compare(&cases);
    digest(&cases, "48fdf06f98366e520d149e83f351043ebcedb8f32ab9cb3a6b8962c0b92446af");
}

fn lexical_cases(preserve_urls: bool) -> Vec<Value> {
    let prefixes = [
        "",
        ".",
        "./",
        "../",
        "/",
        "//",
        "///",
        "~",
        "~/",
        "~fixture/",
        "./~/",
        "../~/",
        "C:",
        "C:/",
        "./C:",
        "é:",
        "\\",
        "\\\\server\\share",
        "\\\\?\\UNC\\server\\share",
        "\\\\.\\device",
        "\\\\?\\C:",
        "https://host/",
    ];
    let components = [
        "",
        ".",
        "..",
        "wheels",
        "a//b",
        "a/./b",
        "a/../b",
        "a\\b",
        "with spaces",
        "日本語",
        "~other",
        "x://y",
    ];
    let suffixes = ["", "/", "//", "/.", "/..", "\\", "\\.\\", "/wheel.whl"];
    let mut cases = Vec::new();
    for (name, flavor) in [("posix", Flavor::Posix), ("windows", Flavor::Windows)] {
        for prefix in prefixes {
            for component in components {
                for suffix in suffixes {
                    let value = format!("{prefix}{component}{suffix}");
                    for home in [
                        None,
                        Some(""),
                        Some("/home/fixture/./"),
                        Some("C:\\Users\\fixture"),
                        Some("~unresolved"),
                    ] {
                        let expand =
                            || expand_with(&value, flavor, |_| Ok(home.map(str::to_owned)));
                        let expected = if preserve_urls {
                            unless_url(&value, expand)
                        } else {
                            expand()
                        };
                        cases.push(json!({
                            "mode": "lexical", "flavor": name, "value": value, "home": home,
                            "preserve_urls": preserve_urls,
                            "expected": outcome(expected),
                        }));
                    }
                }
            }
        }
    }
    assert_eq!(cases.len(), 21120);
    cases
}

#[test]
fn windows_home_selection_matches_ntpath_environment_rules() {
    let profiles = [
        None,
        Some(""),
        Some("C:\\Users\\fixture"),
        Some("C:/Users/fixture"),
        Some("C:\\Users\\fixture\\"),
        Some("C:\\Custom"),
        Some("~unresolved"),
        Some("//server/share/fixture"),
        Some("fixture"),
        Some("/fixture"),
    ];
    let mut cases = Vec::new();
    for profile in profiles {
        for current in [None, Some(""), Some("fixture"), Some("Fixture")] {
            for (drive, path) in [
                (None, None),
                (Some("D:"), Some("\\Users\\fixture")),
                (Some("D:"), Some("C:fixture")),
                (Some("//server/share"), Some("fixture")),
                (None, Some("")),
            ] {
                let mut env = serde_json::Map::new();
                for (key, value) in [
                    ("USERPROFILE", profile),
                    ("USERNAME", current),
                    ("HOMEDRIVE", drive),
                    ("HOMEPATH", path),
                ] {
                    if let Some(value) = value {
                        env.insert(key.into(), json!(value));
                    }
                }
                for user in ["", "fixture", "Fixture", "other", "é:", "~nested"] {
                    let value = format!("~{user}/./wheels/../");
                    let expected = normalize(&value, Flavor::Windows, |name| {
                        Ok(home::windows::resolve(name, |key| {
                            env.get(key).and_then(Value::as_str).map(str::to_owned)
                        }))
                    });
                    cases.push(json!({
                        "mode": "windows_home", "env": env, "value": value,
                        "expected": outcome(expected),
                    }));
                }
            }
        }
    }
    assert_eq!(cases.len(), 1200);
    compare(&cases);
    digest(&cases, "1240de0e17f4bfa48ca6c4ecb3884e35bdd0db15684f06d30f5823fb8ef9ec20");
}

#[cfg(unix)]
#[test]
fn unix_home_lookup_matches_the_current_account_database() {
    let mut users = vec![
        "".into(),
        "root".into(),
        "nobody".into(),
        "hy-no-such-user-6a08e7c1".into(),
        "nul\0user".into(),
    ];
    if let Ok(user) = std::env::var("USER") {
        users.push(user);
    }
    let cases: Vec<_> = users
        .iter()
        .map(|user| {
            let value = format!("~{user}/./wheels/..");
            let expected = normalize(&value, Flavor::Posix, home::resolve);
            json!({"mode": "unix_home", "value": value, "expected": outcome(expected)})
        })
        .collect();
    compare(&cases);
}

fn outcome(result: crate::error::Result<String>) -> Value {
    match result {
        Ok(value) => json!({"value": value}),
        Err(error) => json!({"error": error.to_string()}),
    }
}

fn digest(cases: &[Value], expected: &str) {
    let values: Vec<_> = cases.iter().map(|case| &case["expected"]).collect();
    let digest = format!("{:x}", Sha256::digest(serde_json::to_vec(&values).unwrap()));
    assert_eq!(digest, expected);
}

fn compare(cases: &[Value]) {
    let Some(python) = std::env::var_os("HY_TEST_VENV_ORACLE_PYTHON") else {
        return;
    };
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
        assert_eq!(*actual, case["expected"], "{case}");
    }
}
