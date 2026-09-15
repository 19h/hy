//! Enumerate Unix IPC endpoints and remove entries whose process has exited.

use std::path::Path;

use super::Instance;

pub(super) fn candidates() -> Vec<Instance> {
    discover_at(Path::new("/tmp"), process_alive)
}

fn discover_at(directory: &Path, alive: impl Fn(u32) -> bool) -> Vec<Instance> {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return Vec::new();
    };
    let mut instances = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(pid) = name
            .to_str()
            .and_then(|name| name.strip_prefix("ida_ipc_"))
            .and_then(|pid| pid.parse::<u32>().ok())
            .filter(|pid| *pid > 0 && *pid <= i32::MAX as u32)
        else {
            continue;
        };
        if alive(pid) {
            instances.push(Instance {
                pid,
                socket: entry.path(),
                idb_path: None,
            });
        } else {
            // Match upstream's best-effort unlink: a failed cleanup does not
            // prevent discovery of the remaining endpoints.
            let _ = std::fs::remove_file(entry.path());
        }
    }
    instances
}

fn process_alive(pid: u32) -> bool {
    let Ok(pid) = i32::try_from(pid) else {
        return false;
    };
    if pid <= 0 {
        return false;
    }
    // SAFETY: signal zero only probes this positive PID; no signal is delivered.
    unsafe { libc::kill(pid, 0) == 0 }
}

#[cfg(test)]
mod tests;
