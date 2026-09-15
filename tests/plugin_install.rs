//! Filesystem regressions exercised through the public CLI in isolated user directories.

#[path = "plugin_install/archive.rs"]
mod archive_cases;
#[path = "plugin_install/sources.rs"]
mod sources;
mod support;

use std::fs;

use serde_json::json;
use support::*;

#[test]
fn installs_upstream_manifest_and_preserves_false_setting_default() {
    let sandbox = Sandbox::new();
    let package = sandbox.path().join("plugin.zip");
    archive(&package, "1.0.0", &[]);

    assert_success(&sandbox.run(&["plugin", "install", package.to_str().unwrap()]));
    assert!(sandbox.path().join("idausr/plugins/example/plugin.py").is_file());
    assert!(!sandbox.path().join("idausr/plugins/example/package").exists());

    assert!(!sandbox.path().join("idausr/ida-config.json").exists());
    let value = sandbox.run(&["plugin", "config", "example", "get", "enabled"]);
    assert_success(&value);
    assert_eq!(String::from_utf8_lossy(&value.stdout).trim(), "false");

    let output = sandbox.run(&["plugin", "status", "--offline", "--json"]);
    assert_success(&output);
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["plugins"][0]["version"], "1.0.0");
}

#[test]
fn failed_replacement_preserves_installed_files() {
    let sandbox = Sandbox::new();
    let original = sandbox.path().join("original.zip");
    archive(&original, "1.0.0", &[("package/original.txt", b"retain me")]);
    assert_success(&sandbox.run(&["plugin", "install", original.to_str().unwrap()]));

    let invalid = sandbox.path().join("invalid.zip");
    archive(&invalid, "2.0.0", &[("package/../outside.txt", b"must not be extracted")]);
    let output = sandbox.run(&["plugin", "install", "--force", invalid.to_str().unwrap()]);
    assert!(!output.status.success());
    assert_eq!(
        fs::read(sandbox.path().join("idausr/plugins/example/original.txt")).unwrap(),
        b"retain me",
    );
    assert!(!sandbox.path().join("idausr/outside.txt").exists());
}

#[test]
fn invalid_configuration_is_preserved_and_prevents_installation() {
    for invalid in [
        "{broken json",
        "[]",
        r#"{"Plugins": []}"#,
        r#"{"Plugins": {"example": {"settings": false}}}"#,
    ] {
        let sandbox = Sandbox::new();
        let idausr = sandbox.path().join("idausr");
        fs::create_dir_all(&idausr).unwrap();
        let config = idausr.join("ida-config.json");
        fs::write(&config, invalid).unwrap();
        let package = sandbox.path().join("plugin.zip");
        archive(&package, "1.0.0", &[]);

        let install = sandbox.run(&["plugin", "install", package.to_str().unwrap()]);
        assert!(!install.status.success(), "accepted invalid config: {invalid}");
        assert!(!idausr.join("plugins/example").exists());
        assert_eq!(fs::read_to_string(&config).unwrap(), invalid);

        let update = sandbox.run(&["plugin", "config", "example", "set", "enabled", "true"]);
        assert!(!update.status.success());
        assert_eq!(fs::read_to_string(&config).unwrap(), invalid);
    }
}

#[test]
fn rejects_invalid_boolean_instead_of_silently_writing_false() {
    let sandbox = Sandbox::new();
    let package = sandbox.path().join("plugin.zip");
    archive(&package, "1.0.0", &[]);
    assert_success(&sandbox.run(&["plugin", "install", package.to_str().unwrap()]));
    assert_success(&sandbox.run(&["plugin", "config", "example", "set", "enabled", "true"]));
    let config = sandbox.path().join("idausr/ida-config.json");
    let original = fs::read(&config).unwrap();

    let output = sandbox.run(&["plugin", "config", "example", "set", "enabled", "typo"]);
    assert!(!output.status.success());
    assert_eq!(fs::read(config).unwrap(), original);
}

#[test]
fn repository_search_distinguishes_keywords_names_ranges_and_exact_versions() {
    let sandbox = Sandbox::new();
    let repository = sandbox.path().join("repository");
    fs::create_dir(&repository).unwrap();
    for version in ["1.9.0", "1.10.0", "2.0.0"] {
        archive(&repository.join(format!("example-{version}.zip")), version, &[]);
    }
    let url = url::Url::from_directory_path(&repository).unwrap();
    assert_success(&sandbox.run(&["plugin", "repo", "add", "local", url.as_str()]));

    let query = |query: &str| {
        let output = sandbox.run(&["plugin", "search", "--offline", "--json", query]);
        assert_success(&output);
        serde_json::from_slice::<serde_json::Value>(&output.stdout).unwrap()
    };
    let keyword = query("exam");
    assert_eq!(keyword["results"][0]["name"], "example");
    assert_eq!(keyword["results"][0]["version"], "2.0.0");
    assert_eq!(keyword["results"][0]["repo"], "local");

    let named = query("example");
    assert_eq!(named["versions"][0]["version"], "2.0.0");
    assert_eq!(named["versions"][1]["version"], "1.10.0");
    assert_eq!(named["versions"][2]["version"], "1.9.0");
    assert!(named["plugin"].get("platforms").is_none());

    let range = query("example>=1.10,<2");
    assert_eq!(range["versions"].as_array().unwrap().len(), 1);
    assert_eq!(range["versions"][0]["version"], "1.10.0");

    let exact = query("local/example==1.9.0");
    assert_eq!(exact["plugin"]["version"], "1.9.0");
    assert_eq!(exact["download_locations"].as_array().unwrap().len(), 1);
    assert!(exact["download_locations"][0]["url"].as_str().unwrap().ends_with("example-1.9.0.zip"));

    let missing = sandbox.run(&["plugin", "search", "--offline", "--json", "example==9.0.0"]);
    assert!(!missing.status.success());
    let error: serde_json::Value = serde_json::from_slice(&missing.stdout).unwrap();
    assert!(error["error"].as_str().unwrap().contains("9.0.0"));
}

