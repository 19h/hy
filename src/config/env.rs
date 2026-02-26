//! Environment variable configuration.
//!
//! All environment-driven settings are read once at startup and exposed
//! through the [`Env`] singleton.  Values fall back to compiled-in defaults.

use std::sync::OnceLock;

/// Global environment configuration singleton.
static ENV: OnceLock<Env> = OnceLock::new();

/// Read-only snapshot of all environment-driven configuration.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct Env {
    // ── API ─────────────────────────────────────────────────────────────
    pub api_key: Option<String>,
    pub api_url: String,
    pub cloud_url: String,
    pub portal_url: String,
    pub release_url: String,

    // ── GitHub ──────────────────────────────────────────────────────────
    pub github_token: Option<String>,
    pub github_api_url: String,
    pub github_url: String,

    // ── Supabase ────────────────────────────────────────────────────────
    pub supabase_anon_key: String,
    pub supabase_url: String,

    // ── Versioning / identity ───────────────────────────────────────────
    pub version: String,
    pub binary_name: String,
    pub version_extra: String,
    pub mode: String,
    pub debug: bool,
    pub disable_updates: bool,
    pub quiet: bool,

    // ── IDA-specific ────────────────────────────────────────────────────
    pub idausr: Option<String>,
    pub idadir: Option<String>,
    pub hcli_idausr: Option<String>,
    pub current_ida_install_dir: Option<String>,
    pub current_ida_platform: Option<String>,
    pub current_ida_version: Option<String>,
    pub current_ida_python_exe: Option<String>,
}

impl Env {
    /// Return the global `Env` instance, initialising it on first call.
    pub fn global() -> &'static Self {
        ENV.get_or_init(Self::from_environment)
    }

    /// Build an `Env` from the actual process environment.
    fn from_environment() -> Self {
        Self {
            api_key: var_opt("HCLI_API_KEY"),
            api_url: var_or("HCLI_API_URL", "https://api.eu.hex-rays.com"),
            cloud_url: var_or("HCLI_CLOUD_URL", "https://api.hcli.run"),
            portal_url: var_or("HCLI_PORTAL_URL", "https://my.hex-rays.com"),
            release_url: var_or("HCLI_RELEASE_URL", "https://hcli.docs.hex-rays.com"),

            github_token: var_opt("GITHUB_TOKEN").or_else(|| var_opt("GH_TOKEN")),
            github_api_url: var_or("GITHUB_API_URL", "https://api.github.com"),
            github_url: var_or(
                "HCLI_GITHUB_URL",
                "https://github.com/HexRaysSA/ida-hcli",
            ),

            supabase_anon_key: var_or(
                "HCLI_SUPABASE_ANON_KEY",
                "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJpc3MiOiJzdXBhYmFzZSIsInJlZiI6ImF0aGF3ZXRjYW9zb2Zyd29vaXhsIiwicm9sZSI6ImFub24iLCJpYXQiOjE3MjYxNDAxNzYsImV4cCI6MjA0MTcxNjE3Nn0.cOkB4DJ-jeT2aSItfSFsk2C6wtJ2f1UfErWzsf8144o",
            ),
            supabase_url: var_or("HCLI_SUPABASE_URL", "https://auth.hex-rays.com"),

            version: var_or("HCLI_VERSION", env!("CARGO_PKG_VERSION")),
            binary_name: var_or("HCLI_BINARY_NAME", "hcli"),
            version_extra: var_or("HCLI_VERSION_EXTRA", ""),
            mode: var_or("HCLI_MODE", "user"),
            debug: var_bool("HCLI_DEBUG"),
            disable_updates: var_bool("HCLI_DISABLE_UPDATES"),
            quiet: false, // set at runtime via CLI flag

            idausr: var_opt("IDAUSR"),
            idadir: var_opt("IDADIR"),
            hcli_idausr: var_opt("HCLI_IDAUSR"),
            current_ida_install_dir: var_opt("HCLI_CURRENT_IDA_INSTALL_DIR"),
            current_ida_platform: var_opt("HCLI_CURRENT_IDA_PLATFORM"),
            current_ida_version: var_opt("HCLI_CURRENT_IDA_VERSION"),
            current_ida_python_exe: var_opt("HCLI_CURRENT_IDA_PYTHON_EXE"),
        }
    }

    /// Full version string including any extra suffix.
    #[allow(dead_code)]
    pub fn full_version(&self) -> String {
        format!("{}{}", self.version, self.version_extra)
    }

    /// OAuth redirect URL for the local callback server.
    pub fn oauth_redirect_url(&self) -> &'static str {
        "http://localhost:9999/callback"
    }

    /// Port for the local OAuth callback server.
    pub fn oauth_server_port(&self) -> u16 {
        9999
    }
}

// ── helpers ─────────────────────────────────────────────────────────────

fn var_opt(key: &str) -> Option<String> {
    std::env::var(key).ok().filter(|v| !v.is_empty())
}

fn var_or(key: &str, default: &str) -> String {
    std::env::var(key)
        .ok()
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| default.to_owned())
}

fn var_bool(key: &str) -> bool {
    std::env::var(key)
        .ok()
        .map(|v| matches!(v.to_lowercase().as_str(), "true" | "yes" | "on" | "1"))
        .unwrap_or(false)
}
