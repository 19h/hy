use std::io::{Cursor, Write};
use std::process::{Command, Stdio};

use base64::Engine;
use base64::prelude::BASE64_STANDARD;
use serde_json::{Value, json};
#[cfg(not(windows))]
use sha2::{Digest, Sha256};

use super::fixtures::{Member, central_offset, zip};
use super::*;

mod cases;

fn error_category(error: ZipError) -> &'static str {
    match error {
        ZipError::InvalidArchive(_) | ZipError::FileNotFound => "badzip",
        ZipError::UnsupportedArchive(_) | ZipError::CompressionMethodNotSupported(_) => {
            "unsupported"
        }
        ZipError::Io(error) if error.to_string() == "Invalid checksum" => "badzip",
        ZipError::Io(error)
            if error.get_ref().is_some_and(|inner| inner.is::<std::string::FromUtf8Error>()) =>
        {
            "unicode"
        }
        _ => "other",
    }
}

fn observe(bytes: &[u8]) -> Value {
    let mut archive = match Archive::new(Cursor::new(bytes)) {
        Ok(archive) => archive,
        Err(error) => return json!({"error": error_category(error)}),
    };
    let names: Vec<_> = archive.file_names().map(str::to_owned).collect();
    let reads: Vec<_> = names
        .iter()
        .map(|name| match archive.read(name) {
            Ok(bytes) => json!({"bytes": BASE64_STANDARD.encode(bytes)}),
            Err(error) => json!({"error": error_category(error)}),
        })
        .collect();
    json!({"names": names, "reads": reads})
}

#[test]
fn named_reads_match_python_directory_order_encoding_and_corruption() {
    let cases = cases::all();
    let expected: Vec<_> = cases.iter().map(|(_, bytes)| observe(bytes)).collect();
    if let Some(python) = std::env::var_os("HY_TEST_VENV_ORACLE_PYTHON") {
        let encoded: Vec<_> =
            cases.iter().map(|(_, bytes)| BASE64_STANDARD.encode(bytes)).collect();
        let mut child = Command::new(python)
            .args(["-I", "-B", "-c", include_str!("reference.py")])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        // Feed through a separate thread: the oracle can emit more than one pipe buffer.
        let mut stdin = child.stdin.take().unwrap();
        let input = serde_json::to_vec(&encoded).unwrap();
        let writer = std::thread::spawn(move || stdin.write_all(&input).unwrap());
        let output = child.wait_with_output().unwrap();
        writer.join().unwrap();
        assert!(output.status.success());
        let actual: Vec<Value> = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(actual.len(), expected.len());
        for (index, (actual, expected)) in actual.iter().zip(&expected).enumerate() {
            assert_eq!(actual, expected, "{}", cases[index].0);
        }
    }
    assert_eq!(cases.len(), 1025);
    // Windows also sanitizes backslashes; its names are compared by the runtime oracle.
    #[cfg(not(windows))]
    assert_eq!(
        format!("{:x}", Sha256::digest(serde_json::to_vec(&expected).unwrap())),
        "26895d011a0359fb13444ace7a75331cb312752db387a745cc0ffb6720781cca",
    );
}

#[test]
fn repeated_names_use_last_data_without_losing_distinct_encoded_names() {
    let bytes = zip(&[
        Member::new(b"same", b"first"),
        Member::new(b"\xc3\xa9", b"CP437"),
        Member::new(b"same", b"last"),
        Member::new(b"\xc3\xa9", b"UTF8").utf8(),
    ]);
    let mut archive = Archive::new(Cursor::new(bytes)).unwrap();
    assert_eq!(archive.file_names().collect::<Vec<_>>(), ["same", "├⌐", "same", "é"]);
    assert_eq!(archive.read("same").unwrap(), b"last");
    assert_eq!(archive.read("├⌐").unwrap(), b"CP437");
    assert_eq!(archive.read("é").unwrap(), b"UTF8");
    assert!(matches!(archive.read("missing"), Err(ZipError::FileNotFound)));
}
