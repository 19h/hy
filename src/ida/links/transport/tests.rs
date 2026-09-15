use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV6};
use std::time::Duration;

use sha2::{Digest, Sha256};

use super::{Transport, addresses, hostname, redirect};

mod cookies;
mod server;
use server::{Reply, Server};

const TEST_TIMEOUT: Duration = Duration::from_millis(600);

fn pinned(url: &url::Url, addresses: Vec<SocketAddr>) -> Transport {
    Transport::with_addresses(url, Some(addresses), false, TEST_TIMEOUT).unwrap()
}

#[test]
fn address_validation_preserves_order_and_rejects_any_blocked_result() {
    let first = "8.8.8.8:443".parse().unwrap();
    let second = "1.1.1.1:443".parse().unwrap();
    assert_eq!(addresses::validate(vec![first, second, first]).unwrap(), [first, second]);
    assert!(addresses::validate(vec![]).is_err());
    assert!(addresses::validate(vec![first, "127.0.0.1:443".parse().unwrap()]).is_err());
}

#[test]
fn blocks_private_and_embedded_addresses() {
    for ip in [
        "127.0.0.1",
        "10.0.0.1",
        "100.64.0.1",
        "169.254.1.1",
        "::1",
        "::ffff:127.0.0.1",
        "2001::1",
        "2002:7f00:1::",
    ] {
        assert!(addresses::blocked(ip.parse().unwrap()), "{ip}");
    }
    assert!(!addresses::blocked("8.8.8.8".parse().unwrap()));
}

#[test]
fn ip_classification_matches_the_pinned_python_digest() {
    let mut digest = Sha256::new();
    let mut count = 0;
    let mut blocked = 0;
    let mut record = |ip: IpAddr| {
        let result = u8::from(addresses::blocked(ip));
        digest.update([result]);
        count += 1;
        blocked += u32::from(result);
    };
    for first in 0..=u8::MAX {
        for second in [0, 16, 18, 19, 31, 51, 63, 64, 100, 127, 128, 168, 254, 255] {
            for third in [0, 2, 51, 100, 113, 255] {
                for last in [0, 9, 10, 255] {
                    record(Ipv4Addr::new(first, second, third, last).into());
                }
            }
        }
    }
    for part in 0..=u16::MAX {
        for (first, second) in [(part, 0), (0x2001, part), (0x3fff, part)] {
            record(Ipv6Addr::new(first, second, 0, 0, 0, 0, 0, 1).into());
        }
    }
    assert_eq!(count, 282_624);
    assert_eq!(blocked, 73_896);
    assert_eq!(
        format!("{:x}", digest.finalize()),
        "12cf5623b3f909af1e07f24a441f7889f49f146373b2eff28cc487623323fc5d"
    );
    for (text, expected) in [
        ("192.0.0.9", false),
        ("192.0.0.10", false),
        ("2001:1::1", false),
        ("2001:1::2", false),
        ("2001:1::3", true),
        ("2001:3::", false),
        ("2001:4:112::", false),
        ("2001:20::", false),
        ("2001:30::", false),
        ("fec0::", false),
        ("3fff:fff::", true),
        ("3fff:1000::", false),
        ("::ffff:8.8.8.8", false),
        ("::ffff:127.0.0.1", true),
        ("2002:0808:0808::", false),
        ("2002:7f00:1::", true),
    ] {
        assert_eq!(addresses::blocked(text.parse().unwrap()), expected, "{text}");
    }
}

#[test]
fn url_validation_accepts_userinfo_and_unbrackets_ipv6_for_resolution() {
    assert_eq!(
        hostname(&url::Url::parse("https://user:pass@example.test/a").unwrap()).unwrap(),
        "example.test"
    );
    assert_eq!(hostname(&url::Url::parse("http://[::1]:8080/a").unwrap()).unwrap(), "::1");
    assert!(hostname(&url::Url::parse("ftp://example.test/a").unwrap()).is_err());
}

#[tokio::test]
async fn connection_failure_tries_the_next_pin_and_preserves_the_original_host() {
    // An invalid link-local interface scope fails without a reusable-port race.
    let unavailable_address =
        SocketAddr::V6(SocketAddrV6::new("fe80::1".parse().unwrap(), 443, 0, u32::MAX));
    let server = Server::start(vec![Reply::ok(b"content")]).await;
    let url = url::Url::parse("http://user:pass@fixture.test/resource?token=fixture").unwrap();
    let transport = pinned(&url, vec![unavailable_address, server.address]);
    assert_eq!(transport.response(&url).await.unwrap().bytes().await.unwrap(), "content");
    let requests = server.requests();
    assert_eq!(requests.len(), 1);
    let request = requests[0].to_ascii_lowercase();
    assert!(request.starts_with("get /resource?token=fixture http/1.1\r\n"), "{request}");
    assert!(request.contains("\r\nhost: fixture.test\r\n"), "{request}");
    assert!(!request.contains("authorization:"), "{request}");
}

#[tokio::test]
async fn private_override_retains_url_basic_authentication() {
    let server = Server::start(vec![Reply::ok(b"content")]).await;
    let mut url = server.url();
    url.set_username("user").unwrap();
    url.set_password(Some("pass")).unwrap();
    let transport = Transport::with_addresses(&url, None, true, TEST_TIMEOUT).unwrap();
    transport.response(&url).await.unwrap();
    assert!(
        server.requests()[0].to_ascii_lowercase().contains("authorization: basic dxnlcjpwyxnz")
    );
}

