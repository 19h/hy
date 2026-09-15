//! Exercise the callback over real loopback sockets without launching a browser.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::thread::JoinHandle;
use std::time::Duration;

use serde_json::json;

use super::{MAX_BODY_BYTES, OAuthServer, OAuthTokens};

struct Callback {
    address: SocketAddr,
    worker: Option<JoinHandle<Option<OAuthTokens>>>,
}

impl Callback {
    fn start(timeout: Duration) -> Self {
        let server = OAuthServer::bind(0).unwrap();
        let address = server.listener.local_addr().unwrap();
        Self {
            address,
            worker: Some(std::thread::spawn(move || server.run(timeout).unwrap())),
        }
    }

    fn connect(&self) -> TcpStream {
        let stream = TcpStream::connect(self.address).unwrap();
        stream.set_read_timeout(Some(Duration::from_secs(1))).unwrap();
        stream.set_write_timeout(Some(Duration::from_secs(1))).unwrap();
        stream.set_nodelay(true).unwrap();
        stream
    }

    fn request(&self, method: &str, path: &str, body: &str) -> String {
        let mut stream = self.connect();
        write!(
            stream,
            "{method} {path} HTTP/1.1\r\nHost: localhost\r\nContent-Length: {}\r\n\r\n{body}",
            body.len()
        )
        .unwrap();
        read_response(stream)
    }

    fn finish(mut self) -> Option<OAuthTokens> {
        self.worker.take().unwrap().join().unwrap()
    }
}

