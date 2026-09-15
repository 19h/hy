//! Supabase session management: token refresh and JWT decoding.
//!
//! Earlier SDK-based clients stored a full GoTrue session under the config key
//! `"supabase.auth.token"` (as a JSON string).  This module reads that
//! session, refreshes the access token when expired, and keeps the config
//! in sync so both the Python and Rust tools can coexist.

use serde::{Deserialize, Serialize};

use crate::config::{ConfigStore, Env};
use crate::error::{Error, Result};

/// Config store key used by the Supabase GoTrue SDK.
pub(super) const SUPABASE_SESSION_KEY: &str = "supabase.auth.token";

/// Seconds before expiry at which we proactively refresh (matches GoTrue EXPIRY_MARGIN).
const EXPIRY_MARGIN: i64 = 10;

// ── Session model ──────────────────────────────────────────────────────

/// GoTrue token response and legacy SDK session. Expiry metadata is optional.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SupabaseSession {
    pub access_token: String,
    #[serde(default)]
    pub refresh_token: String,
    pub expires_in: Option<i64>,
    pub expires_at: Option<i64>,
    pub token_type: Option<String>,
    #[serde(default)]
    pub provider_token: Option<String>,
    #[serde(default)]
    pub provider_refresh_token: Option<String>,
    #[serde(default)]
    pub user: Option<serde_json::Value>,
}

// ── JWT helpers ────────────────────────────────────────────────────────

/// Minimally decode a JWT payload without signature verification.
/// We only need claims like `exp` and `email`.
fn decode_jwt_payload(token: &str) -> Option<serde_json::Value> {
    use base64::prelude::*;

    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3 {
        return None;
    }
    let payload = parts[1];
    // JWT uses base64url (no padding).
    let bytes = BASE64_URL_SAFE_NO_PAD.decode(payload).ok()?;
    serde_json::from_slice(&bytes).ok()
}

/// Extract the `email` claim from a JWT access token.
pub fn email_from_jwt(token: &str) -> Option<String> {
    let claims = decode_jwt_payload(token)?;
    claims.get("email")?.as_str().map(String::from)
}

/// Check whether a JWT is expired (or will expire within EXPIRY_MARGIN seconds).
pub fn is_token_expired(token: &str) -> bool {
    let Some(claims) = decode_jwt_payload(token) else {
        return true; // can't decode → treat as expired
    };
    let Some(exp) = claims.get("exp").and_then(|v| v.as_i64()) else {
        return true;
    };
    let now = chrono::Utc::now().timestamp();
    exp <= now + EXPIRY_MARGIN
}

// ── Session persistence ────────────────────────────────────────────────

/// Load the Supabase session from the config store.
pub fn load_session() -> Result<Option<SupabaseSession>> {
    let store = ConfigStore::global();
    let Some(value) = store.get_value(SUPABASE_SESSION_KEY) else {
        return Ok(None);
    };
    if value.is_null() {
        return Ok(None);
    }
    let raw = value.as_str().ok_or_else(|| {
        Error::Authentication("stored Supabase session must be a JSON string".into())
    })?;
    serde_json::from_str(raw)
        .map(Some)
        .map_err(|error| Error::Authentication(format!("invalid stored Supabase session: {error}")))
}

/// Encode a session for an atomic commit alongside its credential record.
pub fn config_entry(session: &SupabaseSession) -> Result<(String, serde_json::Value)> {
    Ok((SUPABASE_SESSION_KEY.into(), serde_json::Value::String(serde_json::to_string(session)?)))
}

// ── Token refresh ──────────────────────────────────────────────────────

/// Ensure the current access token is fresh.  If expired, refresh it using
/// the Supabase `/auth/v1/token?grant_type=refresh_token` endpoint.
///
/// The caller persists the returned session and credential together.
pub fn ensure_fresh_session(expected_email: &str) -> Result<SupabaseSession> {
    let session = load_session()?.ok_or_else(|| {
        Error::Authentication("No Supabase session found. Run `hy login` first.".into())
    })?;
    verify_session_email(&session.access_token, expected_email)?;

    if is_token_expired(&session.access_token) {
        let refreshed = refresh_token(&session.refresh_token)?;
        verify_session_email(&refreshed.access_token, expected_email)?;
        Ok(refreshed)
    } else {
        Ok(session)
    }
}

fn verify_session_email(token: &str, expected: &str) -> Result<()> {
    if email_from_jwt(token).is_some_and(|email| email != expected) {
        return Err(Error::Authentication(
            "Supabase session belongs to a different credential; log in to the selected account"
                .into(),
        ));
    }
    Ok(())
}

/// Call the Supabase refresh endpoint.
fn refresh_token(refresh_tok: &str) -> Result<SupabaseSession> {
    let env = Env::global();
    let url = format!("{}/auth/v1/token?grant_type=refresh_token", env.supabase_url);

    let client =
        reqwest::blocking::Client::builder().timeout(std::time::Duration::from_secs(15)).build()?;

    let resp = client
        .post(&url)
        .header("Content-Type", "application/json;charset=UTF-8")
        .header("apiKey", &env.supabase_anon_key)
        .header("Authorization", format!("Bearer {}", env.supabase_anon_key))
        .header("X-Supabase-Api-Version", "2024-01-01")
        .json(&serde_json::json!({ "refresh_token": refresh_tok }))
        .send()?;

    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().unwrap_or_default();
        return Err(Error::Authentication(format!("Token refresh failed ({status}): {body}")));
    }

    let session: SupabaseSession = resp.json()?;
    Ok(session)
}
