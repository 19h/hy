//! Central authentication service (singleton).

use std::sync::{Mutex, OnceLock};

use crate::auth::credentials::{CredentialType, Credentials, CredentialsConfig};
use crate::config::{ConfigStore, Env};
use crate::error::{Error, Result};

fn config_key() -> String {
    format!("{}.credentials", Env::global().config_namespace)
}

/// Global auth service instance.
static AUTH: OnceLock<Mutex<AuthService>> = OnceLock::new();

/// Manages credentials, login flows, and token retrieval.
#[derive(Debug)]
pub struct AuthService {
    config: CredentialsConfig,
    current: Option<String>, // name of current credential
    forced: Option<String>,  // from --auth-credentials
    initialised: bool,
    validated_token: Option<String>,
}

impl AuthService {
    // ── singleton ───────────────────────────────────────────────────────

    /// Get a locked reference to the global service.
    pub fn global() -> std::sync::MutexGuard<'static, Self> {
        AUTH.get_or_init(|| Mutex::new(Self::new())).lock().expect("auth service lock poisoned")
    }

    fn new() -> Self {
        Self {
            config: CredentialsConfig::default(),
            current: None,
            forced: None,
            initialised: false,
            validated_token: None,
        }
    }

    // ── initialisation ──────────────────────────────────────────────────

    /// Load persisted credentials from the config store.
    pub fn init(&mut self, forced_credentials: Option<&str>) -> Result<()> {
        if self.initialised {
            return Ok(());
        }
        self.load_config()?;
        self.forced = forced_credentials.map(String::from);
        self.resolve_current();
        self.initialised = true;
        Ok(())
    }

    fn load_config(&mut self) -> Result<()> {
        let store = ConfigStore::global();
        if let Some(value) = store.get_value(&config_key()).filter(|value| !value.is_null()) {
            if !value.is_object() {
                return Err(Error::Authentication(
                    "invalid stored credentials: expected an object".into(),
                ));
            }
            self.config = serde_json::from_value(value.clone()).map_err(|error| {
                Error::Authentication(format!("invalid stored credentials: {error}"))
            })?;
        }
        Ok(())
    }

    fn commit_config(&mut self, candidate: CredentialsConfig) -> Result<()> {
        self.commit_state(candidate, None, None)
    }

    fn commit_state(
        &mut self,
        candidate: CredentialsConfig,
        session: Option<&super::session::SupabaseSession>,
        login_email: Option<&str>,
    ) -> Result<()> {
        let mut values = vec![(config_key(), Some(serde_json::to_value(&candidate)?))];
        if let Some(session) = session {
            let (key, value) = super::session::config_entry(session)?;
            values.push((key, Some(value)));
        } else if self.removes_legacy_session(&candidate) {
            values.push((super::session::SUPABASE_SESSION_KEY.into(), None));
        }
        if let Some(email) = login_email {
            values.push((
                format!("{}.login.email", Env::global().config_namespace),
                Some(serde_json::json!(email)),
            ));
        }
        ConfigStore::global().commit_changes(values)?;
        self.config = candidate;
        Ok(())
    }

    fn removes_legacy_session(&self, candidate: &CredentialsConfig) -> bool {
        let removed: Vec<_> =
            self.config
                .credentials
                .values()
                .filter(|credential| {
                    credential.cred_type == CredentialType::Interactive
                        && candidate.credentials.get(&credential.name).is_none_or(|retained| {
                            retained.cred_type != CredentialType::Interactive
                        })
                })
                .collect();
        if removed.is_empty() {
            return false;
        }
        if !candidate
            .credentials
            .values()
            .any(|credential| credential.cred_type == CredentialType::Interactive)
        {
            return true;
        }
        let Ok(Some(session)) = super::session::load_session() else {
            return false;
        };
        let email = super::session::email_from_jwt(&session.access_token);
        removed.iter().any(|credential| {
            credential.token.as_deref() == Some(session.access_token.as_str())
                || email.as_deref() == Some(credential.email.as_str())
        })
    }

    fn resolve_current(&mut self) {
        self.validated_token = None;
        let env = Env::global();

        // Environment API key always wins.
        if env.api_key.is_some() {
            self.current = None;
            return;
        }

        if let Some(ref forced) = self.forced
            && self.config.credentials.contains_key(forced)
        {
            self.current = Some(forced.clone());
            return;
        }

        self.current = self.config.default.clone();
    }

    // ── queries ─────────────────────────────────────────────────────────

    /// May perform blocking token validation; call from a blocking worker.
    pub fn is_logged_in(&mut self) -> bool {
        self.api_key().is_some_and(|key| !key.is_empty())
            || self.access_token().ok().flatten().is_some()
    }

    pub fn current_credentials(&self) -> Option<&Credentials> {
        self.current.as_deref().and_then(|name| self.config.credentials.get(name))
    }

    pub fn list_credentials(&self) -> Vec<&Credentials> {
        self.config.credentials.values().collect()
    }

    pub fn default_name(&self) -> Option<&str> {
        self.config.default.as_deref()
    }

    pub(super) fn forced_name(&self) -> Option<&str> {
        self.forced.as_deref()
    }

    /// Get the API key to use for requests.
    pub fn api_key(&self) -> Option<String> {
        if let Some(ref key) = Env::global().api_key {
            return Some(key.clone());
        }
        self.current_credentials()
            .filter(|c| c.cred_type == CredentialType::Key)
            .and_then(|c| c.token.clone())
            .filter(|token| !token.is_empty())
    }

    /// Get the bearer token for interactive sessions.
    ///
    /// If the stored token is expired, tries to refresh it via the Supabase
    /// session stored in `"supabase.auth.token"`.
    pub fn access_token(&mut self) -> Result<Option<String>> {
        let Some(cred) = self.current_credentials() else {
            return Ok(None);
        };
        if cred.cred_type != CredentialType::Interactive {
            return Ok(None);
        }

        let Some(token) = cred.token.as_deref().filter(|token| !token.is_empty()) else {
            return Ok(None);
        };
        if self.validated_token.as_deref() == Some(token) {
            return Ok(Some(token.to_owned()));
        }
        // GoTrue is authoritative even for opaque or expired-looking tokens.
        // The legacy refresh extension runs only after the server rejects one.
        let validation_error = match super::gotrue::GoTrueClient::new()?.user_email(token) {
            Ok(_) => {
                self.validated_token = Some(token.to_owned());
                return Ok(self.validated_token.clone());
            }
            Err(error) => error,
        };
        if !crate::auth::session::is_token_expired(token)
            || crate::auth::session::load_session()?.is_none()
        {
            return Err(validation_error);
        }

        // Token missing or expired — try refreshing from the Supabase session.
        let name = cred.name.clone();
        let session = crate::auth::session::ensure_fresh_session(&cred.email)?;
        let fresh_token = session.access_token.clone();
        super::gotrue::GoTrueClient::new()?.user_email(&fresh_token)?;
        let mut candidate = self.config.clone();
        if let Some(credential) = candidate.credentials.get_mut(&name) {
            credential.token = Some(fresh_token.clone());
            credential.touch();
        }
        self.commit_state(candidate, Some(&session), None)?;
        self.validated_token = Some(fresh_token.clone());
        Ok(Some(fresh_token))
    }

    /// Upstream get_user touches managed credentials when resolving their identity.
    /// In async command contexts an environment key uses upstream's placeholder.
    pub fn get_user_email(&mut self) -> Result<Option<String>> {
        if Env::global().api_key.is_some() {
            return Ok(Some("api-key-user".into()));
        }
        let Some(credential) = self.current_credentials().cloned() else {
            return Ok(None);
        };
        let mut candidate = self.config.clone();
        candidate.credentials.get_mut(&credential.name).expect("selected credential").touch();
        self.commit_config(candidate)?;
        Ok(Some(credential.email))
    }

    // ── mutations ───────────────────────────────────────────────────────

    pub fn remove_credentials(&mut self, name: &str) -> Result<bool> {
        let mut candidate = self.config.clone();
        if !candidate.remove(name) {
            return Ok(false);
        }
        self.commit_config(candidate)?;
        self.resolve_current();
        Ok(true)
    }

    pub fn remove_all_credentials(&mut self) -> Result<usize> {
        let count = self.config.credentials.len();
        self.commit_config(CredentialsConfig::default())?;
        self.resolve_current();
        Ok(count)
    }

    pub fn set_default(&mut self, name: &str) -> Result<bool> {
        let mut candidate = self.config.clone();
        if !candidate.set_default(name) {
            return Ok(false);
        }
        self.commit_config(candidate)?;
        self.resolve_current();
        Ok(true)
    }

    /// Create or update interactive credentials after a successful login.
    pub(super) fn upsert_interactive(
        &mut self,
        email: &str,
        token: &str,
        name: Option<&str>,
        session: Option<&super::session::SupabaseSession>,
        login_email: Option<&str>,
    ) -> Result<Credentials> {
        let mut candidate = self.config.clone();
        let credential = if let Some(existing) =
            candidate.find_by_email_and_type(email, CredentialType::Interactive).cloned()
        {
            let mut credential = existing;
            credential.token = Some(token.to_owned());
            credential.touch();
            credential
        } else {
            let name = candidate.unique_name(name.unwrap_or(email));
            Credentials::new(name, CredentialType::Interactive, token, email)
        };
        candidate.add(credential.clone());
        candidate.set_default(&credential.name);
        self.commit_state(candidate, session, login_email)?;
        self.current = Some(credential.name.clone());
        self.validated_token = Some(token.to_owned());
        Ok(credential)
    }

    /// Persist an API key after the caller has validated it with whoami.
    pub fn add_api_key_credential(
        &mut self,
        name: &str,
        token: &str,
        email: &str,
    ) -> Result<Credentials> {
        let mut candidate = self.config.clone();
        candidate.remove(name);
        let credential = Credentials::new(name, CredentialType::Key, token, email);
        candidate.add(credential.clone());
        self.commit_config(candidate)?;
        self.resolve_current();
        Ok(credential)
    }

    /// End the interactive session while retaining the upstream credential record.
    /// Remote sign-out is best effort; local persistence failures are reported.
    pub fn logout_current(&mut self) -> Result<()> {
        if let Some(credential) = self.current_credentials()
            && credential.cred_type == CredentialType::Interactive
        {
            ConfigStore::global()
                .commit_changes([(super::session::SUPABASE_SESSION_KEY.into(), None)])?;
            if let Some(token) = credential.token.as_deref().filter(|token| !token.is_empty()) {
                let result = super::gotrue::GoTrueClient::new().and_then(|client| {
                    client.request(reqwest::Method::POST, "logout", Some(token))?.send()?;
                    Ok(())
                });
                if let Err(error) = result {
                    tracing::debug!(%error, "GoTrue sign-out unavailable");
                }
            }
        }
        self.validated_token = None;
        Ok(())
    }
}
