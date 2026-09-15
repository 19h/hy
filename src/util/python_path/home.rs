//! Resolve a tilde component against the host; empty home variables remain set.

use crate::error::Result;

#[cfg(unix)]
mod unix;
#[cfg(any(windows, test))]
pub(super) mod windows;

pub(super) fn resolve(user: &str) -> Result<Option<String>> {
    #[cfg(unix)]
    {
        let home = if user.is_empty() {
            std::env::var_os("HOME").map(|value| value.to_string_lossy().into_owned())
        } else {
            None
        };
        let home = match home {
            Some(home) => Some(home),
            None => unix::account_home(user)?,
        };
        Ok(home.map(|home| {
            let home = home.trim_end_matches('/');
            if home.is_empty() {
                "/".into()
            } else {
                home.into()
            }
        }))
    }
    #[cfg(windows)]
    {
        Ok(windows::resolve(user, |name| std::env::var(name).ok()))
    }
}
