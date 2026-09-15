use crate::support::*;
use serde_json::json;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

pub(super) struct Rig {
    pub sandbox: Sandbox,
    pub tools: PathBuf,
    pub target: PathBuf,
    pub events: PathBuf,
    template: PathBuf,
    creator: PathBuf,
}

pub(super) fn script(path: &Path, source: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, source).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}

const PYTHON: &str = r#"#!/bin/sh
printf '%s|%s\n' "$0" "$*" >> "$HY_TEST_EVENTS"
case "$1:$2" in
  '-c:import pip') [ "${HY_TEST_NO_PIP:-0}" = 0 ] ;;
  '-c:'*)
    case "$2" in
      *'__hy__:'*) printf '__hy__:%s\n' "$HY_TEST_PROBE" ;;
      *) printf '%s\n' "${HY_TEST_PYTHON_VERSION:-3.13}" ;;
    esac ;;
  '-m:venv') "$HY_TEST_CREATE" "$3" ;;
  '-m:ensurepip') exit "${HY_TEST_ENSUREPIP_STATUS:-0}" ;;
  '-m:pip') case "$*" in *faildep*) echo 'fixture dependency failure' >&2; exit 1 ;; esac ;;
  *) exit 1 ;;
esac
"#;

impl Rig {
    pub fn new(uv: bool) -> Self {
        let sandbox = Sandbox::new();
        let tools = sandbox.path().join("tools");
        fs::create_dir_all(&tools).unwrap();
        let template = sandbox.path().join("python-template");
        script(&template, PYTHON);
        let creator = sandbox.path().join("create-fixture");
        script(
            &creator,
            r#"#!/bin/sh
for target do :; done
/bin/mkdir -p "$target/bin"
/bin/cp "$HY_TEST_TEMPLATE" "$target/bin/python"
printf 'home = fixture\n' > "$target/pyvenv.cfg"
"#,
        );
        if uv {
            script(
                &tools.join("uv"),
                r#"#!/bin/sh
printf 'uv|%s\n' "$*" >> "$HY_TEST_EVENTS"
"$HY_TEST_CREATE" "$@"
"#,
            );
        }
        let target = sandbox.path().join("environment with spaces");
        let events = sandbox.path().join("events");
        Self {
            sandbox,
            tools,
            target,
            events,
            template,
            creator,
        }
    }

    pub fn python(&self, path: &Path) {
        script(path, PYTHON);
    }

    pub fn existing(&self, filename: &str) -> PathBuf {
        let executable = self.target.join("bin").join(filename);
        self.python(&executable);
        fs::write(self.target.join("pyvenv.cfg"), b"home = fixture\n").unwrap();
        executable
    }

    pub fn idat(&self, registered: &Path) -> PathBuf {
        let install = self.sandbox.path().join("ida");
        script(&install.join("idat"), include_str!("../support/idat.sh"));
        self.python(registered);
        install
    }

    pub fn command(&self, json_output: bool, configure: bool) -> Command {
        let mut args = vec![
            "ida",
            "python",
            "create-environment",
            "--path",
            self.target.to_str().unwrap(),
            "--python-version",
            "3.13",
        ];
        if json_output {
            args.push("--json");
        }
        if !configure {
            args.push("--no-configure-env-var");
        }
        let mut command = self.sandbox.command(&args);
        let probe = json!({"executable":"python", "prefix":"base", "base_prefix":"base", "version":"3.13",
            "pip_available":true, "externally_managed":false, "scripts":"bin", "virtual_env":null});
        command
            .env("PATH", &self.tools)
            .env("HY_TEST_EVENTS", &self.events)
            .env("HY_TEST_CREATE", &self.creator)
            .env("HY_TEST_TEMPLATE", &self.template)
            .env("HY_TEST_PROBE", probe.to_string())
            .env("HCLI_CURRENT_IDA_INSTALL_DIR", self.sandbox.path().join("missing-ida"));
        command
    }

    pub fn calls(&self) -> String {
        fs::read_to_string(&self.events).unwrap_or_default()
    }

    pub fn plugin(&self, name: &str, dependencies: serde_json::Value) {
        let path = self.sandbox.path().join("idausr/plugins").join(name);
        fs::create_dir_all(&path).unwrap();
        let mut manifest = identity_manifest("1.0", "https://github.com/example/plugins");
        manifest["plugin"]["name"] = json!(name);
        manifest["plugin"]["pythonDependencies"] = dependencies;
        fs::write(path.join("ida-plugin.json"), serde_json::to_vec(&manifest).unwrap()).unwrap();
        fs::write(path.join("plugin.py"), b"# fixture\n").unwrap();
    }
}
