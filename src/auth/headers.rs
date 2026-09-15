//! Resolve credentials outside async workers and validate HTTP header values.

use reqwest::header::{AUTHORIZATION, HeaderMap, HeaderValue};

use super::AuthService;
use crate::error::{Error, Result};

pub async fn request_headers(required: bool) -> Result<HeaderMap> {
    tokio::task::spawn_blocking(move || build_headers(required))
        .await
        .map_err(|error| Error::Authentication(format!("authentication worker failed: {error}")))?
}

fn build_headers(required: bool) -> Result<HeaderMap> {
    let mut auth = AuthService::global();
    let mut headers = HeaderMap::new();
    if let Some(key) = auth.api_key() {
        let value = HeaderValue::from_str(&key)
            .map_err(|_| Error::Authentication("invalid API key header".into()))?;
        headers.insert("x-api-key", value);
    } else if let Some(token) = match auth.access_token() {
        Ok(token) => token,
        Err(error) if !required => {
            tracing::debug!(%error, "interactive credentials unavailable for optional request");
            None
        }
        Err(error) => return Err(error),
    } {
        let value = HeaderValue::from_str(&format!("Bearer {token}"))
            .map_err(|_| Error::Authentication("invalid bearer token header".into()))?;
        headers.insert(AUTHORIZATION, value);
    }
    if required && headers.is_empty() {
        return Err(Error::NotLoggedIn);
    }
    Ok(headers)
}
