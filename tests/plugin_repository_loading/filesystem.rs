use super::*;

#[test]
fn files_precede_subdirectories_and_keep_lexical_urls() {
    let sandbox = Sandbox::new();
    let root = sandbox.path().join("repository");
    fs::create_dir_all(root.join("a/nested")).unwrap();
    fs::write(root.join("a/nested/first.zip"), package("example", "1.0")).unwrap();
    fs::write(root.join("z.zip"), package("EXAMPLE", "1")).unwrap();
    fs::write(root.join("ignored.ZIP"), b"not ZIP").unwrap();
    fs::write(root.join("ignored.txt"), b"not ZIP").unwrap();
    let result = snapshot(&sandbox, &root, "filesystem");
    assert_eq!(result["success"], true);
    assert_eq!(result["snapshot"]["plugins"][0]["name"], "example");
    assert_eq!(result["snapshot"]["plugins"].as_array().unwrap().len(), 1);
}

#[test]
fn invalid_file_references_stop_one_archive_but_invalid_descriptors_are_skipped() {
    for invalid_first in [false, true] {
        let sandbox = Sandbox::new();
        let root = sandbox.path().join("repository");
        fs::create_dir(&root).unwrap();
        let first = descriptor("first", "1");
        let invalid = descriptor("invalid", "1");
        let last = descriptor("last", "1");
        let mut members = vec![("invalid/ida-plugin.json", invalid.as_slice())];
        if !invalid_first {
            members.insert(0, ("first/ida-plugin.json", first.as_slice()));
        }
        members.extend([
            ("first/plugin.py", b"# first".as_slice()),
            ("last/ida-plugin.json", last.as_slice()),
            ("last/plugin.py", b"# last".as_slice()),
        ]);
        fs::write(root.join("stopped.zip"), zip(&members)).unwrap();
        fs::write(
            root.join("independent.zip"),
            zip(&[
                ("broken/ida-plugin.json", b"not JSON"),
                ("ida-plugin.json", &descriptor("independent", "1")),
                ("plugin.py", b"# independent"),
            ]),
        )
        .unwrap();
        let result = snapshot(&sandbox, &root, "filesystem");
        assert_eq!(result["success"], true);
        let names: Vec<_> = result["snapshot"]["plugins"]
            .as_array()
            .unwrap()
            .iter()
            .map(|plugin| plugin["name"].as_str().unwrap())
            .collect();
        assert_eq!(
            names,
            if invalid_first {
                vec!["independent"]
            } else {
                vec!["first", "independent"]
            }
        );
    }
}

#[test]
fn malformed_archives_fail_the_repository_instead_of_disappearing() {
    let sandbox = Sandbox::new();
    let root = sandbox.path().join("repository");
    fs::create_dir(&root).unwrap();
    fs::write(root.join("valid.zip"), package("example", "1")).unwrap();
    fs::write(root.join("broken.zip"), b"not ZIP").unwrap();
    assert_eq!(snapshot(&sandbox, &root, "filesystem")["success"], false);
}

#[cfg(unix)]
#[test]
fn file_symlinks_keep_alias_urls_and_directory_symlinks_are_not_walked() {
    use std::os::unix::fs::symlink;

    let sandbox = Sandbox::new();
    let root = sandbox.path().join("repository");
    let outside = sandbox.path().join("outside");
    fs::create_dir(&root).unwrap();
    fs::create_dir(&outside).unwrap();
    fs::write(outside.join("source.zip"), package("example", "1")).unwrap();
    symlink(outside.join("source.zip"), root.join("alias.zip")).unwrap();
    symlink(&outside, root.join("linked-directory")).unwrap();
    let result = snapshot(&sandbox, &root, "filesystem");
    assert_eq!(result["success"], true);
    let locations = result["snapshot"]["plugins"][0]["versions"]["1"].as_array().unwrap();
    assert_eq!(locations.len(), 1);
    assert!(locations[0]["url"].as_str().unwrap().ends_with("/repository/alias.zip"));
    symlink(outside.join("missing"), root.join("broken.zip")).unwrap();
    assert_eq!(snapshot(&sandbox, &root, "filesystem")["success"], false);
}