#[tokio::test]
async fn a_transport_cannot_send_to_another_origin() {
    let server = Server::start(vec![Reply::ok(b"unused")]).await;
    let original = url::Url::parse("http://fixture.test/resource").unwrap();
    let error = pinned(&original, vec![server.address]).response(&server.url()).await.unwrap_err();
    assert!(error.to_string().contains("origin does not match"), "{error}");
    assert!(server.requests().is_empty());
}

#[tokio::test]
async fn an_http_error_does_not_try_another_pin() {
    let first = Server::start(vec![Reply::status(503)]).await;
    let second = Server::start(vec![Reply::ok(b"unused")]).await;
    let url = url::Url::parse("http://fixture.test/resource").unwrap();
    let error = pinned(&url, vec![first.address, second.address]).response(&url).await.unwrap_err();
    assert!(error.to_string().contains("HTTP 503"), "{error}");
    assert_eq!(first.requests().len(), 1);
    assert!(second.requests().is_empty());
}

#[tokio::test]
async fn header_and_body_read_timeouts_do_not_try_another_pin() {
    for delay_headers in [true, false] {
        let mut reply = Reply::ok(b"delayed");
        if delay_headers {
            reply.header_delay = Duration::from_secs(3);
        } else {
            reply.chunks[0].0 = Duration::from_secs(3);
        }
        let first = Server::start(vec![reply]).await;
        let second = Server::start(vec![Reply::ok(b"unused")]).await;
        let url = url::Url::parse("http://fixture.test/resource").unwrap();
        let transport = pinned(&url, vec![first.address, second.address]);
        if delay_headers {
            let error = transport.response(&url).await.unwrap_err();
            assert!(matches!(error, crate::error::Error::Http(error) if error.is_timeout()));
        } else {
            let error = transport.response(&url).await.unwrap().bytes().await.unwrap_err();
            assert!(error.is_timeout(), "{error}");
        }
        assert_eq!(first.requests().len(), 1);
        assert!(second.requests().is_empty());
    }
}

#[tokio::test]
async fn a_progressing_body_can_outlast_the_read_timeout() {
    let mut reply = Reply::ok(b"");
    reply.chunks = vec![(Duration::from_millis(250), b"x".to_vec()); 4];
    let server = Server::start(vec![reply]).await;
    let url = url::Url::parse("http://fixture.test/resource").unwrap();
    let started = std::time::Instant::now();
    let bytes =
        pinned(&url, vec![server.address]).response(&url).await.unwrap().bytes().await.unwrap();
    assert_eq!(bytes, "xxxx");
    assert!(started.elapsed() > TEST_TIMEOUT);
}

#[tokio::test]
async fn redirects_follow_supported_statuses_and_preserve_relative_queries() {
    for status in [301, 302, 303, 307, 308] {
        let server = Server::start(vec![
            Reply::redirect(status, "../done?token=next"),
            Reply::ok(b"content"),
        ])
        .await;
        let mut url = server.url();
        url.set_path("/api/start");
        let transport = Transport::with_addresses(&url, None, true, TEST_TIMEOUT).unwrap();
        assert_eq!(transport.response(&url).await.unwrap().bytes().await.unwrap(), "content");
        assert!(server.requests()[1].starts_with("GET /done?token=next HTTP/1.1\r\n"));
    }
    for status in [300, 302, 304] {
        let server = Server::start(vec![Reply::status(status)]).await;
        let url = server.url();
        let transport = Transport::with_addresses(&url, None, true, TEST_TIMEOUT).unwrap();
        let error = transport.response(&url).await.unwrap_err();
        assert!(error.to_string().contains(&format!("HTTP {status}")), "{error}");
        assert_eq!(server.requests().len(), 1);
    }
}

#[tokio::test]
async fn redirect_targets_are_revalidated_before_connection() {
    let target = Server::start(vec![Reply::ok(b"unused")]).await;
    for location in [target.url().to_string(), "ftp://example.test/file".into()] {
        let server = Server::start(vec![Reply::redirect(302, &location)]).await;
        let url = url::Url::parse("http://fixture.test/resource").unwrap();
        let error = pinned(&url, vec![server.address]).response(&url).await.unwrap_err();
        assert!(
            error.to_string().contains("non-public") || error.to_string().contains("HTTP(S)"),
            "{error}"
        );
        assert_eq!(server.requests().len(), 1);
        assert!(target.requests().is_empty());
    }
}

#[tokio::test]
async fn five_redirects_succeed_but_a_sixth_exhausts_the_budget() {
    for succeeds in [true, false] {
        let mut replies = vec![Reply::redirect(302, "/next"); 5];
        replies.push(if succeeds {
            Reply::ok(b"done")
        } else {
            Reply::redirect(302, "/next")
        });
        let server = Server::start(replies).await;
        let url = server.url();
        let transport = Transport::with_addresses(&url, None, true, TEST_TIMEOUT).unwrap();
        let result = transport.response(&url).await;
        assert_eq!(result.is_ok(), succeeds);
        if let Err(error) = result {
            assert!(error.to_string().contains("too many KE redirects"), "{error}");
        }
        assert_eq!(server.requests().len(), 6);
    }
}

#[test]
fn https_redirects_can_select_http_targets_as_upstream_allows() {
    let url = url::Url::parse("https://example.test/resource").unwrap();
    let response: reqwest::Response = hyper::Response::builder()
        .status(302)
        .header("Location", "http://example.test/file")
        .body(Vec::<u8>::new())
        .unwrap()
        .into();
    assert_eq!(redirect(&url, &response).unwrap().unwrap().as_str(), "http://example.test/file");
}
