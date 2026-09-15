//! Compile and execute AppleScript fixtures without registering an application.

use std::fs;
use std::os::unix::fs::PermissionsExt;

use super::*;

#[test]
fn compiles_handler_without_registering_it() {
    let temporary = tempfile::tempdir().unwrap();
    let app = temporary.path().join("Test.app");
    compile("/tmp/a space/a'quote/a\"quote/hy", &temporary.path().join("Logs"), &app).unwrap();
    assert!(app.join("Contents/MacOS/applet").is_file());
    let plist = Command::new("plutil")
        .args(["-extract", "CFBundleURLTypes.0.CFBundleURLSchemes.0", "raw", "-o", "-"])
        .arg(app.join("Contents/Info.plist"))
        .output()
        .unwrap();
    assert!(plist.status.success());
    assert_eq!(String::from_utf8(plist.stdout).unwrap().trim(), "ida");
}

#[test]
fn handler_preserves_literal_url_arguments_resets_python_environment_and_logs_both_streams() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path();
    let executable = root.join("hy with ' and \" and $ chars");
    fs::write(
        &executable,
        concat!(
            "#!/bin/sh\n",
            "printf '%s\\n' \"$@\" > \"$0.argv\"\n",
            "printf '%s\\n' \"${PYTHONHOME-unset}\" \"${PYTHONPATH-unset}\" \"${PYTHONEXECUTABLE-unset}\" \"${PYTHONSTARTUP-unset}\" > \"$0.environment\"\n",
            "printf 'fixture stdout\\n'\n",
            "printf 'fixture stderr\\n' >&2\n",
        ),
    ).unwrap();
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o755)).unwrap();
    let logs = root.join("missing ' and \" logs");
    let app = root.join("Fixture.app");
    compile(executable.to_str().unwrap(), &logs, &app).unwrap();
    assert!(!logs.exists());
    let compiled_script = app.join("Contents/Resources/Scripts/main.scpt");
    let url = "ida://source/a%20b.i64/functions?rva=0x1&literal='\"$()\\%22";
    let output = Command::new("osascript")
        .arg("-e")
        .arg(format!(
            "set handlerScript to load script POSIX file \"{}\"",
            literal(compiled_script.to_str().unwrap())
        ))
        .arg("-e")
        .arg(format!("tell handlerScript to open location \"{}\"", literal(url)))
        .env("HOME", root)
        .env_remove("ZDOTDIR")
        .env("PYTHONHOME", "/incorrect-home")
        .env("PYTHONPATH", "/incorrect-path")
        .env("PYTHONEXECUTABLE", "/incorrect-executable")
        .env("PYTHONSTARTUP", "/incorrect-startup")
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert_eq!(
        fs::read_to_string(format!("{}.argv", executable.display())).unwrap(),
        format!("ida\nopen\n--\n{url}\n")
    );
    assert_eq!(
        fs::read_to_string(format!("{}.environment", executable.display())).unwrap(),
        "unset\nunset\nunset\nunset\n"
    );
    let log = fs::read_to_string(logs.join("idb_handler.log")).unwrap();
    assert!(log.contains("fixture stdout\n"), "{log}");
    assert!(log.contains("fixture stderr\n"), "{log}");
}
