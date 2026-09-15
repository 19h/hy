use super::super::Transport;
use super::server::{Reply, Server};
use super::{TEST_TIMEOUT, pinned};

fn cookie(request: &str) -> Option<&str> {
    request
        .lines()
        .filter_map(|line| line.split_once(':'))
        .find(|(name, _)| name.eq_ignore_ascii_case("cookie"))
        .map(|(_, value)| value.trim())
}

#[tokio::test]
async fn responses_update_the_session_but_new_sessions_start_empty() {
    let server = Server::start(vec![
        Reply::ok(b"metadata").cookie("session=one; Path=/"),
        Reply::ok(b"content"),
        Reply::ok(b"independent"),
    ])
    .await;
    let url = server.url();
    let transport = Transport::with_addresses(&url, None, true, TEST_TIMEOUT).unwrap();
    transport.response(&url).await.unwrap();
    transport.response(&url).await.unwrap();
    let independent = Transport::with_addresses(&url, None, true, TEST_TIMEOUT).unwrap();
    independent.response(&url).await.unwrap();
    let requests = server.requests();
    assert_eq!(cookie(&requests[0]), None);
    assert_eq!(cookie(&requests[1]), Some("session=one"));
    assert_eq!(cookie(&requests[2]), None);
}

#[tokio::test]
async fn redirects_share_cookies_with_the_original_transport() {
    let target =
        Server::start(vec![Reply::ok(b"redirected").cookie("session=updated; Path=/")]).await;
    let source = Server::start(vec![
        Reply::redirect(302, target.url().as_str()).cookie("session=initial; Path=/"),
        Reply::ok(b"content"),
    ])
    .await;
    let url = source.url();
    let transport = Transport::with_addresses(&url, None, true, TEST_TIMEOUT).unwrap();
    transport.response(&url).await.unwrap();
    transport.response(&url).await.unwrap();
    assert_eq!(cookie(&target.requests()[0]), Some("session=initial"));
    assert_eq!(cookie(&source.requests()[1]), Some("session=updated"));
}

#[tokio::test]
async fn pins_use_ip_cookie_scope_even_when_hostnames_differ() {
    let server = Server::start(vec![
        Reply::ok(b"metadata")
            .cookie("ip=one; Path=/")
            .cookie("domain=ignored; Domain=first.test; Path=/"),
        Reply::ok(b"content"),
    ])
    .await;
    let first_url = url::Url::parse("http://first.test/metadata").unwrap();
    let second_url = url::Url::parse("http://second.test/content").unwrap();
    let first = pinned(&first_url, vec![server.address]);
    first.response(&first_url).await.unwrap();
    let second =
        Transport::from_addresses(&second_url, Some(vec![server.address]), first.session.clone())
            .unwrap();
    second.response(&second_url).await.unwrap();
    let requests = server.requests();
    assert_eq!(cookie(&requests[1]), Some("ip=one"));
    assert!(requests[1].to_ascii_lowercase().contains("host: second.test\r\n"));
}

#[tokio::test]
async fn http_errors_still_update_cookie_state() {
    let server = Server::start(vec![
        Reply::status(404).cookie("session=from-error; Path=/"),
        Reply::ok(b"content"),
    ])
    .await;
    let url = server.url();
    let transport = Transport::with_addresses(&url, None, true, TEST_TIMEOUT).unwrap();
    assert!(transport.response(&url).await.is_err());
    transport.response(&url).await.unwrap();
    assert_eq!(cookie(&server.requests()[1]), Some("session=from-error"));
}
