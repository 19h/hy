//! Runtime observations follow source order; a successful override does not trigger IDA.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use super::types::*;
use crate::error::{Error, Result};
use crate::ida::python::{Probe, checks, env_var, environment, probe_ida, resolve};

impl From<Probe> for IdatProbe {
    fn from(probe: Probe) -> Self {
        let mut parts = probe.version.split('.');
        Self {
            frozen: probe.frozen,
            externally_managed: probe.externally_managed,
            version_major: parts.next().and_then(|part| part.parse().ok()).unwrap_or(0),
            version_minor: parts.next().and_then(|part| part.parse().ok()).unwrap_or(0),
            prefix: probe.prefix,
            base_prefix: probe.base_prefix,
            executable: probe.executable,
            virtual_env: probe.virtual_env,
            idapython_venv_executable: probe.idapython_venv_executable,
        }
    }
}

pub(super) fn exception(error: &Error) -> String {
    let class = match error {
        Error::PythonNotFound(_) => "PythonNotFoundError",
        Error::UnicodeDecode(_) => "UnicodeDecodeError",
        Error::IdaProbe(_) => "RuntimeError",
        Error::Io(error) => match error.kind() {
            std::io::ErrorKind::NotFound => "FileNotFoundError",
            std::io::ErrorKind::PermissionDenied => "PermissionError",
            _ => "OSError",
        },
        Error::Json(_) => "JSONDecodeError",
        _ => "RuntimeError",
    };
    format!("{class}: {error}")
}

fn candidates() -> Vec<CandidateVirtualEnvironment> {
    let mut seen = HashSet::new();
    std::env::var_os("PATH")
        .into_iter()
        .flat_map(|path| std::env::split_paths(&path).collect::<Vec<_>>())
        .filter(|path| {
            matches!(path.file_name().and_then(|name| name.to_str()), Some("bin" | "Scripts"))
        })
        .filter_map(|path| path.parent().map(Path::to_owned))
        .filter(|root| root.join("pyvenv.cfg").is_file())
        .filter(|root| {
            crate::util::realpath::resolve(root).is_ok_and(|resolved| seen.insert(resolved))
        })
        .filter(|root| !environment::uv_ephemeral(root))
        .map(|path| CandidateVirtualEnvironment {
            path,
            source: "PATH",
        })
        .collect()
}

pub(super) async fn python_environment() -> Result<PythonEnvironment> {
    let virtual_env = std::env::var("VIRTUAL_ENV").ok();
    let user_virtual_env = environment::shell_venv();
    let candidate_virtual_envs = candidates();
    let variable = env_var("IDAPYTHON_VENV_EXECUTABLE");
    let mut report = PythonEnvironment {
        virtual_env_is_uv_cache: virtual_env
            .as_ref()
            .is_some_and(|root| environment::uv_ephemeral(Path::new(root))),
        virtual_env,
        user_virtual_env,
        candidate_virtual_envs,
        idapython_venv_executable_exists: variable.as_ref().map(|path| Path::new(path).is_file()),
        idapython_venv_executable: variable,
        python_exe: None,
        python_exe_source: None,
        python_exe_error: None,
        externally_managed: false,
        idat_probe: None,
        idat_probe_error: None,
    };
    match resolve().await {
        Ok(resolved) => {
            report.externally_managed =
                resolved.ida_probe.as_ref().is_some_and(|probe| probe.externally_managed)
                    && environment::venv_root(&resolved.exe).is_none();
            report.python_exe = Some(resolved.exe);
            report.python_exe_source = Some(resolved.source);
            report.idat_probe = resolved.ida_probe.map(IdatProbe::from);
        }
        Err(error @ Error::PythonNotFound(_)) => {
            report.python_exe_error = Some(exception(&error));
            match probe_ida().await {
                Ok(probe) => report.idat_probe = Some(probe.into()),
                Err(error) => report.idat_probe_error = Some(exception(&error)),
            }
        }
        Err(error) => return Err(error),
    }
    Ok(report)
}

pub(super) async fn virtual_environment(
    probe: Option<&IdatProbe>,
) -> Result<Option<VirtualEnvironment>> {
    let Some(root) =
        probe.and_then(|probe| probe.virtual_env.as_deref()).filter(|path| !path.is_empty())
    else {
        return Ok(None);
    };
    let root = Path::new(root);
    let config = environment::config(root);
    Ok(Some(VirtualEnvironment {
        venv: root.into(),
        home: config.get("home").cloned(),
        system_site_packages: config.get("include-system-site-packages").cloned(),
        python_version: environment::venv_version(root).await?,
    }))
}

pub(super) async fn python_version() -> Result<PythonVersion> {
    let mut report = PythonVersion {
        final_python_exe: None,
        final_python_exe_error: None,
        probed_version: None,
        probed_version_error: None,
        hcli_interpreter_version: "not applicable (native Rust)",
        hcli_interpreter_path: crate::util::io::executable_path(),
    };
    match resolve().await {
        Ok(resolved) => {
            report.probed_version = checks::version(&resolved.exe).await?;
            if report.probed_version.is_none() {
                report.probed_version_error =
                    Some(format!("failed to run {}", resolved.exe.display()));
            }
            report.final_python_exe = Some(resolved.exe);
        }
        Err(error) => report.final_python_exe_error = Some(exception(&error)),
    }
    Ok(report)
}

pub(super) async fn mismatches(
    probe: Option<&IdatProbe>,
    executable: Option<&Path>,
) -> Result<Vec<VersionMismatch>> {
    let Some(probe) = probe else {
        return Ok(Vec::new());
    };
    let ida_version = format!("{}.{}", probe.version_major, probe.version_minor);
    let mut seen = HashSet::new();
    let mut results = Vec::new();
    let requested = probe
        .idapython_venv_executable
        .as_deref()
        .and_then(|path| environment::venv_root(Path::new(path)));
    for (root, source) in [
        (
            probe.virtual_env.as_deref().filter(|value| !value.is_empty()).map(PathBuf::from),
            "the virtualenv activated inside IDA ($VIRTUAL_ENV)",
        ),
        (requested, "the virtualenv requested by $IDAPYTHON_VENV_EXECUTABLE"),
    ] {
        let Some(root) = root else {
            continue;
        };
        let Some(normalized) = environment::normalized(&root) else {
            continue;
        };
        if !seen.insert(normalized) {
            continue;
        }
        if let Some(version) =
            environment::venv_version(&root).await?.filter(|version| version != &ida_version)
        {
            results.push(VersionMismatch {
                ida_version: ida_version.clone(),
                other_version: version,
                other_path: root,
                other_source: source.into(),
            });
        }
    }
    if let Some(executable) = executable {
        let already_seen = environment::venv_root(executable)
            .and_then(|root| environment::normalized(&root))
            .is_some_and(|root| seen.contains(&root));
        if !already_seen
            && let Some(version) =
                checks::version(executable).await?.filter(|version| version != &ida_version)
        {
            results.push(VersionMismatch {
                ida_version,
                other_version: version,
                other_path: executable.into(),
                other_source: "the interpreter HCLI would install plugin dependencies into".into(),
            });
        }
    }
    Ok(results)
}
