use super::AgentConfig;
use serde_json::{json, Value};
use std::path::PathBuf;

pub struct CodexConfig;

const HOOK_EVENTS: &[&str] = &["SessionStart", "SessionStop"];

impl CodexConfig {
    fn config_path_inner() -> PathBuf {
        let home = dirs::home_dir().expect("could not determine home directory");
        home.join(".codex").join("hooks.json")
    }

    fn read_config(path: &PathBuf) -> Result<Value, String> {
        if path.exists() {
            let content = std::fs::read_to_string(path)
                .map_err(|e| format!("failed to read {}: {}", path.display(), e))?;
            serde_json::from_str(&content)
                .map_err(|e| format!("failed to parse {}: {}", path.display(), e))
        } else {
            Ok(json!({}))
        }
    }

    fn write_config(path: &PathBuf, value: &Value) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("failed to create dir {}: {}", parent.display(), e))?;
        }
        let content = serde_json::to_string_pretty(value)
            .map_err(|e| format!("failed to serialize config: {}", e))?;
        std::fs::write(path, content)
            .map_err(|e| format!("failed to write {}: {}", path.display(), e))
    }

    fn make_hook_entry(nose_bin: &str, event: &str) -> Value {
        json!({
            "command": format!("{} hook-handler --agent codex --event {}", nose_bin, event),
            "_nose_managed": true
        })
    }

    fn is_nose_managed(entry: &Value) -> bool {
        entry
            .get("_nose_managed")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    }
}

impl AgentConfig for CodexConfig {
    fn name(&self) -> &'static str {
        "Codex CLI"
    }

    fn config_path(&self) -> PathBuf {
        Self::config_path_inner()
    }

    fn is_installed(&self) -> bool {
        let home = dirs::home_dir().expect("could not determine home directory");
        home.join(".codex").exists()
    }

    fn install_hooks(&self, nose_bin: &str) -> Result<String, String> {
        let path = Self::config_path_inner();
        let mut config = Self::read_config(&path)?;

        let config_map = config
            .as_object_mut()
            .ok_or("codex config is not an object")?;

        let mut installed = Vec::new();

        for event in HOOK_EVENTS {
            let event_arr = config_map
                .entry(event.to_string())
                .or_insert_with(|| json!([]));

            let arr = event_arr
                .as_array_mut()
                .ok_or(format!("{} is not an array", event))?;

            // Remove existing nose-managed entries first
            arr.retain(|entry| !CodexConfig::is_nose_managed(entry));

            arr.push(Self::make_hook_entry(nose_bin, event));
            installed.push(*event);
        }

        Self::write_config(&path, &config)?;

        Ok(format!(
            "Codex CLI: installed hooks for {}",
            installed.join(", ")
        ))
    }

    fn uninstall_hooks(&self) -> Result<String, String> {
        let path = Self::config_path_inner();
        if !path.exists() {
            return Ok("Codex CLI: no config found, nothing to uninstall".to_string());
        }

        let mut config = Self::read_config(&path)?;
        let mut removed = Vec::new();

        if let Some(config_map) = config.as_object_mut() {
            for event in HOOK_EVENTS {
                if let Some(arr) = config_map.get_mut(*event).and_then(|v| v.as_array_mut()) {
                    let before = arr.len();
                    arr.retain(|entry| !CodexConfig::is_nose_managed(entry));
                    if arr.len() < before {
                        removed.push(*event);
                    }
                }
            }
        }

        Self::write_config(&path, &config)?;

        if removed.is_empty() {
            Ok("Codex CLI: no nose-managed hooks found".to_string())
        } else {
            Ok(format!(
                "Codex CLI: removed hooks for {}",
                removed.join(", ")
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_make_hook_entry() {
        let entry = CodexConfig::make_hook_entry("/usr/bin/nose", "SessionStart");
        assert!(entry["command"]
            .as_str()
            .unwrap()
            .contains("hook-handler --agent codex --event SessionStart"));
        assert_eq!(entry["_nose_managed"], true);
    }

    #[test]
    fn test_is_nose_managed() {
        let managed = json!({"command": "nose hook-handler", "_nose_managed": true});
        assert!(CodexConfig::is_nose_managed(&managed));

        let not_managed = json!({"command": "something"});
        assert!(!CodexConfig::is_nose_managed(&not_managed));
    }
}
