use serde_json::{Map, Value};
use std::path::PathBuf;

/// Generic, file-backed JSON key-value store persisted across restarts. Callers pick their own
/// section key (a page's stable numeric id turned into a string by PageManager, a hardware
/// provider's calibration key, etc.) and treat their stored value as an opaque
/// `serde_json::Value` -- Config itself has no knowledge of what any section means.
///
/// Reads and writes always go straight to the file (read-modify-write on `set_section`) rather
/// than caching state in memory, so independent `Config::load()` instances -- e.g. one owned by
/// PageManager and one owned by a hardware provider constructed earlier in main.rs -- can coexist
/// without one clobbering the other's more recent write: every write starts from the file's
/// current contents, never a stale in-memory snapshot.
pub struct Config {
    path: PathBuf,
}

impl Config {
    pub fn load() -> Self {
        Config { path: Self::default_path() }
    }

    fn default_path() -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/home/user".to_string());
        PathBuf::from(format!("{home}/Work/Niva_Dashboard_Rpi/Niva_dashboard_rpi/State/config.json"))
    }

    fn read_all(&self) -> Map<String, Value> {
        std::fs::read_to_string(&self.path).ok()
            .and_then(|contents| serde_json::from_str::<Value>(&contents).ok())
            .and_then(|root| root.as_object().cloned())
            .unwrap_or_default()
    }

    /// The persisted value for `key`, or `Value::Null` if none was ever stored -- callers must
    /// treat Null as "use defaults".
    pub fn section(&self, key: &str) -> Value {
        self.read_all().get(key).cloned().unwrap_or(Value::Null)
    }

    /// Persists `value` under `key`. A `Value::Null` removes the section instead of storing it,
    /// so callers with nothing to persist don't bloat the file.
    pub fn set_section(&self, key: &str, value: Value) {
        let mut sections = self.read_all();
        if value.is_null() {
            sections.remove(key);
        } else {
            sections.insert(key.to_string(), value);
        }

        let Ok(json) = serde_json::to_string_pretty(&Value::Object(sections)) else { return };
        if let Some(parent) = self.path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Err(e) = std::fs::write(&self.path, json) {
            log::warn!("Config: failed to persist to {:?}: {}", self.path, e);
        }
    }
}
