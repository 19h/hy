//! HTTP forward requests and CONNECT tunnels, with owned connection tasks.

use std::sync::Arc;
use std::time::Duration;

use futures_util::StreamExt;
use http_body_util::{BodyExt, Empty};
use hyper::body::Bytes;
use hyper::{Method, Request};
use hyper_util::rt::TokioIo;
use reqwest::header::{HeaderMap, PROXY_AUTHORIZATION};
use tokio::io::{AsyncRead, AsyncWrite};

use super::Failure;
use super::config::Proxy;
use super::timed::Timed;

trait Io: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send> Io for T {}
type Socket = Box<dyn Io>;
type Sender = hyper::client::conn::http1::SendRequest<Empty<Bytes>>;

struct Driver(tokio::task::JoinHandle<()>);
impl Drop for Driver {
    fn drop(&mut self) {
        self.0.abort();
    }
}

async fn handshake(socket: Socket) -> Result<(Sender, Driver), Failure> {
    let (sender, connection) = hyper::client::conn::http1::handshake(TokioIo::new(socket))
        .await
        .map_err(|error| Failure::new(error, false))?;
    let driver = Driver(tokio::spawn(async move {
        let _ = connection.with_upgrades().await;
    }));
    Ok((sender, driver))
}

async fn tls(
    socket: Socket,
    name: &str,
    config: Arc<rustls::ClientConfig>,
    timeout: Duration,
) -> Result<Socket, Failure> {
    let name = rustls::pki_types::ServerName::try_from(name.to_owned())
        .map_err(|error| Failure::new(error, true))?;
    let socket = tokio::time::timeout(
        timeout,
        tokio_rustls::TlsConnector::from(config).connect(name, socket),
    )
    .await
    .map_err(|error| Failure::new(error, true))?
    .map_err(|error| Failure::new(error, true))?;
    Ok(Box::new(socket))
}

pub(super) async fn get(
    proxy: &Proxy,
    destination: &url::Url,
    mut headers: HeaderMap,
    outer_name: &str,
    contexts: &super::super::tls::Contexts,
    timeout: Duration,
) -> Result<reqwest::Response, Failure> {
    let host = super::super::hostname(&proxy.url).map_err(|error| Failure::new(error, true))?;
    let port = proxy.url.port_or_known_default().expect("HTTP proxy port");
    let tcp = tokio::time::timeout(timeout, tokio::net::TcpStream::connect((host.as_str(), port)))
        .await
        .map_err(|error| Failure::new(error, true))?
        .map_err(|error| Failure::new(error, true))?;
    tcp.set_nodelay(true).map_err(|error| Failure::new(error, true))?;
    let mut socket: Socket = Box::new(Timed::new(tcp, timeout));
    if proxy.url.scheme() == "https" {
        socket = tls(socket, outer_name, contexts.proxy.clone(), timeout).await?;
    }
    let mut drivers = Vec::new();
    let (mut sender, driver) = handshake(socket).await?;
    drivers.push(driver);
    let target = if destination.scheme() == "https" {
        let host =
            super::super::hostname(destination).map_err(|error| Failure::new(error, false))?;
        let port = destination.port_or_known_default().expect("HTTPS port");
        let authority = format!("{host}:{port}");
        let mut request = Request::builder()
            .method(Method::CONNECT)
            .uri(&authority)
            .header("host", &authority)
            .header("accept", "*/*");
        if let Some(auth) = &proxy.authorization {
            request = request.header(PROXY_AUTHORIZATION, auth);
        }
        let request = request.body(Empty::new()).map_err(|error| Failure::new(error, false))?;
        let response =
            sender.send_request(request).await.map_err(|error| Failure::new(error, false))?;
        if !response.status().is_success() {
            return Err(Failure::new(
                format!("CONNECT returned HTTP {}", response.status().as_u16()),
                false,
            ));
        }
        let upgraded =
            hyper::upgrade::on(response).await.map_err(|error| Failure::new(error, false))?;
        let socket =
            tls(Box::new(TokioIo::new(upgraded)), &host, contexts.origin.clone(), timeout).await?;
        let (inner, driver) = handshake(socket).await?;
        sender = inner;
        drivers.push(driver);
        destination[url::Position::BeforePath..url::Position::AfterQuery].to_owned()
    } else {
        if let Some(auth) = &proxy.authorization {
            headers.insert(PROXY_AUTHORIZATION, auth.clone());
        }
        let mut url = destination.clone();
        url.set_fragment(None);
        url.set_username("").expect("HTTP URL");
        url.set_password(None).expect("HTTP URL");
        url.to_string()
    };
    let mut request = Request::builder()
        .method(Method::GET)
        .uri(target)
        .body(Empty::new())
        .map_err(|error| Failure::new(error, false))?;
    *request.headers_mut() = headers;
    let response =
        sender.send_request(request).await.map_err(|error| Failure::new(error, false))?;
    let (parts, body) = response.into_parts();
    let stream = futures_util::stream::unfold(
        (body.into_data_stream(), drivers),
        |(mut body, drivers)| async move { body.next().await.map(|chunk| (chunk, (body, drivers))) },
    );
    Ok(reqwest::Response::from(hyper::Response::from_parts(
        parts,
        reqwest::Body::wrap_stream(stream),
    )))
}
