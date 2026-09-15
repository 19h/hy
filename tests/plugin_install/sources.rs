//! Acquisition branches must retain source spelling through download selection.

use super::*;
use support::http::{Response, Server};

#[cfg(unix)]
#[test]
fn file_url_installation_preserves_symlink_parent_resolution_and_ignores_authority() {
    let sandbox = Sandbox::new();
    let root = sandbox.path();
    fs::create_dir_all(root.join("nested/inside")).unwrap();
    archive(&root.join("plugin.zip"), "1", &[]);
    archive(&root.join("nested/plugin.zip"), "2", &[]);
    std::os::unix::fs::symlink(root.join("nested/inside"), root.join("link")).unwrap();
    let source = format!("file://elsewhere{}/link/../plugin.zip", root.display());
    assert_success(&sandbox.run(&["plugin", "install", &source]));
    let installed: serde_json::Value = serde_json::from_slice(
        &fs::read(root.join("idausr/plugins/example/ida-plugin.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(installed["plugin"]["version"], "2");
}

#[test]
fn existing_non_zip_files_fall_through_to_repository_selection() {
    let sandbox = Sandbox::new();
    archive(&sandbox.path().join("example"), "1", &[]);
    let repository = sandbox.path().join("repository");
    fs::create_dir(&repository).unwrap();
    archive(&repository.join("example.zip"), "2", &[]);
    let output = sandbox
        .command(&["plugin", "--repo", repository.to_str().unwrap(), "install", "example"])
        .current_dir(sandbox.path())
        .output()
        .unwrap();
    assert_success(&output);
    let installed: serde_json::Value = serde_json::from_slice(
        &fs::read(sandbox.path().join("idausr/plugins/example/ida-plugin.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(installed["plugin"]["version"], "2");

    let uppercase = sandbox.path().join("archive.ZIP");
    archive(&uppercase, "3", &[]);
    let output = sandbox.run(&["plugin", "install", "-U", uppercase.to_str().unwrap()]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("plugin reference"));
}

#[test]
fn zip_named_directories_need_root_metadata_to_enter_directory_packaging() {
    for root_descriptor in [false, true] {
        let sandbox = Sandbox::new();
        let directory = sandbox.path().join("source.zip");
        let root = if root_descriptor {
            directory.clone()
        } else {
            directory.join("nested")
        };
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join("ida-plugin.json"),
            serde_json::to_vec(&identity_manifest("1", "https://github.com/example/source"))
                .unwrap(),
        )
        .unwrap();
        fs::write(root.join("plugin.py"), b"fixture").unwrap();
        let output = sandbox.run(&["plugin", "install", directory.to_str().unwrap()]);
        assert_eq!(
            output.status.success(),
            root_descriptor,
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(sandbox.path().join("idausr/plugins/example").exists(), root_descriptor);
        assert!(root.join("plugin.py").is_file());
    }
}

#[test]
fn direct_http_and_short_file_schemes_do_not_enter_download_transport() {
    let sandbox = Sandbox::new();
    let server = Server::start(|_, _| Response::missing());
    let package = sandbox.path().join("plugin.data");
    archive(&package, "1", &[]);
    for value in [format!("{}/plugin.zip", server.url), format!("file:{}", package.display())] {
        let output = sandbox.run(&["plugin", "install", &value]);
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains("plugin reference"));
        assert!(!sandbox.path().join("idausr/plugins/example").exists());
    }
    assert!(server.requests().is_empty());
    let url = url::Url::from_file_path(package).unwrap();
    assert_success(&sandbox.run(&["plugin", "install", url.as_str()]));
}

#[test]
fn github_release_requests_preserve_source_case_suffix_and_tag_rules() {
    for (source, endpoint) in [
        ("HTTPS://GitHub.Com/Owner/Repo", "/repos/Owner/Repo/releases/latest"),
        ("https://github.com/Owner/Repo.git/", "/repos/Owner/Repo.git/releases/latest"),
        ("https://github.com/Owner/Repo.git.git", "/repos/Owner/Repo.git/releases/latest"),
        (
            "https://github.com/Owner/Repo.git@v1/part+build/",
            "/repos/Owner/Repo/releases/tags/v1/part+build",
        ),
    ] {
        let sandbox = Sandbox::new();
        let package = sandbox.path().join("plugin.zip");
        archive(&package, "1", &[]);
        let bytes = fs::read(package).unwrap();
        let server = Server::start(move |request, base| {
            if request.path == endpoint {
                Response::json(
                    json!({"assets": [{"name": "plugin.zip", "browser_download_url": format!("{base}/plugin.zip"), "size": bytes.len()}]}),
                )
            } else if request.path == "/plugin.zip" {
                Response::zip(bytes.clone())
            } else {
                Response::missing()
            }
        });
        let output = sandbox
            .command(&["plugin", "install", source])
            .env("GITHUB_API_URL", &server.url)
            .output()
            .unwrap();
        assert_success(&output);
        let paths: Vec<_> = server.requests().into_iter().map(|request| request.path).collect();
        assert_eq!(paths, [endpoint, "/plugin.zip"], "{source}");
        assert!(sandbox.path().join("idausr/plugins/example/plugin.py").is_file());
    }
}
