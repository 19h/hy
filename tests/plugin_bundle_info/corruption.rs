use std::fs;
use std::io::{Cursor, Read};

use super::{Sandbox, bundle, descriptor, inspect, report, zip, zip_with_method};

#[path = "../plugin_bundle_dependencies/reference.rs"]
mod creation_reference;

#[derive(Clone, Copy, Debug)]
enum Damage {
    Checksum,
    LocalSignature,
    UnsupportedCompression,
    Encrypted,
    EncryptedBadHeader,
    DeflateStream,
}

impl Damage {
    const ALL: [Self; 6] = [
        Self::Checksum,
        Self::LocalSignature,
        Self::UnsupportedCompression,
        Self::Encrypted,
        Self::EncryptedBadHeader,
        Self::DeflateStream,
    ];

    fn skipped(self) -> bool {
        matches!(self, Self::Checksum | Self::LocalSignature | Self::EncryptedBadHeader)
    }
}

struct HeaderOffsets {
    local: usize,
    central: usize,
    data: usize,
}

fn header_offsets(bytes: &[u8], name: &str) -> HeaderOffsets {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let entry = archive.by_name(name).unwrap();
    let offsets = HeaderOffsets {
        local: entry.header_start() as usize,
        central: entry.central_header_start() as usize,
        data: entry.data_start().unwrap() as usize,
    };
    assert_eq!(&bytes[offsets.local..offsets.local + 4], b"PK\x03\x04");
    assert_eq!(&bytes[offsets.central..offsets.central + 4], b"PK\x01\x02");
    offsets
}

fn damage(bytes: &mut [u8], name: &str, damage: Damage) {
    let HeaderOffsets {
        local,
        central,
        data,
    } = header_offsets(bytes, name);
    match damage {
        Damage::Checksum => bytes[central + 16] ^= 1,
        Damage::LocalSignature => bytes[local] = 0,
        Damage::UnsupportedCompression => {
            bytes[central + 10..central + 12].copy_from_slice(&10_u16.to_le_bytes());
            bytes[local + 8..local + 10].copy_from_slice(&10_u16.to_le_bytes());
        }
        Damage::Encrypted | Damage::EncryptedBadHeader => {
            bytes[central + 8] |= 1;
            bytes[local + 6] |= 1;
            if matches!(damage, Damage::EncryptedBadHeader) {
                bytes[local] = 0;
            }
        }
        Damage::DeflateStream => {
            assert_eq!(&bytes[central + 10..central + 12], &8_u16.to_le_bytes());
            // DEFLATE BTYPE=3 is reserved and cannot start a valid stream.
            bytes[data] = 7;
        }
    }
}

fn plugin() -> Vec<u8> {
    zip(&[("ida-plugin.json", &descriptor("example", "1")), ("plugin.py", b"# fixture")])
}

#[test]
fn damaged_outer_plugin_members_follow_source_skip_and_error_boundaries() {
    let sandbox = Sandbox::new();
    let path = sandbox.path().join("bundle.zip");
    let plugin = plugin();
    for kind in Damage::ALL {
        for healthy_after in [false, true] {
            let mut members = vec![("plugins/damaged.zip", plugin.as_slice())];
            if healthy_after {
                members.push(("plugins/healthy.zip", plugin.as_slice()));
            }
            let mut bytes = bundle(&members);
            damage(&mut bytes, "plugins/damaged.zip", kind);
            fs::write(&path, bytes).unwrap();
            let result = inspect(&sandbox, &path, true);
            assert_eq!(result["success"], kind.skipped(), "{kind:?}");
            let plugins = match (kind.skipped(), healthy_after) {
                (true, true) => "  plugins: 1\n    example: 1\n",
                (true, false) => "  plugins: (none)\n",
                (false, _) => "",
            };
            assert_eq!(result["report"], report(&path, plugins), "{kind:?}");
        }
    }
}

#[test]
fn damaged_unselected_outer_members_are_never_opened() {
    let sandbox = Sandbox::new();
    let path = sandbox.path().join("bundle.zip");
    let plugin = plugin();
    for name in ["unrelated/package.zip", "plugins/package.ZIP", "plugins/readme.txt"] {
        for kind in Damage::ALL {
            let mut bytes = bundle(&[(name, &plugin), ("plugins/healthy.zip", &plugin)]);
            damage(&mut bytes, name, kind);
            fs::write(&path, bytes).unwrap();
            let result = inspect(&sandbox, &path, true);
            assert_eq!(result["success"], true, "{name}: {kind:?}");
            assert_eq!(result["report"], report(&path, "  plugins: 1\n    example: 1\n"));
        }
    }
}

#[test]
fn inner_file_references_use_names_without_reading_their_contents() {
    let sandbox = Sandbox::new();
    let path = sandbox.path().join("bundle.zip");
    for name in ["plugin.py", "unrelated.txt"] {
        for kind in Damage::ALL {
            let mut plugin = zip(&[
                ("ida-plugin.json", &descriptor("example", "1")),
                ("plugin.py", b"# fixture"),
                ("unrelated.txt", b"not a descriptor"),
            ]);
            damage(&mut plugin, name, kind);
            fs::write(&path, bundle(&[("plugins/package.zip", &plugin)])).unwrap();
            let result = inspect(&sandbox, &path, true);
            assert_eq!(result["success"], true, "{name}: {kind:?}");
            assert_eq!(result["report"], report(&path, "  plugins: 1\n    example: 1\n"));
        }
    }
}

