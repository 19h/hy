//! Cache freshness, force, transfer failure, and path confinement regressions.

mod support;

use serde_json::json;
use sha2::{Digest, Sha256};
use std::fs;
use support::{
    auth::*,
    http::{Response, Server},
    *,
};

#[test]
fn cache_reuse_requires_successful_head_and_force_skips_the_check() {
    for (status, force) in [(200, false), (403, false), (500, false), (200, true)] {
        let sandbox = Sandbox::new();
        write_config(&sandbox, &stored("key", "fixture-key"));
        let cache = sandbox.path().join("cache/downloads/release/file");
        fs::create_dir_all(cache.parent().unwrap()).unwrap();
        fs::write(&cache, b"stale").unwrap();
        fs::write(cache.with_file_name("file.sha256"), format!("{:x}", Sha256::digest(b"stale")))
            .unwrap();
        let server = Server::start(move |request, base| match request.path.as_str() {
            "/api/assets/installers/release/file" => Response::json(json!({
                "key": "/release/file", "filename": "file", "url": format!("{base}/file.bin")
            })),
            "/file.bin" => Response {
                status: if request.method == "HEAD" {
                    status
                } else {
                    200
                },
                content_type: "application/octet-stream",
                body: b"fresh".to_vec(),
            },
            _ => Response::missing(),
        });
        let mut args =
            vec!["download", "/release/file", "--output-dir", sandbox.path().to_str().unwrap()];
        if force {
            args.push("--force");
        }
        assert_success(&command(&sandbox, &server, &args).output().unwrap());
        let expected = if status == 200 && !force {
            b"stale"
        } else {
            b"fresh"
        };
        assert_eq!(fs::read(sandbox.path().join("file.bin")).unwrap(), expected);
        assert_eq!(
            fs::metadata(sandbox.path().join("file.bin")).unwrap().modified().unwrap(),
            fs::metadata(&cache).unwrap().modified().unwrap()
        );
        let methods: Vec<_> = server.requests().into_iter().map(|request| request.method).collect();
        assert_eq!(
            methods,
            if force {
                vec!["GET", "GET"]
            } else if status == 200 {
                vec!["GET", "HEAD"]
            } else {
                vec!["GET", "HEAD", "GET"]
            }
        );
    }
}

#[test]
fn interrupted_transfer_preserves_existing_cache_and_target_and_removes_staging() {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::time::{Duration, Instant};

    let sandbox = Sandbox::new();
    write_config(&sandbox, &stored("key", "fixture-key"));
    let cache = sandbox.path().join("cache/downloads/fixture");
    fs::create_dir_all(cache.parent().unwrap()).unwrap();
    fs::write(&cache, b"old cache").unwrap();
    let target = sandbox.path().join("file.bin");
    fs::write(&target, b"old target").unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let url = format!("http://{}/file.bin", listener.local_addr().unwrap());
    let worker = std::thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(3);
        let mut stream = loop {
            match listener.accept() {
                Ok((stream, _)) => break stream,
                Err(error)
                    if error.kind() == std::io::ErrorKind::WouldBlock
                        && Instant::now() < deadline =>
                {
                    std::thread::sleep(Duration::from_millis(5))
                }
                Err(error) => panic!("content fixture did not receive a connection: {error}"),
            }
        };
        stream.set_nonblocking(false).unwrap();
        stream.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
        let mut request = Vec::new();
        while !request.windows(4).any(|bytes| bytes == b"\r\n\r\n") {
            let mut bytes = [0; 1024];
            let count = stream.read(&mut bytes).unwrap();
            assert!(count > 0 && request.len() < 16_384);
            request.extend_from_slice(&bytes[..count]);
        }
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 100\r\nConnection: close\r\n\r\nshort")
            .unwrap();
    });
    let server = Server::start(move |_, _| {
        Response::json(json!({"key": "fixture", "filename": "file", "url": url}))
    });
    let output = command(
        &sandbox,
        &server,
        &["download", "fixture", "--force", "--output-dir", sandbox.path().to_str().unwrap()],
    )
    .output()
    .unwrap();
    worker.join().unwrap();
    assert!(!output.status.success());
    assert_eq!(fs::read(&cache).unwrap(), b"old cache");
    assert_eq!(fs::read(target).unwrap(), b"old target");
    let files: Vec<_> = fs::read_dir(cache.parent().unwrap())
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    assert_eq!(files, ["fixture"]);
}

#[test]
fn parent_traversal_cache_keys_are_rejected_before_content_download() {
    let sandbox = Sandbox::new();
    write_config(&sandbox, &stored("key", "fixture-key"));
    let server = Server::start(|_, base| {
        Response::json(json!({
            "key": "../outside", "filename": "file", "url": format!("{base}/content")
        }))
    });
    assert!(
        !command(
            &sandbox,
            &server,
            &["download", "../outside", "--output-dir", sandbox.path().to_str().unwrap()]
        )
        .output()
        .unwrap()
        .status
        .success()
    );
    assert_eq!(server.requests().len(), 1);
    assert!(!sandbox.path().join("cache/outside").exists());
}

#[test]
fn empty_cache_override_uses_the_platform_default() {
    let sandbox = Sandbox::new();
    write_config(&sandbox, &stored("key", "fixture-key"));
    let server = Server::start(|request, base| match request.path.as_str() {
        "/api/assets/installers/fixture" => Response::json(json!({
            "key": "fixture", "filename": "file", "url": format!("{base}/file.bin")
        })),
        "/file.bin" => Response {
            status: 200,
            content_type: "application/octet-stream",
            body: b"fixture".to_vec(),
        },
        _ => Response::missing(),
    });
    assert_success(
        &command(&sandbox, &server, &["download", "fixture", "--output-dir", "output"])
            .current_dir(sandbox.path())
            .env("HCLI_CACHE_DIR", "")
            .output()
            .unwrap(),
    );
    assert!(!sandbox.path().join("downloads").exists());
    assert_eq!(fs::read(sandbox.path().join("output/file.bin")).unwrap(), b"fixture");
    let root = if cfg!(target_os = "macos") {
        sandbox.path().join("Library/Caches/hex-rays/hcli")
    } else if cfg!(target_os = "windows") {
        sandbox.path().join("local/hex-rays/hcli/cache")
    } else {
        sandbox.path().join("cache")
    };
    assert_eq!(fs::read(root.join("downloads/fixture")).unwrap(), b"fixture");
}
