//! Worker assertion failures must not abort the test process during cleanup.

mod support;

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

use support::http::Server;

#[test]
fn simultaneous_worker_and_test_failures_preserve_the_original_panic() {
    let result = std::panic::catch_unwind(|| {
        let server = Server::start(|_, _| panic!("worker assertion"));
        let mut stream = TcpStream::connect(server.url.strip_prefix("http://").unwrap()).unwrap();
        stream.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
        stream
            .write_all(b"GET / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
            .unwrap();
        let mut response = Vec::new();
        stream.read_to_end(&mut response).unwrap();
        assert!(response.is_empty());
        panic!("original test assertion");
    });
    assert_eq!(result.unwrap_err().downcast_ref::<&str>(), Some(&"original test assertion"));
}
