//! Source-compatible reference spellings across lint, indexing and installation.

use super::*;

#[allow(dead_code)]
#[path = "../../src/util/python_zip/fixtures.rs"]
mod fixtures;

use fixtures::{Member, zip};

#[test]
fn literal_reference_names_survive_cataloguing_installation_and_inline_dependencies() {
    const SCRIPT: &[u8] = b"# /// script\n# dependencies = ['fixture-dependency']\n# ///\n";
    for (reference, filename) in [
        ("with:colon.py", "with:colon.py"),
        ("with\\slash.py", "with\\slash.py"),
        ("..\\entry.py", "..\\entry.py"),
        ("nested//./entry.py", "nested/entry.py"),
    ] {
        let sandbox = Sandbox::new();
        let mut descriptor = identity_manifest("1", "https://github.com/example/files");
        descriptor["plugin"]["entryPoint"] = json!(reference);
        descriptor["plugin"]["pythonDependencies"] = json!("inline");
        let package = sandbox.path().join("plugin.zip");
        fs::write(
            &package,
            zip(&[
                Member::new(b"package/ida-plugin.json", &serde_json::to_vec(&descriptor).unwrap()),
                Member::new(format!("package/{filename}").as_bytes(), SCRIPT),
            ]),
        )
        .unwrap();
        let lint = sandbox.run(&["plugin", "lint", package.to_str().unwrap()]);
        assert_success(&lint);
        assert!(!String::from_utf8_lossy(&lint.stdout).contains("Error:"));
        let search = sandbox.run(&[
            "plugin",
            "--repo",
            sandbox.path().to_str().unwrap(),
            "search",
            "--json",
            "example==1",
        ]);
        assert_success(&search);
        let report: Value = serde_json::from_slice(&search.stdout).unwrap();
        assert_eq!(report["plugin"]["entryPoint"], reference);

        let python = sandbox.path().join("python");
        fake_python(&python);
        let arguments = sandbox.path().join("pip-arguments");
        assert_success(&sandbox.run_with_env(
            &["plugin", "--no-python-environment-check", "install", package.to_str().unwrap()],
            &[("HCLI_CURRENT_IDA_PYTHON_EXE", &python), ("HY_TEST_PIP_ARGUMENTS", &arguments)],
        ));
        assert_eq!(
            fs::read(sandbox.path().join("idausr/plugins/example").join(filename)).unwrap(),
            SCRIPT,
        );
        assert!(fs::read_to_string(arguments).unwrap().contains("fixture-dependency"));
    }
}

#[test]
fn directory_references_are_valid_for_lint_and_editable_registration() {
    for reference in ["entry.py", "entry.py/", "entry.py/.", ""] {
        let sandbox = Sandbox::new();
        let source = sandbox.path().join("source");
        fs::create_dir_all(source.join("entry.py")).unwrap();
        let mut descriptor = identity_manifest("1", "https://github.com/example/files");
        descriptor["plugin"]["entryPoint"] = json!(reference);
        fs::write(source.join("ida-plugin.json"), serde_json::to_vec(&descriptor).unwrap())
            .unwrap();
        let output = sandbox.run(&["plugin", "lint", source.to_str().unwrap()]);
        assert_success(&output);
        assert!(!String::from_utf8_lossy(&output.stdout).contains("Error:"));
        assert_success(&sandbox.run(&[
            "plugin",
            "--no-python-environment-check",
            "install",
            "--editable",
            source.to_str().unwrap(),
        ]));
        let installed = sandbox.path().join("idausr/plugins/example");
        assert!(installed.is_symlink());
        assert!(installed.join("entry.py").is_dir());
        assert_eq!(installed_manifest(&sandbox)["plugin"]["entryPoint"], reference);
    }
}
