//! Credential types and persistent credential configuration.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// The kind of authentication a credential represents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CredentialType {
    Interactive,
    Key,
}

impl std::fmt::Display for CredentialType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Interactive => f.write_str("Interactive"),
            Self::Key => f.write_str("API Key"),
        }
    }
}

/// A single set of stored credentials.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Credentials {
    pub name: String,
    #[serde(rename = "type")]
    pub cred_type: CredentialType,
    pub email: String,
    pub created_at: DateTime<Utc>,
    pub last_used: DateTime<Utc>,
    pub token: Option<String>,
}

impl Credentials {
    /// Create a new credential set.
    pub fn new(
        name: impl Into<String>,
        cred_type: CredentialType,
        token: impl Into<String>,
        email: impl Into<String>,
    ) -> Self {
        let now = Utc::now();
        Self {
            name: name.into(),
            cred_type,
            email: email.into(),
            created_at: now,
            last_used: now,
            token: Some(token.into()),
        }
    }

    /// Human-readable label for display.
    pub fn label(&self) -> String {
        match self.cred_type {
            CredentialType::Key => format!("{} [{}]", self.name, self.email),
            CredentialType::Interactive => self.email.clone(),
        }
    }

    /// Touch the `last_used` timestamp.
    pub fn touch(&mut self) {
        self.last_used = Utc::now();
    }
}

/// Top-level credentials configuration persisted in the config store.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CredentialsConfig {
    pub default: Option<String>,
    pub credentials: BTreeMap<String, Credentials>,
}

impl CredentialsConfig {
    /// Add a credential set.  Sets it as default if none exists yet.
    pub fn add(&mut self, cred: Credentials) {
        let name = cred.name.clone();
        self.credentials.insert(name.clone(), cred);
        if self.default.is_none() {
            self.default = Some(name);
        }
    }

    /// Remove a credential set by name.  Returns `true` if it existed.
    pub fn remove(&mut self, name: &str) -> bool {
        if self.credentials.remove(name).is_some() {
            if self.default.as_deref() == Some(name) {
                self.default = self.credentials.keys().next().cloned();
            }
            true
        } else {
            false
        }
    }

    /// Get the default credential set.
    #[allow(dead_code)]
    pub fn default_credentials(&self) -> Option<&Credentials> {
        self.default
            .as_deref()
            .and_then(|name| self.credentials.get(name))
    }

    /// Set the default by name.  Returns `false` if the name is unknown.
    pub fn set_default(&mut self, name: &str) -> bool {
        if self.credentials.contains_key(name) {
            self.default = Some(name.to_owned());
            true
        } else {
            false
        }
    }

    /// Find a credential by email and type.
    pub fn find_by_email_and_type(
        &self,
        email: &str,
        cred_type: CredentialType,
    ) -> Option<&Credentials> {
        self.credentials
            .values()
            .find(|c| c.email == email && c.cred_type == cred_type)
    }

    /// Generate a unique name starting from `base`.
    pub fn unique_name(&self, base: &str) -> String {
        if !self.credentials.contains_key(base) {
            return base.to_owned();
        }
        for i in 1.. {
            let candidate = format!("{base}-{i}");
            if !self.credentials.contains_key(&candidate) {
                return candidate;
            }
        }
        unreachable!()
    }
}
