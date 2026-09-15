//! Owned IDA IPC server for end-to-end link tests.

use std::collections::VecDeque;
use std::fs;
use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::process::{Child, Command};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use serde_json::{Value, json};

pub struct IpcFixture {
    process: Child,
    path: PathBuf,
    name: String,
    owns_socket: bool,
    stopping: Arc<AtomicBool>,
    navigation: Arc<Mutex<Vec<Value>>>,
    analysis_queries: Arc<AtomicUsize>,
    worker: Option<JoinHandle<()>>,
}

impl IpcFixture {
    pub fn pid(&self) -> u32 {
        self.process.id()
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn navigations(&self) -> Vec<Value> {
        self.navigation.lock().unwrap().clone()
    }

    pub fn analysis_query_count(&self) -> usize {
        self.analysis_queries.load(Ordering::Relaxed)
    }

    pub fn start(reject_navigation: bool) -> Self {
        Self::with_analysis(
            reject_navigation,
            vec![json!({"status":"ok","analysis_complete":true})],
        )
    }

    pub fn with_analysis(reject_navigation: bool, responses: Vec<Value>) -> Self {
        Self::configured(reject_navigation, responses, None, None)
    }

    pub fn for_database(path: PathBuf, reject_navigation: bool) -> Self {
        Self::configured(reject_navigation, Vec::new(), Some(path), None)
    }

    pub fn for_database_after_launch(path: PathBuf, marker: PathBuf) -> Self {
        Self::configured(false, Vec::new(), Some(path), Some(marker))
    }

    fn configured(
        reject_navigation: bool,
        responses: Vec<Value>,
        database: Option<PathBuf>,
        ready_after: Option<PathBuf>,
    ) -> Self {
        let process = Command::new("/bin/sleep").arg("300").spawn().unwrap();
        let mut fixture = Self {
            path: PathBuf::from(format!("/tmp/ida_ipc_{}", process.id())),
            name: format!("hy-fixture-{}-%20.i64", process.id()),
            process,
            owns_socket: false,
            stopping: Arc::new(AtomicBool::new(false)),
            navigation: Arc::new(Mutex::new(Vec::new())),
            analysis_queries: Arc::new(AtomicUsize::new(0)),
            worker: None,
        };
        let listener = UnixListener::bind(&fixture.path).unwrap();
        fixture.owns_socket = true;
        fs::set_permissions(&fixture.path, fs::Permissions::from_mode(0o600)).unwrap();
        listener.set_nonblocking(true).unwrap();
        let stopping = fixture.stopping.clone();
        let navigation = fixture.navigation.clone();
        let database =
            database.unwrap_or_else(|| PathBuf::from(format!("/fixture/{}", fixture.name)));
        let analysis_queries = fixture.analysis_queries.clone();
        fixture.worker = Some(thread::spawn(move || {
            let mut responses: VecDeque<_> = responses.into();
            let mut last_analysis = json!({"status":"ok","analysis_complete":true});
            while !stopping.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        let request = read_request(&mut stream);
                        let analysis = request["cmd"] == "is_analysis_complete";
                        let response = if request["cmd"] == "get_info" {
                            let loaded = ready_after.as_ref().is_none_or(|marker| marker.exists());
                            json!({"status":"ok","idb_path":loaded.then_some(&database)})
                        } else if analysis {
                            if let Some(response) = responses.pop_front() {
                                last_analysis = response;
                            }
                            last_analysis.clone()
                        } else {
                            assert_eq!(request["cmd"], "open_ida_link");
                            navigation.lock().unwrap().push(request);
                            if reject_navigation {
                                json!({"status":"error","message":"fixture navigation rejected"})
                            } else {
                                json!({"status":"ok"})
                            }
                        };
                        // A cancelled IPC request can close before the reply.
                        let _ = stream.write_all(&serde_json::to_vec(&response).unwrap());
                        if analysis {
                            analysis_queries.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(5));
                    }
                    Err(error) => panic!("accept IPC fixture: {error}"),
                }
            }
        }));
        fixture
    }
}

impl Drop for IpcFixture {
    fn drop(&mut self) {
        self.stopping.store(true, Ordering::Relaxed);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
        let _ = self.process.kill();
        let _ = self.process.wait();
        if self.owns_socket {
            let _ = fs::remove_file(&self.path);
        }
    }
}

fn read_request(stream: &mut UnixStream) -> Value {
    // Accepted sockets can inherit the listener's nonblocking mode on macOS.
    // Request reads use the bounded blocking timeout below on every platform.
    stream.set_nonblocking(false).unwrap();
    stream.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
    let mut bytes = Vec::new();
    let mut chunk = [0; 1024];
    loop {
        let count = stream.read(&mut chunk).unwrap();
        assert!(count > 0 && bytes.len() + count <= 8192);
        bytes.extend_from_slice(&chunk[..count]);
        match serde_json::from_slice(&bytes) {
            Ok(request) => return request,
            Err(error) if error.is_eof() => (),
            Err(error) => panic!("invalid fixture request: {error}"),
        }
    }
}
