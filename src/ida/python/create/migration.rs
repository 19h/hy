//! Collect readable plugin requirements and report each migration independently.

use std::path::Path;

use serde::Serialize;

use crate::error::{Error, Result};
use crate::ida::python::{PipOptions, install_dependencies};

#[derive(Debug, Serialize)]
pub struct PluginMigration {
    pub name: String,
    pub dependencies: Vec<String>,
    pub success: bool,
    pub error: Option<String>,
}

pub(super) async fn run(
    executable: &Path,
    reinstall: bool,
    ui: &super::ui::Ui,
) -> Result<(Vec<PluginMigration>, bool)> {
    let mut plugins = Vec::new();
    for plugin in crate::plugin::installed_plugins()? {
        match crate::plugin::dependencies_from_directory(&plugin.metadata, &plugin.path) {
            Ok(dependencies) if !dependencies.is_empty() => {
                plugins.push((plugin.metadata.name, dependencies))
            }
            Err(error) => {
                tracing::debug!(path=%plugin.path.display(), %error, "skipping unreadable plugin dependencies")
            }
            _ => (),
        }
    }
    if plugins.is_empty() {
        return Ok((Vec::new(), false));
    }
    for (name, dependencies) in &plugins {
        ui.print(&format!("  {name}: {}", dependencies.join(", ")));
    }
    if !reinstall || !ui.confirm("Install these dependencies?")? {
        ui.print("Skipped plugin dependency installation. Reinstall these plugins to restore their dependencies.");
        return Ok((Vec::new(), true));
    }
    let mut migrations = Vec::new();
    for (name, dependencies) in plugins {
        let error =
            match install_dependencies(executable, &dependencies, &PipOptions::default(), None)
                .await
            {
                Ok(()) => None,
                Err(Error::PythonPackages(message)) => Some(message),
                Err(error) => return Err(error),
            };
        ui.print(&format!(
            "{} dependencies for {name}",
            if error.is_none() {
                "Installed"
            } else {
                "Failed"
            }
        ));
        migrations.push(PluginMigration {
            name,
            dependencies,
            success: error.is_none(),
            error,
        });
    }
    Ok((migrations, false))
}