#[test]
fn status_reports_multiple_names_and_fails_if_any_are_missing() {
    let sandbox = Sandbox::new();
    let package = sandbox.path().join("plugin.zip");
    archive(&package, "1.0.0", &[]);
    assert_success(&sandbox.run(&["plugin", "install", package.to_str().unwrap()]));

    let output =
        sandbox.run(&["plugin", "status", "--skip-upgrade-check", "--json", "example", "missing"]);
    assert!(!output.status.success());
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["plugins"][0]["name"], "example");
    assert_eq!(report["plugins"][0]["upgrade_checked"], false);
    assert_eq!(report["plugins"][1], json!({"name": "missing", "installed": false}));
}

#[test]
fn explain_environment_reports_missing_installation_without_failing() {
    let sandbox = Sandbox::new();
    let output = sandbox.run(&["ida", "python", "explain-environment", "--json"]);
    assert_success(&output);
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["experimental"], true);
    assert_eq!(report["selected_installation"]["install_dir"], serde_json::Value::Null);
    assert!(report["selected_installation"]["install_dir_error"].is_string());
    assert_eq!(report["python_environment"], serde_json::Value::Null);
    assert!(report.get("findings").is_none());

    let doctor = sandbox.run(&["ida", "python", "doctor", "--json"]);
    assert!(!doctor.status.success());
    let report: serde_json::Value = serde_json::from_slice(&doctor.stdout).unwrap();
    assert_eq!(report["ok"], false);
    assert!(
        report["findings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|finding| finding["id"] == "python-exe-not-found")
    );
}

#[test]
fn malformed_global_configuration_is_never_replaced() {
    let sandbox = Sandbox::new();
    let configuration = if cfg!(target_os = "macos") {
        sandbox.path().join("Library/Application Support/hcli/config.json")
    } else if cfg!(windows) {
        sandbox.path().join("local/hex-rays/hcli/config.json")
    } else {
        sandbox.path().join("config/hcli/config.json")
    };
    fs::create_dir_all(configuration.parent().unwrap()).unwrap();
    fs::write(&configuration, "{invalid").unwrap();
    let output = sandbox.run(&["ida", "list"]);
    assert!(!output.status.success());
    assert_eq!(fs::read_to_string(configuration).unwrap(), "{invalid");
}

#[test]
fn repository_override_resolves_an_offline_install_without_changing_configuration() {
    let sandbox = Sandbox::new();
    let repository = sandbox.path().join("repository");
    fs::create_dir(&repository).unwrap();
    archive(&repository.join("example.zip"), "1.0.0", &[]);
    let output = sandbox.run(&[
        "plugin",
        "--repo",
        repository.to_str().unwrap(),
        "--offline",
        "--pip-find-links",
        repository.to_str().unwrap(),
        "install",
        "example==1.0.0",
    ]);
    assert_success(&output);
    assert!(sandbox.path().join("idausr/plugins/example/plugin.py").is_file());
    assert!(!sandbox.path().join("idausr/ida-config.json").exists());
    let scoped = sandbox.run(&[
        "plugin",
        "--repo",
        repository.to_str().unwrap(),
        "install",
        "custom/example",
    ]);
    assert!(!scoped.status.success());
    assert!(String::from_utf8_lossy(&scoped.stderr).contains("repository prefixes"));
}

#[cfg(unix)]
#[test]
fn plugin_install_passes_pip_options_to_the_selected_interpreter() {
    let sandbox = Sandbox::new();
    let interpreter = sandbox.path().join("fake-python");
    let arguments = sandbox.path().join("arguments");
    fake_python(&interpreter);
    let package = sandbox.path().join("plugin.zip");
    archive_with_dependencies(&package, "1.0.0", &[], &["example-dependency==1.2.3"]);
    let output = sandbox.run_with_env(
        &[
            "plugin",
            "--pip-index-url",
            "https://primary.example/simple",
            "--pip-extra-index-url",
            "https://extra.example/simple",
            "--pip-find-links",
            "/tmp/wheels with spaces",
            "--offline",
            "--no-python-environment-check",
            "install",
            "--no-build-isolation",
            package.to_str().unwrap(),
        ],
        &[("HCLI_CURRENT_IDA_PYTHON_EXE", &interpreter), ("HY_TEST_PIP_ARGUMENTS", &arguments)],
    );
    assert_success(&output);
    let arguments = fs::read_to_string(arguments).unwrap();
    assert!(arguments.contains("--index-url\nhttps://primary.example/simple\n"));
    assert!(arguments.contains("--extra-index-url\nhttps://extra.example/simple\n"));
    assert!(arguments.contains("--find-links\n/tmp/wheels with spaces\n"));
    assert!(arguments.contains("--no-index\n"));
    assert!(arguments.contains("--no-build-isolation\n"));
    assert!(arguments.ends_with("example-dependency==1.2.3\n"));
    assert!(!arguments.lines().any(|argument| argument == "--"));
}
