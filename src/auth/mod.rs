//! Authentication: credential storage, OAuth flows, and API key management.

mod credentials;
mod gotrue;
mod headers;
mod login;
mod oauth;
mod policy;
pub mod service;
pub mod session;
mod status;

pub use credentials::CredentialsConfig;
pub use headers::request_headers;
pub use service::AuthService;
pub use status::show_resolved_login_info;
