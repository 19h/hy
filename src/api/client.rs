//! Authenticated async HTTP client for the Hex-Rays API.

use std::path::{Path, PathBuf};

use indicatif::{ProgressBar, ProgressStyle};
use reqwest::{header, Client, Response};
use serde::de::DeserializeOwned;
use tokio::io::AsyncWriteExt;

use crate::auth::service::AuthService;
use crate::auth::CredentialType;
use crate::config::Env;
use crate::error::{Error, Result};
use crate::util::cache::cache_dir;
use crate::util::io::check_free_space;

/// Reusable API client wrapping [`reqwest::Client`].
#[derive(Debug, Clone)]
pub struct ApiClient {
    inner: Client,
    base_url: String,
}

impl ApiClient {
    /// Create a new client targeting the configured API URL.
    pub fn new() -> Result<Self> {
        let env = Env::global();
        let inner = Client::builder()
            .user_agent(format!("hcli/{}", env.version))
            .timeout(std::time::Duration::from_secs(60))
            .build()?;
        Ok(Self {
            inner,
            base_url: env.api_url.clone(),
        })
    }

    // ── header injection ────────────────────────────────────────────────

    fn auth_headers(&self) -> Result<header::HeaderMap> {
        let mut headers = header::HeaderMap::new();
        headers.insert(header::CONTENT_TYPE, "application/json".parse().unwrap());
        headers.insert(header::ACCEPT, "application/json".parse().unwrap());

        let mut auth = AuthService::global();
        if !auth.is_logged_in() {
            return Err(Error::NotLoggedIn);
        }

        let (cred_type, _) = auth.auth_type();
        match cred_type {
            CredentialType::Interactive => {
                if let Some(token) = auth.access_token() {
                    headers.insert(
                        header::AUTHORIZATION,
                        format!("Bearer {token}").parse().unwrap(),
                    );
                }
            }
            CredentialType::Key => {
                if let Some(key) = auth.api_key() {
                    headers.insert("x-api-key", key.parse::<header::HeaderValue>().unwrap());
                }
            }
        }

        Ok(headers)
    }

    // ── response handling ───────────────────────────────────────────────

    async fn handle(response: Response) -> Result<Response> {
        let status = response.status().as_u16();
        if status >= 400 {
            let body = response.text().await.unwrap_or_default();
            return Err(Error::from_status(status, &body));
        }
        Ok(response)
    }

    // ── JSON helpers ────────────────────────────────────────────────────

