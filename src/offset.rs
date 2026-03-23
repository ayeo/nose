use std::collections::HashMap;
use std::path::PathBuf;

use crate::adapter::Adapter;
use crate::watch::parse_file_from_offset as watch_parse_file_from_offset;

/// Returns the path to the offsets file: `~/.nose/offsets.json`.
fn offsets_path() -> PathBuf {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    home.join(".nose").join("offsets.json")
}

/// Load saved byte offsets from `~/.nose/offsets.json`.
/// Returns an empty map if the file does not exist or cannot be parsed.
pub fn load_offsets() -> HashMap<PathBuf, u64> {
    let path = offsets_path();
    if !path.exists() {
        return HashMap::new();
    }
    let data = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(_) => return HashMap::new(),
    };
    let raw: HashMap<String, u64> = match serde_json::from_str(&data) {
        Ok(m) => m,
        Err(_) => return HashMap::new(),
    };
    raw.into_iter().map(|(k, v)| (PathBuf::from(k), v)).collect()
}

/// Save byte offsets to `~/.nose/offsets.json`.
/// Creates `~/.nose/` directory if needed. Silently ignores write errors.
pub fn save_offsets(offsets: &HashMap<PathBuf, u64>) {
    let path = offsets_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let raw: HashMap<String, u64> = offsets
        .iter()
        .map(|(k, v)| (k.to_string_lossy().into_owned(), *v))
        .collect();
    if let Ok(json) = serde_json::to_string_pretty(&raw) {
        let _ = std::fs::write(&path, json);
    }
}

/// Parse a session file from the given byte offset using the supplied adapter.
/// Returns `(events, new_offset)`. Delegates to `watch::parse_file_from_offset`.
pub fn parse_file_from_offset(
    path: &std::path::Path,
    offset: u64,
    adapter: &dyn Adapter,
    session_id: &str,
    workspace: &str,
) -> Result<(Vec<crate::event::Event>, u64), Box<dyn std::error::Error>> {
    watch_parse_file_from_offset(path, offset, adapter, session_id, workspace)
}
