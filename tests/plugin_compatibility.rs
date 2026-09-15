//! Compatibility checks use the declared IDA versions and selected executable.

mod support;

use std::fs;

use serde_json::{Value, json};
use support::*;

#[test]
fn installation_checks_exact_ida_versions_and_service_pack_ranges() {
    for (declaration, current, compatible) in [
        (json!(["9.0"]), "9.0", true),
        (json!(["9.0"]), "9.0sp1", false),
        (json!(">=9.0sp1,<9.3"), "9.0sp1", true),
        (json!(">=9.0sp1,<9.3"), "9.0", false),
        (json!([]), "9.4", false),
        (Value::Null, "9.4", true),
        (Value::Null, "11.0", false),
    ] {
        let sandbox = Sandbox::new();
        let package = sandbox.path().join("plugin.zip");
        let mut manifest = identity_manifest("1", "https://github.com/example/compatibility");
        if !declaration.is_null() {
            manifest["plugin"]["idaVersions"] = declaration.clone();
        }
        archive_manifest(&package, &manifest, &[]);
        let output = sandbox
            .command(&["plugin", "install", package.to_str().unwrap()])
            .env("HCLI_CURRENT_IDA_VERSION", current)
            .output()
            .unwrap();
        assert_eq!(
            output.status.success(),
            compatible,
            "{declaration} with IDA {current}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(sandbox.path().join("idausr/plugins/example").exists(), compatible);
    }
}

#[test]
fn installation_uses_idas_architecture_and_honors_the_platform_override() {
    for (architecture, machine, use_override) in [
        ("x86_64", 0x0100_0007_u32, false),
        ("aarch64", 0x0100_000c_u32, false),
        ("aarch64", 0x0100_0007_u32, true),
    ] {
        let sandbox = Sandbox::new();
        let installation = sandbox.path().join("ida");
        fs::create_dir(&installation).unwrap();
        // The header reader supports all executable formats on every host OS.
        let mut header = vec![0; 64];
        header[..4].copy_from_slice(b"\xcf\xfa\xed\xfe");
        header[4..8].copy_from_slice(&machine.to_le_bytes());
        fs::write(
            installation.join(if cfg!(windows) {
                "ida.exe"
            } else {
                "ida"
            }),
            header,
        )
        .unwrap();

        let platform = format!("{}-{architecture}", std::env::consts::OS);
        let package = sandbox.path().join("plugin.zip");
        let mut manifest = identity_manifest("1", "https://github.com/example/architecture");
        manifest["plugin"]["platforms"] = json!([platform]);
        archive_manifest(&package, &manifest, &[]);
        let mut command = sandbox.command(&["plugin", "install", package.to_str().unwrap()]);
        command.env("HCLI_CURRENT_IDA_INSTALL_DIR", &installation);
        if use_override {
            command.env("HCLI_CURRENT_IDA_PLATFORM", &platform);
        } else {
            command.env_remove("HCLI_CURRENT_IDA_PLATFORM");
        }
        assert_success(&command.output().unwrap());
    }
}

#[test]
fn unknown_architectures_and_empty_platforms_cannot_publish_plugins() {
    for empty_platforms in [false, true] {
        let sandbox = Sandbox::new();
        let installation = sandbox.path().join("ida");
        fs::create_dir(&installation).unwrap();
        fs::write(
            installation.join(if cfg!(windows) {
                "ida.exe"
            } else {
                "ida"
            }),
            b"unknown executable",
        )
        .unwrap();
        let package = sandbox.path().join("plugin.zip");
        let mut manifest = identity_manifest("1", "https://github.com/example/architecture");
        if empty_platforms {
            manifest["plugin"]["platforms"] = json!([]);
        }
        archive_manifest(&package, &manifest, &[]);
        let mut command = sandbox.command(&["plugin", "install", package.to_str().unwrap()]);
        command.env("HCLI_CURRENT_IDA_INSTALL_DIR", &installation);
        if !empty_platforms {
            command.env_remove("HCLI_CURRENT_IDA_PLATFORM");
        }
        let output = command.output().unwrap();
        assert!(!output.status.success());
        let expected = if empty_platforms {
            "not compatible"
        } else {
            "unrecognized IDA executable architecture"
        };
        assert!(String::from_utf8_lossy(&output.stderr).contains(expected));
        assert!(!sandbox.path().join("idausr/plugins/example").exists());
    }
}
