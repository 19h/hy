use serde_json::{Value, json};

use super::*;

#[test]
fn asset_models_preserve_defaults_coercion_and_numeric_metadata_equality() {
    let base = json!({"filename": "fixture.txt", "key": "fixture"});
    let asset: Asset = serde_json::from_value(base.clone()).unwrap();
    assert_eq!(asset.size.to_string(), "0");
    assert_eq!(asset.version.to_string(), "0");
    let mut inputs = vec![base.clone()];
    for field in [
        "size",
        "version",
        "filename",
        "key",
        "email",
        "code",
        "created_at",
        "expires_at",
        "url",
        "metadata",
    ] {
        for value in [
            json!(null),
            json!(true),
            json!(1),
            json!(-1),
            json!(1.0),
            json!(1.5),
            json!(""),
            json!("1.0"),
            json!("1_000"),
            json!("-184467440737095516160"),
            json!([]),
            json!({"value": 1}),
        ] {
            let mut input = base.clone();
            input[field] = value;
            inputs.push(input);
        }
    }
    for field in ["filename", "key"] {
        let mut input = base.clone();
        input.as_object_mut().unwrap().remove(field);
        inputs.push(input);
    }
    let expected: Vec<_> = inputs
        .iter()
        .map(|input| {
            serde_json::from_value::<Asset>(input.clone())
                .ok()
                .map(|asset| serde_json::to_value(asset).unwrap())
        })
        .collect();

    let mut left = base.clone();
    left["metadata"] = json!({"nested": [true, {"value": 1.0}]});
    let mut right = base.clone();
    right["metadata"] = json!({"nested": [1, {"value": true}]});
    let first: Asset = serde_json::from_value(left.clone()).unwrap();
    let second: Asset = serde_json::from_value(right.clone()).unwrap();
    assert_eq!(first, second);
    let mut different = second.clone();
    different.key = "different".into();
    assert_ne!(first, different);
    compare_upstream(&inputs, &expected, &[left, right]);
}

fn compare_upstream(inputs: &[Value], expected: &[Option<Value>], equal: &[Value]) {
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
import ast, json, sys, pydantic
assert pydantic.__version__ == '2.12.5'
path = sys.argv[1]
with open(path) as source:
    classes = [node for node in ast.parse(source.read()).body if isinstance(node, ast.ClassDef) and node.name == 'Asset']
namespace = {'BaseModel': pydantic.BaseModel}
exec(compile(ast.Module(body=classes, type_ignores=[]), path, 'exec'), namespace)
Asset = namespace['Asset']
data = json.load(sys.stdin)
assert Asset(**data['equal'][0]) == Asset(**data['equal'][1])
results = []
for value in data['inputs']:
    try:
        results.append(Asset(**value).model_dump())
    except pydantic.ValidationError:
        results.append(None)
print(json.dumps(results))
"#;
    let mut child = Command::new(python)
        .args(["-I", "-B", "-c", script])
        .arg(source.join("src/hcli/lib/api/asset.py"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(&serde_json::to_vec(&json!({"inputs": inputs, "equal": equal})).unwrap())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    let actual: Vec<Option<Value>> = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(actual.len(), expected.len());
    for (index, (actual, expected)) in actual.iter().zip(expected).enumerate() {
        assert_eq!(actual, expected, "{}", inputs[index]);
    }
}
