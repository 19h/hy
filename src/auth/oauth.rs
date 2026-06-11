//! Local HTTP server for capturing OAuth redirect tokens.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::error::Result;

/// HTML page served at the OAuth callback endpoint.
const CALLBACK_HTML: &str = r#"<!DOCTYPE html>
<html lang="en">
<head><meta charset="UTF-8"><title>Login</title></head>
<body>
<script>
const hp = new URLSearchParams(window.location.hash.substring(1));
const at = hp.get("access_token");
const rt = hp.get("refresh_token");
if (at) {
  fetch("http://localhost:9999/token", {
    method: "POST",
    headers: {"Content-Type":"application/json"},
    body: JSON.stringify({access_token:at,refresh_token:rt}),
  })
  .then(()=>{document.body.innerHTML="Login successful! You can close this tab.";})
  .catch(e=>{document.body.innerHTML="Error saving token.";});
} else {
  document.body.innerHTML="No token found in URL.";
}
</script>
</body>
</html>"#;

/// Token pair received from the OAuth callback.
#[derive(Debug, Clone)]
pub struct OAuthTokens {
    pub access_token: String,
    pub refresh_token: Option<String>,
}

/// A tiny HTTP server that listens for the OAuth redirect.
pub struct OAuthServer {
    port: u16,
    result: Arc<Mutex<Option<OAuthTokens>>>,
}

impl OAuthServer {
    pub fn new(port: u16) -> Self {
        Self {
            port,
            result: Arc::new(Mutex::new(None)),
        }
    }

    /// Run the server, blocking until a token is received or timeout elapses.
    ///
    /// Returns `None` on timeout.
    pub fn run(&self, timeout: Duration) -> Result<Option<OAuthTokens>> {
        let listener = TcpListener::bind(("127.0.0.1", self.port))
            .map_err(|e| crate::error::Error::OAuthFailed(format!("bind failed: {e}")))?;
        listener
            .set_nonblocking(true)
            .map_err(|e| crate::error::Error::OAuthFailed(format!("set_nonblocking: {e}")))?;

        let deadline = std::time::Instant::now() + timeout;

        while std::time::Instant::now() < deadline {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    let mut buf = vec![0u8; 8192];
                    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
                    let n = stream.read(&mut buf).unwrap_or(0);
                    let request = String::from_utf8_lossy(&buf[..n]);

                    if request.starts_with("GET /callback") {
                        // Serve the HTML page that extracts tokens from the hash.
                        let response = format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\n\r\n{}",
                            CALLBACK_HTML.len(),
                            CALLBACK_HTML
                        );
                        let _ = stream.write_all(response.as_bytes());
                    } else if request.starts_with("POST /token") {
                        // Extract JSON body.
                        if let Some(body_start) = request.find("\r\n\r\n") {
                            let body = &request[body_start + 4..];
                            if let Ok(val) = serde_json::from_str::<serde_json::Value>(body)
                                && let Some(at) = val.get("access_token").and_then(|v| v.as_str()) {
                                    let tokens = OAuthTokens {
                                        access_token: at.to_owned(),
                                        refresh_token: val
                                            .get("refresh_token")
                                            .and_then(|v| v.as_str())
                                            .map(String::from),
                                    };
                                    *self.result.lock().unwrap() = Some(tokens);

                                    let resp = "HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n";
                                    let _ = stream.write_all(resp.as_bytes());

                                    // Token received — break out.
                                    break;
                                }
                        }
                        let resp = "HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\n\r\n";
                        let _ = stream.write_all(resp.as_bytes());
                    } else {
                        let resp = "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n";
                        let _ = stream.write_all(resp.as_bytes());
                    }
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(100));
                }
                Err(e) => {
                    return Err(crate::error::Error::OAuthFailed(format!(
                        "accept failed: {e}"
                    )));
                }
            }
        }

        Ok(self.result.lock().unwrap().clone())
    }
}
