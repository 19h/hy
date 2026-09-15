//! Drive the actual CLI through an isolated controlling terminal on Unix.

use std::fs::File;
use std::io::{Read, Write};
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::process::CommandExt;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

pub struct Terminal {
    child: Child,
    master: File,
    output: String,
    consumed: usize,
}

impl Terminal {
    pub fn start(mut command: Command) -> Self {
        let mut master = -1;
        let mut slave = -1;
        let mut size = libc::winsize {
            ws_row: 40,
            ws_col: 160,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };
        // SAFETY: openpty receives writable descriptor slots and a valid winsize.
        // Returned descriptors are immediately transferred to File owners.
        let result = unsafe {
            libc::openpty(
                &mut master,
                &mut slave,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                &raw mut size,
            )
        };
        assert_eq!(result, 0, "openpty: {}", std::io::Error::last_os_error());
        let master = unsafe { File::from_raw_fd(master) };
        let slave = unsafe { File::from_raw_fd(slave) };
        for descriptor in [master.as_raw_fd(), slave.as_raw_fd()] {
            // SAFETY: both descriptors are open and owned above. Prevent them from
            // leaking into exec; Command handles duplication onto standard streams.
            assert_ne!(unsafe { libc::fcntl(descriptor, libc::F_SETFD, libc::FD_CLOEXEC) }, -1);
        }
        command
            .stdin(Stdio::from(slave.try_clone().unwrap()))
            .stdout(Stdio::from(slave.try_clone().unwrap()))
            .stderr(Stdio::from(slave))
            .env("TERM", "xterm-256color");
        // SAFETY: the child hook calls setsid/ioctl without allocation or locks.
        // stdin is already the slave PTY; attach it as the new session's terminal.
        unsafe {
            command.pre_exec(|| {
                if libc::setsid() == -1 || libc::ioctl(0, libc::TIOCSCTTY as _, 0) == -1 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
        Self {
            child: command.spawn().expect("spawn CLI in PTY"),
            master,
            output: String::new(),
            consumed: 0,
        }
    }

    fn read_available(&mut self) {
        let mut descriptor = libc::pollfd {
            fd: self.master.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        // SAFETY: poll receives one initialized descriptor valid for this call.
        if unsafe { libc::poll(&mut descriptor, 1, 50) } > 0 {
            let mut bytes = [0; 4096];
            if let Ok(length) = self.master.read(&mut bytes) {
                self.output.push_str(&String::from_utf8_lossy(&bytes[..length]));
            }
        }
    }

    pub fn wait_for(&mut self, text: &str) {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if let Some(position) = self.output[self.consumed..].find(text) {
                self.consumed += position + text.len();
                return;
            }
            assert!(Instant::now() < deadline, "waiting for {text:?}: {}", self.output);
            self.read_available();
            if let Some(status) = self.child.try_wait().unwrap() {
                self.read_available();
                assert!(
                    self.output[self.consumed..].contains(text),
                    "CLI exited {status} before {text:?}: {}",
                    self.output
                );
            }
        }
    }

    pub fn send(&mut self, text: &str) {
        self.master.write_all(text.as_bytes()).unwrap();
    }

    pub fn finish(mut self) -> (ExitStatus, String) {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            self.read_available();
            if let Some(status) = self.child.try_wait().unwrap() {
                self.read_available();
                return (status, self.output.clone());
            }
            assert!(Instant::now() < deadline, "CLI did not exit: {}", self.output);
        }
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}
