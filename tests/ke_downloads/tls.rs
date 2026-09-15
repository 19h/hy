//! CA-file environment behavior through the KE command and real TLS sockets.

use std::sync::{Arc, Mutex};

use tokio::io::{AsyncReadExt, AsyncWriteExt};

use super::fixture::{CONTENT, Fixture, response};
use super::support::assert_success;

struct Server {
    address: std::net::SocketAddr,
    certificate: String,
    requests: Arc<Mutex<Vec<String>>>,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl Server {
    async fn start() -> Self {
        let generated = rcgen::generate_simple_self_signed(vec!["127.0.0.1".into()]).unwrap();
        let certificate = generated.cert.pem();
        let key =
            rustls::pki_types::PrivatePkcs8KeyDer::from(generated.signing_key.serialize_der());
        let config = rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(vec![generated.cert.der().clone()], key.into())
            .unwrap();
        let acceptor = tokio_rustls::TlsAcceptor::from(Arc::new(config));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let requests = Arc::new(Mutex::new(Vec::new()));
        let captured = requests.clone();
        let task = tokio::spawn(async move {
            loop {
                let (socket, _) = listener.accept().await.unwrap();
                let Ok(mut socket) = acceptor.accept(socket).await else {
                    continue;
                };
                let mut header = Vec::new();
                while !header.ends_with(b"\r\n\r\n") {
                    header.push(socket.read_u8().await.unwrap());
                    assert!(header.len() < 65536);
                }
                let header = String::from_utf8(header).unwrap();
                let content = header.lines().next().unwrap().contains("/content?");
                captured.lock().unwrap().push(header);
                let body = if content {
                    CONTENT
                } else {
                    b"{}"
                };
                let header = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                socket.write_all(header.as_bytes()).await.unwrap();
                socket.write_all(body).await.unwrap();
            }
        });
        Self {
            address,
            certificate,
            requests,
            task,
        }
    }
}

#[tokio::test]
async fn ssl_cert_file_controls_direct_download_trust() {
    for trusted in [true, false] {
        let fixture = Fixture::new(response(200, b"{}"), response(200, CONTENT));
        fixture.seed();
        let server = Server::start().await;
        let file = fixture.sandbox.path().join("roots.pem");
        let certificate = if trusted {
            server.certificate.clone()
        } else {
            rcgen::generate_simple_self_signed(vec!["unrelated.test".into()]).unwrap().cert.pem()
        };
        std::fs::write(&file, certificate).unwrap();
        let content = format!(
            "https://{}/api/objects/{}/content?token=fixture",
            server.address, fixture.hash
        );
        let mut uri = fixture.uri();
        uri.query_pairs_mut().clear().append_pair("url", &content);
        let mut command = fixture.command_for_uri(uri.as_str(), true);
        command
            .env("SSL_CERT_FILE", &file)
            .env("SSL_CERT_DIR", fixture.sandbox.path().join("absent-directory"));
        let output = tokio::task::spawn_blocking(move || command.output().unwrap()).await.unwrap();
        if trusted {
            assert_success(&output);
            assert_eq!(std::fs::read(fixture.path()).unwrap(), CONTENT);
            assert_eq!(std::fs::read(fixture.sidecar()).unwrap(), b"{}");
            assert_eq!(server.requests.lock().unwrap().len(), 2);
        } else {
            assert!(!output.status.success());
            assert_eq!(std::fs::read(fixture.path()).unwrap(), b"old content");
            assert_eq!(std::fs::read(fixture.sidecar()).unwrap(), br#"{"old":true}"#);
            assert!(server.requests.lock().unwrap().is_empty());
        }
        assert!(fixture.server.requests().is_empty());
        fixture.assert_no_staging_files();
        if let Some(python) = std::env::var_os("HY_TEST_HTTPX_ORACLE_PYTHON") {
            let output = tokio::task::spawn_blocking(move || {
                std::process::Command::new(python)
                    .args([
                        "-I",
                        "-B",
                        "-c",
                        r#"
import httpx, sys
try:
    response = httpx.get(sys.argv[1], timeout=2)
    response.raise_for_status()
except httpx.HTTPError:
    print('rejected')
else:
    print('accepted')
"#,
                        &content,
                    ])
                    .env("SSL_CERT_FILE", file)
                    .env_remove("SSL_CERT_DIR")
                    .env("NO_PROXY", "*")
                    .env("no_proxy", "*")
                    .output()
                    .unwrap()
            })
            .await
            .unwrap();
            assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
            assert_eq!(
                String::from_utf8(output.stdout).unwrap().trim(),
                if trusted {
                    "accepted"
                } else {
                    "rejected"
                }
            );
        }
    }
}
