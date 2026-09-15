use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde_json::{Value, json};

use super::*;

fn compare_source(operation: &str, values: &[String], expected: &Value) {
    let Some(python) = std::env::var_os("HY_TEST_BUNDLE_ORACLE_PYTHON") else {
        return;
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
    child
        .stdin
        .take()
        .unwrap()
        .write_all(&serde_json::to_vec(&json!({"operation": operation, "values": values})).unwrap())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success(), "source oracle failed");
    let actual: Value = serde_json::from_slice(&output.stdout).unwrap();
    let actual = actual.as_array().unwrap();
    let expected = expected.as_array().unwrap();
    assert_eq!(actual.len(), expected.len());
    for (index, (actual, expected)) in actual.iter().zip(expected).enumerate() {
        assert_eq!(actual, expected, "case {index}: {:?}", values.get(index));
    }
}

#[test]
fn file_url_paths_match_posix_and_windows_conversion() {
    let mut values = Vec::new();
    for scheme in ["file:", "FILE:", "\0 \tfile:", "fi\nle:", "https:"] {
        for authority in [
            "",
            "//",
            "//localhost",
            "//LOCALHOST",
            "//elsewhere",
            "//user:pass@host:bad",
            "//[::1]",
            "//[v1.future]",
            "//[Vf.future]",
            "//[::1%scope]",
            "//[::1%]",
            "//[127.0.0.1]",
            "//[::1]junk",
            "//[::1",
            "//a[b]",
            "//user[x]@v1.future",
            "//host\u{2100}name",
            "//host\u{ff1a}name",
            "//λ.invalid",
        ] {
            for path in [
                "",
                "/",
                "//",
                "///",
                "////",
                "/a/b",
                "/a/./b//",
                "/a/../b",
                "/file/.",
                "/a;b",
                "/a?ignored#fragment",
                "/a%3Fb%23c",
                "/space%20+name ",
                "/%ff/%c3%a9",
                "/%ed%a0%80",
                "/%00",
                "/%zz",
                "/%2f%2fhost/%2fleaf",
                "/C:/a/b",
                "/C|/a/b",
                "/|/bad",
                "/1|/bad",
                "/C|a|b",
                "relative",
                "/a%5Cb",
                "//localhost/a",
                "//LOCALHOST/a",
                "////C:/a",
            ] {
                values.push(format!("{scheme}{authority}{path}"));
            }
        }
    }
    // Cover every octet and representative UTF-8 lead/continuation pairs without
    // requiring filenames that this filesystem can represent.
    for first in 0..=255 {
        values.push(format!("file:///a%{first:02X}b"));
        for second in [0, 0x2f, 0x41, 0x80, 0xa0, 0xbf, 0xc0, 0xed, 0xff] {
            values.push(format!("file:///a%{first:02X}%{second:02X}b"));
        }
    }
    for character in char::from_u32(0x80).unwrap()..=char::MAX {
        if authority::nfkc_delimiter(character) {
            for combining in ["", "\u{300}", "\u{338}", "\u{fe00}", "\u{200d}"] {
                values.push(format!("file://a{character}{combining}b/path"));
            }
        }
    }
    assert_eq!(values.len(), 5_315);
    let expected: Vec<_> = values
        .iter()
        .map(|value| match raw_path(value) {
            Ok(Some(path)) => {
                json!({"posix": posix_path(&path), "windows": windows_path(&path).ok()})
            }
            _ => Value::Null,
        })
        .collect();
    compare_source("paths", &values, &json!(expected));
    assert_eq!(posix_path("a/./b//"), b"a/b");
    assert_eq!(posix_path("a/../b"), b"a/../b");
    assert_eq!(posix_path("/%ff"), b"/\xff");
}

#[test]
fn nfkc_delimiters_match_the_complete_source_unicode_table() {
    let native: Vec<_> = (128..=0x10ffff)
        .filter_map(char::from_u32)
        .filter(|character| authority::nfkc_delimiter(*character))
        .map(u32::from)
        .collect();
    assert_eq!(native.len(), 19);
    compare_source("delimiters", &[], &json!(native));
}

#[cfg(unix)]
#[tokio::test]
async fn archive_reads_preserve_file_url_bytes_and_lexical_path_rules() {
    use std::fs;
    use std::os::unix::ffi::OsStringExt;
    use std::os::unix::fs::symlink;

    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path();
    fs::write(root.join("file"), b"root payload").unwrap();
    fs::write(root.join("with space+"), b"space payload").unwrap();
    fs::write(root.join("with;params"), b"semicolon payload").unwrap();
    let raw = root.join(std::ffi::OsString::from_vec(b"raw-\xff".to_vec()));
    if let Err(error) = fs::write(raw, b"raw payload") {
        assert_eq!(error.raw_os_error(), Some(libc::EILSEQ), "unexpected fixture failure");
        eprintln!("filesystem rejects non-UTF-8 filenames; compare the read failure instead");
    }
    fs::create_dir_all(root.join("nested/inside")).unwrap();
    fs::write(root.join("nested/file"), b"nested payload").unwrap();
    symlink(root.join("nested/inside"), root.join("link")).unwrap();
    let mut values = Vec::new();
    for authority in ["", "localhost", "elsewhere", "user:secret@host:bad", "[::1]"] {
        for path in [
            "file",
            "file/.",
            "file/",
            "with%20space+",
            "with;params",
            "raw-%ff",
            "link/../file",
            "missing",
            "%00",
        ] {
            values.push(format!("file://{authority}{}/{path}", root.display()));
        }
    }
    let mut expected = Vec::new();
    assert_eq!(values.len(), 45);
    for value in &values {
        expected.push(json!(crate::plugin::index::fetch(value).await.ok()));
    }
    assert_eq!(expected[0], json!(b"root payload"));
    assert_eq!(expected[6], json!(b"nested payload"));
    compare_source("read", &values, &json!(expected));
}
