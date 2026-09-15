use std::fs;
use std::os::unix::net::UnixListener;

use super::*;

#[test]
fn discovery_retains_live_candidates_and_cleans_stale_entries() {
    let directory = tempfile::tempdir().unwrap();
    let live = directory.path().join("ida_ipc_90");
    let stale = directory.path().join("ida_ipc_7");
    let _live_listener = UnixListener::bind(&live).unwrap();
    let _stale_listener = UnixListener::bind(&stale).unwrap();
    // Failed unlink operations are advisory, including an unexpected directory.
    fs::create_dir(directory.path().join("ida_ipc_8")).unwrap();
    for name in ["unrelated", "ida_ipc_invalid", "ida_ipc_0", "ida_ipc_-1", "ida_ipc_2147483648"] {
        fs::write(directory.path().join(name), b"retain").unwrap();
    }
    let instances = discover_at(directory.path(), |pid| pid == 90);
    assert_eq!(instances.len(), 1);
    assert_eq!(instances[0].pid, 90);
    assert_eq!(instances[0].socket, live);
    assert!(instances[0].idb_path.is_none());
    assert!(live.exists());
    assert!(!stale.exists());
    assert!(directory.path().join("ida_ipc_8").is_dir());
    for name in ["unrelated", "ida_ipc_invalid", "ida_ipc_0", "ida_ipc_-1", "ida_ipc_2147483648"] {
        assert_eq!(fs::read(directory.path().join(name)).unwrap(), b"retain");
    }
}

#[test]
fn absent_discovery_directories_produce_no_candidates() {
    let directory = tempfile::tempdir().unwrap();
    let instances = discover_at(&directory.path().join("missing"), |_| {
        panic!("an absent directory has no processes to probe")
    });
    assert!(instances.is_empty());
}

#[test]
fn liveness_probes_accept_the_current_process_and_reject_process_group_identifiers() {
    assert!(process_alive(std::process::id()));
    assert!(!process_alive(0));
    assert!(!process_alive(u32::MAX));
}
