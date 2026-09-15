//! Isolated CLI fixtures shared by integration tests.
#![allow(dead_code)]

use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Output};

use serde_json::json;
use tempfile::TempDir;
use zip::write::SimpleFileOptions;

pub mod auth;
pub mod http;
#[cfg(unix)]
pub mod ida_ipc;
pub mod ke_dialogs;
#[cfg(unix)]
pub mod terminal;

/// Detached fixture programs can publish their result after the CLI has exited.
pub fn assert_file_eventually(path: &Path, expected: &[u8]) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    let mut actual = Vec::new();
    while std::time::Instant::now() < deadline {
        if let Ok(bytes) = fs::read(path) {
            if bytes == expected {
                return;
            }
            actual = bytes;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    panic!("fixture output at {}: expected {:?}, received {:?}", path.display(), expected, actual);
}

pub struct Sandbox {
    home: TempDir,
}

impl Sandbox {
    pub fn new() -> Self {
        Self {
            home: tempfile::tempdir().expect("create test home"),
        }
    }

    pub fn path(&self) -> &Path {
        self.home.path()
    }

    pub fn config_path(&self) -> std::path::PathBuf {
        if cfg!(target_os = "macos") {
            self.path().join("Library/Application Support/hcli/config.json")
        } else if cfg!(windows) {
            self.path().join("local/hex-rays/hcli/config.json")
        } else {
            self.path().join("config/hcli/config.json")
        }
    }

    pub fn run(&self, args: &[&str]) -> Output {
        self.run_with_env(args, &[])
    }

    pub fn run_with_env(&self, args: &[&str], environment: &[(&str, &Path)]) -> Output {
        self.command(args).envs(environment.iter().copied()).output().expect("run hy")
    }

    pub fn command(&self, args: &[&str]) -> Command {
        self.command_for(Path::new(env!("CARGO_BIN_EXE_hy")), args)
    }

    pub fn command_for(&self, executable: &Path, args: &[&str]) -> Command {
        let mut command = Command::new(executable);
        for (name, _) in std::env::vars_os() {
            if name.to_str().is_some_and(|name| name.to_ascii_lowercase().ends_with("_proxy")) {
                command.env_remove(name);
            }
        }
        command.env_remove("REQUEST_METHOD").env_remove("SSL_CERT_FILE").env_remove("SSL_CERT_DIR");
        // An unused scheme suppresses OS discovery without creating HTTP mounts.
        command.env("HY_TEST_UNUSED_PROXY", "fixture");
        command
            .arg("--disable-updates")
            .args(args)
            .env("HOME", self.path())
            .env("USERPROFILE", self.path())
            .env("XDG_CONFIG_HOME", self.path().join("config"))
            .env("LOCALAPPDATA", self.path().join("local"))
            .env("HCLI_CACHE_DIR", self.path().join("cache"))
            .env("XDG_CACHE_HOME", self.path().join("cache"))
            .env("HCLI_IDAUSR", self.path().join("idausr"))
            .env("HCLI_CURRENT_IDA_VERSION", "9.4")
            .env_remove("HCLI_API_KEY")
            .env_remove("HCLI_AUTH_CREDENTIALS")
            .env("HCLI_CONFIG_NAMESPACE", "hcli")
            .env("HCLI_EXTENSION_PYTHON", "")
            .env_remove("GITHUB_TOKEN")
            .env_remove("GH_TOKEN")
            .env_remove("HCLI_GITHUB_URL")
            .env_remove("GITHUB_API_URL")
            .env_remove("HCLI_VERSION")
            .env_remove("HCLI_CURRENT_IDA_INSTALL_DIR")
            .env("HCLI_CURRENT_IDA_PYTHON_EXE", self.path().join("unconfigured-python"))
            .env(
                "HCLI_CURRENT_IDA_PLATFORM",
                format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH),
            )
            .env_remove("IDADIR")
            .env_remove("IDAPYTHON_VENV_EXECUTABLE");
        command
            .env_remove("HCLI_KE_DOWNLOADS_DIR")
            .env_remove("HCLI_KE_DOWNLOADS_RETENTION_DAYS")
            .env_remove("HCLI_KE_ALLOW_PRIVATE_HOSTS")
            .env_remove("HCLI_KE_SKIP_CONFIRM")
            .env_remove("HCLI_KE_MAX_DOWNLOAD_MB");
        command
            .env_remove("HY_TEST_DOWNLOAD_SDIST")
            .env_remove("HY_TEST_EXPECT_WHEEL")
            .env_remove("HY_TEST_PURELIB");
        command
    }
}

