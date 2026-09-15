//! Read-only platform proxy discovery matching urllib.request.getproxies.

pub(super) type ProxyMap = std::collections::HashMap<String, String>;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(any(windows, test))]
mod windows;

#[cfg(target_os = "macos")]
pub(super) fn discover() -> ProxyMap {
    macos::discover()
}
#[cfg(windows)]
pub(super) fn discover() -> ProxyMap {
    windows::discover()
}

#[cfg(not(any(target_os = "macos", windows)))]
pub(super) fn discover() -> ProxyMap {
    ProxyMap::new()
}
