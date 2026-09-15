//! Collected observations; policy distinguishes unknown values from failed checks.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub(super) enum System {
    Windows,
    Mac,
    Linux,
}

impl System {
    pub fn current() -> Self {
        if cfg!(windows) {
            Self::Windows
        } else if cfg!(target_os = "macos") {
            Self::Mac
        } else {
            Self::Linux
        }
    }

    pub fn python(self, root: &Path) -> PathBuf {
        root.join(match self {
            Self::Windows => "Scripts/python.exe",
            Self::Mac | Self::Linux => "bin/python",
        })
    }

    pub fn export(self, value: &Path) -> String {
        let value = value.display();
        match self {
            Self::Windows => format!(
                "[Environment]::SetEnvironmentVariable(\"IDAPYTHON_VENV_EXECUTABLE\", \"{value}\", \"User\")"
            ),
            Self::Mac | Self::Linux => format!("export IDAPYTHON_VENV_EXECUTABLE=\"{value}\""),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub(super) struct State {
    pub python_exe: PathBuf,
    pub python_exe_exists: bool,
    pub source: String,
    pub system: System,
    pub idausr: Option<PathBuf>,
    pub venv_root: Option<PathBuf>,
    pub pip_available: Option<bool>,
    pub python_version: Option<String>,
    pub ida_python_version: Option<String>,
    pub externally_managed: bool,
    pub uv_ephemeral: bool,
    pub idapython_venv_executable: Option<PathBuf>,
    pub idapython_venv_executable_exists: bool,
    pub shell_virtual_env: Option<PathBuf>,
    pub idapythonrc_path: Option<PathBuf>,
    pub idapythonrc_activates_venv: bool,
    pub base_prefix: Option<PathBuf>,
    pub conda: bool,
    // Resolve filesystem-dependent predicates once, before applying policy.
    pub variable_selects_venv: bool,
    pub homebrew: bool,
}

impl State {
    pub fn recommended_venv(&self) -> PathBuf {
        self.idausr.as_ref().map_or_else(|| "~/.idapro/venv".into(), |root| root.join("venv"))
    }

    pub fn create_hint(&self, binary: &str) -> String {
        let root = self.recommended_venv();
        let version = self
            .ida_python_version
            .as_deref()
            .filter(|value| !value.is_empty())
            .or(self.python_version.as_deref().filter(|value| !value.is_empty()))
            .unwrap_or("3.X");
        let export = self.system.export(&self.system.python(&root));
        let root = root.display();
        format!(
            "Run `{binary} ida python create-environment`. It creates a virtual environment at \
             {root} with Python {version} and configures IDA to use it.\n\
             Or do it yourself:\n  uv venv --seed --python {version} {root}\n  {export}"
        )
    }
}
