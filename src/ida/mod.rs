//! IDA Pro installation discovery, management, and interaction.

mod architecture;
mod discovery;
mod install;
pub mod ipc;
pub(crate) mod launch;
pub mod links;
mod paths;
mod product;
mod protocol;
pub mod python;
mod version;

pub use architecture::{binary_architecture, current_ida_platform};
pub use discovery::{find_standard_installations, generate_instance_name};
pub use install::*;
pub use paths::*;
pub use product::IdaProduct;
pub use protocol::{register_protocol_handler, unregister_protocol_handler};
pub use version::{detect_ida_version, instance_version};
