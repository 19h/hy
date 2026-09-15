//! Streaming downloads with cache validation and staged cache publication.

use std::path::{Component, Path, PathBuf};

use reqwest::header;
use tokio::io::AsyncWriteExt;

use super::ApiClient;
use crate::error::{Error, Result};
use crate::util::cache::cache_dir;
use crate::util::io::check_free_space;

/// Path of the checksum sidecar file for a cached download.
fn checksum_sidecar(cache_path: &Path) -> PathBuf {
    let mut name = cache_path.file_name().unwrap_or_default().to_os_string();
    name.push(".sha256");
    cache_path.with_file_name(name)
}

/// Verify a cached file against its checksum sidecar. Files without a
/// sidecar (downloaded by older versions) pass by default.
fn cache_checksum_ok(cache_path: &Path) -> bool {
    use sha2::Digest;

    let sidecar = checksum_sidecar(cache_path);
    let Ok(expected) = std::fs::read_to_string(&sidecar) else {
        return true;
    };
    let Ok(mut file) = std::fs::File::open(cache_path) else {
        return false;
    };
    let mut hasher = sha2::Sha256::new();
    if std::io::copy(&mut file, &mut hasher).is_err() {
        return false;
    }
    format!("{:x}", hasher.finalize()) == expected.trim()
}

impl ApiClient {
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
        let target_dir = expand_directory(target_dir)?;
        std::fs::create_dir_all(&target_dir)?;

        // Determine filename.
        let filename = target_filename
            .filter(|name| !name.is_empty())
            .map(String::from)
            .or_else(|| {
                url::Url::parse(url).ok().and_then(|u| {
                    Path::new(u.path()).file_name().map(|name| name.to_string_lossy().into_owned())
                })
            })
            .unwrap_or_else(|| "download".into());

        // Cache path.
        if Path::new(&filename).components().count() != 1
            || !matches!(
                Path::new(&filename).components().next(),
                Some(std::path::Component::Normal(_))
            )
            || filename.contains('\\')
        {
            return Err(Error::Other("download filename must be a single path component".into()));
        }
        let cache_key = asset_key.filter(|key| !key.is_empty()).unwrap_or(&filename);
        let cache_path = cache_dir("downloads").join(cache_relative_path(cache_key)?);
        if let Some(parent) = cache_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let target_path = target_dir.join(&filename);

        if !force && self.cache_is_current(url, &cache_path, auth).await? {
            crate::util::fmt::info(&format!("Using cached file: {}", cache_path.display()));
        } else {
            self.download_to_cache(url, &filename, &cache_path, auth).await?;
        }
        check_free_space(&target_dir, std::fs::metadata(&cache_path)?.len())?;
        crate::util::files::copy_metadata(&cache_path, &target_path)?;
        Ok(target_path)
    }

    async fn cache_is_current(&self, url: &str, cache: &Path, auth: bool) -> Result<bool> {
        let Ok(metadata) = std::fs::metadata(cache) else {
            return Ok(false);
        };
        let mut request = self.inner.head(url);
        if auth {
            request = request.headers(self.auth_headers().await?);
        }
        let remote_size = self
            .send_following(request.build()?)
            .await
            .ok()
            .filter(|response| response.status().as_u16() < 400)
            .and_then(|response| {
                response
                    .headers()
                    .get(header::CONTENT_LENGTH)
                    .and_then(|value| value.to_str().ok()?.parse::<u64>().ok())
            });
        Ok(remote_size == Some(metadata.len()) && cache_checksum_ok(cache))
    }

    async fn download_to_cache(
        &self,
        url: &str,
        filename: &str,
        cache_path: &Path,
        auth: bool,
    ) -> Result<()> {
        // Stream download.
        let mut req = self.inner.get(url);
        if auth {
            req = req.headers(self.auth_headers().await?);
        }
        let mut resp = self.send_following(req.build()?).await?;
        if resp.status().as_u16() >= 400 {
            // Upstream classifies streamed responses before reading their body.
            // Response.json() is unavailable here, so it uses the status fallback.
            return Err(super::json::status_error(resp.status().as_u16(), &[]));
        }

        let total = resp
            .headers()
            .get(header::CONTENT_LENGTH)
            .and_then(|v| v.to_str().ok()?.parse::<u64>().ok())
            .unwrap_or(0);

        if total > 0 {
            check_free_space(cache_path.parent().unwrap_or(Path::new(".")), total)?;
        }

        let pb = crate::util::tui::byte_progress(total, format!("Downloading {filename}"));

        let staged = tempfile::NamedTempFile::new_in(cache_path.parent().unwrap())?;
        let mut file = tokio::fs::File::from_std(staged.reopen()?);
        use sha2::Digest;
        let mut hasher = sha2::Sha256::new();
        while let Some(chunk) = resp.chunk().await? {
            hasher.update(&chunk);
            file.write_all(&chunk).await?;
            pb.inc(chunk.len() as u64);
        }
        file.flush().await?;
        drop(file);

        staged.persist(cache_path).map_err(|error| Error::Io(error.error))?;

        // Record the checksum so future cache hits can be verified.
        let checksum = format!("{:x}", hasher.finalize());
        let _ = std::fs::write(checksum_sidecar(cache_path), &checksum);

        pb.finish_and_clear();

        Ok(())
    }
}

fn expand_directory(path: &Path) -> Result<PathBuf> {
    let expanded = match path.strip_prefix("~") {
        Ok(suffix) => dirs::home_dir()
            .ok_or_else(|| Error::Other("home directory unavailable".into()))?
            .join(suffix),
        Err(_) => path.to_path_buf(),
    };
    Ok(std::path::absolute(expanded)?)
}

/// Treat API keys as cache-relative paths even when the API prints a leading slash.
fn cache_relative_path(key: &str) -> Result<PathBuf> {
    let key = key.trim_start_matches('/');
    let path = Path::new(key);
    if key.is_empty()
        || key.contains('\\')
        || path.components().any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(Error::Other("invalid download cache key".into()));
    }
    Ok(path.to_path_buf())
}
