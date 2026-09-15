//! Lint findings, source handling and command exit status.

#[path = "plugin_lint/archive.rs"]
mod archive;
mod support;

use std::fs;
use std::io::Write;

use serde_json::json;
use support::{
    http::{Response, Server},
    *,
};

#[test]
fn lint_handles_directories_multiple_plugins_and_remote_archives() {
    let sandbox = Sandbox::new();
    let descriptor = identity_manifest("1", "https://github.com/example/lint");
    let directory = sandbox.path().join("source");
    fs::create_dir(&directory).unwrap();
    fs::write(directory.join("ida-plugin.json"), serde_json::to_vec(&descriptor).unwrap()).unwrap();
    fs::write(directory.join("plugin.py"), b"fixture").unwrap();
    fs::write(directory.join("readme.txt"), b"fixture").unwrap();
    let output = sandbox.run(&["plugin", "lint", directory.to_str().unwrap()]);
    assert_success(&output);
    assert!(String::from_utf8_lossy(&output.stdout).contains("rename readme.txt to README.md"));

    let package = sandbox.path().join("plugins.zip");
    let mut archive = zip::ZipWriter::new(fs::File::create(&package).unwrap());
    for name in ["first", "second"] {
        let mut descriptor = descriptor.clone();
        descriptor["plugin"]["name"] = json!(name);
        for (file, bytes) in [
            ("ida-plugin.json", serde_json::to_vec(&descriptor).unwrap()),
            ("plugin.py", b"fixture".to_vec()),
        ] {
            archive
                .start_file(format!("{name}/{file}"), zip::write::SimpleFileOptions::default())
                .unwrap();
            archive.write_all(&bytes).unwrap();
        }
    }
    archive.finish().unwrap();
    let bytes = fs::read(&package).unwrap();
    let server = Server::start(move |_, _| Response::zip(bytes.clone()));
    for path in [package.to_str().unwrap(), &server.url] {
        let output = sandbox.run(&["plugin", "lint", path]);
        assert_success(&output);
        let report = String::from_utf8_lossy(&output.stdout);
        assert!(report.contains(":first/ida-plugin.json"));
        assert!(report.contains(":second/ida-plugin.json"));
    }
    assert_eq!(server.requests().len(), 1);
}

#[test]
fn lint_reports_validation_findings_but_rejects_missing_paths() {
    let sandbox = Sandbox::new();
    let package = sandbox.path().join("plugin.zip");
    let mut descriptor = identity_manifest("1", "https://github.com/example/lint");
    descriptor["plugin"]["logoPath"] = json!("absent.svg");
    archive_manifest(&package, &descriptor, &[]);
    let output = sandbox.run(&["plugin", "lint", package.to_str().unwrap()]);
    assert_success(&output);
    assert!(String::from_utf8_lossy(&output.stdout).contains("logo file not found"));
    assert!(
        !sandbox
            .run(&["plugin", "lint", sandbox.path().join("absent").to_str().unwrap()])
            .status
            .success()
    );
}

