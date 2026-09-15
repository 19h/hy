//! Exercise setup against real temporary installations and configuration files.

use std::fs;
use std::path::Path;

use serde_json::Value;

use super::*;

fn installation(root: &Path, name: &str, version: &str) -> PathBuf {
    let path = root.join(name);
    fs::create_dir_all(path.join("python")).unwrap();
    fs::write(path.join("ida"), b"fixture").unwrap();
    fs::write(path.join("ida.exe"), b"fixture").unwrap();
    fs::write(path.join("python/ida_pro.py"), format!("# IDA SDK v{version}\n")).unwrap();
    path
}

#[test]
fn existing_instances_preserve_missing_empty_and_stale_defaults_without_writes() {
    for default in [None, Some(""), Some("missing"), Some("existing")] {
        let temporary = tempfile::tempdir().unwrap();
        let path = temporary.path().join("config.json");
        let mut initial = json!({"ida.instances":{"existing":"/stale/path"},"sentinel":"retain"});
        if let Some(default) = default {
            initial["ida.default"] = json!(default);
        }
        let before = serde_json::to_vec(&initial).unwrap();
        fs::write(&path, &before).unwrap();
        let store = ConfigStore::open_at(path.clone()).unwrap();
        assert!(report_existing(&store).unwrap());
        assert_eq!(fs::read(path).unwrap(), before);
    }
}

#[test]
fn discovery_uses_numeric_versions_and_first_duplicate_name_then_commits_the_default() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path();
    let older = installation(root, "ida-pro-9.9", "9.9");
    let newest = installation(root, "ida-pro-9.10", "9.10");
    let duplicate = installation(&root.join("duplicate"), "ida-pro-9.10", "99.0");
    let invalid = root.join("empty");
    fs::create_dir(&invalid).unwrap();
    let config_path = root.join("config.json");
    fs::write(&config_path, br#"{"ida.default":"stale","sentinel":"retain"}"#).unwrap();
    let mut store = ConfigStore::open_at(config_path.clone()).unwrap();
    assert!(!report_existing(&store).unwrap());
    register_discovered(&mut store, vec![invalid, older.clone(), newest.clone(), duplicate])
        .unwrap();
    let saved: Value = serde_json::from_slice(&fs::read(config_path).unwrap()).unwrap();
    assert_eq!(saved["ida.default"], "ida-pro-9.10");
    assert_eq!(
        saved["ida.instances"],
        json!({"ida-pro-9.9":older.canonicalize().unwrap(),"ida-pro-9.10":newest.canonicalize().unwrap()})
    );
    assert_eq!(saved["sentinel"], "retain");
    assert_eq!(store.get_str("ida.default"), Some("ida-pro-9.10"));
}

#[test]
fn no_valid_discoveries_do_not_create_configuration() {
    let temporary = tempfile::tempdir().unwrap();
    let path = temporary.path().join("config.json");
    let mut store = ConfigStore::open_at(path.clone()).unwrap();
    register_discovered(&mut store, vec![temporary.path().to_owned()]).unwrap();
    assert!(!path.exists());
    assert!(store.get_value("ida.instances").is_none());
}

#[test]
fn failed_publication_preserves_instance_and_default_state() {
    let temporary = tempfile::tempdir().unwrap();
    let path = temporary.path().join("config.json");
    let mut store = ConfigStore::open_at(path.clone()).unwrap();
    let installation = installation(temporary.path(), "ida-pro-9.4", "9.4");
    fs::create_dir(&path).unwrap();
    fs::write(path.join("sentinel"), b"retain").unwrap();
    assert!(register_discovered(&mut store, vec![installation]).is_err());
    assert!(store.get_value("ida.instances").is_none());
    assert!(store.get_value("ida.default").is_none());
    assert_eq!(fs::read(path.join("sentinel")).unwrap(), b"retain");
}
