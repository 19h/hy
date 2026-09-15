//! Authenticated async HTTP client for the Hex-Rays API.

use reqwest::{Client, header};
use serde::de::DeserializeOwned;

use super::response::Response;
use crate::config::Env;
use crate::error::Result;

/// Reusable API client wrapping [`reqwest::Client`].
#[derive(Clone)]
pub struct ApiClient {
    pub(super) inner: Client,
    pub(super) cookies: super::session::Cookies,
    base_url: String,
}

impl std::fmt::Debug for ApiClient {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ApiClient")
            .field("base_url", &self.base_url)
            .finish_non_exhaustive()
    }
}

impl ApiClient {
    /// Create a new client targeting the configured API URL.
    pub fn new() -> Result<Self> {
        let env = Env::global();
        Ok(Self {
            inner: http_client()?,
            cookies: super::session::shared_cookies(),
            base_url: env.api_url.clone(),
        })
    }

    // ── header injection ────────────────────────────────────────────────

    pub(super) async fn auth_headers(&self) -> Result<header::HeaderMap> {
        let mut headers = crate::auth::request_headers(true).await?;
        headers.insert(header::CONTENT_TYPE, header::HeaderValue::from_static("application/json"));
        headers.insert(header::ACCEPT, header::HeaderValue::from_static("application/json"));
        Ok(headers)
    }

    // ── response handling ───────────────────────────────────────────────

    pub(super) async fn handle(response: Response) -> Result<Response> {
        let status = response.status().as_u16();
        if status >= 400 {
            let bytes = response.bytes().await?;
            return Err(super::json::status_error(status, &bytes));
        }
        Ok(response)
    }

    // ── JSON helpers ────────────────────────────────────────────────────

    async fn get_response(&self, path: &str) -> Result<Response> {
        let url = format!("{}{path}", self.base_url);
        let headers = self.auth_headers().await?;
        self.send(self.inner.get(&url).headers(headers).build()?).await
    }

    pub async fn get_json<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        let resp = Self::handle(self.get_response(path).await?).await?;
        super::json::decode(&resp.bytes().await?)
    }

    pub async fn post_json<T: DeserializeOwned>(
        &self,
        path: &str,
        body: &impl serde::Serialize,
    ) -> Result<T> {
        let url = format!("{}{path}", self.base_url);
        let headers = self.auth_headers().await?;
        let resp = self.send(self.inner.post(&url).headers(headers).json(body).build()?).await?;
        let resp = Self::handle(resp).await?;
        super::json::decode(&resp.bytes().await?)
    }

    pub async fn delete_json<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        let url = format!("{}{path}", self.base_url);
        let headers = self.auth_headers().await?;
        let resp = self.send(self.inner.delete(&url).headers(headers).build()?).await?;
        let resp = Self::handle(resp).await?;
        super::json::decode(&resp.bytes().await?)
    }

    // ── standalone key validation ───────────────────────────────────────

    /// Validate an API key by calling `/api/whoami` with it directly.
    /// Returns the user's email on success, or an error if the key is invalid.
    pub async fn validate_api_key(key: &str) -> Result<String> {
        let env = Env::global();
        let client = http_client()?;
        let resp = client
            .get(format!("{}/api/whoami", env.api_url))
            .header("x-api-key", key)
            .header(header::ACCEPT, "application/json")
            .header(header::CONTENT_TYPE, "application/json")
            .send()
            .await?;
        let resp = Response::new(resp);
        let status = resp.status().as_u16();
        if status >= 400 {
            let bytes = resp.bytes().await?;
            return Err(super::json::status_error(status, &bytes));
        }
        let user: crate::api::AuthUser = super::json::decode(&resp.bytes().await?)?;
        Ok(user.email)
    }
}

fn http_client() -> Result<Client> {
    Ok(Client::builder()
        .default_headers(super::response::default_headers())
        .user_agent(format!("hcli/{}", Env::global().version))
        .connect_timeout(std::time::Duration::from_secs(60))
        .read_timeout(std::time::Duration::from_secs(60))
        .redirect(reqwest::redirect::Policy::none())
        // The shared decoder owns content decoding and retains wire headers,
        // even if another dependency enables reqwest's optional codec features.
        .no_gzip()
        .no_deflate()
        .no_brotli()
        .no_zstd()
        .build()?)
}
