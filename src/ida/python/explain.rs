//! Environment report orchestration; observations, notes and rendering are separate.

use crate::error::Result;
use crate::ida;

mod collect;
mod installation;
mod notes;
mod render;
mod types;

#[cfg(test)]
mod tests;

pub use types::EnvironmentReport;
use types::{Installation, KnownInstallations, SelectedInstallation};

pub async fn explain() -> Result<EnvironmentReport> {
    let mut paths = ida::find_standard_installations().await;
    paths.sort();
    let known_installations = KnownInstallations {
        installations: paths
            .into_iter()
            .map(|path| Installation {
                version: ida::version::sdk_version(&path).or_else(|| {
                    ida::version::version_in_name(
                        path.file_name().and_then(|name| name.to_str()).unwrap_or(""),
                    )
                }),
                path,
            })
            .collect(),
        error: None,
    };
    let selected_installation = match ida::resolve_install_dir() {
        Ok(selected) => SelectedInstallation {
            install_dir: Some(selected.path),
            install_dir_source: Some(selected.source),
            install_dir_error: None,
        },
        Err(error) => SelectedInstallation {
            install_dir: None,
            install_dir_source: None,
            install_dir_error: Some(error.to_string()),
        },
    };
    let mut report = EnvironmentReport {
        experimental: true,
        known_installations,
        architecture_and_version: selected_installation
            .install_dir
            .as_deref()
            .map(installation::architecture_and_version),
        selected_installation,
        python_environment: None,
        idapython_virtualenv: None,
        python_version: None,
        python_version_mismatches: Vec::new(),
        python_version_mismatch_error: None,
        notes: Vec::new(),
    };
    if report.selected_installation.install_dir.is_none() {
        return Ok(report);
    }
    let environment = collect::python_environment().await?;
    let venv = collect::virtual_environment(environment.idat_probe.as_ref()).await?;
    let version = collect::python_version().await?;
    match collect::mismatches(environment.idat_probe.as_ref(), version.final_python_exe.as_deref())
        .await
    {
        Ok(mismatches) => report.python_version_mismatches = mismatches,
        Err(error) => report.python_version_mismatch_error = Some(collect::exception(&error)),
    }
    report.notes = notes::collect(
        &environment,
        venv.as_ref(),
        &version,
        &crate::config::Env::global().binary_name,
    );
    report.python_environment = Some(environment);
    report.idapython_virtualenv = venv;
    report.python_version = Some(version);
    Ok(report)
}

impl EnvironmentReport {
    pub fn print_text(&self) {
        print!("{}", render::text(self));
    }
}
