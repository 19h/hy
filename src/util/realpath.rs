//! Resolve existing links and missing path tails without creating files.
//!
//! Unix follows CPython's non-strict realpath behavior. Windows resolves existing
//! prefixes but rejects unresolved reparse points; native runtime parity is open.

#[cfg(unix)]
mod unix;
#[cfg(unix)]
pub use unix::resolve;

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::resolve;
