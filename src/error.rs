//! Unified error types for the hcli application.

use std::path::PathBuf;

mod plugin_access;
pub use plugin_access::PluginAccessDenied;

/// Top-level error type for all hcli operations.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("child process exited with status {0}")]
    ChildExit(i32),
    #[error("{0}")]
    PythonPackages(String),
    #[error("{0}")]
    IdaProbe(String),
    #[error("{0}")]
    PythonNotFound(String),
    #[error("{0}")]
    UnicodeDecode(String),
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
    Api {
        status: u16,
        message: String,
    },

    // ── Auth errors ─────────────────────────────────────────────────────
    #[error("Not logged in — run `hy login` first")]
    NotLoggedIn,

    #[allow(dead_code)]
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
    /// Python ValueError from catalogue acquisition; archive callers skip it.
    #[error("{0}")]
    GitHubValue(String),

    #[error("{0}")]
    PluginAccessDenied(#[from] PluginAccessDenied),

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
    #[allow(dead_code)]
    #[error("IDA installation not found")]
    IdaNotFound,

    #[error("IDA installation failed: {0}")]
    IdaInstallFailed(String),

    #[error("Failed to launch IDA: {0}")]
    IdaLaunch(#[source] Box<Error>),

    // ── Update errors ───────────────────────────────────────────────────
    #[error("Update failed: {0}")]
    UpdateFailed(String),

    // ── Serialization ───────────────────────────────────────────────────
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    // ── HTTP ────────────────────────────────────────────────────────────
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("HTTP {status} while fetching {url}")]
    RepositoryHttp {
        status: u16,
        url: String,
    },

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
    pub(crate) fn from_status_message(status: u16, message: Option<String>) -> Self {
        match status {
            401 => Self::Authentication("Authentication failed".into()),
            403 => Self::Forbidden("Access forbidden".into()),
            404 => Self::NotFound("Resource not found".into()),
            429 => Self::RateLimit,
            _ => {
                let message = message.unwrap_or_else(|| format!("HTTP {status}"));
                Self::Api {
                    status,
                    message,
                }
            }
        }
    }
}
