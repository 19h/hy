//! Blocking requests to the upstream GoTrue authentication service.

use std::time::Duration;

use reqwest::Method;
use reqwest::blocking::{Client, RequestBuilder};
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderValue};
use serde::Deserialize;

use crate::config::Env;
use crate::error::{Error, Result};

pub(super) struct GoTrueClient {
    client: Client,
    base_url: String,
    headers: HeaderMap,
}

#[derive(Deserialize)]
struct User {
    email: Option<String>,
}

impl GoTrueClient {
    pub fn new() -> Result<Self> {
        let env = Env::global();
        let mut headers = HeaderMap::new();
        headers.insert(
            "apikey",
            HeaderValue::from_str(&env.supabase_anon_key)
                .map_err(|_| Error::Authentication("invalid GoTrue API key header".into()))?,
        );
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        Ok(Self {
            client: Client::builder()
                .timeout(Duration::from_secs(15))
                .redirect(reqwest::redirect::Policy::none())
                .build()?,
            base_url: format!("{}/auth/v1", env.supabase_url.trim_end_matches('/')),
            headers,
        })
    }

    pub fn request(
        &self,
        method: Method,
        endpoint: &str,
        token: Option<&str>,
    ) -> Result<RequestBuilder> {
        let token =
            token.filter(|token| !token.is_empty()).unwrap_or(&Env::global().supabase_anon_key);
        let bearer = HeaderValue::from_str(&format!("Bearer {token}"))
            .map_err(|_| Error::Authentication("invalid bearer token header".into()))?;
        Ok(self
            .client
            .request(method, format!("{}/{endpoint}", self.base_url))
            .headers(self.headers.clone())
            .header(AUTHORIZATION, bearer))
    }

    /// The server response establishes validity; decoded JWT claims do not.
    pub fn user_email(&self, token: &str) -> Result<String> {
        if token.is_empty() {
            return Err(Error::Authentication("empty interactive token".into()));
        }
        let response = self.request(Method::GET, "user", Some(token))?.send()?;
        if !response.status().is_success() {
            return Err(Error::Authentication(format!(
                "GoTrue user validation failed ({}); run `hy login`",
                response.status()
            )));
        }
        let user: User = response
            .json()
            .map_err(|_| Error::Authentication("invalid GoTrue user response".into()))?;
        user.email
            .filter(|email| !email.is_empty())
            .ok_or_else(|| Error::Authentication("GoTrue user response has no email".into()))
    }
}