pub fn archive(path: &Path, version: &str, members: &[(&str, &[u8])]) {
    archive_with_dependencies(path, version, members, &[]);
}

pub fn archive_with_dependencies(
    path: &Path,
    version: &str,
    members: &[(&str, &[u8])],
    dependencies: &[&str],
) {
    let manifest = json!({
        "IDAMetadataDescriptorVersion": 1,
        "plugin": {
            "name": "example",
            "version": version,
            "entryPoint": "plugin.py",
            "urls": {"repository": "https://github.com/example/plugins"},
            "authors": [{"email": "author@example.test"}],
            "pythonDependencies": dependencies,
            "settings": [{
                "key": "enabled",
                "name": "Enabled",
                "type": "boolean",
                "required": true,
                "default": false,
            }],
        },
    });

    archive_manifest(path, &manifest, members);
}

pub fn archive_manifest(path: &Path, manifest: &serde_json::Value, members: &[(&str, &[u8])]) {
    let file = fs::File::create(path).expect("create archive");
    let mut archive = zip::ZipWriter::new(file);
    let options = SimpleFileOptions::default();

    archive.start_file("package/ida-plugin.json", options).unwrap();
    archive.write_all(serde_json::to_string(&manifest).unwrap().as_bytes()).unwrap();
    archive.start_file("package/plugin.py", options).unwrap();
    archive.write_all(b"# test fixture\n").unwrap();

    for (name, bytes) in members {
        archive.start_file(*name, options).unwrap();
        archive.write_all(bytes).unwrap();
    }

    archive.finish().unwrap();
}

pub fn identity_manifest(version: &str, host: &str) -> serde_json::Value {
    json!({
        "IDAMetadataDescriptorVersion": 1,
        "plugin": {
            "name": "example",
            "version": version,
            "entryPoint": "plugin.py",
            "urls": {"repository": host},
            "authors": [{"email": "author@example.test"}],
        },
    })
}

pub fn repository_snapshot(package: &Path, manifest: &serde_json::Value) -> serde_json::Value {
    use sha2::{Digest, Sha256};

    let metadata = &manifest["plugin"];
    let version = metadata["version"].as_str().unwrap();
    json!({
        "version": 1,
        "plugins": [{
            "name": metadata["name"],
            "host": metadata["urls"]["repository"],
            "versions": {
                version: [{
                    "url": url::Url::from_file_path(package).unwrap().as_str(),
                    "sha256": format!("{:x}", Sha256::digest(fs::read(package).unwrap())),
                    "metadata": manifest,
                }],
            },
        }],
    })
}

pub fn add_repository(sandbox: &Sandbox, name: &str, path: &Path) {
    let url = url::Url::from_file_path(path).unwrap();
    assert_success(&sandbox.run(&["plugin", "repo", "add", name, url.as_str()]));
}

pub fn installed_manifest(sandbox: &Sandbox) -> serde_json::Value {
    let path = sandbox.path().join("idausr/plugins/example/ida-plugin.json");
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

#[cfg(unix)]
pub fn fake_python(path: &Path) {
    use std::os::unix::fs::PermissionsExt;

    fs::write(path, include_str!("../fixtures/pip.sh")).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}

pub fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "command failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}
