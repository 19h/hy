//! HTTP/1 callback transport for the browser OAuth flow.

use std::convert::Infallible;
use std::net::TcpListener;
use std::time::Duration;

use http_body_util::{BodyExt, Full, LengthLimitError, Limited};
use hyper::body::{Body, Bytes, Incoming};
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Method, Request, Response, StatusCode};
use hyper_util::rt::{TokioIo, TokioTimer};
use serde::Deserialize;
use tokio::sync::mpsc;
use tokio::task::JoinSet;

use crate::error::{Error, Result};

const CALLBACK_HTML: &str = include_str!("callback.html");
const MAX_BODY_BYTES: usize = 64 * 1024;
const MAX_HEADER_BYTES: usize = 16 * 1024;
const MAX_CONNECTIONS: usize = 16;
const CONNECTION_TIMEOUT: Duration = Duration::from_secs(2);

/// Parsed transport data; GoTrue must validate it before credentials are saved.
#[derive(Clone, Deserialize)]
pub struct OAuthTokens {
    pub access_token: String,
    pub refresh_token: Option<String>,
}

/// Own the bound listener before launching the browser.
pub struct OAuthServer {
    listener: TcpListener,
}

impl OAuthServer {
    pub fn bind(port: u16) -> Result<Self> {
        let listener = TcpListener::bind(("127.0.0.1", port))
            .map_err(|error| Error::OAuthFailed(format!("callback bind failed: {error}")))?;
        listener.set_nonblocking(true)?;
        Ok(Self {
            listener,
        })
    }

    /// Run on the blocking login worker. The deadline covers every active socket.
    pub fn run(self, timeout: Duration) -> Result<Option<OAuthTokens>> {
        tokio::runtime::Builder::new_current_thread().enable_all().build()?.block_on(async {
            tokio::time::timeout(timeout, self.serve()).await.unwrap_or(Ok(None))
        })
    }

    async fn serve(self) -> Result<Option<OAuthTokens>> {
        let listener = tokio::net::TcpListener::from_std(self.listener)?;
        let mut connections = JoinSet::new();
        loop {
            tokio::select! {
                accepted = listener.accept(), if connections.len() < MAX_CONNECTIONS => {
                    let (stream, _) = accepted?;
                    connections.spawn(async move {
                        tokio::time::timeout(CONNECTION_TIMEOUT, serve_connection(stream))
                            .await.unwrap_or(None)
                    });
                }
                completed = connections.join_next(), if !connections.is_empty() => {
                    match completed {
                        Some(Ok(Some(tokens))) => return Ok(Some(tokens)),
                        Some(Err(error)) => tracing::debug!(%error, "callback task failed"),
                        _ => {}
                    }
                }
            }
        }
        // Dropping JoinSet aborts all remaining connections, including browser
        // preconnections that have not sent a request.
    }
}

async fn serve_connection(stream: tokio::net::TcpStream) -> Option<OAuthTokens> {
    let (sender, mut receiver) = mpsc::channel(1);
    let service = service_fn(move |request| handle(request, sender.clone()));
    let result = http1::Builder::new()
        .keep_alive(false)
        .timer(TokioTimer::new())
        .header_read_timeout(CONNECTION_TIMEOUT)
        .max_headers(100)
        .max_buf_size(MAX_HEADER_BYTES)
        .serve_connection(TokioIo::new(stream), service)
        .await;
    if let Err(error) = result {
        tracing::debug!(%error, "callback connection closed");
    }
    // Return after the response has been flushed, so shutting down the listener
    // cannot discard the successful browser response.
    receiver.try_recv().ok()
}

async fn handle(
    request: Request<Incoming>,
    sender: mpsc::Sender<OAuthTokens>,
) -> std::result::Result<Response<Full<Bytes>>, Infallible> {
    let callback = request.method() == Method::GET && request.uri().path().starts_with("/callback");
    let token = request.method() == Method::POST && request.uri() == "/token";
    // Drain bounded bodies even on unknown routes. Closing a TCP socket with an
    // unread request body can reset it before the client receives our response.
    let body = match read_body(request).await {
        Ok(body) => body,
        Err(status) => return Ok(response(status, "text/plain", "Invalid token request")),
    };
    let response = if callback {
        response(StatusCode::OK, "text/html; charset=utf-8", CALLBACK_HTML)
    } else if token {
        receive_tokens(&body, sender)
    } else {
        response(StatusCode::NOT_FOUND, "text/plain", "Not found")
    };
    Ok(response)
}

async fn read_body(request: Request<Incoming>) -> std::result::Result<Bytes, StatusCode> {
    if request.body().size_hint().lower() > MAX_BODY_BYTES as u64 {
        return Err(StatusCode::PAYLOAD_TOO_LARGE);
    }
    match Limited::new(request.into_body(), MAX_BODY_BYTES).collect().await {
        Ok(body) => Ok(body.to_bytes()),
        Err(error) if error.is::<LengthLimitError>() => Err(StatusCode::PAYLOAD_TOO_LARGE),
        Err(_) => Err(StatusCode::BAD_REQUEST),
    }
}

fn receive_tokens(body: &[u8], sender: mpsc::Sender<OAuthTokens>) -> Response<Full<Bytes>> {
    let tokens: OAuthTokens = match serde_json::from_slice(body) {
        Ok(tokens) => tokens,
        Err(_) => return response(StatusCode::BAD_REQUEST, "text/plain", "Invalid token request"),
    };
    if tokens.access_token.is_empty() {
        return response(StatusCode::BAD_REQUEST, "text/plain", "Missing access token");
    }
    if sender.try_send(tokens).is_err() {
        return response(StatusCode::CONFLICT, "text/plain", "Login response already received");
    }
    response(StatusCode::OK, "text/plain", "Token received.")
}

fn response(
    status: StatusCode,
    content_type: &'static str,
    body: &'static str,
) -> Response<Full<Bytes>> {
    let mut response = Response::new(Full::new(Bytes::from_static(body.as_bytes())));
    *response.status_mut() = status;
    response
        .headers_mut()
        .insert(hyper::header::CONTENT_TYPE, content_type.parse().expect("static content type"));
    response
}

#[cfg(test)]
#[path = "oauth_tests.rs"]
mod tests;
