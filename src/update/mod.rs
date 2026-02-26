//! Self-update: version checking, GitHub release download, binary replacement.

mod release;
mod version;

pub use release::*;
pub use version::*;
