use std::path::Path;

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use super::{Operation, PipOptions};

mod reference;

#[test]
fn pip_argument_plans_match_upstream_defaults_sources_and_bundle_merging() {
    let mut cases = Vec::new();
    for operation in [Operation::Resolve, Operation::Install] {
        for mask in 0u32..32 {
            for index in [None, Some(""), Some("https://fixture/simple")] {
                for extras in [vec![], vec!["".into(), "https://extra/simple".into()]] {
                    for links in
                        [vec![], vec!["/path with spaces".into(), "https://wheels/".into()]]
                    {
                        for bundled in [false, true] {
                            let options = PipOptions {
                                index_url: index.map(str::to_owned),
                                extra_index_urls: extras.clone(),
                                find_links: links.clone(),
                                no_index: mask & 1 != 0,
                                isolated: mask & 2 != 0,
                                no_cache_dir: mask & 4 != 0,
                                disable_pip_version_check: mask & 8 != 0,
                                no_build_isolation: mask & 16 != 0,
                                skip_environment_check: false,
                            };
                            let dependencies = vec![
                                "fixture[extra]>=1; python_version > '3'".into(),
                                "--pre".into(),
                            ];
                            let command = super::command(
                                Path::new("/fixture/python"),
                                &dependencies,
                                &options,
                                bundled.then_some(Path::new("/bundle wheels")),
                                operation,
                            );
                            let command = command.as_std();
                            assert_eq!(command.get_envs().count(), 0);
                            assert!(command.get_current_dir().is_none());
                            let argv: Vec<_> = std::iter::once(command.get_program())
                                .chain(command.get_args())
                                .map(|arg| arg.to_str().unwrap())
                                .collect();
                            let expected = json!({"argv": argv, "custom_sources": options.has_custom_sources()});
                            cases.push(json!({
                                "mode": "plan", "resolve": matches!(operation, Operation::Resolve),
                                "options": options, "bundled": bundled, "dependencies": dependencies,
                                "expected": expected,
                            }));
                        }
                    }
                }
            }
        }
    }
    assert_eq!(cases.len(), 1536);
    reference::compare(&cases);
    digest(&cases, "3c6b7102c56f44e4965c5c29493f53a896cce60217c84d289cbba45f6c1094c5");
}

#[test]
fn pip_error_messages_match_upstream_decoding_stream_order_and_known_errors() {
    let streams = [
        b"".as_slice(),
        b" \r\n\t",
        b"ordinary output\n",
        b"\xff\xe2\x82bad\n",
        "\u{85}\u{2000}日本語\u{1c}".as_bytes(),
        b"externally-managed-environment",
        b"no such option: --dry-run",
        b"no such option: --dry-run\nexternally-managed-environment",
        b"Externally-Managed-Environment\0tail",
    ];
    let mut cases = Vec::new();
    for binary in ["hy", "custom hcli"] {
        for stdout in streams {
            for stderr in streams {
                cases.push(json!({
                    "mode": "error", "binary": binary, "stdout": stdout, "stderr": stderr,
                    "expected": super::errors::message(Path::new("/fixture/python"), stdout, stderr, binary),
                }));
            }
        }
    }
    assert_eq!(cases.len(), 162);
    reference::compare(&cases);
    digest(&cases, "a33b36067c937af90e3e38019123079f96413e2c4084c9fcd505dd13a616c097");
}

fn digest(cases: &[Value], expected_digest: &str) {
    let expected: Vec<_> = cases.iter().map(|case| &case["expected"]).collect();
    let hash = format!("{:x}", Sha256::digest(serde_json::to_vec(&expected).unwrap()));
    assert_eq!(hash, expected_digest);
}
