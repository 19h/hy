use super::*;
use std::io::Cursor;

fn rename(bytes: &mut [u8], from: &str, to: &str) {
    assert_eq!(from.len(), to.len());
    let (local, central) = {
        let mut archive = zip::ZipArchive::new(Cursor::new(&*bytes)).unwrap();
        let member = archive.by_name(from).unwrap();
        (member.header_start() as usize + 30, member.central_header_start() as usize + 46)
    };
    bytes[local..local + from.len()].copy_from_slice(to.as_bytes());
    bytes[central..central + from.len()].copy_from_slice(to.as_bytes());
}

#[test]
fn installation_ignores_members_outside_the_selected_subtree_and_git_contents() {
    let sandbox = Sandbox::new();
    let package = sandbox.path().join("plugin.zip");
    archive(
        &package,
        "1.0.0",
        &[
            ("../outside.txt", b"ignored"),
            ("package/.git/config", b"ignored"),
            ("ignored/corrupt", b"ignored"),
            ("package/retained.txt", b"retained"),
        ],
    );
    let file = fs::OpenOptions::new().read(true).write(true).open(&package).unwrap();
    let mut writer = zip::ZipWriter::new_append(file).unwrap();
    writer
        .add_symlink("ignored/link", "missing", zip::write::SimpleFileOptions::default())
        .unwrap();
    writer.finish().unwrap();
    let mut bytes = fs::read(&package).unwrap();
    let offset = {
        let mut archive = zip::ZipArchive::new(Cursor::new(&bytes)).unwrap();
        archive.by_name("ignored/corrupt").unwrap().header_start() as usize
    };
    bytes[offset] = 0;
    fs::write(&package, bytes).unwrap();
    assert_success(&sandbox.run(&["plugin", "install", package.to_str().unwrap()]));
    let installed = sandbox.path().join("idausr/plugins/example");
    assert_eq!(fs::read(installed.join("retained.txt")).unwrap(), b"retained");
    assert!(!installed.join(".git").exists());
    assert!(!sandbox.path().join("idausr/outside.txt").exists());
}

#[test]
fn duplicate_files_use_last_bytes_and_duplicate_descriptors_still_count() {
    let sandbox = Sandbox::new();
    let package = sandbox.path().join("plugin.zip");
    archive(&package, "1", &[("package/second.py", b"# duplicate replacement")]);
    let mut bytes = fs::read(&package).unwrap();
    rename(&mut bytes, "package/second.py", "package/plugin.py");
    fs::write(&package, bytes).unwrap();
    assert_success(&sandbox.run(&["plugin", "install", package.to_str().unwrap()]));
    assert_eq!(
        fs::read(sandbox.path().join("idausr/plugins/example/plugin.py")).unwrap(),
        b"# duplicate replacement"
    );

    let duplicate = sandbox.path().join("duplicate.zip");
    let descriptor = identity_manifest("2", "https://github.com/example/plugins");
    archive_manifest(
        &duplicate,
        &descriptor,
        &[("package/idb-plugin.json", &serde_json::to_vec(&descriptor).unwrap())],
    );
    let mut bytes = fs::read(&duplicate).unwrap();
    rename(&mut bytes, "package/idb-plugin.json", "package/ida-plugin.json");
    fs::write(&duplicate, bytes).unwrap();
    let output = sandbox.run(&["plugin", "install", "--force", duplicate.to_str().unwrap()]);
    assert!(!output.status.success());
    assert_eq!(installed_manifest(&sandbox)["plugin"]["version"], "1");
}

#[test]
fn named_install_stops_before_unrelated_descriptor_reads() {
    let sandbox = Sandbox::new();
    let package = sandbox.path().join("plugin.zip");
    let descriptor = identity_manifest("1", "https://github.com/example/plugins");
    let later =
        serde_json::to_vec(&identity_manifest("2", "https://github.com/example/plugins")).unwrap();
    archive_manifest(&package, &descriptor, &[("other/ida-plugin.json", &later)]);
    let mut bytes = fs::read(&package).unwrap();
    let central = {
        let mut archive = zip::ZipArchive::new(Cursor::new(&bytes)).unwrap();
        archive.by_name("other/ida-plugin.json").unwrap().central_header_start() as usize
    };
    bytes[central + 16] ^= 1;
    fs::write(&package, bytes).unwrap();
    let repository = sandbox.path().join("repo.json");
    fs::write(
        &repository,
        serde_json::to_vec(&repository_snapshot(&package, &descriptor)).unwrap(),
    )
    .unwrap();
    assert_success(&sandbox.run(&[
        "plugin",
        "--repo",
        repository.to_str().unwrap(),
        "install",
        "example==1",
    ]));
    assert_eq!(installed_manifest(&sandbox)["plugin"]["version"], "1");
    assert!(!sandbox.path().join("idausr/plugins/example/other").exists());
}

#[cfg(unix)]
#[test]
fn archive_extraction_uses_created_file_permissions() {
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;

    let sandbox = Sandbox::new();
    let package = sandbox.path().join("plugin.zip");
    let descriptor =
        serde_json::to_vec(&identity_manifest("1", "https://github.com/example/plugins")).unwrap();
    let mut writer = zip::ZipWriter::new(fs::File::create(&package).unwrap());
    let options = zip::write::SimpleFileOptions::default().unix_permissions(0o755);
    for (name, bytes) in [("ida-plugin.json", descriptor.as_slice()), ("plugin.py", b"# fixture")] {
        writer.start_file(name, options).unwrap();
        writer.write_all(bytes).unwrap();
    }
    writer.finish().unwrap();
    assert_success(&sandbox.run(&["plugin", "install", package.to_str().unwrap()]));
    let mode = fs::metadata(sandbox.path().join("idausr/plugins/example/plugin.py"))
        .unwrap()
        .permissions()
        .mode();
    assert_eq!(mode & 0o111, 0);
}
