use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};

use super::super::{Session, Transport};
use super::*;

#[test]
fn ca_files_reject_missing_empty_and_invalid_certificate_data() {
    let directory = tempfile::tempdir().unwrap();
    let file = directory.path().join("roots.pem");
    assert!(Contexts::from_ca_file(Some(&file)).is_err());
    for content in
        ["", "not a certificate", "-----BEGIN CERTIFICATE-----\nYWJj\n-----END CERTIFICATE-----\n"]
    {
        std::fs::write(&file, content).unwrap();
        assert!(Contexts::from_ca_file(Some(&file)).is_err(), "{content:?}");
    }
}

#[test]
fn ca_file_loading_matches_cpython_for_pem_bundles() {
    let certificate = rcgen::generate_simple_self_signed(vec!["origin.test".into()]).unwrap();
    let pem = certificate.cert.pem();
    let invalid = "-----BEGIN CERTIFICATE-----\nYWJj\n-----END CERTIFICATE-----\n";
    let cases = [
        (String::new(), false),
        ("not a certificate".into(), false),
        (invalid.into(), false),
        (pem.clone(), true),
        (format!("{pem}{pem}"), true),
        (format!("comment\n{pem}"), true),
        (format!("{pem}comment\n"), true),
        (pem.replace('\n', "\r\n"), true),
        (pem.trim_end().to_owned(), true),
        (format!("{}{pem}", certificate.signing_key.serialize_pem()), true),
        (format!("{pem}{invalid}"), false),
    ];
    let directory = tempfile::tempdir().unwrap();
    let mut paths = vec![directory.path().join("missing.pem")];
    let mut results = vec![false];
    for (index, (contents, accepted)) in cases.iter().enumerate() {
        let path = directory.path().join(format!("{index}.pem"));
        std::fs::write(&path, contents).unwrap();
        let result = Contexts::from_ca_file(Some(&path)).is_ok();
        assert_eq!(result, *accepted, "case {index}");
        paths.push(path);
        results.push(result);
    }
    if let Some(python) = std::env::var_os("HY_TEST_HTTPX_ORACLE_PYTHON") {
        let output = std::process::Command::new(python)
            .args([
                "-I",
                "-B",
                "-c",
                r#"
import json, ssl, sys
results = []
for path in sys.argv[1:]:
    try:
        ssl.create_default_context(cafile=path)
    except (OSError, ValueError):
        results.append(False)
    else:
        results.append(True)
print(json.dumps(results))
"#,
            ])
            .args(&paths)
            .output()
            .unwrap();
        assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
        let expected: Vec<bool> = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(results, expected);
    }
}

#[tokio::test]
async fn direct_tls_uses_the_session_ca_file_and_verifies_the_original_host() {
    for (trusted, matching_name) in [(true, true), (false, true), (true, false)] {
        let certificate = rcgen::generate_simple_self_signed(vec!["origin.test".into()]).unwrap();
        let key =
            rustls::pki_types::PrivatePkcs8KeyDer::from(certificate.signing_key.serialize_der());
        let server_config = rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(vec![certificate.cert.der().clone()], key.into())
            .unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let task = tokio::spawn(async move {
            let (socket, _) = listener.accept().await.unwrap();
            let acceptor = tokio_rustls::TlsAcceptor::from(Arc::new(server_config));
            let Ok(mut socket) = acceptor.accept(socket).await else {
                return None;
            };
            let name = socket.get_ref().1.server_name().map(str::to_owned);
            let mut header = Vec::new();
            while !header.ends_with(b"\r\n\r\n") {
                header.push(socket.read_u8().await.unwrap());
                assert!(header.len() < 65536);
            }
            socket
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}")
                .await
                .unwrap();
            Some((name, String::from_utf8(header).unwrap()))
        });
        let directory = tempfile::tempdir().unwrap();
        let file = directory.path().join("roots.pem");
        std::fs::write(&file, certificate.cert.pem()).unwrap();
        let mut session = Session::new(true, Duration::from_secs(2));
        if trusted {
            session.tls = Contexts::from_ca_file(Some(&file)).unwrap();
        }
        let host = if matching_name {
            "origin.test"
        } else {
            "wrong.test"
        };
        let url = url::Url::parse(&format!("https://{host}:{}/content", address.port())).unwrap();
        let transport = Transport::from_addresses(&url, Some(vec![address]), session).unwrap();
        let response = transport.response(&url).await;
        if trusted && matching_name {
            assert_eq!(response.unwrap().bytes().await.unwrap().as_ref(), b"{}");
            let (name, header) = task.await.unwrap().unwrap();
            assert_eq!(name.as_deref(), Some("origin.test"));
            assert!(
                header
                    .to_lowercase()
                    .contains(&format!("host: origin.test:{}\r\n", address.port()))
            );
        } else {
            assert!(response.is_err(), "accepted trusted={trusted}, matching_name={matching_name}");
            assert!(task.await.unwrap().is_none());
        }
    }
}
