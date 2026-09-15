//! Duplicate names must retain iteration order and resolve to the last member.

use super::*;

fn rename(bytes: &mut [u8], from: &str, to: &str) {
    assert_eq!(from.len(), to.len());
    let (local, central) = {
        let mut archive = zip::ZipArchive::new(Cursor::new(&*bytes)).unwrap();
        let entry = archive.by_name(from).unwrap();
        (entry.header_start() as usize + 30, entry.central_header_start() as usize + 46)
    };
    bytes[local..local + from.len()].copy_from_slice(to.as_bytes());
    bytes[central..central + from.len()].copy_from_slice(to.as_bytes());
}

#[test]
fn duplicate_inner_descriptors_repeat_the_last_descriptor() {
    let sandbox = Sandbox::new();
    let path = sandbox.path().join("bundle.zip");
    for first in [descriptor("example", "1"), b"invalid JSON".to_vec()] {
        for last in [descriptor("example", "2"), b"invalid JSON".to_vec()] {
            let mut plugin = zip(&[
                ("a/ida-plugin.json", &first),
                ("b/ida-plugin.json", &last),
                ("a/plugin.py", b"# fixture"),
            ]);
            rename(&mut plugin, "b/ida-plugin.json", "a/ida-plugin.json");
            fs::write(&path, bundle(&[("plugins/example.zip", &plugin)])).unwrap();
            let result = inspect(&sandbox, &path, true);
            assert_eq!(result["success"], true);
            let report = result["report"].as_str().unwrap();
            assert!(!report.contains("example: 1"));
            assert_eq!(report.contains("example: 2"), last != b"invalid JSON");
        }
    }
}

#[test]
fn duplicate_outer_archives_and_manifests_use_the_last_member() {
    let sandbox = Sandbox::new();
    let path = sandbox.path().join("bundle.zip");
    let plugin = |version| {
        zip(&[("ida-plugin.json", &descriptor("example", version)), ("plugin.py", b"# fixture")])
    };
    for first in [plugin("1"), b"invalid ZIP".to_vec()] {
        for last in [plugin("2"), b"invalid ZIP".to_vec()] {
            let mut bytes = bundle(&[("plugins/a.zip", &first), ("plugins/b.zip", &last)]);
            rename(&mut bytes, "plugins/b.zip", "plugins/a.zip");
            fs::write(&path, bytes).unwrap();
            let result = inspect(&sandbox, &path, true);
            assert_eq!(result["success"], last != b"invalid ZIP");
            assert!(!result["report"].as_str().unwrap().contains("example: 1"));
        }
    }
    let valid = bundle(&[]);
    let mut archive = zip::ZipArchive::new(Cursor::new(valid)).unwrap();
    let mut manifest = Vec::new();
    std::io::Read::read_to_end(&mut archive.by_name("plugin-bundle.json").unwrap(), &mut manifest)
        .unwrap();
    for (first, last, success) in [
        (b"invalid JSON".as_slice(), manifest.as_slice(), true),
        (manifest.as_slice(), b"invalid JSON".as_slice(), false),
    ] {
        let mut bytes = zip(&[("plugin-bundle.json", first), ("plugin-bundl2.json", last)]);
        rename(&mut bytes, "plugin-bundl2.json", "plugin-bundle.json");
        fs::write(&path, bytes).unwrap();
        assert_eq!(inspect(&sandbox, &path, true)["success"], success);
    }
}
