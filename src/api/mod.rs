//! HTTP API client with automatic auth header injection.

mod client;
mod models;

pub use client::ApiClient;
pub use models::*;
