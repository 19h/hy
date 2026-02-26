//! Authentication: credential storage, OAuth flows, and API key management.

mod credentials;
mod oauth;
pub mod service;
pub mod session;

pub use credentials::CredentialType;
pub use service::AuthService;
