//! Replace an isolated executable copy using local GitHub release fixtures.
#![cfg(unix)]

mod support;

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

use serde_json::json;
use support::{
    http::{Response, Server},
    *,
};

const REPLACEMENT: &[u8] = b"verified replacement fixture\n";

fn copied_binary(sandbox: &Sandbox) -> PathBuf {
    let path = sandbox.path().join("hy");
    fs::copy(env!("CARGO_BIN_EXE_hy"), &path).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o751)).unwrap();
    path
}

fn asset_name() -> String {
    let os = if cfg!(target_os = "macos") {
        "mac"
    } else {
        "linux"
    };
    let arch = if cfg!(target_arch = "aarch64") {
        if cfg!(target_os = "macos") {
            "arm64"
        } else {
            "aarch64"
        }
    } else {
        "x86_64"
    };
    format!("hy-{os}-{arch}")
}

#[test]
fn native_update_preserves_release_tags_and_replaces_only_the_copied_executable() {
    for prereleases in [false, true] {
        let sandbox = Sandbox::new();
        let binary = copied_binary(&sandbox);
        let tag = if prereleases {
            "v3.0.0-rc.1"
        } else {
            "2.0.0+build.5"
        };
        let server = Server::start(move |request, _| match request.path.as_str() {
            "/repos/19h/hy/releases?per_page=100&page=1" => Response::json(json!([
                {"tag_name": "2.0.0+build.5"},
                {"tag_name": "v3.0.0-rc.1"},
                {"tag_name": "v1.0.0", "draft": true},
                {"tag_name": "not-a-version"},
                {"tag_name": null},
                {},
            ])),
            path if path == format!("/repos/19h/hy/releases/tags/{tag}") => Response::json(json!({
                "tag_name": tag,
                "assets": [
                    {"id": 7, "name": asset_name(), "size": REPLACEMENT.len()},
                    {"id": null, "name": asset_name(), "size": REPLACEMENT.len()},
                    {"id": 8, "name": asset_name(), "size": 0},
                ],
            })),
            "/repos/19h/hy/releases/assets/7" => Response::zip(REPLACEMENT.to_vec()),
            _ => Response::missing(),
        });
        let mut args = vec!["update", "--mode", "binary", "--auto-install"];
        if prereleases {
            args.push("--include-prereleases");
        }
        let output = sandbox
            .command_for(&binary, &args)
            .env("HCLI_GITHUB_URL", "git@github.com:19h/hy.git")
            .env("GITHUB_API_URL", &server.url)
            .env("GITHUB_TOKEN", "fixture-token")
            .output()
            .unwrap();
        assert_success(&output);
        assert_eq!(fs::read(&binary).unwrap(), REPLACEMENT);
        assert_eq!(fs::metadata(&binary).unwrap().permissions().mode() & 0o777, 0o751);
        let requests = server.requests();
        assert_eq!(requests.len(), 3);
        assert!(requests.iter().all(|request| {
            request.headers.to_ascii_lowercase().contains("authorization: bearer fixture-token")
        }));
        assert!(requests[2].headers.contains("application/octet-stream"));
    }
}

#[test]
fn failed_update_downloads_preserve_the_executable_and_remove_staging_files() {
    for failure in ["empty", "short", "long", "http", "created", "partial", "ambiguous"] {
        let sandbox = Sandbox::new();
        let binary = copied_binary(&sandbox);
        let original = fs::read(&binary).unwrap();
        let server = Server::start(move |request, _| match request.path.as_str() {
            "/repos/19h/hy/releases?per_page=100&page=1" => {
                Response::json(json!([{"tag_name": "v2.0.0"}]))
            }
            "/repos/19h/hy/releases/tags/v2.0.0" => {
                let mut assets =
                    vec![json!({"id": 7, "name": format!("../{}", asset_name()), "size": 4})];
                if failure == "ambiguous" {
                    assets.push(assets[0].clone());
                }
                Response::json(json!({"tag_name": "v2.0.0", "assets": assets}))
            }
            "/repos/19h/hy/releases/assets/7" => match failure {
                "empty" => Response::zip(vec![]),
                "short" => Response::zip(b"bad".to_vec()),
                "long" => Response::zip(b"too long".to_vec()),
                "created" | "partial" => Response {
                    status: if failure == "created" {
                        201
                    } else {
                        206
                    },
                    content_type: "application/octet-stream",
                    body: b"size".to_vec(),
                },
                _ => Response::missing(),
            },
            _ => Response::missing(),
        });
        let output = sandbox
            .command_for(&binary, &["update", "--mode", "binary", "--auto-install"])
            .env("HCLI_GITHUB_URL", "https://github.com/19h/hy")
            .env("GITHUB_API_URL", &server.url)
            .output()
            .unwrap();
        assert_eq!(output.status.success(), failure == "ambiguous", "{failure}");
        assert_eq!(fs::read(&binary).unwrap(), original, "{failure}");
        let files: Vec<_> =
            fs::read_dir(sandbox.path()).unwrap().map(|entry| entry.unwrap().file_name()).collect();
        assert_eq!(files, ["hy"], "unexpected staging files after {failure}");
        assert_eq!(
            server.requests().len(),
            if failure == "ambiguous" {
                2
            } else {
                3
            }
        );
    }
}

