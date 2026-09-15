//! Resolve bundle targets independently of repository and archive operations.

use super::plugin_cmd::BundleCreateArgs;
use crate::error::{Error, Result};
use crate::plugin::bundle::{
    ALL_PLATFORMS, PipTarget, SUPPORTED_PYTHON_VERSIONS, resolve_platform_alias,
};
use crate::util::strings::python_trim;

#[cfg(test)]
mod tests;

pub async fn resolve(args: &BundleCreateArgs) -> Result<Vec<PipTarget>> {
    resolve_with(args, crate::ida::current_ida_platform, crate::ida::python::detect_current_version)
        .await
}

async fn resolve_with<F: std::future::Future<Output = Result<String>>>(
    args: &BundleCreateArgs,
    mut current_platform: impl FnMut() -> Result<String>,
    mut current_python: impl FnMut() -> F,
) -> Result<Vec<PipTarget>> {
    if !args.targets.is_empty() {
        if !args.platforms.is_empty() || !args.pythons.is_empty() {
            return Err(Error::Other(
                "--target cannot be combined with --platform or --python".into(),
            ));
        }
        return args.targets.iter().map(|target| PipTarget::parse(target)).collect();
    }
    if args.platforms.is_empty() {
        return Err(Error::Other(
            "--platform is required\n  use --platform current for this machine, \
             or --platform all for all supported platforms"
                .into(),
        ));
    }
    if args.pythons.is_empty() {
        return Err(Error::Other(
            "--python is required\n  use --python current for this machine, \
             or --python all for all supported versions"
                .into(),
        ));
    }
    let mut platforms = Vec::new();
    for platform in &args.platforms {
        match python_trim(platform).to_lowercase().as_str() {
            "all" => platforms.extend(ALL_PLATFORMS.iter().map(|value| (*value).to_owned())),
            "current" => platforms.push(current_platform()?),
            _ => platforms.push(resolve_platform_alias(platform)?),
        }
    }
    let mut pythons = Vec::new();
    for python in &args.pythons {
        match python_trim(python).to_lowercase().as_str() {
            "all" => {
                pythons.extend(SUPPORTED_PYTHON_VERSIONS.iter().map(|value| (*value).to_owned()))
            }
            "current" => {
                pythons.push(current_python().await?);
            }
            _ => pythons.push(python.clone()),
        }
    }
    let mut seen = std::collections::HashSet::new();
    let mut targets = Vec::new();
    for platform in platforms {
        for python in &pythons {
            let target = PipTarget::new(&platform, python)?;
            if seen.insert(target.id()) {
                targets.push(target);
            }
        }
    }
    Ok(targets)
}
