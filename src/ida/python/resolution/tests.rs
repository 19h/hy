//! Filesystem-backed selection cases compared with the upstream derivation function.

use std::fs;
use std::io::Write;
use std::process::{Command, Stdio};

use serde_json::{Value, json};

use super::*;

struct Fixture {
    directory: tempfile::TempDir,
    base: PathBuf,
    venv: PathBuf,
    requested: PathBuf,
    standalone: PathBuf,
    store: PathBuf,
}

impl Fixture {
    fn new(profile: usize) -> Self {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        let base = root.join("base");
        let venv = root.join("venv");
        let requested = root.join("requested");
        let standalone = root.join("standalone/Python-custom");
        let store = root.join("Microsoft/WindowsApps/store/bin/python");
        for root in [&base, &venv, &requested] {
            fs::create_dir_all(root).unwrap();
            for (index, path) in layout::candidates(root, Some("3.13")).iter().enumerate() {
                fs::create_dir_all(path.parent().unwrap()).unwrap();
                if profile == 1 && root != &requested || profile == 2 && index == 0 {
                    continue;
                }
                if profile == 3 && index == 0 {
                    fs::create_dir(path).unwrap();
                } else {
                    fs::write(path, "fixture").unwrap();
                }
            }
        }
        for root in [&venv, &requested] {
            fs::write(root.join("pyvenv.cfg"), "").unwrap();
        }
        for path in [&standalone, &store, &directory.path().join("idat")] {
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, "fixture").unwrap();
        }
        fs::write(store.parent().unwrap().parent().unwrap().join("pyvenv.cfg"), "").unwrap();
        Self {
            directory,
            base,
            venv,
            requested,
            standalone,
            store,
        }
    }

    fn cases(&self) -> Vec<Value> {
        let text = |path: &Path| path.to_string_lossy().into_owned();
        let base = text(&self.base);
        let venv = text(&self.venv);
        let requested = text(&self.requested);
        let root = self.directory.path();
        let missing = text(&root.join("missing"));
        let python = |root: &Path| text(&layout::candidates(root, None)[0]);
        let executable_values = [
            None,
            Some(String::new()),
            Some(python(&self.base)),
            Some(python(&self.requested)),
            Some(text(&self.standalone)),
            Some(text(&root.join("idat"))),
            Some(text(&self.store)),
            Some(missing.clone()),
        ];
        let requested_values = [
            None,
            Some(String::new()),
            Some(python(&self.requested)),
            Some(text(&self.standalone)),
            Some(text(&self.store)),
            Some(missing.clone()),
        ];
        let virtual_values =
            [None, Some(String::new()), Some(base.clone()), Some(requested), Some("None".into())];
        let prefix_pairs = [
            (&base, &base),
            (&venv, &base),
            (&base, &venv),
            (&missing, &missing),
            (&String::new(), &base),
            (&String::new(), &String::new()),
        ];
        let mut cases = Vec::new();
        for (prefix, base_prefix) in prefix_pairs {
            for executable in &executable_values {
                for requested in &requested_values {
                    for virtual_env in &virtual_values {
                        for frozen in [false, true] {
                            let info = Probe {
                                frozen,
                                prefix: prefix.clone(),
                                base_prefix: base_prefix.clone(),
                                executable: executable.clone(),
                                virtual_env: virtual_env.clone(),
                                idapython_venv_executable: requested.clone(),
                                version: "3.13".into(),
                                externally_managed: false,
                            };
                            let expected = match derive(&info, "fixture-hy") {
                                Ok(path) => json!({"path": path}),
                                Err(error) => json!({"error": error.to_string()}),
                            };
                            if frozen {
                                assert_eq!(
                                    expected,
                                    json!({"error": "IDA is running as a frozen application, cannot detect Python executable"})
                                );
                            }
                            cases.push(json!({"info": info, "expected": expected}));
                        }
                    }
                }
            }
        }
        cases
    }
}

#[test]
fn derivation_matches_source_prefix_venv_executable_and_frozen_precedence() {
    let mut fixtures = Vec::new();
    let mut cases = Vec::new();
    for profile in 0..4 {
        let fixture = Fixture::new(profile);
        cases.extend(fixture.cases());
        fixtures.push(fixture);
    }
    assert_eq!(cases.len(), 11_520);
    #[cfg(unix)]
    {
        use sha2::{Digest, Sha256};
        let expected: Vec<_> = cases.iter().map(|case| &case["expected"]).collect();
        let mut normalized = serde_json::to_string(&expected).unwrap();
        for (index, fixture) in fixtures.iter().enumerate() {
            normalized = normalized
                .replace(fixture.directory.path().to_str().unwrap(), &format!("/fixture/{index}"));
        }
        assert_eq!(
            format!("{:x}", Sha256::digest(normalized.as_bytes())),
            "9136a3481267e720dd39e0dcdf780d2848ecc633edc509db8171ec003c50be53",
        );
    }
    compare(&cases);
}

fn compare(cases: &[Value]) {
    let Some(python) = std::env::var_os("HY_TEST_VENV_ORACLE_PYTHON") else {
        return;
    };
    let source = std::env::var_os("HY_TEST_HCLI_SOURCE").map(PathBuf::from).unwrap_or_else(|| {
        Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("ida-hcli")
    });
    let mut child = Command::new(python)
        .args(["-I", "-B", "-c", include_str!("tests/reference.py")])
        .arg(source)
        .arg(super::super::PROBE)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(&serde_json::to_vec(cases).unwrap()).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success(), "{output:?}");
    let actual: Vec<Value> = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(actual.len(), cases.len());
    for (actual, case) in actual.iter().zip(cases) {
        assert_eq!(*actual, case["expected"], "{}", case["info"]);
    }
}
