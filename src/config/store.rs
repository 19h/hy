//! Persistent JSON configuration store.
//!
//! Reads / writes `$XDG_CONFIG_HOME/hcli/config.json` (or the platform
//! equivalent via the `dirs` crate).

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use serde_json::Value;

use crate::config::Env;
use crate::error::Result;

/// Global config store singleton.
static STORE: OnceLock<Mutex<ConfigStore>> = OnceLock::new();

/// Persistent JSON key-value store backed by a single file.
#[derive(Debug)]
pub struct ConfigStore {
    path: PathBuf,
    data: Value,
}

impl ConfigStore {
    // ── singleton access ────────────────────────────────────────────────

    /// Obtain a locked reference to the global config store.
    pub fn global() -> std::sync::MutexGuard<'static, Self> {
        STORE
            .get_or_init(|| {
                let store = Self::open().unwrap_or_else(|_| Self::empty());
                Mutex::new(store)
            })
            .lock()
            .expect("config store lock poisoned")
    }

    // ── construction ────────────────────────────────────────────────────

    fn config_dir() -> PathBuf {
        // Match Python platformdirs.user_config_dir("hcli", "hex-rays"):
        //   macOS:   ~/Library/Application Support/hcli
        //   Linux:   $XDG_CONFIG_HOME/hcli  (default ~/.config/hcli)
        //   Windows: %LOCALAPPDATA%\hex-rays\hcli
        #[cfg(target_os = "windows")]
        {
            // platformdirs uses Local (not Roaming) and prepends appauthor.
            dirs::data_local_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("hex-rays")
                .join("hcli")
        }
        #[cfg(not(target_os = "windows"))]
        {
            dirs::config_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("hcli")
        }
    }

    fn config_path() -> PathBuf {
        Self::config_dir().join("config.json")
    }

    fn empty() -> Self {
        Self {
            path: Self::config_path(),
            data: Value::Object(serde_json::Map::new()),
        }
    }

    fn open() -> Result<Self> {
        let path = Self::config_path();
        if path.exists() {
            let text = std::fs::read_to_string(&path)?;
            let data: Value =
                serde_json::from_str(&text).unwrap_or(Value::Object(Default::default()));
            let mut store = Self { path, data };
            store.migrate();
            Ok(store)
        } else {
            Ok(Self::empty())
        }
    }

    fn migrate(&mut self) {
        let current = self.get_str("version").unwrap_or_default();
        let target = &Env::global().version;
        if current != *target {
            self.set_str("version", target);
            let _ = self.flush();
        }
    }

    // ── string accessors ────────────────────────────────────────────────

    pub fn get_str(&self, key: &str) -> Option<&str> {
        self.data.get(key)?.as_str()
    }

    pub fn set_str(&mut self, key: &str, value: &str) {
        self.data
            .as_object_mut()
            .expect("root must be object")
            .insert(key.to_owned(), Value::String(value.to_owned()));
        let _ = self.flush();
    }

    pub fn remove(&mut self, key: &str) {
        if let Some(obj) = self.data.as_object_mut() {
            obj.remove(key);
            let _ = self.flush();
        }
    }

    // ── typed object accessors ──────────────────────────────────────────

    pub fn get_value(&self, key: &str) -> Option<&Value> {
        self.data.get(key)
    }

    pub fn set_value(&mut self, key: &str, value: Value) {
        self.data
            .as_object_mut()
            .expect("root must be object")
            .insert(key.to_owned(), value);
        let _ = self.flush();
    }

    #[allow(dead_code)]
    pub fn has(&self, key: &str) -> bool {
        self.data.get(key).is_some_and(|v| !v.is_null())
    }

    // ── dotted-path accessors ───────────────────────────────────────────

    /// Navigate into the JSON tree along a dotted key path (e.g. `"ke.ida.instances"`).
    fn resolve_path<'a>(root: &'a Value, dotted: &str) -> Option<&'a Value> {
        let mut current = root;
        for part in dotted.split('.') {
            current = current.get(part)?;
        }
        Some(current)
    }

    /// Read a nested value by dotted path.
    pub fn get_nested(&self, dotted: &str) -> Option<&Value> {
        Self::resolve_path(&self.data, dotted)
    }

    /// Read a nested string by dotted path.
    pub fn get_nested_str(&self, dotted: &str) -> Option<&str> {
        self.get_nested(dotted)?.as_str()
    }

    /// Read a nested object as `HashMap<String, String>` by dotted path.
    pub fn get_string_map(&self, dotted: &str) -> HashMap<String, String> {
        self.get_nested(dotted)
            .and_then(|v| v.as_object())
            .map(|obj| {
                obj.iter()
                    .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_owned())))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Write a value at a dotted path, creating intermediate objects as needed.
    pub fn set_nested(&mut self, dotted: &str, value: Value) {
        let parts: Vec<&str> = dotted.split('.').collect();
        let mut current = &mut self.data;
        for &part in &parts[..parts.len() - 1] {
            if !current.get(part).is_some_and(|v| v.is_object()) {
                current
                    .as_object_mut()
                    .expect("intermediate must be object")
                    .insert(part.to_owned(), Value::Object(Default::default()));
            }
            current = current.get_mut(part).unwrap();
        }
        let last = parts.last().unwrap();
        current
            .as_object_mut()
            .expect("parent must be object")
            .insert((*last).to_owned(), value);
        let _ = self.flush();
    }

    /// Remove a key at a dotted path.
    pub fn remove_nested(&mut self, dotted: &str) {
        let parts: Vec<&str> = dotted.split('.').collect();
        if parts.len() == 1 {
            self.remove(parts[0]);
            return;
        }
        // Navigate to parent.
        let mut current = &mut self.data;
        for &part in &parts[..parts.len() - 1] {
            match current.get_mut(part) {
                Some(v) if v.is_object() => current = v,
                _ => return, // Path doesn't exist, nothing to remove.
            }
        }
        if let Some(obj) = current.as_object_mut() {
            let last = parts.last().unwrap();
            obj.remove(*last);
        }
        let _ = self.flush();
    }

    // ── persistence ─────────────────────────────────────────────────────

    fn flush(&self) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let text = serde_json::to_string_pretty(&self.data)?;
        std::fs::write(&self.path, text)?;
        Ok(())
    }
}
