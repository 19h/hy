//! Explicit proxy routing keeps request authority independent of Host and TLS.

use std::time::Duration;

use reqwest::header::{
    ACCEPT, ACCEPT_ENCODING, AUTHORIZATION, COOKIE, HOST, HeaderMap, HeaderValue,
};

use crate::error::{Error, Result};

use super::tls::Contexts;

mod config;
mod environment;
mod rules;
mod system;
mod timed;
mod wire;

#[derive(Clone, Default)]
pub(super) struct Routing {
    config: config::Config,
}

#[derive(Debug)]
pub(super) struct Failure {
    pub(super) error: Error,
    pub(super) connect: bool,
}

impl Failure {
    fn new(error: impl std::fmt::Display, connect: bool) -> Self {
        Self {
            error: Error::Other(format!("KE proxy request failed: {error}")),
            connect,
        }
    }
}

impl Routing {
    pub(super) fn from_environment() -> Result<Self> {
        let config = config::Config::from_environment()?;
        Ok(Self {
            config,
        })
    }

    pub(super) fn selected(&self, destination: &url::Url) -> bool {
        self.config.select(destination).is_some()
    }

    pub(super) async fn get(
        &self,
        original: &url::Url,
        destination: &url::Url,
        pinned: bool,
        cookie: Option<HeaderValue>,
        tls: &Contexts,
        timeout: Duration,
    ) -> std::result::Result<reqwest::Response, Failure> {
        let proxy = self.config.select(destination).expect("selected proxy route");
        let mut headers = HeaderMap::new();
        let host = &original[url::Position::BeforeHost..url::Position::AfterPort];
        headers
            .insert(HOST, HeaderValue::from_str(host).map_err(|error| Failure::new(error, false))?);
        headers.insert(ACCEPT, HeaderValue::from_static("*/*"));
        headers.insert(ACCEPT_ENCODING, HeaderValue::from_static("gzip, deflate"));
        if let Some(cookie) = cookie {
            headers.insert(COOKIE, cookie);
        }
        if let Some(auth) = config::authorization(original) {
            headers.insert(AUTHORIZATION, auth);
        }
        // HTTPcore forwards the SNI extension to the proxy's outer connection,
        // but uses the rewritten URL's host for TLS inside a CONNECT tunnel.
        let outer_name = if pinned {
            super::hostname(original)
        } else {
            super::hostname(&proxy.url)
        }
        .map_err(|error| Failure::new(error, true))?;
        wire::get(proxy, destination, headers, &outer_name, tls, timeout).await
    }
}

#[cfg(test)]
mod tests;
