//! Gather each observation independently so a failed probe does not invent facts.

use super::{
    paths,
    state::{State, System},
};
use crate::error::Result;
use crate::ida::python::{ResolvedPython, checks, env_var, probe_ida};

#[derive(Clone, Copy)]
pub(super) enum ProbeMode {
    ResolutionOnly,
    AllowAdditional,
}

pub(super) async fn collect(resolved: &ResolvedPython, mode: ProbeMode) -> Result<State> {
    let root = paths::venv_root(&resolved.exe);
    let externally_managed =
        root.is_none() && resolved.ida_probe.as_ref().is_some_and(|probe| probe.externally_managed);
    let pip_available = if externally_managed {
        None
    } else {
        Some(checks::has_pip(&resolved.exe).await)
    };
    let python_version = checks::version(&resolved.exe).await?;
    let probe = match &resolved.ida_probe {
        Some(probe) => Some(probe.clone()),
        None if matches!(mode, ProbeMode::AllowAdditional)
            && resolved.source == "$IDAPYTHON_VENV_EXECUTABLE" =>
        {
            probe_ida().await.ok()
        }
        None => None,
    };
    let idausr = crate::ida::ida_user_dir();
    let rc = idausr.join("idapythonrc.py");
    let rc = rc.is_file().then_some(rc);
    let activates = rc.as_ref().and_then(|path| std::fs::read(path).ok()).is_some_and(|bytes| {
        let text = String::from_utf8_lossy(&bytes);
        ["addsitedir", "activate_this"].iter().any(|marker| text.contains(marker))
    });
    let variable = env_var("IDAPYTHON_VENV_EXECUTABLE").map(std::path::PathBuf::from);
    let base_prefix = probe
        .as_ref()
        .map(|probe| &probe.base_prefix)
        .filter(|prefix| !prefix.is_empty())
        .map(std::path::PathBuf::from);
    Ok(State {
        python_exe_exists: resolved.exe.is_file(),
        source: resolved.source.clone(),
        system: System::current(),
        idausr: Some(idausr),
        pip_available,
        python_version,
        ida_python_version: probe.as_ref().map(|probe| probe.version.clone()),
        externally_managed,
        uv_ephemeral: root.as_deref().is_some_and(paths::uv_ephemeral),
        idapython_venv_executable_exists: variable.as_ref().is_some_and(|path| path.is_file()),
        variable_selects_venv: paths::variable_selects(variable.as_deref(), root.as_deref()),
        idapython_venv_executable: variable,
        shell_virtual_env: paths::shell_venv(),
        idapythonrc_path: rc,
        idapythonrc_activates_venv: activates,
        homebrew: paths::homebrew(&resolved.exe)
            || base_prefix.as_deref().is_some_and(paths::homebrew),
        base_prefix,
        conda: resolved
            .exe
            .ancestors()
            .skip(1)
            .take(2)
            .any(|prefix| prefix.join("conda-meta").is_dir()),
        python_exe: resolved.exe.clone(),
        venv_root: root,
    })
}