impl Drop for Callback {
    fn drop(&mut self) {
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn read_response(stream: impl Read) -> String {
    let mut stream = BufReader::new(stream);
    let mut response = String::new();
    while !response.ends_with("\r\n\r\n") {
        assert_ne!(stream.read_line(&mut response).unwrap(), 0, "incomplete HTTP headers");
    }
    // These callback responses use Content-Length. TCP EOF is not their message
    // boundary (RFC 9112 §6.3), particularly after an oversized request is rejected.
    let mut length = None;
    for (name, value) in response.lines().filter_map(|line| line.split_once(':')) {
        assert!(!name.eq_ignore_ascii_case("transfer-encoding"), "unexpected response framing");
        if name.eq_ignore_ascii_case("content-length") {
            length = Some(value.trim().parse::<usize>().unwrap());
        }
    }
    let length = length.expect("callback response Content-Length");
    let mut body = vec![0; length];
    stream.read_exact(&mut body).unwrap();
    response.push_str(&String::from_utf8(body).unwrap());
    response
}

#[test]
fn response_reader_accepts_complete_frames_and_rejects_truncated_bodies() {
    struct Reset;
    impl Read for Reset {
        fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
            Err(std::io::ErrorKind::ConnectionReset.into())
        }
    }
    let complete = b"HTTP/1.1 413 Payload Too Large\r\nContent-Length: 3\r\n\r\nabc";
    assert_eq!(read_response(std::io::Cursor::new(complete).chain(Reset)).as_bytes(), complete);
    let truncated = &complete[..complete.len() - 1];
    assert!(
        std::panic::catch_unwind(|| read_response(std::io::Cursor::new(truncated).chain(Reset)))
            .is_err()
    );
}

fn valid_body() -> String {
    json!({"access_token": "fixture-token", "refresh_token": "fixture-refresh"}).to_string()
}

#[test]
fn fragmented_large_tokens_are_read_completely_and_response_is_flushed() {
    let callback = Callback::start(Duration::from_secs(3));
    let access = "a".repeat(12_000);
    let body = json!({"access_token": access, "refresh_token": "refresh"}).to_string();
    let mut stream = callback.connect();
    for header in [
        "POST /to".to_string(),
        format!("ken HTTP/1.1\r\nHost: localhost\r\nContent-Length: {}\r\n", body.len()),
        "\r\n".into(),
    ] {
        stream.write_all(header.as_bytes()).unwrap();
        std::thread::sleep(Duration::from_millis(10));
    }
    for part in body.as_bytes().chunks(1000) {
        stream.write_all(part).unwrap();
        std::thread::sleep(Duration::from_millis(2));
    }
    let response = read_response(stream);
    assert!(response.starts_with("HTTP/1.1 200"), "{response}");
    assert!(response.ends_with("Token received."));
    let tokens = callback.finish().unwrap();
    assert_eq!(tokens.access_token, access);
    assert_eq!(tokens.refresh_token.as_deref(), Some("refresh"));
}

#[test]
fn invalid_requests_leave_the_server_available_for_a_valid_callback() {
    let callback = Callback::start(Duration::from_secs(3));
    for (method, path, body, status) in [
        ("GET", "/token", "", 404),
        ("POST", "/token-extra", "{}", 404),
        ("POST", "/token?extra=1", "{}", 404),
        ("POST", "/token", "not json", 400),
        ("POST", "/token", "{}", 400),
        ("POST", "/token", r#"{"access_token":""}"#, 400),
        ("POST", "/token", r#"{"access_token":false}"#, 400),
        ("POST", "/token", r#"{"access_token":"fixture","refresh_token":[]}"#, 400),
    ] {
        let response = callback.request(method, path, body);
        assert!(response.starts_with(&format!("HTTP/1.1 {status}")), "{response}");
    }
    assert!(callback.request("POST", "/token", &valid_body()).starts_with("HTTP/1.1 200"));
    assert_eq!(callback.finish().unwrap().access_token, "fixture-token");
}

#[test]
fn callback_page_is_served_completely_with_redirect_query_parameters() {
    let callback = Callback::start(Duration::from_secs(3));
    let response = callback.request("GET", "/callback?provider=google", "");
    assert!(response.starts_with("HTTP/1.1 200"));
    assert!(response.contains("text/html; charset=utf-8"));
    let (headers, page) = response.split_once("\r\n\r\n").unwrap();
    let content_length: usize = headers
        .lines()
        .find_map(|line| {
            line.to_ascii_lowercase().strip_prefix("content-length: ").map(str::to_owned)
        })
        .unwrap()
        .parse()
        .unwrap();
    assert_eq!(content_length, page.len());
    assert!(!page.is_empty());
    callback.request("POST", "/token", &valid_body());
    assert!(callback.finish().is_some());
}

#[test]
fn idle_browser_connections_do_not_block_the_callback() {
    let callback = Callback::start(Duration::from_secs(3));
    let _idle = callback.connect();
    let mut partial = callback.connect();
    partial
        .write_all(b"POST /token HTTP/1.1\r\nHost: localhost\r\nContent-Length: 1000\r\n\r\n{")
        .unwrap();
    // The one-second client deadline is shorter than the idle connection timeout.
    assert!(callback.request("POST", "/token", &valid_body()).starts_with("HTTP/1.1 200"));
    assert!(callback.finish().is_some());
}

#[test]
fn timeout_closes_the_listener_and_partially_received_requests() {
    let callback = Callback::start(Duration::from_millis(100));
    let address = callback.address;
    let mut stream = callback.connect();
    stream
        .write_all(b"POST /token HTTP/1.1\r\nHost: localhost\r\nContent-Length: 1000\r\n\r\n{")
        .unwrap();
    assert!(callback.finish().is_none());
    assert!(TcpStream::connect(address).is_err());
    let mut byte = [0];
    match stream.read(&mut byte) {
        Ok(0) => {}
        Err(error) if error.kind() == std::io::ErrorKind::ConnectionReset => {}
        other => panic!("pending connection remained open: {other:?}"),
    }
}

#[test]
fn body_limits_apply_to_declared_and_chunked_requests() {
    let callback = Callback::start(Duration::from_secs(3));
    let mut declared = callback.connect();
    write!(
        declared,
        "POST /token HTTP/1.1\r\nHost: localhost\r\nContent-Length: {}\r\n\r\n",
        MAX_BODY_BYTES + 1
    )
    .unwrap();
    assert!(read_response(declared).starts_with("HTTP/1.1 413"));
    let mut chunked = callback.connect();
    let body = "x".repeat(MAX_BODY_BYTES + 1);
    write!(chunked, "POST /token HTTP/1.1\r\nHost: localhost\r\nTransfer-Encoding: chunked\r\n\r\n{:x}\r\n{body}\r\n0\r\n\r\n", body.len()).unwrap();
    assert!(read_response(chunked).starts_with("HTTP/1.1 413"));
    callback.request("POST", "/token", &valid_body());
    assert!(callback.finish().is_some());
}

#[test]
fn callback_bind_conflicts_are_reported_before_login_can_launch() {
    let server = OAuthServer::bind(0).unwrap();
    let port = server.listener.local_addr().unwrap().port();
    assert!(OAuthServer::bind(port).is_err());
}

#[test]
#[ignore = "manual browser fixture; open the printed loopback URL within 120 seconds"]
fn browser_callback_round_trip() {
    let server = OAuthServer::bind(0).unwrap();
    let address = server.listener.local_addr().unwrap();
    println!(
        "BROWSER_CALLBACK_URL=http://{address}/callback#access_token=browser-fixture-token&refresh_token=browser-fixture-refresh"
    );
    let tokens = server.run(Duration::from_secs(120)).unwrap().expect("browser token submission");
    assert_eq!(tokens.access_token, "browser-fixture-token");
    assert_eq!(tokens.refresh_token.as_deref(), Some("browser-fixture-refresh"));
}
