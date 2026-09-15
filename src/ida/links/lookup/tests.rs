//! Filesystem lookup and legacy-source migration against isolated configuration.

use std::fs;

use super::*;

#[test]
fn lookup_requires_the_requested_filename_and_supports_globs() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path();
    fs::create_dir(root.join("nested")).unwrap();
    let file = root.join("nested/sample.i64");
    fs::write(&file, b"database").unwrap();
    assert_eq!(find_in_directory(root, "sample.i64"), Some(file.clone()));
    assert_eq!(find_in_directory(root, "*.i64"), Some(file.clone()));
    assert_eq!(find_in_directory(root, "s[ae]mple.i64"), Some(file));
    assert_eq!(find_in_directory(root, "sample"), None);
    assert_eq!(find_in_directory(root, "sample.idb"), None);
    assert_eq!(find_in_directory(&root.join("missing"), "*.i64"), None);
}

#[test]
fn encoded_filenames_are_literal_and_root_matches_precede_descendants() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path();
    fs::create_dir(root.join("nested")).unwrap();
    for name in ["a%20b.i64", "a b.i64", "nested/a%20b.i64"] {
        fs::write(root.join(name), b"database").unwrap();
    }
    assert_eq!(find_in_directory(root, "a%20b.i64"), Some(root.join("a%20b.i64")));
    assert_eq!(find_in_directory(root, "a b.i64"), Some(root.join("a b.i64")));
}

#[cfg(unix)]
#[test]
fn file_symlinks_are_eligible_but_directory_symlinks_are_not_traversed() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().join("source");
    let outside = temporary.path().join("outside");
    fs::create_dir(&root).unwrap();
    fs::create_dir(&outside).unwrap();
    fs::write(outside.join("hidden.i64"), b"database").unwrap();
    std::os::unix::fs::symlink(&outside, root.join("directory-link")).unwrap();
    assert_eq!(find_in_directory(&root, "hidden.i64"), None);
    std::os::unix::fs::symlink(outside.join("hidden.i64"), root.join("alias.i64")).unwrap();
    assert_eq!(find_in_directory(&root, "alias.i64"), Some(root.join("alias.i64")));
    assert_eq!(find_in_directory(&root, "*.i64"), Some(root.join("alias.i64")));
}

#[test]
fn legacy_paths_migrate_atomically_in_order_only_when_sources_are_empty() {
    let temporary = tempfile::tempdir().unwrap();
    let path = temporary.path().join("config.json");
    fs::write(&path, br#"{"idb.search-paths":["/first","/second"],"sentinel":"retain"}"#).unwrap();
    let mut store = ConfigStore::open_at(path.clone()).unwrap();
    let migrated = sources(&mut store).unwrap();
    assert_eq!(migrated.keys().map(String::as_str).collect::<Vec<_>>(), ["source-1", "source-2"]);
    assert_eq!(migrated["source-1"], "/first");
    assert_eq!(migrated["source-2"], "/second");
    let saved: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    assert!(saved.get("idb.search-paths").is_none());
    assert_eq!(saved["sentinel"], "retain");
    let before = fs::read(&path).unwrap();
    assert_eq!(sources(&mut store).unwrap(), migrated);
    assert_eq!(fs::read(&path).unwrap(), before);

    fs::write(&path, br#"{"idb.sources":{"existing":"/current"},"idb.search-paths":["/legacy"]}"#)
        .unwrap();
    let before = fs::read(&path).unwrap();
    let mut store = ConfigStore::open_at(path.clone()).unwrap();
    assert_eq!(sources(&mut store).unwrap()["existing"], "/current");
    assert_eq!(fs::read(path).unwrap(), before);
}

#[test]
fn failed_legacy_migration_preserves_in_memory_paths() {
    let temporary = tempfile::tempdir().unwrap();
    let path = temporary.path().join("config.json");
    fs::write(&path, br#"{"idb.search-paths":["/original"]}"#).unwrap();
    let mut store = ConfigStore::open_at(path.clone()).unwrap();
    fs::remove_file(&path).unwrap();
    fs::create_dir(&path).unwrap();
    fs::write(path.join("sentinel"), b"retain").unwrap();
    assert!(sources(&mut store).is_err());
    assert_eq!(store.get_value("idb.search-paths"), Some(&json!(["/original"])));
    assert!(store.get_value("idb.sources").is_none());
    assert_eq!(fs::read(path.join("sentinel")).unwrap(), b"retain");
}
