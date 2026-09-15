use std::os::unix::fs::{PermissionsExt, symlink};

use super::*;

fn source(sandbox: &Sandbox) -> std::path::PathBuf {
    let source = sandbox.path().join("source");
    fs::create_dir(&source).unwrap();
    fs::write(
        source.join("ida-plugin.json"),
        serde_json::to_vec(&identity_manifest("1", "https://github.com/example/directory"))
            .unwrap(),
    )
    .unwrap();
    fs::write(source.join("plugin.py"), b"original payload").unwrap();
    source
}

#[test]
fn every_valid_descriptor_counts_before_an_upgrade_can_be_skipped() {
    for name in ["example", "other"] {
        let sandbox = Sandbox::new();
        let source = source(&sandbox);
        assert_success(&sandbox.run(&["plugin", "install", source.to_str().unwrap()]));
        let mut descriptor = identity_manifest("1", "https://github.com/example/directory");
        descriptor["plugin"]["name"] = json!(name);
        fs::create_dir(source.join("nested")).unwrap();
        fs::write(source.join("nested/ida-plugin.json"), serde_json::to_vec(&descriptor).unwrap())
            .unwrap();
        let config = fs::read(sandbox.config_path()).ok();
        let output = sandbox.run(&["plugin", "install", "-U", source.to_str().unwrap()]);
        assert!(!output.status.success(), "accepted second descriptor named {name}");
        assert!(String::from_utf8_lossy(&output.stderr).contains("containing one plugin"));
        assert_eq!(fs::read(sandbox.config_path()).ok(), config);
        assert_eq!(
            fs::read(sandbox.path().join("idausr/plugins/example/plugin.py")).unwrap(),
            b"original payload",
        );
    }
}

#[test]
fn one_nested_valid_descriptor_can_replace_an_invalid_root_descriptor() {
    let sandbox = Sandbox::new();
    let source = source(&sandbox);
    fs::create_dir(source.join("nested")).unwrap();
    fs::rename(source.join("ida-plugin.json"), source.join("nested/ida-plugin.json")).unwrap();
    fs::rename(source.join("plugin.py"), source.join("nested/plugin.py")).unwrap();
    fs::write(source.join("ida-plugin.json"), b"invalid json").unwrap();
    fs::write(source.join("outside"), b"not selected").unwrap();
    assert_success(&sandbox.run(&["plugin", "install", source.to_str().unwrap()]));
    let target = sandbox.path().join("idausr/plugins/example");
    assert_eq!(fs::read(target.join("plugin.py")).unwrap(), b"original payload");
    assert!(!target.join("outside").exists());
    assert!(!target.join("nested").exists());
}

#[test]
fn excluded_descriptors_do_not_count_as_plugins() {
    let sandbox = Sandbox::new();
    let source = source(&sandbox);
    for excluded in [".git", ".hg", ".svn", "__pycache__", ".DS_Store"] {
        fs::create_dir(source.join(excluded)).unwrap();
        fs::copy(source.join("ida-plugin.json"), source.join(excluded).join("ida-plugin.json"))
            .unwrap();
    }
    assert_success(&sandbox.run(&["plugin", "install", source.to_str().unwrap()]));
}

#[test]
fn packaging_failure_precedes_equal_version_upgrade_skip() {
    let sandbox = Sandbox::new();
    let source = source(&sandbox);
    assert_success(&sandbox.run(&["plugin", "install", source.to_str().unwrap()]));
    symlink(source.join("absent"), source.join("unrelated-link")).unwrap();
    let output = sandbox.run(&["plugin", "install", "-U", source.to_str().unwrap()]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("No such file or directory"));
    assert_eq!(
        fs::read(sandbox.path().join("idausr/plugins/example/plugin.py")).unwrap(),
        b"original payload",
    );
}

#[test]
fn regular_installation_uses_created_file_permissions_and_keeps_source_modes() {
    let sandbox = Sandbox::new();
    let source = source(&sandbox);
    let script = source.join("plugin.py");
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
    assert_success(&sandbox.run(&["plugin", "install", source.to_str().unwrap()]));
    let installed = sandbox.path().join("idausr/plugins/example/plugin.py");
    assert_eq!(fs::metadata(installed).unwrap().permissions().mode() & 0o111, 0);
    assert_eq!(fs::metadata(script).unwrap().permissions().mode() & 0o777, 0o755);
}

#[test]
fn home_expansion_and_directory_links_select_the_local_source() {
    for editable in [false, true] {
        let sandbox = Sandbox::new();
        let source = source(&sandbox);
        symlink(&source, sandbox.path().join("linked-source")).unwrap();
        let mut args = vec!["plugin", "install", "~/linked-source"];
        if editable {
            args.push("--editable");
        }
        assert_success(&sandbox.run(&args));
        let installed = sandbox.path().join("idausr/plugins/example");
        assert_eq!(installed.is_symlink(), editable);
        assert_eq!(fs::read(installed.join("plugin.py")).unwrap(), b"original payload");
        if editable {
            assert_eq!(installed.canonicalize().unwrap(), source.canonicalize().unwrap());
        }
    }
}

#[test]
fn a_bare_directory_without_metadata_falls_through_to_repository_selection() {
    let sandbox = Sandbox::new();
    fs::create_dir(sandbox.path().join("example")).unwrap();
    let repository = sandbox.path().join("repository");
    fs::create_dir(&repository).unwrap();
    archive(&repository.join("example.zip"), "1", &[]);
    let output = sandbox
        .command(&["plugin", "--repo", repository.to_str().unwrap(), "install", "example"])
        .current_dir(sandbox.path())
        .output()
        .unwrap();
    assert_success(&output);
    assert!(sandbox.path().join("idausr/plugins/example/plugin.py").is_file());
}
