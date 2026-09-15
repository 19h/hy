//! Present validated authentication state without exposing credential tokens.

use super::{AuthService, credentials::CredentialType};
use crate::config::Env;
use crate::error::{Error, Result};

/// Display status for upstream's synchronous commands. Async login and key
/// installation use AuthService's local presentation and environment placeholder.
pub async fn show_resolved_login_info() -> Result<()> {
    let environment_key = Env::global().api_key.clone();
    let has_environment_key = environment_key.is_some();
    tokio::task::spawn_blocking(move || {
        let mut auth = AuthService::global();
        auth.init(None)?;
        if !has_environment_key {
            auth.show_login_info();
        }
        Ok::<_, Error>(())
    })
    .await
    .map_err(|error| Error::Authentication(format!("authentication worker failed: {error}")))??;

    if let Some(key) = environment_key {
        // The standalone request runs after releasing the auth mutex.
        let email = crate::api::ApiClient::validate_api_key(&key)
            .await
            .unwrap_or_else(|_| "api-key-user".into());
        AuthService::show_environment_login_info(&email);
    }
    Ok(())
}

impl AuthService {
    /// Show current login status to the console.
    pub fn show_login_info(&mut self) {
        use console::style;

        if !self.is_logged_in() {
            eprintln!("You are not logged in.");
            return;
        }

        let env = Env::global();
        if env.api_key.is_some() {
            // Async upstream login/switch/install flows use this placeholder.
            Self::show_environment_login_info("api-key-user");
            return;
        }

        if let Some(cred) = self.current_credentials() {
            if self.list_credentials().len() <= 1 {
                eprintln!("You are logged in as {}", style(&cred.email).green());
            } else {
                let kind = match cred.cred_type {
                    CredentialType::Key => format!("API key '{}'", cred.name),
                    CredentialType::Interactive => {
                        format!("interactive login '{}'", cred.name)
                    }
                };
                let suffix = if self.forced_name().is_some() {
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

    fn show_environment_login_info(email: &str) {
        eprintln!(
            "You are logged in as {} using an API key from HCLI_API_KEY environment variable",
            console::style(email).green()
        );
    }
}