#[test]
fn force_controls_reinstalling_the_current_version() {
    for force in [false, true] {
        let sandbox = Sandbox::new();
        let binary = copied_binary(&sandbox);
        let original = fs::read(&binary).unwrap();
        let version = env!("CARGO_PKG_VERSION");
        let server = Server::start(move |request, _| {
            if request.path.ends_with("releases?per_page=100&page=1") {
                Response::json(json!([{"tag_name": version}]))
            } else if request.path.ends_with(&format!("releases/tags/{version}")) {
                Response::json(
                    json!({"tag_name": version, "assets": [{"id": 7, "name": asset_name(), "size": REPLACEMENT.len()}]}),
                )
            } else if request.path.ends_with("releases/assets/7") {
                Response::zip(REPLACEMENT.to_vec())
            } else {
                Response::missing()
            }
        });
        let mut args = vec!["update", "--mode", "binary", "--auto-install"];
        if force {
            args.push("--force");
        }
        let output = sandbox
            .command_for(&binary, &args)
            .env("HCLI_GITHUB_URL", "https://github.com/19h/hy")
            .env("GITHUB_API_URL", &server.url)
            .output()
            .unwrap();
        assert_success(&output);
        assert_eq!(
            fs::read(&binary).unwrap(),
            if force {
                REPLACEMENT
            } else {
                &original
            }
        );
        assert_eq!(
            server.requests().len(),
            if force {
                3
            } else {
                1
            }
        );
    }
}

#[test]
fn release_pagination_and_api_errors_do_not_modify_the_executable() {
    for api_error in [false, true] {
        let sandbox = Sandbox::new();
        let binary = copied_binary(&sandbox);
        let original = fs::read(&binary).unwrap();
        let server = Server::start(move |request, _| {
            if api_error {
                return Response::missing();
            }
            if request.path.ends_with("page=1") {
                Response::json(json!(vec![json!({"tag_name": "v0.1.0"}); 100]))
            } else if request.path.ends_with("page=2") {
                Response::json(json!([{"tag_name": env!("CARGO_PKG_VERSION")}]))
            } else {
                Response::missing()
            }
        });
        let output = sandbox
            .command_for(&binary, &["update", "--mode", "binary", "--auto-install"])
            .env("GITHUB_API_URL", &server.url)
            .output()
            .unwrap();
        assert_eq!(output.status.success(), !api_error);
        assert_eq!(fs::read(&binary).unwrap(), original);
        assert_eq!(
            server.requests().len(),
            if api_error {
                1
            } else {
                2
            }
        );
        assert!(
            server
                .requests()
                .iter()
                .all(|request| request.path.starts_with("/repos/19h/hy/releases?"))
        );
    }
}

#[test]
fn draft_release_and_last_equal_precedence_tag_select_the_download() {
    let sandbox = Sandbox::new();
    let binary = copied_binary(&sandbox);
    let server = Server::start(move |request, _| match request.path.as_str() {
        "/repos/19h/hy/releases?per_page=100&page=1" => Response::json(json!([
            {"tag_name": "2.0.0"},
            {"tag_name": "3.0.0+first", "draft": true},
            {"tag_name": "v3.0.0+last", "draft": true},
            {"tag_name": "4.0.0-rc.1"},
        ])),
        "/repos/19h/hy/releases/tags/v3.0.0+last" => Response::json(json!({
            "assets": [{"id": 7, "name": asset_name(), "size": REPLACEMENT.len()}],
        })),
        "/repos/19h/hy/releases/assets/7" => Response::zip(REPLACEMENT.to_vec()),
        _ => Response::missing(),
    });
    let output = sandbox
        .command_for(&binary, &["update", "--mode", "binary", "--auto-install"])
        .env("GITHUB_API_URL", &server.url)
        .output()
        .unwrap();
    assert_success(&output);
    assert_eq!(fs::read(&binary).unwrap(), REPLACEMENT);
    assert_eq!(server.requests().len(), 3);
}

#[test]
fn metadata_messages_stop_discovery_and_invalid_requirements_fail_before_http() {
    for scenario in ["release-message", "asset-message", "no-assets", "invalid-current"] {
        let sandbox = Sandbox::new();
        let server = Server::start(move |request, _| {
            let listing = request.path.contains("per_page=100");
            if listing && scenario != "release-message" {
                return Response::json(json!([{"tag_name": "2.0.0"}]));
            }
            if scenario == "no-assets" {
                return Response::json(json!({"assets": []}));
            }
            Response {
                status: 403,
                content_type: "application/json",
                body: br#"{"message":"API rate limit exceeded"}"#.to_vec(),
            }
        });
        let version = if scenario == "invalid-current" {
            "invalid"
        } else {
            "1.0.0"
        };
        let output = sandbox
            .command(&["update", "--mode", "binary", "--auto-install"])
            .env("GITHUB_API_URL", &server.url)
            .env("HCLI_VERSION", version)
            .output()
            .unwrap();
        assert_eq!(output.status.success(), scenario != "invalid-current", "{scenario}");
        let requests = server.requests();
        let expected = match scenario {
            "release-message" => 1,
            "invalid-current" => 0,
            _ => 2,
        };
        assert_eq!(requests.len(), expected, "{scenario}");
        assert!(!String::from_utf8_lossy(&output.stderr).contains("Update available:"));
    }
}
