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

enum Decision {
    Stop,
    Follow(Next),
    Reject(Rejection),
}

enum Rejection {
    Scheme(String),
    Loop,
}

impl Rejection {
    fn message(self, reason: &str) -> String {
        match self {
            Self::Scheme(url) => format!("{reason} - Redirection to url '{url}' is not allowed"),
            Self::Loop => format!(
                "The HTTP server returned a redirect error that would lead to an infinite loop.\n\
                 The last 30x error message was:\n{reason}"
            ),
        }
    }
}

impl History {
    fn next(
        &mut self,
        current: &str,
        method: &Method,
        request_headers: &HeaderMap,
        status: u16,
        response_headers: &HeaderMap,
    ) -> Result<Decision> {
        if !matches!(status, 301 | 302 | 303 | 307 | 308) {
            return Ok(Decision::Stop);
        }
        // Unlike HTTPX, urllib uses the first Location, with URI as a fallback.
        let Some(location) =
            response_headers.get(header::LOCATION).or_else(|| response_headers.get("uri"))
        else {
            return Ok(Decision::Stop);
        };
        let Some(url) = target::resolve(current, location.as_bytes())? else {
            let location = location.as_bytes().iter().map(|byte| char::from(*byte)).collect();
            return Ok(Decision::Reject(Rejection::Scheme(location)));
        };
        if !matches!(*method, Method::GET | Method::HEAD)
            && !(*method == Method::POST && matches!(status, 301..=303))
        {
            return Ok(Decision::Stop);
        }
        if self.visits.get(&url).copied().unwrap_or(0) >= MAX_REPEATS
            || self.visits.len() >= MAX_DISTINCT_TARGETS
        {
            return Ok(Decision::Reject(Rejection::Loop));
        }
        *self.visits.entry(url.clone()).or_default() += 1;
        let mut headers = request_headers.clone();
        headers.remove(header::CONTENT_LENGTH);
        headers.remove(header::CONTENT_TYPE);
        Ok(Decision::Follow(Next {
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
        let mut response = client.execute(request).await?;
        let decision = history.next(
            &current,
            &method,
            &headers,
            response.status().as_u16(),
            response.headers(),
        )?;
        let next = match decision {
            Decision::Follow(next) => next,
            Decision::Stop => return Ok(response),
            Decision::Reject(rejection) => {
                let message = rejection.message(&super::http::reason(&response));
                super::http::replace_reason(&mut response, message);
                return Ok(response);
            }
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
