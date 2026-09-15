use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use super::super::super::tls::Contexts;
use super::super::{Routing, config};

trait Io: AsyncRead + AsyncWrite + Send + Unpin {}
impl<T: AsyncRead + AsyncWrite + Send + Unpin> Io for T {}
type Socket = Box<dyn Io>;

struct Server {
    address: std::net::SocketAddr,
    requests: Arc<Mutex<Vec<String>>>,
    names: Arc<Mutex<Vec<Option<String>>>>,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn header(socket: &mut Socket) -> String {
    let mut bytes = Vec::new();
    while !bytes.ends_with(b"\r\n\r\n") {
        bytes.push(socket.read_u8().await.unwrap());
        assert!(bytes.len() < 65536);
    }
    String::from_utf8(bytes).unwrap()
}

fn configurations() -> (Arc<rustls::ServerConfig>, Arc<rustls::ClientConfig>) {
    named_configurations(&["original.test", "203.0.113.1", "127.0.0.1"])
}

fn named_configurations(names: &[&str]) -> (Arc<rustls::ServerConfig>, Arc<rustls::ClientConfig>) {
    let generated = rcgen::generate_simple_self_signed(
        names.iter().map(|name| (*name).to_owned()).collect::<Vec<_>>(),
    )
    .unwrap();
    let certificate = generated.cert.der().clone();
    let key = rustls::pki_types::PrivatePkcs8KeyDer::from(generated.signing_key.serialize_der());
    let server = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(vec![certificate.clone()], key.into())
        .unwrap();
    let mut roots = rustls::RootCertStore::empty();
    roots.add(certificate).unwrap();
    let client =
        rustls::ClientConfig::builder().with_root_certificates(roots).with_no_client_auth();
    (Arc::new(server), Arc::new(client))
}

async fn server(outer_tls: bool, tunnel: bool, config: Arc<rustls::ServerConfig>) -> Server {
    server_with_contexts(outer_tls, tunnel, config.clone(), config).await
}

async fn server_with_contexts(
    outer_tls: bool,
    tunnel: bool,
    outer_config: Arc<rustls::ServerConfig>,
    inner_config: Arc<rustls::ServerConfig>,
) -> Server {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let requests = Arc::new(Mutex::new(Vec::new()));
    let captured = requests.clone();
    let names = Arc::new(Mutex::new(Vec::new()));
    let captured_names = names.clone();
    let task = tokio::spawn(async move {
        let (tcp, _) = listener.accept().await.unwrap();
        let mut socket: Socket = Box::new(tcp);
        if outer_tls {
            let acceptor = tokio_rustls::TlsAcceptor::from(outer_config);
            let Ok(tls) = acceptor.accept(socket).await else {
                return;
            };
            captured_names.lock().unwrap().push(tls.get_ref().1.server_name().map(str::to_owned));
            socket = Box::new(tls);
        }
        if tunnel {
            let request = header(&mut socket).await;
            captured.lock().unwrap().push(request);
            socket.write_all(b"HTTP/1.1 200 Connection established\r\n\r\n").await.unwrap();
            let acceptor = tokio_rustls::TlsAcceptor::from(inner_config);
            let Ok(tls) = acceptor.accept(socket).await else {
                return;
            };
            captured_names.lock().unwrap().push(tls.get_ref().1.server_name().map(str::to_owned));
            socket = Box::new(tls);
        }
        let request = header(&mut socket).await;
        captured.lock().unwrap().push(request);
        socket
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 8\r\nConnection: close\r\n\r\nfixture!")
            .await
            .unwrap();
    });
    Server {
        address,
        requests,
        names,
        task,
    }
}

#[tokio::test]
async fn https_proxy_and_tunnel_use_independent_trust_roots() {
    for (trust_proxy, trust_origin) in [(true, true), (false, true), (true, false)] {
        let (proxy_config, proxy_roots) = configurations();
        let (origin_config, origin_roots) = configurations();
        let server = server_with_contexts(true, true, proxy_config, origin_config).await;
        let tls = Contexts {
            origin: if trust_origin {
                origin_roots.clone()
            } else {
                proxy_roots.clone()
            },
            proxy: if trust_proxy {
                proxy_roots
            } else {
                origin_roots
            },
        };
        let config = config::Config::from_entries(vec![(
            "HTTPS_PROXY".into(),
            format!("https://{}", server.address),
        )])
        .unwrap();
        let routing = Routing {
            config,
        };
        let url = url::Url::parse("https://original.test/content").unwrap();
        let result = routing.get(&url, &url, false, None, &tls, Duration::from_secs(2)).await;
        if trust_proxy && trust_origin {
            assert_eq!(result.unwrap().bytes().await.unwrap().as_ref(), b"fixture!");
            assert_eq!(server.requests.lock().unwrap().len(), 2);
        } else {
            let failure = match result {
                Ok(_) => panic!("accepted trust_proxy={trust_proxy}, trust_origin={trust_origin}"),
                Err(failure) => failure,
            };
            assert!(failure.connect);
            assert_eq!(server.requests.lock().unwrap().len(), usize::from(trust_proxy));
        }
    }
}

#[tokio::test]
async fn forward_and_connect_requests_preserve_observed_tls_identities() {
    for pinned in [false, true] {
        for outer_tls in [false, true] {
            for tunnel in [false, true] {
                let (server_config, tls) = configurations();
                let server = server(outer_tls, tunnel, server_config).await;
                let proxy_scheme = if outer_tls {
                    "https"
                } else {
                    "http"
                };
                let target_scheme = if tunnel {
                    "https"
                } else {
                    "http"
                };
                let config = config::Config::from_entries(vec![(
                    "ALL_PROXY".into(),
                    format!("{proxy_scheme}://fixture:password@{}", server.address),
                )])
                .unwrap();
                let routing = Routing {
                    config,
                };
                let original = url::Url::parse(&format!(
                    "{target_scheme}://original.test:4321/path?token=fixture"
                ))
                .unwrap();
                let mut destination = original.clone();
                if pinned {
                    destination.set_ip_host("203.0.113.1".parse().unwrap()).unwrap();
                }
                let addresses = pinned.then(|| vec!["203.0.113.1:4321".parse().unwrap()]);
                let mut transport = super::super::super::Transport::with_addresses(
                    &original,
                    addresses,
                    true,
                    Duration::from_secs(2),
                )
                .unwrap();
                transport.session.proxies = routing;
                transport.session.tls = Contexts::shared(tls);
                let response = transport.response(&original).await.unwrap();
                assert_eq!(response.bytes().await.unwrap().as_ref(), b"fixture!");
                let requests = server.requests.lock().unwrap();
                let origin = requests.last().unwrap();
                assert!(origin.to_lowercase().contains("host: original.test:4321\r\n"), "{origin}");
                if tunnel {
                    assert!(requests[0].starts_with(&format!(
                        "CONNECT {}:4321 HTTP/1.1",
                        if pinned {
                            "203.0.113.1"
                        } else {
                            "original.test"
                        }
                    )));
                    assert!(requests[0].to_lowercase().contains("proxy-authorization: basic "));
                    assert!(!origin.to_lowercase().contains("proxy-authorization"));
                    assert!(origin.starts_with("GET /path?token=fixture HTTP/1.1"));
                } else {
                    assert!(origin.starts_with(&format!("GET {} HTTP/1.1", destination)));
                    assert!(origin.to_lowercase().contains("proxy-authorization: basic "));
                }
                let mut expected_names = Vec::new();
                if outer_tls {
                    expected_names.push(pinned.then(|| "original.test".to_owned()));
                }
                if tunnel {
                    expected_names.push((!pinned).then(|| "original.test".to_owned()));
                }
                assert_eq!(*server.names.lock().unwrap(), expected_names);
            }
        }
    }
    super::oracle::assert_tls_identities();
}

#[tokio::test(start_paused = true)]
async fn proxy_io_deadlines_reset_after_progress() {
    use super::super::timed::Timed;
    let (read, mut write) = tokio::io::duplex(1);
    let mut read = Timed::new(read, Duration::from_millis(150));
    let writer = tokio::spawn(async move {
        for byte in b"abc" {
            tokio::time::sleep(Duration::from_millis(100)).await;
            write.write_u8(*byte).await.unwrap();
        }
        write
    });
    let start = tokio::time::Instant::now();
    for expected in b"abc" {
        assert_eq!(read.read_u8().await.unwrap(), *expected);
    }
    assert!(start.elapsed() >= Duration::from_millis(300));
    let _writer = writer.await.unwrap();
    assert_eq!(read.read_u8().await.unwrap_err().kind(), std::io::ErrorKind::TimedOut);
    let (write, _reader) = tokio::io::duplex(1);
    let mut write = Timed::new(write, Duration::from_millis(150));
    write.write_u8(1).await.unwrap();
    assert_eq!(write.write_u8(2).await.unwrap_err().kind(), std::io::ErrorKind::TimedOut);
}

#[tokio::test]
async fn bypass_selection_uses_the_pinned_ip_instead_of_the_host_header() {
    for (bypass, proxied) in [("original.test", true), ("127.0.0.1", false)] {
        let (server_config, tls) = configurations();
        let origin = server(false, false, server_config.clone()).await;
        let proxy = server(false, false, server_config).await;
        let original =
            url::Url::parse(&format!("http://original.test:{}/path", origin.address.port()))
                .unwrap();
        let config = config::Config::from_entries(vec![
            ("HTTP_PROXY".into(), format!("http://{}", proxy.address)),
            ("NO_PROXY".into(), bypass.into()),
        ])
        .unwrap();
        let mut transport = super::super::super::Transport::with_addresses(
            &original,
            Some(vec![origin.address]),
            true,
            Duration::from_secs(2),
        )
        .unwrap();
        transport.session.proxies = Routing {
            config,
        };
        transport.session.tls = Contexts::shared(tls);
        assert_eq!(
            transport.response(&original).await.unwrap().bytes().await.unwrap().as_ref(),
            b"fixture!"
        );
        assert_eq!(proxy.requests.lock().unwrap().len(), usize::from(proxied));
        assert_eq!(origin.requests.lock().unwrap().len(), usize::from(!proxied));
    }
}

#[tokio::test]
async fn a_tunnel_rejects_a_dns_only_certificate_for_its_pinned_ip() {
    let (server_config, tls) = named_configurations(&["original.test"]);
    let proxy = server(false, true, server_config).await;
    let config = config::Config::from_entries(vec![(
        "HTTPS_PROXY".into(),
        format!("http://{}", proxy.address),
    )])
    .unwrap();
    let routing = Routing {
        config,
    };
    let tls = Contexts::shared(tls);
    let original = url::Url::parse("https://original.test/path").unwrap();
    let destination = url::Url::parse("https://203.0.113.1/path").unwrap();
    let failure = match routing
        .get(&original, &destination, true, None, &tls, Duration::from_secs(2))
        .await
    {
        Ok(_) => panic!("an IP tunnel accepted a DNS-only certificate"),
        Err(failure) => failure,
    };
    assert!(failure.connect, "{}", failure.error);
    assert_eq!(proxy.requests.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn proxy_connection_refusals_retry_but_header_timeouts_do_not() {
    let (_, tls) = configurations();
    let tls = Contexts::shared(tls);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    drop(listener);
    let original = url::Url::parse("http://original.test/path").unwrap();
    let config =
        config::Config::from_entries(vec![("HTTP_PROXY".into(), format!("http://{address}"))])
            .unwrap();
    let routing = Routing {
        config,
    };
    let failure = match routing
        .get(&original, &original, false, None, &tls, Duration::from_millis(100))
        .await
    {
        Ok(_) => panic!("closed proxy unexpectedly answered"),
        Err(failure) => failure,
    };
    assert!(failure.connect);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let mut socket: Socket = Box::new(stream);
        let _ = header(&mut socket).await;
        std::future::pending::<()>().await;
    });
    let _server = Server {
        address,
        requests: Arc::default(),
        names: Arc::default(),
        task,
    };
    let config =
        config::Config::from_entries(vec![("HTTP_PROXY".into(), format!("http://{address}"))])
            .unwrap();
    let routing = Routing {
        config,
    };
    let failure = match routing
        .get(&original, &original, false, None, &tls, Duration::from_millis(100))
        .await
    {
        Ok(_) => panic!("stalled proxy unexpectedly answered"),
        Err(failure) => failure,
    };
    assert!(!failure.connect);
}