    pub async fn get_json<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        let url = format!("{}{path}", self.base_url);
        let headers = self.auth_headers()?;
        let resp = self.inner.get(&url).headers(headers).send().await?;
        let resp = Self::handle(resp).await?;
        let text = resp.text().await?;
        serde_json::from_str(&text).map_err(|e| {
            // Log the first 500 chars of the body for debugging.
            let preview: String = text.chars().take(500).collect();
            tracing::debug!("JSON parse error for {path}: {e}\nBody: {preview}");
            Error::Other(format!("Failed to parse API response for {path}: {e}"))
        })
    }

    pub async fn post_json<T: DeserializeOwned>(
        &self,
        path: &str,
        body: &impl serde::Serialize,
    ) -> Result<T> {
        let url = format!("{}{path}", self.base_url);
        let headers = self.auth_headers()?;
        let resp = self.inner.post(&url).headers(headers).json(body).send().await?;
        let resp = Self::handle(resp).await?;
        Ok(resp.json().await?)
    }

    pub async fn delete_json<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        let url = format!("{}{path}", self.base_url);
        let headers = self.auth_headers()?;
        let resp = self.inner.delete(&url).headers(headers).send().await?;
        let resp = Self::handle(resp).await?;
        Ok(resp.json().await?)
    }

    // ── standalone key validation ───────────────────────────────────────

    /// Validate an API key by calling `/api/whoami` with it directly.
    /// Returns the user's email on success, or an error if the key is invalid.
    pub async fn validate_api_key(key: &str) -> Result<String> {
        let env = Env::global();
        let client = Client::builder()
            .user_agent(format!("hcli/{}", env.version))
            .timeout(std::time::Duration::from_secs(15))
            .build()?;
        let resp = client
            .get(format!("{}/api/whoami", env.api_url))
            .header("x-api-key", key)
            .header(header::ACCEPT, "application/json")
            .send()
            .await?;
        let status = resp.status().as_u16();
        if status >= 400 {
            let body = resp.text().await.unwrap_or_default();
            return Err(Error::from_status(status, &body));
        }
        let user: crate::api::AuthUser = resp.json().await?;
        Ok(user.email)
    }

    // ── file upload ─────────────────────────────────────────────────────

    pub async fn put_file(&self, url: &str, file_path: &Path) -> Result<()> {
        let meta = std::fs::metadata(file_path)?;
        let file_size = meta.len();

        let content_type = match file_path.extension().and_then(|e| e.to_str()) {
            Some("zip") => "application/zip",
            Some("json") => "application/json",
            _ => "application/octet-stream",
        };

        let pb = ProgressBar::new(file_size);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("{msg} [{bar:40}] {bytes}/{total_bytes} ({eta})")
                .unwrap()
                .progress_chars("=> "),
        );
        pb.set_message(format!(
            "Uploading {}",
            file_path.file_name().unwrap_or_default().to_string_lossy()
        ));

        let bytes = std::fs::read(file_path)?;
        pb.set_position(file_size);

        self.inner
            .put(url)
            .header(header::CONTENT_TYPE, content_type)
            .header(header::CONTENT_LENGTH, file_size)
            .body(bytes)
            .send()
            .await?;

        pb.finish_with_message("Upload complete");
        Ok(())
    }

    // ── file download ───────────────────────────────────────────────────

    pub async fn download_file(
        &self,
        url: &str,
        target_dir: &Path,
        target_filename: Option<&str>,
        force: bool,
        auth: bool,
        asset_key: Option<&str>,
    ) -> Result<PathBuf> {
        std::fs::create_dir_all(target_dir)?;

        // Determine filename.
        let filename = target_filename
            .map(String::from)
            .or_else(|| {
                url::Url::parse(url)
                    .ok()
                    .and_then(|u| {
                        u.path_segments()
                            .and_then(|s| s.last().map(String::from))
                    })
            })
            .unwrap_or_else(|| "download".into());

        // Cache path.
        let cache_key = asset_key.unwrap_or(&filename);
        let cache_path = cache_dir("downloads").join(cache_key);
        if let Some(parent) = cache_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let target_path = target_dir.join(&filename);

        // Check cache.
        if cache_path.exists() && !force {
            if let Ok(meta) = std::fs::metadata(&cache_path) {
                // Quick HEAD check for size match.
                let head_ok = if auth {
                    let headers = self.auth_headers().unwrap_or_default();
                    self.inner
                        .head(url)
                        .headers(headers)
                        .send()
                        .await
                        .ok()
                        .and_then(|r| {
                            r.headers()
                                .get(header::CONTENT_LENGTH)
                                .and_then(|v| v.to_str().ok()?.parse::<u64>().ok())
                        })
                } else {
                    self.inner
                        .head(url)
                        .send()
                        .await
                        .ok()
                        .and_then(|r| {
                            r.headers()
                                .get(header::CONTENT_LENGTH)
                                .and_then(|v| v.to_str().ok()?.parse::<u64>().ok())
                        })
                };

                if head_ok == Some(meta.len()) {
                    check_free_space(target_dir, meta.len())?;
                    std::fs::copy(&cache_path, &target_path)?;
                    eprintln!("Using cached file: {}", cache_path.display());
                    return Ok(target_path);
                }
            }
        }

        // Stream download.
        let mut req = self.inner.get(url);
        if auth {
            if let Ok(headers) = self.auth_headers() {
                req = req.headers(headers);
            }
        }
        let resp = req.send().await?;
        let resp = Self::handle(resp).await?;

        let total = resp
            .headers()
            .get(header::CONTENT_LENGTH)
            .and_then(|v| v.to_str().ok()?.parse::<u64>().ok())
            .unwrap_or(0);

        if total > 0 {
            check_free_space(cache_path.parent().unwrap_or(Path::new(".")), total)?;
        }

        let pb = ProgressBar::new(total);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("{msg} [{bar:40}] {bytes}/{total_bytes} ({eta})")
                .unwrap()
                .progress_chars("=> "),
        );
        pb.set_message(format!("Downloading {filename}"));

        let mut file = tokio::fs::File::create(&cache_path).await.map_err(|e| {
            crate::error::Error::Other(format!(
                "Failed to create cache file {}: {}",
                cache_path.display(),
                e
            ))
        })?;
        let mut stream = resp.bytes_stream();
        use futures_util::StreamExt;
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            file.write_all(&chunk).await?;
            pb.inc(chunk.len() as u64);
        }
        file.flush().await?;
        drop(file);

        pb.finish_with_message("Download complete");

        // Copy from cache to target.
        check_free_space(target_dir, std::fs::metadata(&cache_path)?.len())?;
        std::fs::copy(&cache_path, &target_path).map_err(|e| {
            crate::error::Error::Other(format!(
                "Failed to copy {} to {}: {}",
                cache_path.display(),
                target_path.display(),
                e
            ))
        })?;

        Ok(target_path)
    }
}
