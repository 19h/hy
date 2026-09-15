//! Probe named pipes for the process identifiers returned by Windows.

use std::fs::OpenOptions;
use std::os::windows::fs::OpenOptionsExt;
use std::path::PathBuf;

use windows_sys::Win32::System::ProcessStatus::EnumProcesses;

use super::Instance;

pub(super) fn candidates() -> Vec<Instance> {
    // Keep the pinned upstream's 4096-process enumeration limit.
    let mut pids = [0_u32; 4096];
    let mut returned_bytes = 0_u32;
    // SAFETY: both output pointers reference initialized writable storage, and
    // the supplied byte count is the exact capacity of the PID array.
    let success =
        unsafe { EnumProcesses(pids.as_mut_ptr(), size_of_val(&pids) as u32, &mut returned_bytes) };
    if success == 0 {
        return Vec::new();
    }
    let count = (returned_bytes as usize / size_of::<u32>()).min(pids.len());
    pids[..count]
        .iter()
        .copied()
        .filter(|pid| *pid != 0)
        .filter_map(|pid| {
            let socket = PathBuf::from(format!(r"\\.\pipe\ida_ipc_{pid}"));
            // The discovery probe opens and immediately closes the pipe. The
            // later JSON exchange obtains a separate asynchronous connection.
            let probe = OpenOptions::new().read(true).write(true).share_mode(0).open(&socket);
            probe.ok().map(|_handle| Instance {
                pid,
                socket,
                idb_path: None,
            })
        })
        .collect()
}
