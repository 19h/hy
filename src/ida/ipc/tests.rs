use super::*;

#[tokio::test]
async fn named_lookup_preserves_candidate_order_and_stops_after_the_first_match() {
    use futures_util::FutureExt;

    let directory = tempfile::tempdir().unwrap();
    let mut candidates = Vec::new();
    let mut servers = Vec::new();
    for (pid, response) in [
        (88, json!({"status":"error"})),
        (89, json!({"status":"ok","idb_path":""})),
        (90, json!({"status":"ok","idb_path":"/fixture/selected.i64"})),
    ] {
        let socket = directory.path().join(format!("ida_ipc_{pid}"));
        let listener = tokio::net::UnixListener::bind(&socket).unwrap();
        candidates.push(Instance {
            pid,
            socket,
            idb_path: None,
        });
        servers.push(tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request = [0; 128];
            let count = stream.read(&mut request).await.unwrap();
            assert_eq!(
                serde_json::from_slice::<Value>(&request[..count]).unwrap(),
                json!({"cmd":"get_info"})
            );
            stream.write_all(&serde_json::to_vec(&response).unwrap()).await.unwrap();
        }));
    }
    let socket = directory.path().join("ida_ipc_7");
    let unqueried = tokio::net::UnixListener::bind(&socket).unwrap();
    candidates.push(Instance {
        pid: 7,
        socket,
        idb_path: None,
    });
    let selected = find_in(candidates, |instance| instance.idb_path.is_some()).await.unwrap();
    assert_eq!(selected.pid, 90);
    assert_eq!(selected.idb_path, Some(PathBuf::from("/fixture/selected.i64")));
    assert!(unqueried.accept().now_or_never().is_none());
    for server in servers {
        server.await.unwrap();
    }
}

#[tokio::test]
async fn reads_fragmented_json_from_same_user_peer() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("socket");
    let listener = tokio::net::UnixListener::bind(&path).unwrap();
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut data = [0; 256];
        let n = stream.read(&mut data).await.unwrap();
        assert_eq!(serde_json::from_slice::<Value>(&data[..n]).unwrap()["cmd"], "ping");
        stream.write_all(b"{\"status\":").await.unwrap();
        tokio::task::yield_now().await;
        stream.write_all(b"\"ok\"}").await.unwrap();
    });
    assert_eq!(send(&path, json!({"cmd":"ping"})).await.unwrap()["status"], "ok");
    server.await.unwrap();
}

#[tokio::test]
async fn progressing_response_can_outlast_seven_seconds() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("socket");
    let listener = tokio::net::UnixListener::bind(&path).unwrap();
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut request = [0; 128];
        assert!(stream.read(&mut request).await.unwrap() > 0);
        for chunk in [b"{\"status\":".as_slice(), b"\"ok\"".as_slice(), b"}".as_slice()] {
            tokio::time::sleep(Duration::from_secs(3)).await;
            stream.write_all(chunk).await.unwrap();
        }
    });
    assert_eq!(send(&path, json!({"cmd":"ping"})).await.unwrap(), json!({"status":"ok"}));
    server.await.unwrap();
}

#[tokio::test(start_paused = true)]
async fn an_idle_response_ends_after_five_seconds() {
    let (mut client, _server) = tokio::io::duplex(1024);
    let started = tokio::time::Instant::now();
    let error = exchange(&mut client, &json!({"cmd":"ping"})).await.unwrap_err();
    assert!(error.to_string().contains("No IDA IPC response"));
    assert_eq!(started.elapsed(), READ_TIMEOUT);
}

#[tokio::test(start_paused = true)]
async fn a_blocked_write_ends_after_two_seconds() {
    let (mut client, _server) = tokio::io::duplex(1);
    let started = tokio::time::Instant::now();
    let error = exchange(&mut client, &json!({"cmd":"ping"})).await.unwrap_err();
    assert!(error.to_string().contains("write timed out"));
    assert_eq!(started.elapsed(), CONNECT_TIMEOUT);
}

#[tokio::test]
async fn malformed_responses_still_obey_the_one_mib_bound() {
    let (mut client, mut server) = tokio::io::duplex(8192);
    let writer = tokio::spawn(async move {
        let mut request = [0; 128];
        assert!(server.read(&mut request).await.unwrap() > 0);
        let _ = server.write_all(&vec![b'['; MAX_RESPONSE_BYTES + 1]).await;
    });
    let error = exchange(&mut client, &json!({"cmd":"ping"})).await.unwrap_err();
    assert!(error.to_string().contains("exceeds 1 MiB"));
    drop(client);
    writer.await.unwrap();
}
