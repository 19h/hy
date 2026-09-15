use super::*;

#[test]
fn file_urls_select_directory_bundle_and_snapshot_repositories_through_source_paths() {
    let sandbox = Sandbox::new();
    let root = sandbox.path();
    fs::create_dir_all(root.join("nested/inside")).unwrap();
    std::os::unix::fs::symlink(root.join("nested/inside"), root.join("link")).unwrap();
    let package = package("example", "2");
    fs::create_dir(root.join("nested/directory")).unwrap();
    fs::write(root.join("nested/directory/plugin.zip"), &package).unwrap();
    fs::write(root.join("nested/bundle.zip"), bundle(&[("plugins/example.zip", &package)]))
        .unwrap();
    fs::write(root.join("nested/snapshot.json"), br#"{"version":1,"plugins":[]}"#).unwrap();
    for name in ["directory", "bundle.zip", "snapshot.json"] {
        for prefix in ["file://elsewhere", "FILE://user:secret@host:bad"] {
            let url = format!("{prefix}{}/link/../{name}", root.display());
            let report = snapshot(&sandbox, Path::new(&url), "file-url");
            assert_eq!(report["success"], true, "{url}");
            if name != "snapshot.json" {
                assert_eq!(report["snapshot"]["plugins"][0]["name"], "example");
            }
        }
    }
}
