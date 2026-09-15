use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};

use super::{unix, validate};

fn assert_resolves(path: &Path, expected: &Path) {
    assert_eq!(unix::resolve(path).unwrap(), expected, "{}", path.display());
    if let Some(python) = std::env::var_os("HY_TEST_PATH_ORACLE_PYTHON") {
        const RESOLVE_PATH: &str = concat!(
            "import os, sys; from pathlib import Path; ",
            "sys.stdout.buffer.write(os.fsencode(Path(sys.argv[1]).resolve()))",
        );
        let output = std::process::Command::new(python)
            .args(["-I", "-B", "-c", RESOLVE_PATH])
            .arg(path)
            .output()
            .unwrap();
        assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
        assert_eq!(output.stdout, expected.as_os_str().as_bytes(), "CPython: {}", path.display());
    }
}

#[test]
fn missing_tails_and_repeated_relative_links_resolve_inside_the_root() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().canonicalize().unwrap();
    fs::create_dir(root.join("real")).unwrap();
    symlink("real", root.join("alias")).unwrap();
    for path in [
        root.join("alias/new/file.i64"),
        root.join("alias/../alias/new/file.i64"),
        root.join("missing/../alias/new/file.i64"),
    ] {
        assert_resolves(&path, &root.join("real/new/file.i64"));
        validate(&root, &path).unwrap();
    }
}

#[test]
fn dangling_links_are_checked_by_their_resolved_target() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().canonicalize().unwrap();
    let outside = root.with_extension("outside");
    symlink("missing/deeper", root.join("inside")).unwrap();
    symlink(&outside, root.join("escape")).unwrap();
    assert_resolves(&root.join("inside/file.i64"), &root.join("missing/deeper/file.i64"));
    assert_resolves(&root.join("escape/file.i64"), &outside.join("file.i64"));
    validate(&root, &root.join("inside/file.i64")).unwrap();
    assert!(validate(&root, &root.join("escape/file.i64")).is_err());
    assert!(!root.join("missing").exists());
    assert!(!outside.exists());
}

#[test]
fn non_strict_link_cycles_match_the_python_path_result() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().canonicalize().unwrap();
    symlink("b", root.join("a")).unwrap();
    symlink("a", root.join("b")).unwrap();
    assert_resolves(&root.join("a/file.i64"), &root.join("a/file.i64"));
}

#[test]
fn long_link_chains_do_not_introduce_an_arbitrary_hop_limit() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().canonicalize().unwrap();
    fs::create_dir(root.join("real")).unwrap();
    for index in 0..64 {
        let target = if index == 63 {
            PathBuf::from("real")
        } else {
            PathBuf::from(format!("link-{}", index + 1))
        };
        symlink(target, root.join(format!("link-{index}"))).unwrap();
    }
    assert_resolves(&root.join("link-0/file.i64"), &root.join("real/file.i64"));
}
