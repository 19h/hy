use super::*;

#[test]
fn confirmation_grammars_match_locked_click_and_rich() {
    let inputs: Vec<_> = ["", "y", "Y", "n", "N", "yes", "YES", "no", "No", "true", "1", "ｙ"]
        .into_iter()
        .flat_map(|value| [value.to_owned(), format!(" {value} "), format!("\u{1c}{value}\u{85}")])
        .collect();
    let expected: Vec<_> = inputs
        .iter()
        .map(|value| [Grammar::Overwrite.parse(value), Grammar::Delete.parse(value)])
        .collect();
    assert_eq!(expected[0], [Some(false), None]);
    assert_eq!(expected[3], [Some(true), Some(true)]);
    assert_eq!(expected[15], [Some(true), None]);

    let Some(python) = std::env::var_os("HY_TEST_SHARE_REPORT_ORACLE_PYTHON") else {
        return;
    };
    use std::process::{Command, Stdio};
    let script = r#"
import contextlib, importlib.metadata, io, json, sys
import click
from rich.prompt import Confirm, InvalidResponse
assert importlib.metadata.version('click') == '8.1.8'
assert importlib.metadata.version('rich') == '14.3.2'
inputs = json.load(sys.stdin)
results = []
for value in inputs:
    with contextlib.redirect_stdout(io.StringIO()):
        sys.stdin = io.StringIO(value + '\n')
        try:
            overwrite = click.confirm('Overwrite existing file?')
        except click.Abort:
            overwrite = None
    try:
        delete = Confirm().process_response(value)
    except InvalidResponse:
        delete = None
    results.append([overwrite, delete])
print(json.dumps(results))
"#;
    let mut child = Command::new(python)
        .args(["-I", "-B", "-c", script])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(&serde_json::to_vec(&inputs).unwrap()).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    let actual: Vec<[Option<bool>; 2]> = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(actual, expected);
}
