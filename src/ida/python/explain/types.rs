//! Report records mirror the upstream JSON sections.

use serde::Serialize;
use std::path::PathBuf;

#[derive(Serialize)]
pub struct EnvironmentReport {
    pub(super) experimental: bool,
    pub(super) known_installations: KnownInstallations,
    pub(super) selected_installation: SelectedInstallation,
    pub(super) architecture_and_version: Option<ArchitectureAndVersion>,
    pub(super) python_environment: Option<PythonEnvironment>,
    pub(super) idapython_virtualenv: Option<VirtualEnvironment>,
    pub(super) python_version: Option<PythonVersion>,
    pub(super) python_version_mismatches: Vec<VersionMismatch>,
    pub(super) python_version_mismatch_error: Option<String>,
    pub(super) notes: Vec<Note>,
}

#[derive(Serialize)]
pub(super) struct Installation {
    pub(super) path: PathBuf,
    pub(super) version: Option<String>,
}

#[derive(Serialize)]
pub(super) struct KnownInstallations {
    pub(super) installations: Vec<Installation>,
    pub(super) error: Option<String>,
}

#[derive(Serialize)]
pub(super) struct SelectedInstallation {
    pub(super) install_dir: Option<PathBuf>,
    pub(super) install_dir_source: Option<String>,
    pub(super) install_dir_error: Option<String>,
}

#[derive(Serialize)]
pub(super) struct ArchitectureAndVersion {
    pub(super) ida_binary: Option<PathBuf>,
    pub(super) ida_binary_error: Option<String>,
    pub(super) binary_arch: Option<String>,
    pub(super) binary_arch_error: Option<String>,
    pub(super) platform: Option<String>,
    pub(super) platform_error: Option<String>,
    pub(super) ida_version: Option<String>,
    pub(super) ida_version_source: Option<String>,
    pub(super) ida_version_error: Option<String>,
}

#[derive(Serialize)]
pub(super) struct CandidateVirtualEnvironment {
    pub(super) path: PathBuf,
    pub(super) source: &'static str,
}

#[derive(Serialize)]
pub(super) struct PythonEnvironment {
    pub(super) virtual_env: Option<String>,
    pub(super) virtual_env_is_uv_cache: bool,
    pub(super) user_virtual_env: Option<PathBuf>,
    pub(super) candidate_virtual_envs: Vec<CandidateVirtualEnvironment>,
    pub(super) idapython_venv_executable: Option<String>,
    pub(super) idapython_venv_executable_exists: Option<bool>,
    pub(super) python_exe: Option<PathBuf>,
    pub(super) python_exe_source: Option<String>,
    pub(super) python_exe_error: Option<String>,
    pub(super) externally_managed: bool,
    pub(super) idat_probe: Option<IdatProbe>,
    pub(super) idat_probe_error: Option<String>,
}

#[derive(Serialize)]
pub(super) struct IdatProbe {
    pub(super) frozen: bool,
    pub(super) externally_managed: bool,
    pub(super) prefix: String,
    pub(super) base_prefix: String,
    pub(super) executable: Option<String>,
    pub(super) virtual_env: Option<String>,
    pub(super) idapython_venv_executable: Option<String>,
    pub(super) version_major: i64,
    pub(super) version_minor: i64,
}

#[derive(Serialize)]
pub(super) struct VirtualEnvironment {
    pub(super) venv: PathBuf,
    pub(super) home: Option<String>,
    pub(super) system_site_packages: Option<String>,
    pub(super) python_version: Option<String>,
}

#[derive(Serialize)]
pub(super) struct PythonVersion {
    pub(super) final_python_exe: Option<PathBuf>,
    pub(super) final_python_exe_error: Option<String>,
    pub(super) probed_version: Option<String>,
    pub(super) probed_version_error: Option<String>,
    pub(super) hcli_interpreter_version: &'static str,
    pub(super) hcli_interpreter_path: PathBuf,
}

#[derive(Serialize)]
pub(super) struct VersionMismatch {
    pub(super) ida_version: String,
    pub(super) other_version: String,
    pub(super) other_path: PathBuf,
    pub(super) other_source: String,
}

#[derive(Serialize)]
pub(super) struct Note {
    pub(super) kind: &'static str,
    pub(super) text: String,
}
