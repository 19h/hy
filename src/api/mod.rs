//! HTTP API client with automatic auth header injection.

mod asset_models;
mod assets;
mod client;
mod download;
mod json;
mod licenses;
mod models;
mod redirect;
mod response;
mod session;
mod upload;

pub use asset_models::{Asset, Integer as AssetInteger, PagedAssets};
pub use assets::{UploadOptions, asset_path};
pub use client::ApiClient;
pub use models::*;
