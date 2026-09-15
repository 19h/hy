//! Persistent JSON configuration store.
//!
//! Reads / writes `$XDG_CONFIG_HOME/hcli/config.json` (or the platform
//! equivalent via the `dirs` crate).

use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use indexmap::IndexMap;
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
    /// Load before command dispatch; malformed user data must not become empty defaults.
    pub fn initialize() -> Result<()> {
        if STORE.get().is_none() {
            let store = Self::open()?;
            let _ = STORE.set(Mutex::new(store));
        }
        Ok(())
    }

    // ── singleton access ────────────────────────────────────────────────

    /// Obtain a locked reference to the global config store.
    pub fn global() -> std::sync::MutexGuard<'static, Self> {
        STORE
            .get()
            .expect("configuration must be initialized before command dispatch")
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
            dirs::config_dir().unwrap_or_else(|| PathBuf::from(".")).join("hcli")
        }
    }

    fn config_path() -> PathBuf {
        Self::config_dir().join("config.json")
    }

    pub(crate) fn read_snapshot() -> Result<Self> {
        Self::open()
    }

    fn open() -> Result<Self> {
        Self::open_at(Self::config_path())
    }

    pub(crate) fn open_at(path: PathBuf) -> Result<Self> {
        if path.exists() {
            let text = std::fs::read_to_string(&path)?;
            let data: Value = serde_json::from_str(&text)?;
            if !data.is_object() {
                return Err(crate::error::Error::Other("config root must be an object".into()));
            }
            let mut store = Self {
                path,
                data,
            };
            store.migrate();
            Ok(store)
        } else {
            Ok(Self {
                path,
                data: Value::Object(serde_json::Map::new()),
            })
        }
    }

    fn migrate(&mut self) {
        let env = Env::global();
        for old in ["credentials", "login.email"] {
            let new = format!("{}.{}", env.config_namespace, old);
            if self.data.get(&new).is_none()
                && let Some(value) = self.data.as_object_mut().and_then(|object| object.remove(old))
            {
                self.data[&new] = value;
            }
        }
        // Preserve the Rust port's earlier nested settings when adopting upstream's flat keys.
        for (old, new) in [
            ("ke.ida.instances", "ida.instances"),
            ("ke.ida.default", "ida.default"),
            ("ke.sources", "idb.sources"),
        ] {
            if let Some(value) = Self::take_legacy_path(&mut self.data, old)
                && self.data.get(new).is_none()
            {
                self.data[new] = value;
            }
        }
        self.data[format!("{}.version", env.binary_name)] = serde_json::json!(env.version);
    }

    // ── string accessors ────────────────────────────────────────────────

    pub fn get_str(&self, key: &str) -> Option<&str> {
        self.data.get(key)?.as_str()
    }

    pub fn set_str(&mut self, key: &str, value: &str) -> Result<()> {
        self.set_value(key, Value::String(value.to_owned()))
    }

    // ── typed object accessors ──────────────────────────────────────────

    pub fn get_value(&self, key: &str) -> Option<&Value> {
        self.data.get(key)
    }

    pub fn set_value(&mut self, key: &str, value: Value) -> Result<()> {
        self.set_values([(key.to_owned(), value)])
    }

    /// Commit related flat keys together; retain the old in-memory state on failure.
    pub fn set_values(&mut self, values: impl IntoIterator<Item = (String, Value)>) -> Result<()> {
        self.commit_changes(values.into_iter().map(|(key, value)| (key, Some(value))))
    }

    /// Apply related insertions and removals together; `None` removes a key.
    pub fn commit_changes(
        &mut self,
        changes: impl IntoIterator<Item = (String, Option<Value>)>,
    ) -> Result<()> {
        let mut candidate = self.data.clone();
        let object = candidate.as_object_mut().expect("validated configuration object");
        for (key, value) in changes {
            match value {
                Some(value) => {
                    object.insert(key, value);
                }
                None => {
                    object.shift_remove(&key);
                }
            }
        }
        if candidate == self.data {
            return Ok(());
        }
        self.flush(&candidate)?;
        self.data = candidate;
        Ok(())
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

    fn take_legacy_path(root: &mut Value, dotted: &str) -> Option<Value> {
        let (parent, key) = dotted.rsplit_once('.')?;
        root.pointer_mut(&format!("/{}", parent.replace('.', "/")))?
            .as_object_mut()?
            .shift_remove(key)
    }

    /// Read a nested value by dotted path.
    pub fn get_nested(&self, dotted: &str) -> Option<&Value> {
        self.data.get(dotted).or_else(|| Self::resolve_path(&self.data, dotted))
    }

    /// Read a nested string by dotted path.
    pub fn get_nested_str(&self, dotted: &str) -> Option<&str> {
        self.get_nested(dotted)?.as_str()
    }

    /// Read a string mapping in configuration insertion order.
    pub fn get_string_map(&self, dotted: &str) -> IndexMap<String, String> {
        self.get_nested(dotted)
            .and_then(|v| v.as_object())
            .map(|obj| {
                obj.iter()
                    .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_owned())))
                    .collect()
            })
            .unwrap_or_default()
    }

    // ── persistence ─────────────────────────────────────────────────────

    fn flush(&self, data: &Value) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let text = serde_json::to_string_pretty(data)?;
        use std::io::Write;
        let parent = self.path.parent().unwrap();
        let mut temp = tempfile::NamedTempFile::new_in(parent)?;
        temp.write_all(text.as_bytes())?;
        temp.persist(&self.path).map_err(|e| e.error)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn failed_writes_preserve_memory_and_successful_changes_commit_together() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("config.json");
        let original = json!({"key": "original", "default": "key", "unrelated": [1, 2]});
        std::fs::write(&path, serde_json::to_vec(&original).unwrap()).unwrap();
        let mut store = ConfigStore {
            path: path.clone(),
            data: original.clone(),
        };

        // Replacing a directory with a file fails independently of user permissions.
        let saved = directory.path().join("saved.json");
        std::fs::rename(&path, &saved).unwrap();
        std::fs::create_dir(&path).unwrap();
        assert!(store.set_str("key", "changed").is_err());
        assert!(store.commit_changes([("key".into(), None)]).is_err());
        assert!(
            store.set_values([("first".into(), json!(1)), ("second".into(), json!(2))]).is_err()
        );
        assert_eq!(store.data, original);
        assert_eq!(
            serde_json::from_slice::<Value>(&std::fs::read(&saved).unwrap()).unwrap(),
            original
        );

        std::fs::remove_dir(&path).unwrap();
        std::fs::rename(&saved, &path).unwrap();
        store
            .commit_changes([("key".into(), Some(json!("changed"))), ("default".into(), None)])
            .unwrap();
        let expected = json!({"key": "changed", "unrelated": [1, 2]});
        assert_eq!(store.data, expected);
        assert_eq!(
            serde_json::from_slice::<Value>(&std::fs::read(path).unwrap()).unwrap(),
            expected
        );
    }
}
