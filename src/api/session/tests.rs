use tokio::io::{AsyncReadExt, AsyncWriteExt};

use super::*;

#[tokio::test]
async fn new_clients_and_clones_share_api_cookies_without_debug_disclosure() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let path = format!("/api-session-{}", address.port());
    let cookie_path = path.clone();
    let worker = tokio::spawn(async move {
        let mut requests = Vec::new();
        for index in 0..4 {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = Vec::new();
            while !request.ends_with(b"\r\n\r\n") {
                assert_ne!(socket.read_buf(&mut request).await.unwrap(), 0);
                assert!(request.len() <= 16384);
            }
            requests.push(String::from_utf8(request).unwrap());
            let cookie = match index {
                0 => format!("Set-Cookie: api_session_probe=SecretProbe; Path={cookie_path}\r\n"),
                2 => format!(
                    "Set-Cookie: api_session_probe=deleted; Max-Age=0; Path={cookie_path}\r\n"
                ),
                _ => String::new(),
            };
            socket.write_all(format!("HTTP/1.1 200 OK\r\nContent-Length: 2\r\n{cookie}Connection: close\r\n\r\n{{}}").as_bytes()).await.unwrap();
        }
        requests
    });
    let first = ApiClient::new().unwrap();
    let second = ApiClient::new().unwrap();
    let cloned = first.clone();
    for client in [&first, &second, &cloned, &second] {
        let request = client.inner.get(format!("http://{address}{path}/probe")).build().unwrap();
        let response =
            tokio::time::timeout(std::time::Duration::from_secs(5), client.send(request))
                .await
                .unwrap()
                .unwrap();
        response.bytes().await.unwrap();
        assert!(!format!("{client:?}").contains("SecretProbe"));
    }
    let requests = worker.await.unwrap();
    for (index, request) in requests.iter().enumerate() {
        assert_eq!(
            request.contains("api_session_probe=SecretProbe"),
            matches!(index, 1 | 2),
            "{index}: {request}"
        );
    }
}
