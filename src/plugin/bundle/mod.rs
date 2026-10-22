//! Interoperable HCLI plugin-bundle manifests and archive inspection.

mod cell;
mod create;
mod download;
mod inspect;
mod manifest;
mod targets;
mod wheelhouse;

pub use create::{ResolvedCell, create_bundle};
pub use inspect::{BundleReader, is_plugin_bundle_zip};
pub use manifest::{
    BundleCreatedBy, BundleManifest, BundleTargetPlatformTag, validate_bundle_path,
};
#[cfg(test)]
pub(crate) use targets::reference as target_reference;
pub use targets::{ALL_PLATFORMS, PipTarget, SUPPORTED_PYTHON_VERSIONS, resolve_platform_alias};
