//! Unix account database lookup for pathlib home expansion.

use std::ffi::{CStr, CString};

use crate::error::{Error, Result};

/// Reentrant account lookup keeps concurrent CLI work independent of libc's buffer.
pub(super) fn account_home(user: &str) -> Result<Option<String>> {
    let name = CString::new(user).map_err(|_| Error::Other("embedded null byte".into()))?;
    let mut buffer = vec![0u8; 1024];
    loop {
        let mut record = std::mem::MaybeUninit::<libc::passwd>::uninit();
        let mut found = std::ptr::null_mut();
        // SAFETY: libc receives writable storage valid for this call. `found`
        // points to `record` only on success; pw_dir then points into `buffer`.
        let status = unsafe {
            if user.is_empty() {
                libc::getpwuid_r(
                    libc::getuid(),
                    record.as_mut_ptr(),
                    buffer.as_mut_ptr().cast(),
                    buffer.len(),
                    &mut found,
                )
            } else {
                libc::getpwnam_r(
                    name.as_ptr(),
                    record.as_mut_ptr(),
                    buffer.as_mut_ptr().cast(),
                    buffer.len(),
                    &mut found,
                )
            }
        };
        if status == libc::ERANGE {
            buffer.resize(buffer.len() * 2, 0);
            continue;
        }
        if status != 0 {
            return Err(std::io::Error::from_raw_os_error(status).into());
        }
        if found.is_null() {
            return Ok(None);
        }
        // SAFETY: successful lookup initialized record and its NUL-terminated
        // home field; copy it before either record or backing buffer is dropped.
        let home = unsafe { CStr::from_ptr(record.assume_init().pw_dir) };
        return Ok(Some(home.to_string_lossy().into_owned()));
    }
}
