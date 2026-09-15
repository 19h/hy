//! Enforce authentication constraints on commands that require them.

use super::{AuthService, credentials::CredentialType};
use crate::config::Env;
use crate::error::{Error, Result};

impl AuthService {
    /// Enforce the options used by upstream AuthCommand before command side effects.
    pub fn validate_command_options(&self, auth_type: Option<&str>) -> Result<()> {
        let required = match auth_type.filter(|value| !value.is_empty()) {
            Some("interactive") => Some(CredentialType::Interactive),
            Some("key") => Some(CredentialType::Key),
            Some(value) => {
                return Err(Error::Authentication(format!(
                    "invalid auth type '{value}'; expected 'interactive' or 'key'"
                )));
            }
            None => None,
        };
        if let Some(name) = self.forced_name().filter(|name| !name.is_empty())
            && self.current_credentials().is_none_or(|credential| credential.name != name)
        {
            return Err(Error::Authentication(format!("credentials '{name}' not found")));
        }
        let current_type = if Env::global().api_key.as_ref().is_some_and(|key| !key.is_empty()) {
            CredentialType::Key
        } else {
            self.current_credentials()
                .map(|credential| credential.cred_type)
                .unwrap_or(CredentialType::Interactive)
        };
        if let Some(required) = required
            && required != current_type
        {
            return Err(Error::Authentication(format!(
                "authentication type mismatch: required {required}, current {current_type}"
            )));
        }
        Ok(())
    }
}
