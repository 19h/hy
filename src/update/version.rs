//! Version comparison and background update checking.

use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use semver::Version;

use crate::config::Env;
use crate::util::cache::cache_dir;

/// Parse a version string, stripping a leading `v` if present.
pub fn parse_version(tag: &str) -> Option<Version> {
    let clean = tag.strip_prefix('v').unwrap_or(tag);
    Version::parse(clean).ok()
}

/// True if a version string looks like a pre-release / dev build.
pub fn is_dev_version(v: &Version) -> bool {
    !v.pre.is_empty()
}

/// Background update checker with 24 h caching.
pub struct BackgroundUpdateChecker {
    result: Arc<Mutex<Option<String>>>,
    handle: Option<thread::JoinHandle<()>>,
}

impl BackgroundUpdateChecker {
    pub fn new() -> Self {
        Self {
            result: Arc::new(Mutex::new(None)),
            handle: None,
        }
    }

    /// Start a background thread that checks for updates.
    pub fn start(&mut self) {
        if !should_check() {
            return;
        }

        let result = Arc::clone(&self.result);
        self.handle = Some(thread::spawn(move || {
            if let Some(msg) = check_for_update() {
                *result.lock().unwrap() = Some(msg);
            }
            // Touch the cache timestamp.
            let _ = update_cache_timestamp();
        }));
    }

    /// Block up to `timeout` for the result.
    pub fn get_result(&self, timeout: Duration) -> Option<String> {
        if let Some(ref handle) = self.handle {
            // We can't join with a timeout in stable Rust, but we can poll.
            let start = std::time::Instant::now();
            while start.elapsed() < timeout {
                if handle.is_finished() {
                    return self.result.lock().unwrap().clone();
                }
                thread::sleep(Duration::from_millis(100));
            }
        }
        self.result.lock().unwrap().clone()
    }
}

// ── internal ────────────────────────────────────────────────────────────

fn cache_file() -> std::path::PathBuf {
    cache_dir("updates").join("last_check")
}

fn should_check() -> bool {
    let path = cache_file();
    if !path.exists() {
        return true;
    }
    match std::fs::metadata(&path) {
        Ok(meta) => {
            let age = meta
                .modified()
                .ok()
                .and_then(|t| t.elapsed().ok())
                .unwrap_or(Duration::MAX);
            age > Duration::from_secs(24 * 3600)
        }
        Err(_) => true,
    }
}

fn update_cache_timestamp() -> std::io::Result<()> {
    let path = cache_file();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, Env::global().version.as_bytes())
}

fn check_for_update() -> Option<String> {
    let env = Env::global();
    let current = parse_version(&env.version)?;

    // Quick sync HTTP check against GitHub releases.
    let url = format!(
        "{}/repos/HexRaysSA/ida-hcli/releases/latest",
        env.github_api_url
    );

    let client = reqwest::blocking::Client::builder()
        .user_agent(format!("hcli/{}", env.version))
        .timeout(Duration::from_secs(5))
        .build()
        .ok()?;

    let resp: serde_json::Value = client.get(&url).send().ok()?.json().ok()?;
    let tag = resp.get("tag_name")?.as_str()?;
    let latest = parse_version(tag)?;

    if latest > current && !is_dev_version(&latest) {
        Some(format!(
            "\nUpdate available: {} -> {}. Run `hy update` to install.",
            current, latest
        ))
    } else {
        None
    }
}
