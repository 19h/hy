//! Decode API bodies without changing their original HTTP response headers.

use reqwest::header::{self, HeaderMap, HeaderValue};

use crate::error::Result;
use crate::util::http_body::Decoder;

pub(super) struct Response {
    inner: reqwest::Response,
    decoder: Decoder,
    finished: bool,
}

impl Response {
    pub(super) fn new(inner: reqwest::Response) -> Self {
        let decoder = Decoder::new(inner.headers(), u64::MAX);
        Self {
            inner,
            decoder,
            finished: false,
        }
    }

    pub(super) fn status(&self) -> reqwest::StatusCode {
        self.inner.status()
    }

    pub(super) fn headers(&self) -> &HeaderMap {
        self.inner.headers()
    }

    pub(super) async fn chunk(&mut self) -> Result<Option<Vec<u8>>> {
        while !self.finished {
            let decoded = if let Some(chunk) = self.inner.chunk().await? {
                self.decoder.decode(&chunk)?.into_owned()
            } else {
                self.finished = true;
                self.decoder.finish()?
            };
            if !decoded.is_empty() {
                return Ok(Some(decoded));
            }
        }
        Ok(None)
    }

    pub(super) async fn bytes(mut self) -> Result<Vec<u8>> {
        let mut bytes = Vec::new();
        while let Some(chunk) = self.chunk().await? {
            bytes.extend_from_slice(&chunk);
        }
        Ok(bytes)
    }
}

pub(super) fn default_headers() -> HeaderMap {
    HeaderMap::from_iter([
        (header::ACCEPT, HeaderValue::from_static("*/*")),
        (header::ACCEPT_ENCODING, HeaderValue::from_static("gzip, deflate")),
        (header::CONNECTION, HeaderValue::from_static("keep-alive")),
    ])
}
