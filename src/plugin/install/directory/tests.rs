//! Rust owns fixture mutation; the Python source oracle only reads these trees.

use std::fs::{File, FileTimes, Permissions};
use std::io::{Read, Write};
use std::os::unix::fs::{PermissionsExt, symlink};
use std::os::unix::net::UnixListener;
use std::process::{Command, Stdio};
use std::time::Duration;

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use super::*;

fn write(root: &Path, name: &str) {
    let path = root.join(name);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, format!("fixture: {name}\n")).unwrap();
    File::open(&path)
        .unwrap()
        .set_times(FileTimes::new().set_modified(UNIX_EPOCH + Duration::from_secs(1_700_000_000)))
        .unwrap();
    fs::set_permissions(path, Permissions::from_mode(0o644)).unwrap();
}

fn inventory(bytes: Vec<u8>) -> Value {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let members: Vec<_> = (0..archive.len())
        .map(|index| {
            let mut member = archive.by_index(index).unwrap();
            let mut bytes = Vec::new();
            member.read_to_end(&mut bytes).unwrap();
            assert_eq!(member.compression(), zip::CompressionMethod::Deflated);
            let date = member.last_modified().unwrap();
            json!({
                "name": member.name(),
                "size": member.size(),
                "sha256": format!("{:x}", Sha256::digest(bytes)),
                "date": [date.year(), date.month(), date.day(), date.hour(), date.minute(), date.second()],
                "permissions": member.unix_mode().unwrap() & 0o777,
                "compression": 8,
            })
        })
        .collect();
    json!(members)
}

fn compare_source(directories: &[PathBuf]) -> Vec<Value> {
    let expected: Vec<_> =
        directories.iter().map(|path| pack(path).map(inventory).unwrap_or(Value::Null)).collect();
    let Some(python) = std::env::var_os("HY_TEST_BUNDLE_ORACLE_PYTHON") else {
        return expected;
    };
    let source = std::env::var_os("HY_TEST_HCLI_SOURCE").map(PathBuf::from).unwrap_or_else(|| {
        Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("ida-hcli")
    });
    let mut child = Command::new(python)
        .args(["-I", "-B", "-c", include_str!("reference.py")])
        .arg(source)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(&serde_json::to_vec(directories).unwrap()).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success(), "source oracle failed");
    let actual: Vec<Value> = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(actual.len(), expected.len());
    for ((actual, expected), path) in actual.iter().zip(&expected).zip(directories) {
        assert_eq!(actual, expected, "{}", path.display());
    }
    expected
}

#[test]
fn packing_matches_source_members_filters_links_and_timestamps() {
    let temporary = tempfile::tempdir().unwrap();
    let tree = temporary.path().join("tree");
    fs::create_dir(&tree).unwrap();
    for name in [
        "z",
        "a/file",
        "a/deep/item",
        "a-foo",
        "a.foo",
        "B",
        "é",
        "日本語",
        "C:literal",
        "with\\slash",
        ".venv/data",
        "venv/data",
        ".idea/data",
        ".git/config",
        ".hg/data",
        ".svn/data",
        "__pycache__/data",
        ".DS_Store",
        "a/.git/data",
        "a/.DS_Store",
        "a/.venv/data",
    ] {
        write(&tree, name);
    }
    fs::create_dir(tree.join("empty")).unwrap();
    fs::set_permissions(tree.join("z"), Permissions::from_mode(0o4755)).unwrap();
    symlink(tree.join("z"), tree.join("file-link")).unwrap();
    symlink(tree.join("a"), tree.join("directory-link")).unwrap();
    symlink(tree.join("absent"), tree.join(".git/dangling")).unwrap();
    let mut directories = vec![tree];
    for (index, seconds, nanoseconds) in [
        (0, 1_700_000_000, 0),
        (1, 1_700_000_001, 0),
        (2, 1_700_000_001, 999_999_999),
        (3, 1, 0),
        (4, 4_500_000_000, 0),
    ] {
        let directory = temporary.path().join(format!("timestamp-{index}"));
        write(&directory, "file");
        File::open(directory.join("file"))
            .unwrap()
            .set_times(
                FileTimes::new().set_modified(UNIX_EPOCH + Duration::new(seconds, nanoseconds)),
            )
            .unwrap();
        directories.push(directory);
    }
    for (name, target) in [("dangling", "missing"), ("loop", "link")] {
        let directory = temporary.path().join(name);
        write(&directory, "file");
        symlink(directory.join(target), directory.join("link")).unwrap();
        directories.push(directory);
    }
    let socket = temporary.path().join("socket");
    fs::create_dir(&socket).unwrap();
    let _listener = UnixListener::bind(socket.join("file")).unwrap();
    directories.push(socket);
    let empty = temporary.path().join("empty");
    fs::create_dir(&empty).unwrap();
    directories.push(empty);
    let reports = compare_source(&directories);
    assert_eq!(reports.iter().filter(|report| !report.is_null()).count(), 5);
    let names: Vec<_> = reports[0]
        .as_array()
        .unwrap()
        .iter()
        .map(|member| member["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        names,
        [
            ".idea/data",
            ".venv/data",
            "B",
            "C:literal",
            "a/.venv/data",
            "a/deep/item",
            "a/file",
            "a-foo",
            "a.foo",
            "file-link",
            "venv/data",
            "with\\slash",
            "z",
            "é",
            "日本語"
        ]
    );
}

#[test]
fn unreadable_directories_are_skipped_but_unreadable_files_fail() {
    struct Restore(PathBuf);
    impl Drop for Restore {
        fn drop(&mut self) {
            let _ = fs::set_permissions(&self.0, Permissions::from_mode(0o700));
        }
    }
    let temporary = tempfile::tempdir().unwrap();
    let skipped = temporary.path().join("skipped");
    write(&skipped, "blocked/file");
    let terminal = temporary.path().join("terminal");
    write(&terminal, "file");
    let blocked = [skipped.join("blocked"), terminal.join("file")];
    let _restore = blocked.each_ref().map(|path| Restore(path.clone()));
    for path in &blocked {
        fs::set_permissions(path, Permissions::from_mode(0o000)).unwrap();
    }
    if fs::read_dir(&blocked[0]).is_ok() || File::open(&blocked[1]).is_ok() {
        eprintln!("permission probe unavailable: process can read mode-zero fixture");
        return;
    }
    assert_eq!(compare_source(&[skipped, terminal]), [json!([]), Value::Null]);
}
