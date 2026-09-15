//! urllib redirect decisions and per-original-request loop history.

use std::collections::HashMap;

use reqwest::header::{self, HeaderMap};
use reqwest::{Client, Method, Request, Response};

use crate::error::Result;

mod target;

const MAX_REPEATS: u8 = 4;
const MAX_DISTINCT_TARGETS: usize = 10;

#[derive(Default)]
pub(super) struct History {
    visits: HashMap<String, u8>,
}

struct Next {
    url: String,
    method: Method,
    headers: HeaderMap,
}

impl History {
    fn next(
        &mut self,
        current: &str,
        method: &Method,
        request_headers: &HeaderMap,
        status: u16,
        response_headers: &HeaderMap,
    ) -> Result<Option<Next>> {
        if !matches!(status, 301 | 302 | 303 | 307 | 308) {
            return Ok(None);
        }
        // Unlike HTTPX, urllib uses the first Location, with URI as a fallback.
        let Some(location) =
            response_headers.get(header::LOCATION).or_else(|| response_headers.get("uri"))
        else {
            return Ok(None);
        };
        let Some(url) = target::resolve(current, location.as_bytes())? else {
            return Ok(None);
        };
        if !matches!(*method, Method::GET | Method::HEAD)
            && !(*method == Method::POST && matches!(status, 301..=303))
        {
            return Ok(None);
        }
        if self.visits.get(&url).copied().unwrap_or(0) >= MAX_REPEATS
            || self.visits.len() >= MAX_DISTINCT_TARGETS
        {
            return Ok(None);
        }
        *self.visits.entry(url.clone()).or_default() += 1;
        let mut headers = request_headers.clone();
        headers.remove(header::CONTENT_LENGTH);
        headers.remove(header::CONTENT_TYPE);
        Ok(Some(Next {
            url,
            method: if *method == Method::HEAD {
                Method::HEAD
            } else {
                Method::GET
            },
            headers,
        }))
    }
}

pub(super) async fn send(
    client: &Client,
    mut request: Request,
    history: &mut History,
) -> Result<Response> {
    let mut current = request.url().to_string();
    loop {
        let method = request.method().clone();
        let headers = request.headers().clone();
        let response = client.execute(request).await?;
        let Some(next) = history.next(
            &current,
            &method,
            &headers,
            response.status().as_u16(),
            response.headers(),
        )?
        else {
            return Ok(response);
        };
        // urllib consumes accepted redirect bodies inside urlopen. A read error
        // therefore belongs to the retry boundary, unlike the final body read.
        response.bytes().await?;
        request = Request::new(next.method, target::transport_url(&next.url)?);
        *request.headers_mut() = next.headers;
        current = next.url;
    }
}

#[cfg(test)]
mod tests;
