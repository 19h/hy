use super::*;
use std::os::unix::fs::PermissionsExt;

#[test]
fn unavailable_optional_wheels_do_not_leave_archives_or_partial_downloads() {
    let sandbox = Sandbox::new();
    let repository = sandbox.path().join("repository");
    fs::create_dir(&repository).unwrap();
    let mut root = descriptor("root", "1");
    root["plugin"]["dependencies"] = json!([{"plugin": "optional", "required": false}]);
    root["plugin"]["pythonDependencies"] = json!(["required-wheel"]);
    archive(&repository, "root.zip", &root);
    let mut optional = descriptor("optional", "1");
    optional["plugin"]["pythonDependencies"] = json!(["missing-wheel"]);
    archive(&repository, "optional.zip", &optional);
    let uv = sandbox.path().join("uv");
    fs::write(
        &uv,
        r#"#!/bin/sh
failed=false
for argument in "$@"; do
    if [ "$previous" = --dest ]; then destination=$argument; fi
    if [ "$argument" = missing-wheel ]; then failed=true; fi
    previous=$argument
done
if [ "$failed" = true ]; then
    printf partial > "$destination/rejected.whl"
    printf 'No matching distribution found for missing-wheel\n' >&2
    exit 1
fi
printf accepted > "$destination/required.whl"
"#,
    )
    .unwrap();
    fs::set_permissions(&uv, fs::Permissions::from_mode(0o755)).unwrap();
    let output = sandbox.path().join("bundle.zip");
    let arguments = [
        "plugin",
        "bundle",
        "create",
        "--repo",
        repository.to_str().unwrap(),
        "--path",
        output.to_str().unwrap(),
        "--platform",
        "linux",
        "--python",
        "3.12",
        "root",
    ];
    assert_success(&sandbox.command(&arguments).env("PATH", sandbox.path()).output().unwrap());
    let mut bundle = zip::ZipArchive::new(fs::File::open(&output).unwrap()).unwrap();
    assert!(bundle.by_name("plugins/root-1.zip").is_ok());
    assert!(bundle.by_name("plugins/optional-1.zip").is_err());
    assert!(bundle.by_name("dependencies/python/linux-x86_64-cp312/required.whl").is_ok());
    assert!(bundle.by_name("dependencies/python/linux-x86_64-cp312/rejected.whl").is_err());
    drop(bundle);

    let accepted = fs::read(&output).unwrap();
    fs::remove_file(&uv).unwrap();
    let unavailable = sandbox.command(&arguments).env("PATH", sandbox.path()).output().unwrap();
    assert_eq!(unavailable.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&unavailable.stderr).contains("requires uv"));
    assert_eq!(fs::read(output).unwrap(), accepted);
}
