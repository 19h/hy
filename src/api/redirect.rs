//! Explicit HTTPX redirect rules for file transfers; JSON requests do not follow.

use reqwest::{Method, Request, header};

use super::ApiClient;
use super::response::Response;
use crate::error::{Error, Result};

const MAX_REDIRECTS: usize = 20;

impl ApiClient {
    pub(super) async fn send_following(&self, mut request: Request) -> Result<Response> {
        for redirects in 0..=MAX_REDIRECTS {
            let method = request.method().clone();
            let current = request.url().clone();
            let mut headers = request.headers().clone();
            let has_body = request.body().is_some();
            let response = self.send(request).await?;
            let Some(target) = target(&current, &response)? else {
                return Ok(response);
            };
            let next_method = redirected_method(&method, response.status().as_u16());
            // HTTPX consumes every redirect response before following its target.
            response.bytes().await?;
            if redirects == MAX_REDIRECTS {
                return Err(Error::Other("Exceeded maximum allowed redirects.".into()));
            }
            if has_body && next_method == method {
                // HCLI passes an async generator. HTTPX cannot iterate it twice.
                return Err(Error::Other("cannot replay a consumed upload stream".into()));
            }
            if next_method != method && next_method == Method::GET {
                headers.remove(header::CONTENT_LENGTH);
                headers.remove(header::TRANSFER_ENCODING);
            }
            if current.origin() != target.origin() {
                headers.remove(header::HOST);
                if !is_https_upgrade(&current, &target) {
                    headers.remove(header::AUTHORIZATION);
                }
            }
            headers.remove(header::COOKIE);
            request = Request::new(next_method, target);
            *request.headers_mut() = headers;
        }
        unreachable!("the final redirect returns an error")
    }
}

fn redirected_method(method: &Method, status: u16) -> Method {
    if (matches!(status, 302 | 303) && *method != Method::HEAD)
        || (status == 301 && *method == Method::POST)
    {
        Method::GET
    } else {
        method.clone()
    }
}

fn target(current: &url::Url, response: &Response) -> Result<Option<url::Url>> {
    if !matches!(response.status().as_u16(), 301 | 302 | 303 | 307 | 308) {
        return Ok(None);
    }
    let Some(location) = response.headers().get(header::LOCATION) else {
        return Ok(None);
    };
    let decoder = crate::util::http_headers::TextDecoder::new(response.headers());
    let location = decoder.decode(location);
    let mut target = current
        .join(&location)
        .map_err(|error| Error::Other(format!("invalid redirect URL: {error}")))?;
    if target.fragment().is_none_or(str::is_empty) {
        target.set_fragment(current.fragment());
    }
    Ok(Some(target))
}

fn is_https_upgrade(from: &url::Url, to: &url::Url) -> bool {
    from.scheme() == "http"
        && to.scheme() == "https"
        && from.host_str() == to.host_str()
        && from.port_or_known_default() == Some(80)
        && to.port_or_known_default() == Some(443)
}

#[cfg(test)]
mod tests;