#[test]
fn manifest_and_inner_descriptor_read_errors_remain_terminal() {
    let sandbox = Sandbox::new();
    let path = sandbox.path().join("bundle.zip");
    for kind in Damage::ALL {
        let mut bytes = bundle(&[]);
        damage(&mut bytes, "plugin-bundle.json", kind);
        fs::write(&path, bytes).unwrap();
        let result = inspect(&sandbox, &path, true);
        assert_eq!(result["success"], false, "manifest: {kind:?}");
        assert_eq!(result["report"], "");

        let mut plugin = plugin();
        damage(&mut plugin, "ida-plugin.json", kind);
        fs::write(&path, bundle(&[("plugins/package.zip", &plugin)])).unwrap();
        let result = inspect(&sandbox, &path, true);
        assert_eq!(result["success"], false, "descriptor: {kind:?}");
        assert_eq!(result["report"], report(&path, ""));
    }
}

#[test]
fn file_reference_lookup_preserves_member_spelling_and_ignores_symlink_attributes() {
    let sandbox = Sandbox::new();
    let path = sandbox.path().join("bundle.zip");
    for member_name in ["plugin.py", "plugin.py/", "./plugin.py"] {
        for symlink in [false, true] {
            let mut plugin = zip(&[
                ("ida-plugin.json", &descriptor("example", "1")),
                (member_name, b"# fixture"),
            ]);
            if symlink {
                let header = header_offsets(&plugin, member_name);
                // Unix creator and S_IFLNK attributes; no filesystem link is created.
                plugin[header.central + 5] = 3;
                plugin[header.central + 38..header.central + 42]
                    .copy_from_slice(&(0o120777_u32 << 16).to_le_bytes());
            }
            fs::write(&path, bundle(&[("plugins/package.zip", &plugin)])).unwrap();
            let result = inspect(&sandbox, &path, true);
            assert_eq!(result["success"], true);
            let plugins = if member_name == "plugin.py" {
                "  plugins: 1\n    example: 1\n"
            } else {
                "  plugins: (none)\n"
            };
            assert_eq!(
                result["report"],
                report(&path, plugins),
                "{member_name}, symlink={symlink}"
            );
        }
    }
}

#[test]
fn stored_deflated_and_bzip2_members_can_be_combined_at_both_archive_levels() {
    let sandbox = Sandbox::new();
    let path = sandbox.path().join("bundle.zip");
    let mut template = zip::ZipArchive::new(Cursor::new(bundle(&[]))).unwrap();
    let mut manifest = Vec::new();
    template.by_name("plugin-bundle.json").unwrap().read_to_end(&mut manifest).unwrap();
    let methods = [
        zip::CompressionMethod::Stored,
        zip::CompressionMethod::Deflated,
        zip::CompressionMethod::Bzip2,
    ];
    for inner in methods {
        let plugin = zip_with_method(
            &[("ida-plugin.json", &descriptor("example", "1")), ("plugin.py", b"# fixture")],
            inner,
        );
        for outer in methods {
            let bytes = zip_with_method(
                &[("plugin-bundle.json", &manifest), ("plugins/package.zip", &plugin)],
                outer,
            );
            fs::write(&path, bytes).unwrap();
            let result = inspect(&sandbox, &path, true);
            assert_eq!(result["success"], true, "{outer:?}/{inner:?}");
            assert_eq!(result["report"], report(&path, "  plugins: 1\n    example: 1\n"));
        }
    }
}

#[test]
fn local_creation_preserves_unreadable_unselected_members() {
    let sandbox = Sandbox::new();
    let package_path = sandbox.path().join("package.zip");
    let output_path = sandbox.path().join("bundle.zip");
    for kind in Damage::ALL {
        let mut package = plugin();
        damage(&mut package, "plugin.py", kind);
        fs::write(&package_path, &package).unwrap();
        creation_reference::compare(
            &package_path,
            serde_json::json!({"name": "example", "dependencies": []}),
        );
        let output = sandbox.run(&[
            "plugin",
            "bundle",
            "create",
            "--path",
            output_path.to_str().unwrap(),
            "--target",
            "windows-x86_64-cp312",
            package_path.to_str().unwrap(),
        ]);
        super::support::assert_success(&output);
        let mut archive = zip::ZipArchive::new(fs::File::open(&output_path).unwrap()).unwrap();
        let mut embedded = Vec::new();
        archive.by_name("plugins/example-1.zip").unwrap().read_to_end(&mut embedded).unwrap();
        assert_eq!(embedded, package, "{kind:?}");
        assert_eq!(inspect(&sandbox, &output_path, true)["success"], true, "{kind:?}");
    }
}
