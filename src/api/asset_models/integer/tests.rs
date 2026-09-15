use super::*;

#[test]
fn asset_integer_conversions_match_locked_pydantic() {
    let mut values = vec![
        "null".into(),
        "true".into(),
        "false".into(),
        "[]".into(),
        "{}".into(),
        "0".into(),
        "-1".into(),
        "1.0".into(),
        "1.5".into(),
        "1e20".into(),
        "9223372036854775808".into(),
        "184467440737095516160".into(),
    ];
    let mut frontier = vec![String::new()];
    values.push("\"\"".into());
    for _ in 0..5 {
        frontier = frontier
            .iter()
            .flat_map(|prefix| {
                ['0', '1', '+', '-', '_', '.', ' ', 'e']
                    .into_iter()
                    .map(move |character| format!("{prefix}{character}"))
            })
            .collect();
        values.extend(frontier.iter().map(|value| serde_json::to_string(value).unwrap()));
    }
    for size in [4299, 4300, 4301, 4400] {
        for text in [
            "1".repeat(size),
            "0".repeat(size),
            format!("-{}", "1".repeat(size)),
            format!("+{}", "1".repeat(size)),
            format!("1.{}", "0".repeat(size)),
        ] {
            values.push(serde_json::to_string(&text).unwrap());
        }
        values.push("1".repeat(size));
        values.push(format!("-{}", "1".repeat(size)));
    }
    for text in ["\u{1c}1", "\u{85}1\u{85}", "١", "０", "1\u{a0}", "0__1", "00-1", "0_0.0", "1.0_0"]
    {
        values.push(serde_json::to_string(text).unwrap());
    }
    let expected: Vec<_> = values
        .iter()
        .map(|value| serde_json::from_str::<Integer>(value).ok().map(|value| value.to_string()))
        .collect();
    use sha2::{Digest, Sha256};
    assert_eq!(values.len(), 37498);
    assert_eq!(
        format!("{:x}", Sha256::digest(serde_json::to_vec(&expected).unwrap())),
        "7c78a33d2a4170db1f7c839d0d417f4163d254efb2ac965145172988f0ba0924"
    );
    assert_eq!(serde_json::from_str::<Integer>("\"00-1\"").unwrap().to_string(), "-1");
    assert!(serde_json::from_str::<Integer>("null").is_err());
    if let Some(python) = std::env::var_os("HY_TEST_ASSET_ORACLE_PYTHON") {
        use std::io::Write;
        use std::process::{Command, Stdio};
        let mut child = Command::new(python)
            .args([
                "-I",
                "-B",
                "-c",
                r#"
import json, sys, pydantic
assert pydantic.__version__ == '2.12.5'
adapter = pydantic.TypeAdapter(int)
results = []
for raw in json.load(sys.stdin):
    try:
        results.append(str(adapter.validate_python(json.loads(raw))))
    except (ValueError, OverflowError):
        results.append(None)
print(json.dumps(results))
"#,
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(&serde_json::to_vec(&values).unwrap()).unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(output.status.success());
        let actual: Vec<Option<String>> = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(actual.len(), expected.len());
        for (index, (actual, expected)) in actual.iter().zip(&expected).enumerate() {
            assert_eq!(actual, expected, "input {}", values[index]);
        }
    }
}
