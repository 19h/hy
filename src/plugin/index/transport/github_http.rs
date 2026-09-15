//! GitHub's two independent HTTPX GET operations, without repository credentials.

use std::future::Future;
use std::time::Duration;

use base64::Engine;
use reqwest::header::{self, HeaderMap, HeaderValue};

use crate::error::{Error, Result};
use crate::util::cookies::Jar;
use crate::util::http_headers::TextDecoder;

const MAX_REDIRECTS: usize = 20;

pub(super) async fn fetch(value: &str, timeout: Duration, accept: &'static str) -> Result<Vec<u8>> {
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(timeout)
        .read_timeout(timeout)
        .no_gzip()
        .no_deflate()
        .no_brotli()
        .no_zstd()
        .build()?;
    fetch_using(value, accept, move |url, headers| client.get(url).headers(headers).send()).await
}

async fn fetch_using<S, F>(value: &str, accept: &'static str, mut send: S) -> Result<Vec<u8>>
where
    S: FnMut(url::Url, HeaderMap) -> F,
    F: Future<Output = std::result::Result<reqwest::Response, reqwest::Error>>,
{
    let original = url::Url::parse(value).map_err(invalid_url)?;
    let mut current = original.clone();
    let mut headers = initial_headers(&current, accept)?;
    let mut cookies = Jar::default();
    for redirects in 0..=MAX_REDIRECTS {
        headers.remove(header::COOKIE);
        if let Some(cookie) = cookies.header(&current)? {
            headers.insert(header::COOKIE, cookie);
        }
        // Initial URL credentials become a header once. A redirect URL must not
        // silently replace that header through reqwest's implicit Basic auth.
        let mut wire_url = current.clone();
        let _ = wire_url.set_username("");
        let _ = wire_url.set_password(None);
        let response = send(wire_url, headers.clone()).await?;
        let status = response.status().as_u16();
        cookies.store(response.headers(), &current);
        // HTTPX constructs the redirect request before reading its body.
        let target = redirect_target(&current, status, response.headers())?;
        let body = super::response::decode(response).await?;
        if let Some(target) = target {
            if redirects == MAX_REDIRECTS {
                return Err(Error::Other("Exceeded maximum allowed redirects.".into()));
            }
            if current.origin() != target.origin() && !https_upgrade(&current, &target) {
                headers.remove(header::AUTHORIZATION);
            }
            current = target;
            continue;
        }
        return finish(&original, &current, status, body);
    }
    unreachable!("the final redirect returns an error")
}

fn initial_headers(url: &url::Url, accept: &'static str) -> Result<HeaderMap> {
    let mut headers = HeaderMap::from_iter([
        (header::ACCEPT, HeaderValue::from_static(accept)),
        (header::ACCEPT_ENCODING, HeaderValue::from_static("gzip, deflate")),
        (header::CONNECTION, HeaderValue::from_static("keep-alive")),
        (header::USER_AGENT, HeaderValue::from_static(concat!("hy/", env!("CARGO_PKG_VERSION")))),
    ]);
    if !url.username().is_empty() || url.password().is_some_and(|password| !password.is_empty()) {
        let username = percent_encoding::percent_decode_str(url.username()).decode_utf8_lossy();
        let password =
            percent_encoding::percent_decode_str(url.password().unwrap_or("")).decode_utf8_lossy();
        let credentials =
            base64::engine::general_purpose::STANDARD.encode(format!("{username}:{password}"));
        let value = HeaderValue::from_str(&format!("Basic {credentials}"))
            .map_err(|error| Error::Other(error.to_string()))?;
        headers.insert(header::AUTHORIZATION, value);
    }
    Ok(headers)
}

fn redirect_target(
    current: &url::Url,
    status: u16,
    headers: &HeaderMap,
) -> Result<Option<url::Url>> {
    if !matches!(status, 301 | 302 | 303 | 307 | 308) || !headers.contains_key(header::LOCATION) {
        return Ok(None);
    }
    let decoder = TextDecoder::new(headers);
    let location = headers
        .get_all(header::LOCATION)
        .iter()
        .map(|value| decoder.decode(value))
        .collect::<Vec<_>>()
        .join(", ");
    let mut target = current.join(&location).map_err(invalid_url)?;
    if target.fragment().is_none_or(str::is_empty) {
        target.set_fragment(current.fragment());
    }
    Ok(Some(target))
}

fn https_upgrade(current: &url::Url, target: &url::Url) -> bool {
    current.scheme() == "http"
        && target.scheme() == "https"
        && current.host_str() == target.host_str()
        && current.port_or_known_default() == Some(80)
        && target.port_or_known_default() == Some(443)
}

fn finish(original: &url::Url, current: &url::Url, status: u16, body: Vec<u8>) -> Result<Vec<u8>> {
    if !(200..300).contains(&status) {
        return Err(Error::RepositoryHttp {
            status,
            url: current.to_string(),
        });
    }
    // Upstream checks only the final successful response's scheme. Temporary
    // downgrades and HTTP error responses follow its distinct ordering here.
    if original.scheme() == "https" && current.scheme() != "https" {
        return Err(Error::Other(format!(
            "HTTPS request was redirected to insecure HTTP URL: {current}"
        )));
    }
    Ok(body)
}

fn invalid_url(error: url::ParseError) -> Error {
    Error::Other(format!("invalid HTTP URL: {error}"))
}

#[cfg(test)]
mod tests;
