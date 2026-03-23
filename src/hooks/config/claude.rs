use super::AgentConfig;
use serde_json::{json, Value};
use std::path::PathBuf;

pub struct ClaudeConfig;

const HOOK_EVENTS: &[&str] = &["PreToolUse", "PostToolUse", "SessionStart", "SessionEnd"];

impl ClaudeConfig {
    fn settings_path() -> PathBuf {
        let home = dirs::home_dir().expect("could not determine home directory");
        home.join(".claude").join("settings.json")
    }

    fn read_settings(path: &PathBuf) -> Result<Value, String> {
        if path.exists() {
            let content = std::fs::read_to_string(path)
                .map_err(|e| format!("failed to read {}: {}", path.display(), e))?;
            serde_json::from_str(&content)
                .map_err(|e| format!("failed to parse {}: {}", path.display(), e))
        } else {
            Ok(json!({}))
        }
    }

    fn write_settings(path: &PathBuf, value: &Value) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("failed to create dir {}: {}", parent.display(), e))?;
        }
        let content = serde_json::to_string_pretty(value)
            .map_err(|e| format!("failed to serialize settings: {}", e))?;
        std::fs::write(path, content)
            .map_err(|e| format!("failed to write {}: {}", path.display(), e))
    }

    fn make_hook_entry(nose_bin: &str, event: &str) -> Value {
        json!({
            "matcher": "",
            "hooks": [
                {
                    "type": "command",
                    "command": format!("{} hook-handler --agent claude --event {}", nose_bin, event),
                    "_nose_managed": true
                }
            ]
        })
    }

    fn is_nose_managed(entry: &Value) -> bool {
        if let Some(hooks) = entry.get("hooks").and_then(|h| h.as_array()) {
            hooks.iter().any(|h| {
                h.get("_nose_managed").and_then(|v| v.as_bool()).unwrap_or(false)
            })
        } else {
            false
        }
    }
}

impl AgentConfig for ClaudeConfig {
    fn name(&self) -> &'static str {
        "Claude Code"
    }

    fn config_path(&self) -> PathBuf {
        Self::settings_path()
    }

    fn install_hooks(&self, nose_bin: &str) -> Result<String, String> {
        let path = Self::settings_path();
        let mut settings = Self::read_settings(&path)?;

        let hooks_obj = settings
            .as_object_mut()
            .ok_or("settings is not an object")?
            .entry("hooks")
            .or_insert_with(|| json!({}));

        let hooks_map = hooks_obj
            .as_object_mut()
            .ok_or("hooks is not an object")?;

        let mut installed = Vec::new();

        for event in HOOK_EVENTS {
            let event_arr = hooks_map
                .entry(event.to_string())
                .or_insert_with(|| json!([]));

            let arr = event_arr
                .as_array_mut()
                .ok_or(format!("hooks.{} is not an array", event))?;

            // Remove existing nose-managed entries first
            arr.retain(|entry| !ClaudeConfig::is_nose_managed(entry));

            // Add new entry
            arr.push(Self::make_hook_entry(nose_bin, event));
            installed.push(*event);
        }

        Self::write_settings(&path, &settings)?;

        Ok(format!(
            "Claude Code: installed hooks for {}",
            installed.join(", ")
        ))
    }

    fn uninstall_hooks(&self) -> Result<String, String> {
        let path = Self::settings_path();
        if !path.exists() {
            return Ok("Claude Code: no config found, nothing to uninstall".to_string());
        }

        let mut settings = Self::read_settings(&path)?;

        let mut removed = Vec::new();

        if let Some(hooks_map) = settings
            .as_object_mut()
            .and_then(|s| s.get_mut("hooks"))
            .and_then(|h| h.as_object_mut())
        {
            for event in HOOK_EVENTS {
                if let Some(arr) = hooks_map.get_mut(*event).and_then(|v| v.as_array_mut()) {
                    let before = arr.len();
                    arr.retain(|entry| !ClaudeConfig::is_nose_managed(entry));
                    if arr.len() < before {
                        removed.push(*event);
                    }
                }
            }
        }

        Self::write_settings(&path, &settings)?;

        if removed.is_empty() {
            Ok("Claude Code: no nose-managed hooks found".to_string())
        } else {
            Ok(format!(
                "Claude Code: removed hooks for {}",
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
        let entry = ClaudeConfig::make_hook_entry("/usr/bin/nose", "PreToolUse");
        assert_eq!(entry["matcher"], "");
        let hooks = entry["hooks"].as_array().unwrap();
        assert_eq!(hooks.len(), 1);
        assert_eq!(hooks[0]["type"], "command");
        assert!(hooks[0]["command"]
            .as_str()
            .unwrap()
            .contains("hook-handler --agent claude --event PreToolUse"));
        assert_eq!(hooks[0]["_nose_managed"], true);
    }

    #[test]
    fn test_is_nose_managed() {
        let managed = json!({
            "matcher": "",
            "hooks": [{"type": "command", "command": "nose hook-handler", "_nose_managed": true}]
        });
        assert!(ClaudeConfig::is_nose_managed(&managed));

        let not_managed = json!({
            "matcher": "",
            "hooks": [{"type": "command", "command": "something else"}]
        });
        assert!(!ClaudeConfig::is_nose_managed(&not_managed));
    }

    #[test]
    fn test_install_produces_valid_json() {
        // Build settings in memory to verify structure
        let mut settings = json!({});
        let nose_bin = "/usr/local/bin/nose";

        let hooks_obj = settings
            .as_object_mut()
            .unwrap()
            .entry("hooks")
            .or_insert_with(|| json!({}));
        let hooks_map = hooks_obj.as_object_mut().unwrap();

        for event in HOOK_EVENTS {
            let arr = hooks_map
                .entry(event.to_string())
                .or_insert_with(|| json!([]));
            arr.as_array_mut()
                .unwrap()
                .push(ClaudeConfig::make_hook_entry(nose_bin, event));
        }

        // Verify it round-trips through JSON
        let serialized = serde_json::to_string_pretty(&settings).unwrap();
        let parsed: Value = serde_json::from_str(&serialized).unwrap();
        assert!(parsed["hooks"]["PreToolUse"].as_array().unwrap().len() == 1);
        assert!(parsed["hooks"]["PostToolUse"].as_array().unwrap().len() == 1);
        assert!(parsed["hooks"]["SessionStart"].as_array().unwrap().len() == 1);
        assert!(parsed["hooks"]["SessionEnd"].as_array().unwrap().len() == 1);
    }

    #[test]
    fn test_uninstall_removes_only_managed() {
        let mut settings = json!({
            "hooks": {
                "PreToolUse": [
                    {
                        "matcher": "",
                        "hooks": [{"type": "command", "command": "user-hook"}]
                    },
                    {
                        "matcher": "",
                        "hooks": [{"type": "command", "command": "nose hook-handler", "_nose_managed": true}]
                    }
                ]
            }
        });

        let hooks_map = settings["hooks"].as_object_mut().unwrap();
        let arr = hooks_map["PreToolUse"].as_array_mut().unwrap();
        arr.retain(|entry| !ClaudeConfig::is_nose_managed(entry));

        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["hooks"][0]["command"], "user-hook");
    }
}
