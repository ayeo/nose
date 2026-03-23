use super::AgentConfig;
use serde_json::{json, Value};
use std::path::PathBuf;

pub struct GeminiConfig;

const HOOK_EVENTS: &[&str] = &["BeforeTool", "AfterTool", "SessionStart", "SessionEnd"];

impl GeminiConfig {
    fn settings_path() -> PathBuf {
        let home = dirs::home_dir().expect("could not determine home directory");
        home.join(".gemini").join("settings.json")
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
            "command": format!("{} hook-handler --agent gemini --event {}", nose_bin, event),
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

impl AgentConfig for GeminiConfig {
    fn name(&self) -> &'static str {
        "Gemini CLI"
    }

    fn config_path(&self) -> PathBuf {
        Self::settings_path()
    }

    fn is_installed(&self) -> bool {
        let home = dirs::home_dir().expect("could not determine home directory");
        home.join(".gemini").exists()
    }

    fn install_hooks(&self, nose_bin: &str) -> Result<String, String> {
        let path = Self::settings_path();
        let mut settings = Self::read_settings(&path)?;

        let settings_map = settings
            .as_object_mut()
            .ok_or("gemini settings is not an object")?;

        let hooks_obj = settings_map
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
            arr.retain(|entry| !GeminiConfig::is_nose_managed(entry));

            arr.push(Self::make_hook_entry(nose_bin, event));
            installed.push(*event);
        }

        Self::write_settings(&path, &settings)?;

        Ok(format!(
            "Gemini CLI: installed hooks for {}",
            installed.join(", ")
        ))
    }

    fn uninstall_hooks(&self) -> Result<String, String> {
        let path = Self::settings_path();
        if !path.exists() {
            return Ok("Gemini CLI: no config found, nothing to uninstall".to_string());
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
                    arr.retain(|entry| !GeminiConfig::is_nose_managed(entry));
                    if arr.len() < before {
                        removed.push(*event);
                    }
                }
            }
        }

        Self::write_settings(&path, &settings)?;

        if removed.is_empty() {
            Ok("Gemini CLI: no nose-managed hooks found".to_string())
        } else {
            Ok(format!(
                "Gemini CLI: removed hooks for {}",
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
        let entry = GeminiConfig::make_hook_entry("/usr/bin/nose", "BeforeTool");
        assert!(entry["command"]
            .as_str()
            .unwrap()
            .contains("hook-handler --agent gemini --event BeforeTool"));
        assert_eq!(entry["_nose_managed"], true);
    }

    #[test]
    fn test_is_nose_managed() {
        let managed = json!({"command": "nose hook-handler", "_nose_managed": true});
        assert!(GeminiConfig::is_nose_managed(&managed));

        let not_managed = json!({"command": "something"});
        assert!(!GeminiConfig::is_nose_managed(&not_managed));
    }
}
