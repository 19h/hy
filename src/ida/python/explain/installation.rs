//! Installation observations remain separate from Python runtime observations.

use super::types::*;
use crate::ida;
use std::path::Path;

pub(super) fn architecture_and_version(installation: &Path) -> ArchitectureAndVersion {
    let ida_binary = ida::ida_binary_path(installation).unwrap_or_else(|| {
        ida::executable_dir(installation).join(if cfg!(windows) {
            "ida.exe"
        } else {
            "ida"
        })
    });
    let (binary_arch, binary_arch_error) = match ida::binary_architecture(&ida_binary) {
        Ok(architecture) => (architecture.map(str::to_owned), None),
        Err(error) => (None, Some(error.to_string())),
    };
    let environment = crate::config::Env::global();
    let (platform, platform_error) = match ida::current_ida_platform() {
        Ok(platform) => (Some(platform), None),
        Err(error) => (None, Some(error.to_string())),
    };
    let version = environment
        .current_ida_version
        .clone()
        .map(|version| (version, "$HCLI_CURRENT_IDA_VERSION"))
        .or_else(|| ida::version::installation_version(installation));
    let (ida_version, ida_version_source) = match version {
        Some((version, source)) => (Some(version), Some(source.into())),
        None => (None, None),
    };
    ArchitectureAndVersion {
        ida_binary_error: None,
        ida_binary: Some(ida_binary),
        binary_arch,
        binary_arch_error,
        platform_error,
        platform,
        ida_version_source,
        ida_version_error: ida_version.is_none().then(|| "could not determine IDA version from python/ida_pro.py, the IDA executable, or the installation directory name".into()),
        ida_version,
    }
}
