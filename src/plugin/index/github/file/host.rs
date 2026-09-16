//! urllib's IPv4 localhost/address checks, cached for the lifetime of the process.

use std::net::{Ipv4Addr, ToSocketAddrs};
use std::sync::Mutex;

use crate::error::{Error, Result};
use crate::util::python_json::Text;

static LOCAL_NAMES: Mutex<Option<Vec<Ipv4Addr>>> = Mutex::new(None);

pub(super) fn is_local_address(host: &Text) -> Result<bool> {
    let names = local_names()?;
    Ok(names.iter().any(|address| host.equals(&address.to_string())))
}

pub(super) fn resolves_locally(host: &Text) -> Result<bool> {
    let points: Vec<_> = host.codepoints().collect();
    if let Some(colon) = points.iter().rposition(|&point| point == u32::from(':'))
        && !points[colon + 1..].is_empty()
        && points[colon + 1..].iter().all(|&point| (0x30..=0x39).contains(&point))
    {
        return Ok(false);
    }
    let host = host.to_utf8().map_err(|error| Error::GitHubValue(error.to_string()))?;
    if host.contains('\0') {
        return Err(Error::Other("hostname contains a null byte".into()));
    }
    let address = resolve(&host).ok().and_then(|addresses| addresses.into_iter().next());
    let names = local_names()?;
    Ok(address.is_some_and(|address| names.contains(&address)))
}

fn resolve(host: &str) -> std::io::Result<Vec<Ipv4Addr>> {
    let addresses: Vec<_> = (host, 0)
        .to_socket_addrs()?
        .filter_map(|address| match address.ip() {
            std::net::IpAddr::V4(address) => Some(address),
            std::net::IpAddr::V6(_) => None,
        })
        .collect();
    if addresses.is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "hostname has no IPv4 address",
        ));
    }
    Ok(addresses)
}

fn local_names() -> Result<Vec<Ipv4Addr>> {
    let mut cached = LOCAL_NAMES.lock().map_err(|error| Error::Other(error.to_string()))?;
    if let Some(names) = &*cached {
        return Ok(names.clone());
    }
    let names = (|| {
        let mut names = resolve("localhost")?;
        names.extend(resolve(&hostname()?)?);
        Ok::<_, std::io::Error>(names)
    })()
    .or_else(|_| resolve("localhost").map(|names| names.into_iter().take(1).collect()))
    .map_err(super::url_error)?;
    *cached = Some(names.clone());
    Ok(names)
}

#[cfg(unix)]
fn hostname() -> std::io::Result<String> {
    // Reserve the final byte for termination, and never read beyond the
    // initialized allocation, including hosts whose name fills the buffer.
    let mut bytes = [0_u8; 1024];
    let status = unsafe { libc::gethostname(bytes.as_mut_ptr().cast(), bytes.len() - 1) };
    if status != 0 {
        return Err(std::io::Error::last_os_error());
    }
    let length = bytes.iter().position(|&byte| byte == 0).unwrap_or(bytes.len());
    String::from_utf8(bytes[..length].to_vec())
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))
}

#[cfg(windows)]
fn hostname() -> std::io::Result<String> {
    use windows_sys::Win32::Networking::WinSock;

    let mut data = std::mem::MaybeUninit::<WinSock::WSADATA>::uninit();
    // WSAStartup initializes WSADATA on success. Every successful acquisition is
    // balanced by WSACleanup, independently of Rust's socket initialization.
    let status = unsafe { WinSock::WSAStartup(0x0202, data.as_mut_ptr()) };
    if status != 0 {
        return Err(std::io::Error::from_raw_os_error(status));
    }
    let mut bytes = [0_u8; 1024];
    let status = unsafe { WinSock::gethostname(bytes.as_mut_ptr(), bytes.len() as i32 - 1) };
    let error = if status != 0 {
        Some(unsafe { WinSock::WSAGetLastError() })
    } else {
        None
    };
    unsafe { WinSock::WSACleanup() };
    if let Some(error) = error {
        return Err(std::io::Error::from_raw_os_error(error));
    }
    let length = bytes.iter().position(|&byte| byte == 0).unwrap_or(bytes.len());
    String::from_utf8(bytes[..length].to_vec())
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))
}
