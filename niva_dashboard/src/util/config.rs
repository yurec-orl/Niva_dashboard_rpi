use serde_json::{Map, Value};
use std::path::PathBuf;

const LAST_PAGE_KEY: &str = "last_page";
const PAGES_KEY: &str = "pages";

/// Persists per-page settings and the last-opened page across restarts, keyed by each page's
/// stable numeric id (MAIN_PAGE_ID etc. in page_manager.rs). PageManager treats each page's
/// stored entry as an opaque `serde_json::Value`, round-tripped through
/// `Page::get_config`/`set_config`, so it never needs to know a page's own settings shape.
pub struct Config {
    path: PathBuf,
    pages: Map<String, Value>,
    last_page: Option<u32>,
}

impl Config {
    pub fn load() -> Self {
        let path = Self::default_path();
        let (pages, last_page) = Self::load_from_disk(&path).unwrap_or_default();
        Config { path, pages, last_page }
    }

    fn default_path() -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/home/user".to_string());
        PathBuf::from(format!("{home}/Work/Niva_Dashboard_Rpi/Niva_dashboard_rpi/State/config.json"))
    }

    fn load_from_disk(path: &std::path::Path) -> Option<(Map<String, Value>, Option<u32>)> {
        let contents = std::fs::read_to_string(path).ok()?;
        let root: Value = serde_json::from_str(&contents).ok()?;
        let obj = root.as_object()?;
        let last_page = obj.get(LAST_PAGE_KEY).and_then(Value::as_u64).map(|v| v as u32);
        let pages = obj.get(PAGES_KEY).and_then(Value::as_object).cloned().unwrap_or_default();
        Some((pages, last_page))
    }

    pub fn last_page(&self) -> Option<u32> {
        self.last_page
    }

    /// The persisted config for `page_id`, or `Value::Null` if none was ever stored -- callers
    /// (via `Page::set_config`) must treat Null as "use defaults".
    pub fn page_config(&self, page_id: u32) -> Value {
        self.pages.get(&page_id.to_string()).cloned().unwrap_or(Value::Null)
    }

    /// Called on every page switch: records the outgoing page's config (`None` on the very
    /// first switch, when there's nothing to capture yet -- a Null config is also skipped, so
    /// pages with nothing to persist don't bloat the file) and the page now being displayed,
    /// then writes the whole file. Switches are infrequent enough that a full rewrite each time
    /// is fine.
    pub fn record_switch(&mut self, outgoing: Option<(u32, Value)>, last_page_id: u32) {
        if let Some((id, config)) = outgoing {
            if !config.is_null() {
                self.pages.insert(id.to_string(), config);
            }
        }
        self.last_page = Some(last_page_id);
        self.write();
    }

    fn write(&self) {
        let root = serde_json::json!({
            LAST_PAGE_KEY: self.last_page,
            PAGES_KEY: self.pages,
        });
        let Ok(json) = serde_json::to_string_pretty(&root) else { return };
        if let Some(parent) = self.path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Err(e) = std::fs::write(&self.path, json) {
            log::warn!("Config: failed to persist to {:?}: {}", self.path, e);
        }
    }
}
