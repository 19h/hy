//! Preserve the distinction between absent credentials, rejection and entitlement.

use std::fmt;

#[derive(Debug)]
pub struct PluginAccessDenied {
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "Retain the original URL in the structured upstream error")
    )]
    pub url: String,
    pub status: u16,
    pub authenticated: bool,
    pub repository: Option<String>,
}

impl fmt::Display for PluginAccessDenied {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let where_ = self
            .repository
            .as_ref()
            .filter(|name| !name.is_empty())
            .map_or_else(|| "the plugin repository".into(), |name| format!("repository '{name}'"));
        write!(formatter, "Access denied (HTTP {}) by {where_}. ", self.status)?;
        let binary = &crate::config::Env::global().binary_name;
        if !self.authenticated {
            write!(formatter, "Run '{binary} login' and try again.")
        } else if self.status == 401 {
            write!(
                formatter,
                "Your credentials were rejected: run '{binary} login' again, or check HCLI_API_KEY."
            )
        } else {
            write!(formatter, "Your account is not entitled to it.")
        }
    }
}

impl std::error::Error for PluginAccessDenied {}
