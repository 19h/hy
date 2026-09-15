use serde_json::json;

use super::*;

#[test]
fn size_units_and_invalid_ranges_match_upstream() {
    let mut inputs = vec!["-1".to_owned(), "0".into(), "1".into(), (1_u128 << 80).to_string()];
    for exponent in 1..=5 {
        let boundary = 1024_u64.pow(exponent);
        inputs.extend([boundary - 1, boundary, boundary + 1].map(|value| value.to_string()));
    }
    let expected: Vec<_> = inputs
        .iter()
        .map(|value| {
            let asset: Asset = serde_json::from_value(json!({
                "filename": "fixture", "key": "fixture", "size": value,
            }))
            .unwrap();
            size(&asset).ok()
        })
        .collect();
    assert_eq!(&expected[..4], &[None, Some("0 B".into()), Some("1.0 B".into()), None]);
    assert_eq!(expected[5], Some("1.0 KB".into()));
    assert_eq!(expected[17], None);

    let Some(python) = std::env::var_os("HY_TEST_ASSET_ORACLE_PYTHON") else {
        return;
    };
    use std::io::Write;
    use std::process::{Command, Stdio};
    let source =
        std::env::var_os("HY_TEST_HCLI_SOURCE").map(std::path::PathBuf::from).unwrap_or_else(
            || std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("ida-hcli"),
        );
    let script = r#"
import ast, json, sys
path = sys.argv[1]
with open(path) as source:
    nodes = [node for node in ast.parse(source.read()).body
             if isinstance(node, ast.FunctionDef) and node.name == 'format_size']
assert len(nodes) == 1
namespace = {}
exec(compile(ast.Module(body=nodes, type_ignores=[]), path, 'exec'), namespace)
results = []
for value in json.load(sys.stdin):
    try:
        results.append(namespace['format_size'](int(value)))
    except (ValueError, OverflowError, IndexError):
        results.append(None)
print(json.dumps(results))
"#;
    let mut child = Command::new(python)
        .args(["-I", "-B", "-c", script])
        .arg(source.join("src/hcli/commands/share/list.py"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(&serde_json::to_vec(&inputs).unwrap()).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    let actual: Vec<Option<String>> = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(actual, expected);
}
