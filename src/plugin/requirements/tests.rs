use super::*;
use serde_json::json;

#[test]
fn python_requirements_match_packaging_validation_and_membership() {
    let mut cases = Vec::new();
    for requirement in [
        "",
        " ",
        ",",
        ",,",
        ">=3.12",
        ">=3.12,",
        ",>=3.12",
        ">=3.12,,<4",
        ">=3.12.3,<3.14",
        "~=3.12",
        "~=3.12.2",
        "~=3",
        "==3.12.*",
        "!=3.12.*",
        "==3.12",
        "==3.12+local",
        ">=3.12+local",
        "==v3.12",
        ">=3.12a1",
        "!=3.12a1",
        "<3.12",
        ">3.12",
        ">=3.12.dev1",
        "<=3.12rc1",
        "===",
        "===foobar",
        "===3.12",
        "===3.12.0",
        "===3.12a1",
        "===3.12RC1",
        "===3.12+LOCAL",
        "===foo bar",
        "===foo)",
        "===foo;",
        "===foo(",
        ">=3.12,===3.12",
        "===3.12,===3.12.0",
        ">= 3.12",
        ">=\u{1c}3.12",
    ] {
        let valid = !crate::util::strings::python_trim(requirement).is_empty()
            && PythonRequirement::parse(requirement).is_some();
        for version in [
            "3.11.9",
            "3.12",
            "3.12.0",
            "3.12.2",
            "3.12.3",
            "3.12a1",
            "3.12rc1",
            "3.12.dev1",
            "3.12.post1",
            "3.12+local",
            "3.13.1",
            "4.0",
            "invalid",
        ] {
            cases.push(json!({
                "requirement": requirement,
                "version": version,
                "expected": {"valid": valid, "matches": valid && python_matches(version, requirement)},
            }));
        }
    }
    assert!(python_matches("3.12.3", ">=3.12.3"));
    assert!(!python_matches("3.12.2", ">=3.12.3"));
    assert!(!python_matches("3.12.0", "===3.12"));
    assert!(python_matches("3.12a1", ">=3.11"));
    let Some(python) = std::env::var_os("HY_TEST_RESOLVER_ORACLE_PYTHON") else {
        return;
    };
    use std::io::Write;
    use std::process::{Command, Stdio};
    let mut child = Command::new(python)
        .args(["-I", "-B", "-c", include_str!("reference.py")])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(&serde_json::to_vec(&cases).unwrap()).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    let actual: Vec<serde_json::Value> = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(actual.len(), cases.len());
    for (actual, case) in actual.iter().zip(&cases) {
        assert_eq!(actual, &case["expected"], "{case}");
    }
}
