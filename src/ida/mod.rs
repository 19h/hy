//! IDA Pro installation discovery, management, and interaction.

mod install;
mod paths;
mod protocol;

pub use install::*;
pub use paths::*;
pub use protocol::{register_protocol_handler, unregister_protocol_handler};
