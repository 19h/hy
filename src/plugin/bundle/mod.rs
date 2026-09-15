//! Interoperable HCLI plugin-bundle manifests and archive inspection.

mod create;
mod download;
mod inspect;
mod manifest;
mod metadata;
mod targets;
mod wheelhouse;

pub use create::{ResolvedPluginArchive, create_bundle};
pub use inspect::{BundleReader, is_plugin_bundle_zip};
pub use manifest::{
    BundleCreatedBy, BundleManifest, BundleTargetPlatformTag, validate_bundle_path,
};
pub(crate) use metadata::{single as local_archive_metadata, version as archive_version};
#[cfg(test)]
pub(crate) use targets::reference as target_reference;
pub use targets::{ALL_PLATFORMS, PipTarget, SUPPORTED_PYTHON_VERSIONS, resolve_platform_alias};
