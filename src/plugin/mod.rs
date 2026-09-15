//! IDA plugin management: metadata, installation, repositories.

pub(crate) mod archive_paths;
pub mod bundle;
mod compatibility;
mod config;
mod context;
mod dependencies;
mod editable;
pub(crate) mod files;
pub mod index;
mod install;
mod installed;
mod manifest;
mod metadata;
mod metadata_values;
mod schema_version;
mod settings;
mod unmanaged;
mod validation;
mod version;

pub use compatibility::{
    all_ida_versions, all_platforms, is_ida_version_compatible, is_platform_compatible,
    parse_ida_version,
};
pub use config::*;
pub use context::PluginContext;
pub use dependencies::*;
pub use editable::EditableRegistration;
pub use files::validate_directory_files;
pub use install::{InstallationSource, uninstall};
pub use installed::*;
pub use manifest::*;
pub use metadata::*;
pub use settings::{PluginSetting, SettingType};
pub use unmanaged::{UnmanagedKind, unmanaged_plugins};
pub(crate) use version::valid_specification;
pub use version::{parse_version, version_matches, version_precedence};
