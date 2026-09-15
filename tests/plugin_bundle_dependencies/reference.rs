use std::path::Path;
use std::process::Command;

use serde_json::Value;

pub(super) fn compare(package: &Path, expected: Value) {
    compare_with_home(package, None, expected);
}

pub(super) fn compare_with_home(package: &Path, home: Option<&Path>, expected: Value) {
    if let Some(mut command) = command(package) {
        if let Some(home) = home {
            command.current_dir(home).env("HOME", home);
        }
        assert_result(command, expected);
    }
}

fn command(package: &Path) -> Option<Command> {
    let python = std::env::var_os("HY_TEST_BUNDLE_ORACLE_PYTHON")?;
    let source =
        std::env::var_os("HY_TEST_HCLI_SOURCE").map(std::path::PathBuf::from).unwrap_or_else(
            || Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("ida-hcli"),
        );
    let mut command = Command::new(python);
    command.args(["-I", "-B", "-c", include_str!("reference.py")]).arg(source).arg(package);
    Some(command)
}

fn assert_result(mut command: Command, expected: Value) {
    let output = command.output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let actual: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(actual, expected);
}
