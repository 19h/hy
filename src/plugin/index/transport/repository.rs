//! Per-fetch credential state and the upstream repository redirect policy.

use std::future::Future;

use reqwest::header::{self, HeaderMap, HeaderValue};

use crate::error::{Error, PluginAccessDenied, Result};
use crate::util::http_body::Decoder;
use crate::util::http_headers::TextDecoder;

use super::url_parts::Parts;

const MAX_REDIRECTS: usize = 10;

pub(in crate::plugin::index) fn credential_host(value: &str) -> Result<bool> {
    let parts = Parts::parse(value)?;
    let host = parts.hostname();
    Ok(parts.scheme == "https"
        && (host == "plugins.hex-rays.com" || host.ends_with(".plugins.hex-rays.com")))
}

pub(super) struct Response {
    status: u16,
    headers: HeaderMap,
    body: Vec<u8>,
}

pub(super) async fn fetch(url: &str) -> Result<Vec<u8>> {
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(std::time::Duration::from_secs(30))
        .no_gzip()
        .no_deflate()
        .no_brotli()
        .no_zstd()
        .build()?;
    fetch_using(
        url,
        move |url, headers| {
            let client = client.clone();
            async move {
                let mut response = client.get(url).headers(headers).send().await?;
                let status = response.status().as_u16();
                let headers = response.headers().clone();
                let mut decoder = Decoder::new(&headers, u64::MAX);
                let mut body = Vec::new();
                while let Some(chunk) = response.chunk().await? {
                    body.extend_from_slice(&decoder.decode(&chunk)?);
                }
                body.extend(decoder.finish()?);
                Ok(Response {
                    status,
                    headers,
                    body,
                })
            }
        },
        || crate::auth::request_headers(false),
    )
    .await
}

async fn fetch_using<S, SF, A, AF>(url: &str, mut send: S, mut resolve_auth: A) -> Result<Vec<u8>>
where
    S: FnMut(String, HeaderMap) -> SF,
    SF: Future<Output = Result<Response>>,
    A: FnMut() -> AF,
    AF: Future<Output = Result<HeaderMap>>,
{
    let secure = Parts::parse(url)?.scheme == "https";
    let mut current = url.to_owned();
    let mut auth_headers: Option<HeaderMap> = None;
    let mut cookies = crate::util::cookies::Jar::default();
    for _ in 0..=MAX_REDIRECTS {
        let credentialed = credential_host(&current)?;
        if credentialed && auth_headers.is_none() {
            auth_headers = Some(resolve_auth().await?);
        }
        let mut headers = HeaderMap::from_iter([
            (
                header::USER_AGENT,
                HeaderValue::from_static(concat!("hy/", env!("CARGO_PKG_VERSION"))),
            ),
            (header::ACCEPT, HeaderValue::from_static("*/*")),
            (header::ACCEPT_ENCODING, HeaderValue::from_static("gzip, deflate")),
            (header::CONNECTION, HeaderValue::from_static("keep-alive")),
        ]);
        if credentialed {
            headers.extend(auth_headers.as_ref().expect("eligible credentials resolved").clone());
        }
        let request_url =
            url::Url::parse(&current).map_err(|error| Error::Other(error.to_string()))?;
        if let Some(cookie) = cookies.header(&request_url)? {
            headers.insert(header::COOKIE, cookie);
        }
        // HTTPX's non-streaming get consumes and decodes the response before
        // redirect or status handling, including error and redirect bodies.
        let response = send(current.clone(), headers).await?;
        cookies.store(&response.headers, &request_url);
        if let Some(next) = redirect(&current, response.status, &response.headers)? {
            if secure && Parts::parse(&next)?.scheme != "https" {
                return Err(Error::Other(format!(
                    "HTTPS request was redirected to insecure HTTP URL: {next}"
                )));
            }
            current = next;
            continue;
        }
        if credentialed && matches!(response.status, 401 | 403) {
            return Err(PluginAccessDenied {
                url: url.into(),
                status: response.status,
                authenticated: auth_headers.as_ref().is_some_and(|headers| !headers.is_empty()),
                repository: None,
            }
            .into());
        }
        if !(200..300).contains(&response.status) {
            return Err(Error::RepositoryHttp {
                status: response.status,
                url: current,
            });
        }
        return Ok(response.body);
    }
    Err(Error::Other(format!("too many redirects while fetching: {url}")))
}

fn redirect(current: &str, status: u16, headers: &HeaderMap) -> Result<Option<String>> {
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
    let current = url::Url::parse(current).map_err(|error| Error::Other(error.to_string()))?;
    // urllib.urljoin(base, "") returns base, including its fragment.
    if location.is_empty() {
        return Ok(Some(current.into()));
    }
    Ok(Some(current.join(&location).map_err(|error| Error::Other(error.to_string()))?.into()))
}

#[cfg(test)]
mod tests;