#[test]
fn lint_matches_upstream_findings_and_exit_boundaries() {
    let sandbox = Sandbox::new();
    for case in [
        "recommendations",
        "complete",
        "missing-dependencies",
        "malformed-inline",
        "invalid-metadata",
        "missing-entry",
        "missing-descriptor",
    ] {
        let directory = sandbox.path().join(case);
        fs::create_dir(&directory).unwrap();
        let mut manifest = identity_manifest("1", "https://github.com/example/lint");
        let mut source = b"fixture".as_slice();
        match case {
            "recommendations" => {
                manifest["plugin"]["z-extra"] = json!(true);
                manifest["plugin"]["a-extra"] = json!(true);
                manifest["plugin"]["maintainers"] = json!([{"email": "m@example.test"}]);
            }
            "complete" => {
                manifest["plugin"]["description"] = json!("Fixture");
                manifest["plugin"]["categories"] = json!(["decompilation"]);
                manifest["plugin"]["keywords"] = json!(["fixture"]);
                manifest["plugin"]["license"] = json!("MIT");
                manifest["plugin"]["logoPath"] = json!("logo.svg");
                manifest["plugin"]["authors"][0]["name"] = json!("Fixture");
            }
            "missing-dependencies" => {
                manifest["plugin"]["pythonDependencies"] = json!("absent.txt");
            }
            "malformed-inline" => {
                manifest["plugin"]["pythonDependencies"] = json!("inline");
                source = b"# /// script\n# dependencies = [invalid\n# ///\n";
            }
            "invalid-metadata" => manifest["plugin"]["name"] = json!("invalid/name"),
            _ => {}
        }
        let descriptor = serde_json::to_vec(&manifest).unwrap();
        if case != "missing-descriptor" {
            fs::write(directory.join("ida-plugin.json"), &descriptor).unwrap();
        }
        if case != "missing-entry" {
            fs::write(directory.join("plugin.py"), source).unwrap();
        }
        let readme = if case == "complete" {
            "README.md"
        } else {
            "readme.txt"
        };
        fs::write(directory.join(readme), b"fixture").unwrap();
        fs::write(directory.join("logo.svg"), b"fixture").unwrap();

        let package = sandbox.path().join(format!("{case}.ZIP"));
        let mut archive = zip::ZipWriter::new(fs::File::create(&package).unwrap());
        for entry in fs::read_dir(&directory).unwrap() {
            let entry = entry.unwrap();
            archive
                .start_file(
                    format!("package/{}", entry.file_name().to_str().unwrap()),
                    zip::write::SimpleFileOptions::default(),
                )
                .unwrap();
            archive.write_all(&fs::read(entry.path()).unwrap()).unwrap();
        }
        archive.finish().unwrap();
        let valid = !matches!(case, "invalid-metadata" | "missing-entry" | "missing-descriptor");
        for path in [&directory, &package] {
            let output = sandbox.run(&["plugin", "lint", path.to_str().unwrap()]);
            assert_success(&output);
            let report = String::from_utf8_lossy(&output.stdout);
            assert_eq!(report.trim() == "no recommendations", case == "complete", "{case}");
            assert_eq!(report.contains("Error"), !valid, "{case}: {report}");
            compare_upstream(&sandbox, path, &output, valid);
        }
    }
    for (name, contents) in [("corrupt.zip", b"invalid".as_slice()), ("wrong.txt", b"")] {
        let path = sandbox.path().join(name);
        fs::write(&path, contents).unwrap();
        let output = sandbox.run(&["plugin", "lint", path.to_str().unwrap()]);
        assert!(!output.status.success());
        compare_upstream(&sandbox, &path, &output, false);
    }
    let path = sandbox.path().join("absent");
    let output = sandbox.run(&["plugin", "lint", path.to_str().unwrap()]);
    assert!(!output.status.success());
    compare_upstream(&sandbox, &path, &output, false);
}

#[test]
fn lint_expands_home_and_resolves_directory_aliases() {
    let sandbox = Sandbox::new();
    let source = sandbox.path().join("source");
    fs::create_dir(&source).unwrap();
    fs::write(
        source.join("ida-plugin.json"),
        serde_json::to_vec(&identity_manifest("1", "https://github.com/example/lint")).unwrap(),
    )
    .unwrap();
    fs::write(source.join("plugin.py"), b"fixture").unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(source.join("plugin.py"), source.join("README.md")).unwrap();
    #[cfg(not(unix))]
    fs::write(source.join("README.md"), b"fixture").unwrap();

    for path in ["~/source", "~/source/../source"] {
        let output = sandbox.run(&["plugin", "lint", path]);
        assert_success(&output);
        let report = String::from_utf8_lossy(&output.stdout);
        assert!(report.contains(&*source.canonicalize().unwrap().to_string_lossy()));
        assert!(!report.contains("README.md"));
        compare_upstream(&sandbox, std::path::Path::new(path), &output, true);
    }
}

fn compare_upstream(
    sandbox: &Sandbox,
    path: &std::path::Path,
    native: &std::process::Output,
    compare_text: bool,
) {
    let Some(python) = std::env::var_os("HY_TEST_LINT_ORACLE_PYTHON") else {
        return;
    };
    let output = std::process::Command::new(python)
        .args([
            "-I",
            "-B",
            "-c",
            r#"
import json, sys
from click.testing import CliRunner
from hcli.commands.plugin.lint import lint_plugin_directory
from hcli.lib.console import console
console.width = 10000
console.no_color = True
result = CliRunner().invoke(lint_plugin_directory, [sys.argv[1]], color=False)
print(json.dumps({'success': result.exit_code == 0, 'output': result.output}))
"#,
        ])
        .arg(path)
        .env("HOME", sandbox.path())
        .env("USERPROFILE", sandbox.path())
        .output()
        .unwrap();
    assert_success(&output);
    let oracle: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(oracle["success"], native.status.success(), "{}: {oracle}", path.display());
    if compare_text {
        assert_eq!(
            oracle["output"].as_str().unwrap(),
            String::from_utf8_lossy(&native.stdout),
            "{}",
            path.display()
        );
    }
}
