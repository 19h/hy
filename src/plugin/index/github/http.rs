//! Catalogue HTTP acquisition; final response bodies are read outside retries.

use reqwest::{Client, Response, header, redirect::Policy};
use serde::de::DeserializeOwned;

use crate::error::{Error, Result};
use crate::util::{python_utf8, strings::python_trim};

use super::retry;

pub(super) enum JsonEndpoint {
    Search,
    Graphql,
}

#[derive(Clone, Debug)]
struct ErrorReason(String);

pub(super) fn metadata_client() -> Result<Client> {
    client()
}

fn client() -> Result<Client> {
    Ok(Client::builder()
        .user_agent("Python-urllib/3.13")
        .redirect(Policy::none())
        .retry(reqwest::retry::never())
        .default_headers(header::HeaderMap::from_iter([
            (header::ACCEPT_ENCODING, header::HeaderValue::from_static("identity")),
            (header::CONNECTION, header::HeaderValue::from_static("close")),
        ]))
        .pool_max_idle_per_host(0)
        .no_gzip()
        .no_deflate()
        .no_brotli()
        .no_zstd()
        // urllib.urlopen has no deadline with the default socket configuration.
        .build()?)
}

pub(super) async fn download(url: &str) -> Result<Vec<u8>> {
    let client = client()?;
    let response = retry::send(&client, client.get(url).build()?).await?;
    require_success(&response)?;
    // urllib returns the payload as received. A truncated body must not restart
    // acquisition, and content-encoding must not transform catalogue ZIP bytes.
    Ok(response.bytes().await?.to_vec())
}

pub(super) fn require_success(response: &Response) -> Result<()> {
    if !response.status().is_success() {
        return Err(Error::Other(format!(
            "HTTP Error {}: {}",
            response.status().as_u16(),
            reason(response)
        )));
    }
    Ok(())
}

pub(super) async fn read_json<T: DeserializeOwned>(
    response: Response,
    endpoint: JsonEndpoint,
) -> Result<T> {
    if !response.status().is_success() && matches!(endpoint, JsonEndpoint::Graphql) {
        let status = response.status().as_u16();
        // GraphQL catches HTTPError outside _urlopen_with_retry. Reading and
        // strict decoding can replace that error, but cannot restart the request.
        let bytes = response.bytes().await?;
        let body = python_utf8::decode(&bytes)?;
        return Err(Error::Other(format!("HTTP {status}: {body}")));
    }
    require_success(&response)?;
    Ok(response.json().await?)
}

pub(super) fn reason(response: &Response) -> String {
    if let Some(reason) = response.extensions().get::<ErrorReason>() {
        return reason.0.clone();
    }
    let bytes = response
        .extensions()
        .get::<hyper::ext::ReasonPhrase>()
        .map(|reason| reason.as_bytes())
        .unwrap_or_else(|| response.status().canonical_reason().unwrap_or("").as_bytes());
    let text: String = bytes.iter().map(|byte| char::from(*byte)).collect();
    // http.client decodes the status line as Latin-1, then strips Unicode whitespace.
    python_trim(&text).into()
}

pub(super) fn replace_reason(response: &mut Response, reason: String) {
    response.extensions_mut().insert(ErrorReason(reason));
}

#[cfg(test)]
mod tests;
