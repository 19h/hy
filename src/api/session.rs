//! Process-local API cookies, matching upstream's shared APIClient lifetime.

use std::sync::{Arc, LazyLock, Mutex};

use reqwest::{Request, header};

use super::ApiClient;
use super::response::Response;
use crate::error::Result;
use crate::util::cookies::Jar;

pub(super) type Cookies = Arc<Mutex<Jar>>;

pub(super) fn shared_cookies() -> Cookies {
    static COOKIES: LazyLock<Cookies> = LazyLock::new(|| Arc::new(Mutex::new(Jar::default())));
    Arc::clone(&COOKIES)
}

impl ApiClient {
    pub(super) async fn send(&self, mut request: Request) -> Result<Response> {
        if !request.headers().contains_key(header::COOKIE)
            && let Some(cookie) = self.cookies.lock().unwrap().header(request.url())?
        {
            request.headers_mut().insert(header::COOKIE, cookie);
        }
        let response = self.inner.execute(request).await?;
        // HTTPX extracts cookies before body reading or HTTP status classification.
        self.cookies.lock().unwrap().store(response.headers(), response.url());
        Ok(Response::new(response))
    }
}

#[cfg(test)]
mod tests;
