//! Owned loopback HTTP streams with controllable header and body delays.

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

#[derive(Clone)]
pub(super) struct Reply {
    status: u16,
    location: Option<String>,
    cookies: Vec<String>,
    pub(super) header_delay: Duration,
    pub(super) chunks: Vec<(Duration, Vec<u8>)>,
}

impl Reply {
    pub(super) fn status(status: u16) -> Self {
        Self {
            status,
            location: None,
            cookies: Vec::new(),
            header_delay: Duration::ZERO,
            chunks: Vec::new(),
        }
    }

    pub(super) fn ok(body: &[u8]) -> Self {
        Self {
            chunks: vec![(Duration::ZERO, body.to_vec())],
            ..Self::status(200)
        }
    }

    pub(super) fn redirect(status: u16, location: &str) -> Self {
        Self {
            location: Some(location.into()),
            ..Self::status(status)
        }
    }

    pub(super) fn cookie(mut self, value: &str) -> Self {
        self.cookies.push(value.into());
        self
    }
}

pub(super) struct Server {
    pub(super) address: SocketAddr,
    requests: Arc<Mutex<Vec<String>>>,
    task: tokio::task::JoinHandle<()>,
}

impl Server {
    pub(super) async fn start(replies: Vec<Reply>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let requests = Arc::new(Mutex::new(Vec::new()));
        let captured = requests.clone();
        let task = tokio::spawn(async move {
            for reply in replies {
                let (mut stream, _) = listener.accept().await.unwrap();
                let mut request = Vec::new();
                while !request.ends_with(b"\r\n\r\n") {
                    let mut byte = [0];
                    let count = stream.read(&mut byte).await.unwrap();
                    assert!(count > 0 && request.len() < 64 * 1024);
                    request.extend_from_slice(&byte);
                }
                captured.lock().unwrap().push(String::from_utf8(request).unwrap());
                tokio::time::sleep(reply.header_delay).await;
                let length: usize = reply.chunks.iter().map(|(_, body)| body.len()).sum();
                let location = reply
                    .location
                    .map(|value| format!("Location: {value}\r\n"))
                    .unwrap_or_default();
                let cookies: String =
                    reply.cookies.iter().map(|value| format!("Set-Cookie: {value}\r\n")).collect();
                let header = format!(
                    "HTTP/1.1 {} Fixture\r\nContent-Length: {length}\r\n{location}{cookies}Connection: close\r\n\r\n",
                    reply.status
                );
                if stream.write_all(header.as_bytes()).await.is_err() {
                    continue;
                }
                for (delay, body) in reply.chunks {
                    tokio::time::sleep(delay).await;
                    if stream.write_all(&body).await.is_err() {
                        break;
                    }
                }
            }
        });
        Self {
            address,
            requests,
            task,
        }
    }

    pub(super) fn url(&self) -> url::Url {
        url::Url::parse(&format!("http://{}/", self.address)).unwrap()
    }

    pub(super) fn requests(&self) -> Vec<String> {
        self.requests.lock().unwrap().clone()
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}
