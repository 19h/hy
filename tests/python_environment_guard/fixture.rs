use std::fs;
use std::path::PathBuf;
use std::process::Command;

use crate::support::Sandbox;

pub(super) struct Fixture {
    pub sandbox: Sandbox,
    pub python: PathBuf,
    pub script: PathBuf,
    pub installation: PathBuf,
    events: PathBuf,
}

pub(super) fn executable(path: &std::path::Path, text: &str) {
    use std::os::unix::fs::PermissionsExt;
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}

impl Fixture {
    pub fn new(venv: bool) -> Self {
        let sandbox = Sandbox::new();
        let root = sandbox.path().join("environment");
        let python = root.join("bin/python");
        executable(&python, include_str!("python.sh"));
        if venv {
            fs::write(root.join("pyvenv.cfg"), "home = fixture\n").unwrap();
        }
        let script = root.join("bin/fixture-tool");
        executable(&script, include_str!("script.sh"));
        let installation = sandbox.path().join("ida");
        executable(&installation.join("idat"), include_str!("idat.sh"));
        let events = sandbox.path().join("events");
        Self {
            sandbox,
            python,
            script,
            installation,
            events,
        }
    }

    pub fn command(&self, args: &[&str]) -> Command {
        let mut command = self.sandbox.command(args);
        command
            .env("HCLI_CURRENT_IDA_PYTHON_EXE", &self.python)
            .env("HCLI_CURRENT_IDA_INSTALL_DIR", &self.installation)
            .env("PATH", self.python.parent().unwrap())
            .env("HY_TEST_EVENTS", &self.events)
            .env(
                "HY_TEST_SCRIPT_JSON",
                serde_json::json!({
                    "name": "fixture-tool", "path": self.script,
                    "scripts_dirs": [self.python.parent()], "entry_point": null,
                })
                .to_string(),
            )
            .env("HY_TEST_PURELIB", self.sandbox.path().join("site-packages"))
            .env_remove("VIRTUAL_ENV")
            .env_remove("UV_CACHE_DIR");
        command
    }

    pub fn calls(&self) -> String {
        fs::read_to_string(&self.events).unwrap_or_default()
    }

    pub fn configure(&self, command: &mut Command, version: &str) {
        let prefix = self.python.parent().unwrap().parent().unwrap();
        let probe = serde_json::json!({
            "executable": self.python, "prefix": prefix, "base_prefix": prefix,
            "version_major": 3, "version_minor": version.split('.').nth(1).unwrap().parse::<i64>().unwrap(),
            "pip_available": true, "externally_managed": false, "scripts": self.python.parent(),
        });
        command
            .env_remove("HCLI_CURRENT_IDA_PYTHON_EXE")
            .env("IDAPYTHON_VENV_EXECUTABLE", &self.python)
            .env("HY_TEST_IDA_PROBE", probe.to_string());
    }
}
