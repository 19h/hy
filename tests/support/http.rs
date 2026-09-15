//! Minimal local HTTP fixture with captured requests and deterministic shutdown.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::thread::JoinHandle;
use std::time::Duration;

#[derive(Clone, Debug)]
pub struct Request {
    pub method: String,
    pub path: String,
    pub headers: String,
    pub body: Vec<u8>,
}

#[derive(Clone)]
pub struct Response {
    pub status: u16,
    pub content_type: &'static str,
    pub body: Vec<u8>,
}

impl Response {
    pub fn json(value: serde_json::Value) -> Self {
        Self {
            status: 200,
            content_type: "application/json",
            body: serde_json::to_vec(&value).unwrap(),
        }
    }

    pub fn zip(bytes: Vec<u8>) -> Self {
        Self {
            status: 200,
            content_type: "application/zip",
            body: bytes,
        }
    }

    pub fn missing() -> Self {
        Self {
            status: 404,
            content_type: "text/plain",
            body: b"fixture route not found".to_vec(),
        }
    }
}

pub struct Server {
    pub url: String,
    requests: Arc<Mutex<Vec<Request>>>,
    stopping: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl Server {
    pub fn start(handler: impl Fn(&Request, &str) -> Response + Send + 'static) -> Self {
        Self::start_with_headers(move |request, base| (handler(request, base), Vec::new()))
    }

    pub fn start_with_headers(
        handler: impl Fn(&Request, &str) -> (Response, Vec<(String, String)>) + Send + 'static,
    ) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let requests = Arc::new(Mutex::new(Vec::new()));
        let stopping = Arc::new(AtomicBool::new(false));
        let worker_requests = requests.clone();
        let worker_stopping = stopping.clone();
        let base = url.clone();
        let worker = std::thread::spawn(move || {
            while !worker_stopping.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        // Darwin can inherit O_NONBLOCK from the listening socket.
                        // Wait for request bytes instead of closing an idle connection.
                        stream.set_nonblocking(false).unwrap();
                        if let Ok(request) = read_request(&mut stream) {
                            let (response, headers) = handler(&request, &base);
                            worker_requests.lock().unwrap().push(request);
                            let extra: String = headers
                                .into_iter()
                                .map(|(name, value)| format!("{name}: {value}\r\n"))
                                .collect();
                            let header = format!(
                                "HTTP/1.1 {} Fixture\r\nContent-Type: {}\r\nContent-Length: {}\r\n{extra}Connection: close\r\n\r\n",
                                response.status,
                                response.content_type,
                                response.body.len()
                            );
                            let _ = stream.write_all(header.as_bytes());
                            let _ = stream.write_all(&response.body);
                        }
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(5))
                    }
                    Err(error) => panic!("HTTP fixture accept failed: {error}"),
                }
            }
        });
        Self {
            url,
            requests,
            stopping,
            worker: Some(worker),
        }
    }

    pub fn requests(&self) -> Vec<Request> {
        self.requests.lock().unwrap().clone()
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stopping.store(true, Ordering::Relaxed);
        if let Some(worker) = self.worker.take() {
            let result = worker.join();
            // Preserve the original test failure during unwinding. A second
            // panic in Drop aborts the process and skips other fixtures' cleanup.
            if !std::thread::panicking() {
                result.expect("HTTP fixture worker");
            }
        }
    }
}

fn read_request(stream: &mut TcpStream) -> std::io::Result<Request> {
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    let mut bytes = Vec::new();
    let header_end = loop {
        let mut chunk = [0; 4096];
        let count = stream.read(&mut chunk)?;
        if count == 0 || bytes.len() > 1024 * 1024 {
            return Err(std::io::ErrorKind::InvalidData.into());
        }
        bytes.extend_from_slice(&chunk[..count]);
        if let Some(end) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
            break end + 4;
        }
    };
    let headers = String::from_utf8_lossy(&bytes[..header_end]).into_owned();
    let length = headers
        .lines()
        .filter_map(|line| line.split_once(':'))
        .find(|(name, _)| name.eq_ignore_ascii_case("content-length"))
        .and_then(|(_, value)| value.trim().parse::<usize>().ok())
        .unwrap_or(0);
    while bytes.len() < header_end + length {
        let mut chunk = [0; 4096];
        let count = stream.read(&mut chunk)?;
        if count == 0 {
            return Err(std::io::ErrorKind::UnexpectedEof.into());
        }
        bytes.extend_from_slice(&chunk[..count]);
    }
    let mut line = headers.lines().next().unwrap_or_default().split_whitespace();
    Ok(Request {
        method: line.next().unwrap_or_default().into(),
        path: line.next().unwrap_or_default().into(),
        body: bytes[header_end..].to_vec(),
        headers,
    })
}
