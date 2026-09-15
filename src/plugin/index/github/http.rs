//! Catalogue HTTP acquisition; response bodies are read outside the retry loop.

use reqwest::{Client, Response, header, redirect::Policy};

use crate::error::{Error, Result};

use super::retry;

pub(super) fn metadata_client() -> Result<Client> {
    client(Policy::none())
}

fn client(redirects: Policy) -> Result<Client> {
    Ok(Client::builder()
        .user_agent(concat!("hy/", env!("CARGO_PKG_VERSION")))
        .redirect(redirects)
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
    let client = client(Policy::limited(10))?;
    let response = retry::send(&client, client.get(url).build()?).await?;
    require_success(&response)?;
    // urllib returns the payload as received. A truncated body must not restart
    // acquisition, and content-encoding must not transform catalogue ZIP bytes.
    Ok(response.bytes().await?.to_vec())
}

pub(super) fn require_success(response: &Response) -> Result<()> {
    if !response.status().is_success() {
        return Err(Error::Other(format!("GitHub request failed with HTTP {}", response.status())));
    }
    Ok(())
}
