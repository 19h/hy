use std::fs;
use std::io::Write;
use std::process::{Command, Stdio};

use super::*;

fn label(source: Result<Source>) -> &'static str {
    match source {
        Ok(Source::Directory(_)) => "directory",
        Ok(Source::Archive(_)) => "archive",
        Ok(Source::GitHub) => "github",
        Ok(Source::Download) => "download",
        Ok(Source::Repository) => "repository",
        Err(_) => "error",
    }
}

#[test]
fn acquisition_branches_match_source_paths_urls_and_editable_inputs() {
    let temporary = tempfile::tempdir().unwrap();
    let mut values = Vec::new();
    for name in ["plain", "lower.zip", "upper.ZIP", "archive.zip/", "terminal.zip\n"] {
        for kind in ["missing", "file", "directory", "plugin", "descriptor-directory"] {
            let parent = temporary.path().join(kind);
            fs::create_dir_all(&parent).unwrap();
            let path = parent.join(name);
            // A trailing slash is tested as a spelling of an existing directory.
            if name.ends_with('/') && kind == "file" {
                continue;
            }
            match kind {
                "missing" => {}
                "file" => fs::write(&path, b"fixture").unwrap(),
                _ => {
                    fs::create_dir_all(&path).unwrap();
                    match kind {
                        "plugin" => fs::write(path.join("ida-plugin.json"), b"{}").unwrap(),
                        "descriptor-directory" => {
                            fs::create_dir(path.join("ida-plugin.json")).unwrap()
                        }
                        _ => {}
                    }
                }
            }
            values.push(path.to_str().unwrap().to_owned());
        }
    }
    for scheme in ["https", "HTTPS", "http", "file", "httpſ"] {
        for host in ["github.com", "GitHub.Com", "gıthub.com", "example.test"] {
            for path in [
                "owner/repo",
                "owner/repo.git/",
                "owner/repo@v1/a+tag",
                "owner/repo@/",
                "owner/repo/archive/main.zip",
                "owner/repo/releases/download/v1/a.zip",
                "owner/repo?query",
                "owner/repo#fragment",
                "owner/İſKı",
                "owner/..",
                "owner/repo.git.git",
            ] {
                for suffix in ["", "\n", "\r\n", "/"] {
                    values.push(format!("{scheme}://{host}/{path}{suffix}"));
                }
            }
        }
    }
    values.extend(
        [
            "",
            "example",
            "example==1",
            "file:/missing",
            "file:///missing",
            "~/missing.zip",
            "https://example.test/file\0.zip",
        ]
        .map(str::to_owned),
    );
    let inputs: Vec<_> =
        values.iter().flat_map(|value| [false, true].map(|editable| (value, editable))).collect();
    assert_eq!(inputs.len(), 1_822);
    let expected: Vec<_> =
        inputs.iter().map(|(value, editable)| label(classify(value, *editable))).collect();
    assert!(expected.contains(&"archive"));
    assert!(expected.contains(&"directory"));
    assert!(expected.contains(&"github"));
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
    child.stdin.take().unwrap().write_all(&serde_json::to_vec(&inputs).unwrap()).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success(), "source oracle failed");
    let actual: Vec<String> = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(actual.len(), expected.len());
    for ((actual, expected), input) in actual.iter().zip(expected).zip(inputs) {
        assert_eq!(actual, expected, "{input:?}");
    }
}
