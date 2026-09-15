//! Self-update: version checking, GitHub release download, binary replacement.

mod background;
mod install;
mod release;
mod version;

pub use background::BackgroundUpdateChecker;
pub use install::update_binary;
pub use release::*;
