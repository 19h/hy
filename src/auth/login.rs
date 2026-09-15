//! Blocking OAuth and OTP flows, invoked on the login worker thread.

use std::time::Duration;

use reqwest::Method;

use super::session::SupabaseSession;
use super::{AuthService, credentials::Credentials, gotrue::GoTrueClient, oauth::OAuthServer};
use crate::config::Env;
use crate::error::{Error, Result};

impl AuthService {
    pub fn login_interactive_blocking(
        &mut self,
        name: Option<&str>,
        force: bool,
    ) -> Result<Credentials> {
        let env = Env::global();
        let url = authorization_url(&env.supabase_url, env.oauth_redirect_url(), force)?;
        let server = OAuthServer::bind(env.oauth_server_port())?;
        eprintln!("Open this URL in your browser to continue login:\n  {url}");
        let _ = open::that(url.as_str());

        let tokens = server
            .run(Duration::from_secs(120))?
            .ok_or_else(|| Error::OAuthFailed("Login timeout or cancelled".into()))?;
        let email = GoTrueClient::new()?.user_email(&tokens.access_token)?;
        let session = SupabaseSession {
            access_token: tokens.access_token,
            refresh_token: tokens.refresh_token.unwrap_or_default(),
            expires_in: None,
            expires_at: None,
            token_type: None,
            provider_token: None,
            provider_refresh_token: None,
            user: Some(serde_json::json!({"email": email})),
        };
        self.upsert_interactive(&email, &session.access_token, name, Some(&session), None)
    }

    pub fn send_otp(&self, email: &str) -> Result<()> {
        let response = GoTrueClient::new()?
            .request(Method::POST, "otp", None)?
            .json(&serde_json::json!({"email": email}))
            .send()?;
        if !response.status().is_success() {
            return Err(Error::Authentication(format!(
                "failed to send OTP ({})",
                response.status()
            )));
        }
        Ok(())
    }

    pub fn verify_otp(
        &mut self,
        email: &str,
        otp: &str,
        name: Option<&str>,
    ) -> Result<Credentials> {
        let client = GoTrueClient::new()?;
        let response = client
            .request(Method::POST, "verify", None)?
            .json(&serde_json::json!({"email": email, "token": otp, "type": "email"}))
            .send()?;
        if !response.status().is_success() {
            return Err(Error::Authentication(format!(
                "OTP verification failed ({})",
                response.status()
            )));
        }
        let mut session: SupabaseSession = response
            .json()
            .map_err(|_| Error::Authentication("invalid OTP verification response".into()))?;
        let verified_email = client.user_email(&session.access_token)?;
        session.user = Some(serde_json::json!({"email": verified_email}));
        self.upsert_interactive(email, &session.access_token, name, Some(&session), Some(email))
    }
}

fn authorization_url(base: &str, redirect: &str, force: bool) -> Result<url::Url> {
    let mut url = url::Url::parse(&format!("{}/auth/v1/authorize", base.trim_end_matches('/')))
        .map_err(|error| Error::OAuthFailed(error.to_string()))?;
    url.query_pairs_mut().append_pair("provider", "google").append_pair("redirect_to", redirect);
    if force {
        url.query_pairs_mut().append_pair("prompt", "login");
    }
    Ok(url)
}

#[cfg(test)]
mod tests {
    #[test]
    fn oauth_authorization_preserves_redirect_parameters_and_forced_account_selection() {
        let redirect = "http://localhost:9999/callback?next=a&state=with space";
        for force in [false, true] {
            let url =
                super::authorization_url("https://auth.example.test/", redirect, force).unwrap();
            assert_eq!(url.path(), "/auth/v1/authorize");
            let query: std::collections::BTreeMap<_, _> = url.query_pairs().collect();
            assert_eq!(query.get("provider").map(|value| value.as_ref()), Some("google"));
            assert_eq!(query.get("redirect_to").map(|value| value.as_ref()), Some(redirect));
            assert_eq!(query.get("prompt").map(|value| value.as_ref()), force.then_some("login"));
            assert_eq!(
                query.len(),
                if force {
                    3
                } else {
                    2
                }
            );
        }
    }
}
