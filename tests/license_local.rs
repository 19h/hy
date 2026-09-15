//! License reporting and local installation regressions.

mod support;

use serde_json::json;
use std::fs;
use support::{
    auth::*,
    http::{Response, Server},
    *,
};

#[test]
fn license_tables_group_plans_and_classify_addons_by_product_subtype() {
    let sandbox = Sandbox::new();
    write_config(&sandbox, &stored("key", "fixture-api-key"));
    let server = Server::start(|request, _| match request.path.as_str() {
        "/api/customers" => Response::json(json!([{"id": 17}])),
        "/api/licenses/17?page=1&limit=100" => Response::json(json!({"total": 2, "items": [
            {"pubhash": "96-1234-5678-01", "product_catalog": "subscription", "status": "active",
                "license_type": "named", "edition": {"edition_name": "Pro"}, "addons": []},
            {"pubhash": "ééééé😀", "product_catalog": "legacy", "status": "expired",
                "license_type": "named", "end_date": "invalid-date-text",
                "addons": [
                    {"product": {"id": 1, "code": "HEXX64", "name": "x64", "catalog": "legacy",
                        "product_type": "addon", "product_subtype": "DECOMPILER"}},
                    {"product": {"id": 2, "code": "OTHER", "name": "Other", "catalog": "legacy",
                        "product_type": "decompiler", "product_subtype": "OTHER"}}
                ]}
        ]})),
        _ => Response::missing(),
    });
    let output = command(&sandbox, &server, &["license", "list"]).output().unwrap();
    assert_success(&output);
    let text = String::from_utf8_lossy(&output.stderr);
    for expected in [
        "Perpetual Licenses",
        "Subscription Licenses",
        "96-1234-5678-01",
        "ééééé😀",
        "Never",
        "OTHER",
        "Total: 2",
    ] {
        assert!(text.contains(expected), "{text}");
    }
    assert!(text.find("Perpetual Licenses") < text.find("Subscription Licenses"));
    let output =
        command(&sandbox, &server, &["license", "list", "--plan", "legacy"]).output().unwrap();
    assert_success(&output);
    let text = String::from_utf8_lossy(&output.stderr);
    assert!(!text.contains("Subscription Licenses"));
    assert!(text.contains("Total: 1"));
    for operation in ["list", "get"] {
        assert!(
            !command(&sandbox, &server, &["license", operation, "--plan", "LEGACY"])
                .output()
                .unwrap()
                .status
                .success()
        );
    }
}

#[test]
fn installing_a_license_preserves_bytes_and_modification_time() {
    let sandbox = Sandbox::new();
    let source = sandbox.path().join("fixture.hexlic");
    let target = sandbox.path().join("target");
    fs::write(&source, b"license contents").unwrap();
    fs::create_dir_all(&target).unwrap();
    let modified = std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_600_000_000);
    fs::File::options()
        .write(true)
        .open(&source)
        .unwrap()
        .set_times(fs::FileTimes::new().set_modified(modified))
        .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&source, fs::Permissions::from_mode(0o444)).unwrap();
    }
    assert_success(&sandbox.run(&["license", "install", source.to_str().unwrap(), "~/target"]));
    let installed = target.join("fixture.hexlic");
    assert_eq!(fs::read(&installed).unwrap(), b"license contents");
    assert_eq!(fs::metadata(&installed).unwrap().modified().unwrap(), modified);
    assert_eq!(
        fs::metadata(&installed).unwrap().permissions(),
        fs::metadata(&source).unwrap().permissions()
    );
}

#[test]
fn invalid_sources_and_same_file_destinations_fail_without_destroying_the_license() {
    let sandbox = Sandbox::new();
    let source = sandbox.path().join("fixture.hexlic");
    fs::write(&source, b"preserve me").unwrap();
    for file in [&source, &sandbox.path().join("missing.hexlic"), &sandbox.path().to_path_buf()] {
        let output = sandbox.run(&[
            "license",
            "install",
            file.to_str().unwrap(),
            sandbox.path().to_str().unwrap(),
        ]);
        assert!(!output.status.success());
        assert_eq!(fs::read(&source).unwrap(), b"preserve me");
    }
    #[cfg(unix)]
    {
        let directory = sandbox.path().join("hardlink");
        fs::create_dir_all(&directory).unwrap();
        fs::hard_link(&source, directory.join("fixture.hexlic")).unwrap();
        let output = sandbox.run(&[
            "license",
            "install",
            source.to_str().unwrap(),
            directory.to_str().unwrap(),
        ]);
        assert!(!output.status.success());
        assert_eq!(fs::read(&source).unwrap(), b"preserve me");
    }
}

#[cfg(unix)]
#[test]
fn custom_license_destination_and_creation_confirmation_use_real_prompts() {
    use support::terminal::Terminal;
    for create in [false, true] {
        let sandbox = Sandbox::new();
        let source = sandbox.path().join("fixture.hexlic");
        fs::write(&source, b"fixture").unwrap();
        let target = sandbox.path().join("custom-target");
        let mut terminal =
            Terminal::start(sandbox.command(&["license", "install", source.to_str().unwrap()]));
        terminal.wait_for("Select installation");
        terminal.wait_for("Other (specify custom path)");
        // Up wraps from the first entry to Other, independent of host installations.
        terminal.send("\x1b[A\r");
        terminal.wait_for("Enter the target directory path");
        terminal.send(&format!("{}\r", target.display()));
        terminal.wait_for("Create directory?");
        terminal.send(if create {
            "\r"
        } else {
            "\x1b[B\r"
        });
        let (status, output) = terminal.finish();
        assert!(status.success(), "{output}");
        assert_eq!(target.exists(), create);
        if create {
            assert_eq!(fs::read(target.join("fixture.hexlic")).unwrap(), b"fixture");
        }
    }
}

#[cfg(unix)]
#[test]
fn multiple_customer_selection_uses_the_selected_account() {
    use support::terminal::Terminal;
    let sandbox = Sandbox::new();
    write_config(&sandbox, &stored("key", "fixture-api-key"));
    let server = Server::start(|request, _| match request.path.as_str() {
        "/api/customers" => Response::json(json!([{"id": 17}, {"id": 29}])),
        "/api/licenses/29?page=1&limit=100" => Response::json(json!({"total": 0, "items": []})),
        _ => Response::missing(),
    });
    let mut terminal = Terminal::start(command(&sandbox, &server, &["license", "list"]));
    terminal.wait_for("Select customer");
    terminal.send("\x1b[B\r");
    let (status, output) = terminal.finish();
    assert!(status.success(), "{output}");
    assert_eq!(server.requests()[1].path, "/api/licenses/29?page=1&limit=100");
}
