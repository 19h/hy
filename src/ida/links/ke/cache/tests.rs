use std::time::Duration;

use super::*;

fn file(path: &Path, modified: u64) {
    fs::write(path, b"fixture").unwrap();
    fs::File::options()
        .write(true)
        .open(path)
        .unwrap()
        .set_times(fs::FileTimes::new().set_modified(UNIX_EPOCH + Duration::from_secs(modified)))
        .unwrap();
}

#[test]
fn cleanup_removes_expired_files_then_empty_directories_and_keeps_the_boundary() {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir_all(root.path().join("expired/nested")).unwrap();
    file(&root.path().join("expired/nested/file.i64"), 99);
    file(&root.path().join("expired/file.i64.ke.json"), 99);
    file(&root.path().join("boundary.i64"), 100);
    file(&root.path().join("recent.i64"), 101);
    cleanup_before(root.path(), 100.0).unwrap();
    assert!(!root.path().join("expired").exists());
    assert!(root.path().join("boundary.i64").is_file());
    assert!(root.path().join("recent.i64").is_file());
    assert!(root.path().is_dir());
}

#[cfg(unix)]
#[test]
fn expired_file_symlinks_are_unlinked_without_removing_their_targets() {
    use std::os::unix::fs::symlink;
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    file(&outside.path().join("old.i64"), 1);
    symlink(outside.path().join("old.i64"), root.path().join("old.i64")).unwrap();
    symlink(outside.path(), root.path().join("directory")).unwrap();
    cleanup_before(root.path(), 2.0).unwrap();
    assert!(!root.path().join("old.i64").exists());
    assert_eq!(fs::read(outside.path().join("old.i64")).unwrap(), b"fixture");
    assert!(root.path().join("directory").is_symlink());
}

#[test]
fn missing_roots_and_cleanup_errors_are_advisory() {
    let root = tempfile::tempdir().unwrap();
    cleanup(&root.path().join("missing"), &BigInt::from(3)).unwrap();
    file(&root.path().join("not-directory"), 1);
    cleanup(&root.path().join("not-directory"), &BigInt::from(-1)).unwrap();
    assert_eq!(fs::read(root.path().join("not-directory")).unwrap(), b"fixture");
}

#[test]
fn retention_overflow_precedes_cleanup_but_missing_roots_do_not_convert() {
    let root = tempfile::tempdir().unwrap();
    let overflow = BigInt::from(10_u32).pow(400);
    cleanup(&root.path().join("missing"), &overflow).unwrap();
    file(&root.path().join("old.i64"), 1);
    for days in [overflow.clone(), -overflow] {
        assert!(cleanup(root.path(), &days).is_err());
        assert_eq!(fs::read(root.path().join("old.i64")).unwrap(), b"fixture");
    }
}

#[test]
fn retention_cutoff_rounds_after_integer_multiplication_like_cpython() {
    let now = 1_700_000_000.25;
    let mut cases = vec![BigInt::from(0), BigInt::from(1), BigInt::from(3)];
    for exponent in [52, 53, 63, 64, 128, 1000, 1007, 1008, 1023, 1024, 1400] {
        let center = BigInt::from(1_u32) << exponent;
        for offset in -2..=2 {
            let value: BigInt = &center + offset;
            cases.push(value.clone());
            cases.push(-value);
        }
    }
    let observed: Vec<_> =
        cases.iter().map(|days| cutoff(now, days).ok().map(f64::to_bits)).collect();
    use sha2::{Digest, Sha256};
    assert_eq!(
        format!("{:x}", Sha256::digest(serde_json::to_vec(&observed).unwrap())),
        "df5336d11dbc880e6c7856a80ecc4849e45258bc15ccc88ff38f13cf6ba178d6"
    );
    assert_eq!(observed[0], Some(now.to_bits()));
    assert_eq!(observed[1], Some((now - 86_400.0).to_bits()));
    assert!(observed.last().unwrap().is_none());
    if let Some(python) = std::env::var_os("HY_TEST_HTTPX_ORACLE_PYTHON") {
        let output = std::process::Command::new(python)
            .args([
                "-I",
                "-B",
                "-c",
                r#"
import json, struct, sys
values = []
for value in sys.argv[1:]:
    try:
        cutoff = 1700000000.25 - (int(value) * 24 * 60 * 60)
        values.append(struct.unpack('>Q', struct.pack('>d', cutoff))[0])
    except OverflowError:
        values.append(None)
print(json.dumps(values))
"#,
            ])
            .args(cases.iter().map(ToString::to_string))
            .output()
            .unwrap();
        assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
        let expected: Vec<Option<u64>> = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(observed, expected);
    }
}
