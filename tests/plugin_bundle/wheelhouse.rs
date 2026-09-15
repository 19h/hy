use super::*;

#[test]
fn wheelhouse_install_ignores_unselected_unsafe_and_unreadable_members() {
    let sandbox = Sandbox::new();
    let package = sandbox.path().join("plugin.zip");
    let mut descriptor = identity_manifest("1", "https://github.com/example/wheelhouse");
    descriptor["plugin"]["pythonDependencies"] = json!(["fixture"]);
    archive_manifest(&package, &descriptor, &[]);
    let bundle = sandbox.path().join("bundle.zip");
    fixture_bundle(&bundle, &package, false);
    let file = fs::OpenOptions::new().read(true).write(true).open(&bundle).unwrap();
    let mut archive = zip::ZipWriter::new_append(file).unwrap();
    let options = zip::write::SimpleFileOptions::default();
    archive.start_file("ignored/../unsafe", options).unwrap();
    archive.write_all(b"ignored").unwrap();
    archive.add_symlink("ignored/link", "missing", options).unwrap();
    archive.start_file("ignored/corrupt", options).unwrap();
    archive.write_all(b"ignored").unwrap();
    archive.finish().unwrap();
    let mut bytes = fs::read(&bundle).unwrap();
    let local = {
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(&bytes)).unwrap();
        archive.by_name("ignored/corrupt").unwrap().header_start() as usize
    };
    bytes[local] = 0;
    fs::write(&bundle, bytes).unwrap();

    let python = sandbox.path().join("python");
    fake_python(&python);
    let arguments = sandbox.path().join("pip-arguments");
    let output = sandbox
        .command(&[
            "plugin",
            "--repo",
            bundle.to_str().unwrap(),
            "--offline",
            "--no-python-environment-check",
            "install",
            "example==1",
        ])
        .env("HCLI_CURRENT_IDA_PLATFORM", "windows-x86_64")
        .env("HCLI_CURRENT_IDA_PYTHON_EXE", &python)
        .env("HY_TEST_PIP_ARGUMENTS", &arguments)
        .env("HY_TEST_EXPECT_WHEEL", "fixture-1-py3-none-any.whl")
        .output()
        .unwrap();
    assert_success(&output);
    assert_eq!(installed_manifest(&sandbox)["plugin"]["version"], "1");
    assert_eq!(fs::read_to_string(arguments).unwrap().matches("--no-index").count(), 2);
}
