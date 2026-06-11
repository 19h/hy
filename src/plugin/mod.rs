//! IDA plugin management: metadata, installation, repositories.

pub mod bundle;
mod metadata;
mod repo;

pub use metadata::*;
pub use repo::*;
