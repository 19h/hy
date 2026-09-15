use super::*;

#[test]
fn bundle_snapshots_retain_member_urls_and_outer_archive_order() {
    let sandbox = Sandbox::new();
    let path = sandbox.path().join("bundle.zip");
    let first = package("EXAMPLE", "1.0");
    let last = package("example", "1");
    for reverse in [false, true] {
        let mut members =
            vec![("plugins/z.zip", first.as_slice()), ("plugins/a.zip", last.as_slice())];
        if reverse {
            members.reverse();
        }
        fs::write(&path, bundle(&members)).unwrap();
        let result = snapshot(&sandbox, &path, "bundle");
        assert_eq!(result["success"], true);
        let plugin = &result["snapshot"]["plugins"][0];
        assert_eq!(
            plugin["name"],
            if reverse {
                "EXAMPLE"
            } else {
                "example"
            }
        );
        assert_eq!(plugin["versions"]["1"][0]["url"], "hcli-bundle:plugins/a.zip");
        assert_eq!(plugin["versions"]["1.0"][0]["url"], "hcli-bundle:plugins/z.zip");
    }
}

#[test]
fn unreadable_outer_members_are_skipped_but_invalid_inner_archives_fail_loading() {
    let sandbox = Sandbox::new();
    let path = sandbox.path().join("bundle.zip");
    let good = package("example", "1");
    let bytes = bundle(&[("plugins/good.zip", &good), ("plugins/bad.zip", b"not ZIP")]);
    fs::write(&path, &bytes).unwrap();
    assert_eq!(snapshot(&sandbox, &path, "bundle")["success"], false);
    let mut archive = zip::ZipArchive::new(Cursor::new(&bytes)).unwrap();
    let offset = archive.by_name("plugins/bad.zip").unwrap().header_start() as usize;
    let mut corrupted = bytes.clone();
    corrupted[offset..offset + 4].copy_from_slice(b"bad!");
    fs::write(&path, corrupted).unwrap();
    let result = snapshot(&sandbox, &path, "bundle");
    assert_eq!(result["success"], true);
    assert_eq!(result["snapshot"]["plugins"].as_array().unwrap().len(), 1);
}

#[cfg(unix)]
#[test]
fn bundle_creation_fetches_member_urls_through_the_loaded_repository() {
    use sha2::{Digest, Sha256};
    use std::io::Read;

    let sandbox = Sandbox::new();
    let path = sandbox.path().join("source.zip");
    let bytes = package("example", "1");
    fs::write(&path, bundle(&[("plugins/original.zip", &bytes)])).unwrap();
    let python = sandbox.path().join("python");
    support::fake_python(&python);
    let output_path = sandbox.path().join("output.zip");
    let output = sandbox
        .command(&[
            "plugin",
            "--repo",
            path.to_str().unwrap(),
            "bundle",
            "create",
            "--path",
            output_path.to_str().unwrap(),
            "--python",
            "3.12",
            "--platform",
            "linux",
            "example==1",
        ])
        .env("HCLI_CURRENT_IDA_PYTHON_EXE", python)
        .output()
        .unwrap();
    support::assert_success(&output);
    let mut archive = zip::ZipArchive::new(fs::File::open(output_path).unwrap()).unwrap();
    let mut actual = Vec::new();
    archive.by_name("plugins/example-1.zip").unwrap().read_to_end(&mut actual).unwrap();
    assert_eq!(actual, bytes);
    let expected = json!({"name": "example", "sha256": format!("{:x}", Sha256::digest(&actual))});
    if let Some(source) = source(&path, "bundle-fetch") {
        assert_eq!(source, expected);
    }
}
