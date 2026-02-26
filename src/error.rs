//! Unified error types for the hcli application.

use std::path::PathBuf;

/// Top-level error type for all hcli operations.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    // ── API errors ──────────────────────────────────────────────────────
    #[error("Authentication failed: {0}")]
    Authentication(String),

    #[error("Access forbidden: {0}")]
    Forbidden(String),

    #[error("Resource not found: {0}")]
    NotFound(String),

    #[error("Rate limit exceeded")]
    RateLimit,

    #[error("API error ({status}): {message}")]
    Api { status: u16, message: String },

    // ── Auth errors ─────────────────────────────────────────────────────
    #[error("Not logged in — run `hcli login` first")]
    NotLoggedIn,

    #[error("Credentials not found: {0}")]
    CredentialsNotFound(String),

    #[error("OAuth flow failed: {0}")]
    OAuthFailed(String),

    // ── Filesystem errors ───────────────────────────────────────────────
    #[error("No space left on device at {path}")]
    NoSpace {
        path: PathBuf,
        required: Option<u64>,
        available: Option<u64>,
    },

    #[error("File not found: {0}")]
    FileNotFound(PathBuf),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    // ── Plugin errors ───────────────────────────────────────────────────
    #[error("Plugin already installed: {0}")]
    PluginAlreadyInstalled(String),

    #[error("Plugin not installed: {0}")]
    PluginNotInstalled(String),

    #[error("Platform incompatible: {0}")]
    PlatformIncompatible(String),

    #[error("IDA version incompatible: {0}")]
    IdaVersionIncompatible(String),

    #[error("Invalid plugin name: {0}")]
    InvalidPluginName(String),

    #[error("Plugin installation error: {0}")]
    PluginInstall(String),

    // ── IDA errors ──────────────────────────────────────────────────────
    #[error("IDA installation not found")]
    IdaNotFound,

    #[error("IDA installation failed: {0}")]
    IdaInstallFailed(String),

    // ── Update errors ───────────────────────────────────────────────────
    #[error("Update failed: {0}")]
    UpdateFailed(String),

    // ── Serialization ───────────────────────────────────────────────────
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    // ── HTTP ────────────────────────────────────────────────────────────
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    // ── Archive ──────────────────────────────────────────────────────────
    #[error("ZIP error: {0}")]
    Zip(#[from] zip::result::ZipError),

    // ── Catch-all ───────────────────────────────────────────────────────
    #[error("{0}")]
    Other(String),
}

/// Convenience alias used throughout the crate.
pub type Result<T, E = Error> = std::result::Result<T, E>;

impl Error {
    /// Construct an API error from an HTTP response status and body.
    pub fn from_status(status: u16, body: &str) -> Self {
        match status {
            401 => Self::Authentication("Authentication failed".into()),
            403 => Self::Forbidden("Access forbidden".into()),
            404 => Self::NotFound("Resource not found".into()),
            429 => Self::RateLimit,
            _ => {
                // Try to extract a message field from JSON body.
                let message = serde_json::from_str::<serde_json::Value>(body)
                    .ok()
                    .and_then(|v| v.get("message")?.as_str().map(String::from))
                    .unwrap_or_else(|| format!("HTTP {status}"));
                Self::Api { status, message }
            }
        }
    }
}
