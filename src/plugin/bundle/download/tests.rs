use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use super::{PipOptions, PipTarget};

#[test]
fn download_arguments_match_upstream_for_all_target_platforms_and_pip_flags() {
    let mut cases = Vec::new();
    for platform in crate::plugin::bundle::ALL_PLATFORMS {
        for version in ["3.10", "3.14"] {
            let target = PipTarget::new(platform, version).unwrap();
            for mask in 0u32..32 {
                for index in [None, Some(""), Some("https://index/simple")] {
                    for sources in [false, true] {
                        let options = PipOptions {
                            index_url: index.map(str::to_owned),
                            extra_index_urls: if sources {
                                vec!["".into(), "https://extra/simple".into()]
                            } else {
                                vec![]
                            },
                            find_links: if sources {
                                vec!["wheels with spaces".into(), "https://wheels/".into()]
                            } else {
                                vec![]
                            },
                            no_index: mask & 1 != 0,
                            isolated: mask & 2 != 0,
                            no_cache_dir: mask & 4 != 0,
                            disable_pip_version_check: mask & 8 != 0,
                            no_build_isolation: mask & 16 != 0,
                            skip_environment_check: true,
                        };
                        let dependencies: Vec<String> =
                            vec!["fixture[extra]>=1; python_version > '3'".into(), "--pre".into()];
                        let command = super::command(
                            Path::new("fixture-python"),
                            &dependencies,
                            &target,
                            Path::new("wheelhouse"),
                            &options,
                        )
                        .unwrap();
                        let command = command.as_std();
                        assert_eq!(command.get_envs().count(), 0);
                        assert!(command.get_current_dir().is_none());
                        let argv: Vec<_> = std::iter::once(command.get_program())
                            .chain(command.get_args())
                            .map(|value| value.to_str().unwrap())
                            .collect();
                        cases.push(json!({
                            "mode": "plan", "platform": platform, "version": version,
                            "options": options, "dependencies": dependencies, "expected": argv,
                        }));
                    }
                }
            }
        }
    }
    assert_eq!(cases.len(), 2304);
    compare(&cases);
    digest(&cases, "2248c6de1a79faaf1df8e5bc66d56fe1b859837d45e906a8859858ccf9ae1eba");
}

#[test]
fn download_errors_preserve_unstripped_decoded_streams() {
    let streams = [
        b"".as_slice(),
        b" \r\n\t",
        b"ordinary\n",
        b"\xff\xe2\x82tail",
        "\u{85}日本語\u{1c}".as_bytes(),
        b"externally-managed-environment\0",
    ];
    let target = PipTarget::new("windows-x86_64", "3.12").unwrap();
    let mut cases = Vec::new();
    for stdout in streams {
        for stderr in streams {
            cases.push(json!({
                "mode": "error", "platform": target.ida_platform, "version": target.python_version,
                "stdout": stdout, "stderr": stderr,
                "expected": super::failure(&target, stdout, stderr),
            }));
        }
    }
    assert_eq!(cases.len(), 36);
    compare(&cases);
    digest(&cases, "627ce806f2b83ee9be01aae0d53b84b65c86b02707b71825c0d0d6d0b17b939a");
}

fn digest(cases: &[Value], expected: &str) {
    let values: Vec<_> = cases.iter().map(|case| &case["expected"]).collect();
    let digest = format!("{:x}", Sha256::digest(serde_json::to_vec(&values).unwrap()));
    assert_eq!(digest, expected);
}

fn compare(cases: &[Value]) {
    let Some(python) = std::env::var_os("HY_TEST_VENV_ORACLE_PYTHON") else {
        return;
    };
    let source =
        std::env::var_os("HY_TEST_HCLI_SOURCE").map(std::path::PathBuf::from).unwrap_or_else(
            || Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("ida-hcli"),
        );
    let mut child = Command::new(python)
        .args(["-I", "-B", "-c", include_str!("reference.py")])
        .arg(source)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(&serde_json::to_vec(cases).unwrap()).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    let actual: Vec<Value> = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(actual.len(), cases.len());
    for (actual, case) in actual.iter().zip(cases) {
        assert_eq!(*actual, case["expected"], "{case}");
    }
}
