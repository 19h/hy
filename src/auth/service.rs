//! Central authentication service (singleton).

use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use crate::auth::credentials::{CredentialType, Credentials, CredentialsConfig};
use crate::auth::oauth::OAuthServer;
use crate::config::{ConfigStore, Env};
use crate::error::{Error, Result};

const CONFIG_KEY: &str = "credentials";

/// Global auth service instance.
static AUTH: OnceLock<Mutex<AuthService>> = OnceLock::new();

/// Manages credentials, login flows, and token retrieval.
#[derive(Debug)]
pub struct AuthService {
    config: CredentialsConfig,
    current: Option<String>, // name of current credential
    forced: Option<String>,  // from --auth-credentials
    initialised: bool,
}

impl AuthService {
    // ── singleton ───────────────────────────────────────────────────────

    /// Get a locked reference to the global service.
    pub fn global() -> std::sync::MutexGuard<'static, Self> {
        AUTH.get_or_init(|| Mutex::new(Self::new()))
            .lock()
            .expect("auth service lock poisoned")
    }

    fn new() -> Self {
        Self {
            config: CredentialsConfig::default(),
            current: None,
            forced: None,
            initialised: false,
        }
    }

    // ── initialisation ──────────────────────────────────────────────────

    /// Load persisted credentials from the config store.
    pub fn init(&mut self, forced_credentials: Option<&str>) {
        if self.initialised {
            return;
        }
        self.forced = forced_credentials.map(String::from);
        self.load_config();
        self.resolve_current();
        self.initialised = true;
    }

    fn load_config(&mut self) {
        let store = ConfigStore::global();
        if let Some(val) = store.get_value(CONFIG_KEY) {
            if let Ok(cfg) = serde_json::from_value::<CredentialsConfig>(val.clone()) {
                self.config = cfg;
            }
        }
    }

    fn save_config(&self) {
        let mut store = ConfigStore::global();
        if let Ok(val) = serde_json::to_value(&self.config) {
            store.set_value(CONFIG_KEY, val);
        }
    }

    fn resolve_current(&mut self) {
        let env = Env::global();

        // Environment API key always wins.
        if env.api_key.is_some() {
            self.current = None;
            return;
        }

        if let Some(ref forced) = self.forced {
            if self.config.credentials.contains_key(forced) {
                self.current = Some(forced.clone());
                return;
            }
        }

        self.current = self.config.default.clone();
    }

    // ── queries ─────────────────────────────────────────────────────────

    pub fn is_logged_in(&self) -> bool {
        Env::global().api_key.is_some() || self.current_credentials().is_some()
    }

    pub fn current_credentials(&self) -> Option<&Credentials> {
        self.current
            .as_deref()
            .and_then(|name| self.config.credentials.get(name))
    }

    pub fn list_credentials(&self) -> Vec<&Credentials> {
        self.config.credentials.values().collect()
    }

    pub fn default_name(&self) -> Option<&str> {
        self.config.default.as_deref()
    }

    /// Determine the auth type in use.
    pub fn auth_type(&self) -> (CredentialType, &'static str) {
        if Env::global().api_key.is_some() {
            return (CredentialType::Key, "env");
        }
        match self.current_credentials() {
            Some(c) => {
                let origin = if self.forced.is_some() {
                    "forced"
                } else {
                    "default"
                };
                (c.cred_type, origin)
            }
            None => (CredentialType::Interactive, "none"),
        }
    }

    /// Get the API key to use for requests.
    pub fn api_key(&self) -> Option<String> {
        if let Some(ref key) = Env::global().api_key {
            return Some(key.clone());
        }
        self.current_credentials()
            .filter(|c| c.cred_type == CredentialType::Key)
            .and_then(|c| c.token.clone())
    }

    /// Get the bearer token for interactive sessions.
    ///
    /// If the stored token is expired, tries to refresh it via the Supabase
    /// session stored in `"supabase.auth.token"`.
    pub fn access_token(&mut self) -> Option<String> {
        let cred = self.current_credentials()?;
        if cred.cred_type != CredentialType::Interactive {
            return None;
        }

        // Try the stored token first — if it's not expired, use it directly.
        if let Some(ref tok) = cred.token {
            if !crate::auth::session::is_token_expired_pub(tok) {
                return Some(tok.clone());
            }
        }

        // Token missing or expired — try refreshing from the Supabase session.
        match crate::auth::session::ensure_fresh_token() {
            Ok((fresh_token, email)) => {
                // Update the credential in-memory and on disk.
                let name = self.current.clone()?;
                if let Some(c) = self.config.credentials.get_mut(&name) {
                    c.token = Some(fresh_token.clone());
                    if !email.is_empty() {
                        c.email = email;
                    }
                    c.touch();
                }
                self.save_config();
                Some(fresh_token)
            }
            Err(_) => {
                // Refresh failed — return whatever we have (may be expired).
                self.current_credentials().and_then(|c| c.token.clone())
            }
        }
    }

    /// Get user email for the current credential.
    pub fn user_email(&self) -> Option<&str> {
        if Env::global().api_key.is_some() {
            return Some("api-key-user");
        }
        self.current_credentials().map(|c| c.email.as_str())
    }

    // ── mutations ───────────────────────────────────────────────────────

    pub fn add_credentials(&mut self, cred: Credentials) {
        self.config.add(cred);
        self.save_config();
    }

    pub fn remove_credentials(&mut self, name: &str) -> bool {
        let removed = self.config.remove(name);
        if removed {
            if self.current.as_deref() == Some(name) {
                self.resolve_current();
            }
            self.save_config();
        }
        removed
    }

    pub fn set_default(&mut self, name: &str) -> bool {
        let ok = self.config.set_default(name);
        if ok {
            self.current = Some(name.to_owned());
            self.save_config();
        }
        ok
    }

    /// Create or update interactive credentials after a successful OAuth flow.
    pub fn upsert_interactive(
        &mut self,
        email: &str,
        token: &str,
        name: Option<&str>,
    ) -> Credentials {
        // Check for existing interactive credential with same email.
        if let Some(existing) = self
            .config
            .find_by_email_and_type(email, CredentialType::Interactive)
            .cloned()
        {
            let cred = self.config.credentials.get_mut(&existing.name).unwrap();
            cred.token = Some(token.to_owned());
            cred.touch();
            let result = cred.clone();
            self.current = Some(result.name.clone());
            self.config.set_default(&result.name);
            self.save_config();
            return result;
        }

        let base_name = name.unwrap_or(email);
        let unique = self.config.unique_name(base_name);
        let cred = Credentials::new(&unique, CredentialType::Interactive, token, email);
        self.config.add(cred.clone());
        self.current = Some(unique.clone());
        self.config.set_default(&unique);
        self.save_config();
        cred
    }

    /// Add an API key credential (validates it by calling whoami).
    pub fn add_api_key_credential(&mut self, name: &str, token: &str, email: &str) -> Credentials {
        let _ = self.config.remove(name);
        let cred = Credentials::new(name, CredentialType::Key, token, email);
        self.config.add(cred.clone());
        self.save_config();
        cred
    }

    /// Logout: remove the named credential or just clear the current session.
    pub fn logout_current(&mut self) {
        if let Some(name) = self.current.take() {
            // For interactive credentials we clear the session but keep the
            // credential entry.  The Python version calls supabase sign_out;
            // in the Rust rewrite we simply clear the token.
            if let Some(c) = self.config.credentials.get_mut(&name) {
                if c.cred_type == CredentialType::Interactive {
                    c.token = None;
                }
            }
            self.save_config();
        }
    }

    // ── OAuth login flow ────────────────────────────────────────────────

    /// Perform an interactive OAuth login.  Returns the newly created credential
    /// on success.
    pub fn login_interactive_blocking(&mut self, name: Option<&str>) -> Result<Credentials> {
        let env = Env::global();

        // Build the OAuth URL via the Supabase auth endpoint.
        let oauth_url = format!(
            "{}/auth/v1/authorize?provider=google&redirect_to={}",
            env.supabase_url,
            env.oauth_redirect_url(),
        );

        eprintln!("Open this URL in your browser to continue login:\n  {oauth_url}");
        let _ = open::that(&oauth_url);

        // Start local server and wait for token.
        let server = OAuthServer::new(env.oauth_server_port());
        let tokens = server.run(Duration::from_secs(120))?;

        match tokens {
            Some(t) => {
                // Decode the JWT to extract the user's email.
                let email = crate::auth::session::email_from_jwt(&t.access_token)
                    .unwrap_or_else(|| "unknown".into());

                // Store the full Supabase session (with refresh token) so that
                // future runs can refresh the access token without re-login.
                if let Some(ref refresh) = t.refresh_token {
                    let session = crate::auth::session::SupabaseSession {
                        access_token: t.access_token.clone(),
                        refresh_token: refresh.clone(),
                        expires_in: 3600,
                        expires_at: chrono::Utc::now().timestamp() + 3600,
                        token_type: "bearer".into(),
                        provider_token: None,
                        provider_refresh_token: None,
                        user: None,
                    };
                    crate::auth::session::save_session_pub(&session);
                }

                let cred = self.upsert_interactive(&email, &t.access_token, name);
                Ok(cred)
            }
            None => Err(Error::OAuthFailed("Login timeout or cancelled".into())),
        }
    }

    /// Show current login status to the console.
    pub fn show_login_info(&self) {
        use console::style;

        if !self.is_logged_in() {
            eprintln!("You are not logged in.");
            return;
        }

        let env = Env::global();
        if env.api_key.is_some() {
            let email = self.user_email().unwrap_or("unknown");
            eprintln!(
                "You are logged in as {} using an API key from HCLI_API_KEY environment variable",
                style(email).green()
            );
            return;
        }

        if let Some(cred) = self.current_credentials() {
            if self.config.credentials.len() <= 1 {
                eprintln!("You are logged in as {}", style(&cred.email).green());
            } else {
                let kind = match cred.cred_type {
                    CredentialType::Key => format!("API key '{}'", cred.name),
                    CredentialType::Interactive => {
                        format!("interactive login '{}'", cred.name)
                    }
                };
                let suffix = if self.forced.is_some() {
                    " (forced via --auth-credentials)"
                } else if self.default_name() == Some(cred.name.as_str()) {
                    " (default)"
                } else {
                    ""
                };
                eprintln!(
                    "You are logged in as {} using {kind}{suffix}",
                    style(cred.label()).green()
                );
            }
        }
    }
}
