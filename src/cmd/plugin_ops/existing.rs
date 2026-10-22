//! Validate replacement identity and skip upgrades that cannot change the version.

use super::PluginInstallArgs;
use crate::error::{Error, Result};
use crate::plugin::{self, PluginMetadata};

pub(crate) enum InstallAction {
    Install {
        was_installed: bool,
    },
    Keep(Box<PluginMetadata>),
}

pub(crate) fn installation_action(
    metadata: &PluginMetadata,
    args: &PluginInstallArgs,
) -> Result<InstallAction> {
    let candidate_version = plugin::parse_version(&metadata.version).ok_or_else(|| {
        Error::PluginInstall(format!("invalid plugin version: {}", metadata.version))
    })?;
    let installed = match plugin::installed_plugin_path(&metadata.name) {
        Ok(path) => match plugin::read_installed_metadata_from_path(&path) {
            Ok(metadata) => Some(metadata),
            Err(_) if args.editable => None,
            Err(error) => {
                return Err(Error::PluginInstall(format!(
                    "existing plugin {} is invalid: {error}; remove it with `{} plugin uninstall {}`",
                    metadata.name,
                    crate::config::Env::global().display_command(),
                    metadata.name,
                )));
            }
        },
        Err(Error::PluginNotInstalled(_)) => None,
        Err(error) => return Err(error),
    };
    let Some(installed) = installed else {
        return Ok(InstallAction::Install {
            was_installed: false,
        });
    };
    if installed.normalized_host()? != metadata.normalized_host()? {
        return Err(Error::PluginInstall(
            "replacement belongs to a different plugin repository".into(),
        ));
    }
    let installed_version = plugin::parse_version(&installed.version).ok_or_else(|| {
        Error::PluginInstall(format!("invalid installed plugin version: {}", installed.version))
    })?;
    if args.upgrade
        && !args.editable
        && !candidate_version.cmp_precedence(&installed_version).is_gt()
    {
        return Ok(InstallAction::Keep(Box::new(installed)));
    }
    Ok(InstallAction::Install {
        was_installed: true,
    })
}

pub(super) fn report_current(installed: &PluginMetadata) {
    crate::util::fmt::success(&format!(
        "Already installed {}=={}",
        installed.name, installed.version
    ));
}
