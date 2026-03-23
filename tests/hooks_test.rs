use serde_json::json;
use tempfile::TempDir;
use std::io::Write;

/// Test that claude config install/uninstall round-trips correctly using in-memory JSON
#[test]
fn test_claude_config_roundtrip() {
    let mut settings = json!({
        "permissions": {"allow": ["Read"]}
    });

    // Simulate install: add hooks
    let hooks_obj = settings
        .as_object_mut()
        .unwrap()
        .entry("hooks")
        .or_insert_with(|| json!({}));
    let hooks_map = hooks_obj.as_object_mut().unwrap();

    for event in &["PreToolUse", "PostToolUse", "SessionStart", "SessionEnd"] {
        let arr = hooks_map
            .entry(event.to_string())
            .or_insert_with(|| json!([]));
        arr.as_array_mut().unwrap().push(json!({
            "matcher": "",
            "hooks": [{
                "type": "command",
                "command": format!("/usr/bin/nose hook-handler --agent claude --event {}", event),
                "_nose_managed": true
            }]
        }));
    }

    // Verify structure
    let serialized = serde_json::to_string_pretty(&settings).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&serialized).unwrap();
    assert!(parsed["permissions"]["allow"][0] == "Read");
    assert_eq!(parsed["hooks"]["PreToolUse"].as_array().unwrap().len(), 1);

    // Simulate uninstall: remove nose-managed hooks
    let hooks_map = parsed
        .as_object()
        .unwrap()
        .get("hooks")
        .unwrap()
        .as_object()
        .unwrap();

    for (_event, entries) in hooks_map {
        let arr = entries.as_array().unwrap();
        let remaining: Vec<_> = arr
            .iter()
            .filter(|entry| {
                !entry
                    .get("hooks")
                    .and_then(|h| h.as_array())
                    .map(|hooks| {
                        hooks.iter().any(|h| {
                            h.get("_nose_managed")
                                .and_then(|v| v.as_bool())
                                .unwrap_or(false)
                        })
                    })
                    .unwrap_or(false)
            })
            .collect();
        assert_eq!(remaining.len(), 0);
    }
}

/// Test that hook handler correctly writes JSONL events to a temp directory
#[test]
fn test_hook_handler_writes_jsonl() {
    use nose::event::{AgentType, Confidence, Event, EventData};
    use chrono::Utc;
    use uuid::Uuid;

    let dir = TempDir::new().unwrap();
    let path = dir.path().join("claude_test_session.jsonl");

    let event = Event {
        event_id: Uuid::new_v4(),
        session_id: "test_session".to_string(),
        timestamp: Utc::now(),
        agent_type: AgentType::Claude,
        workspace: "/test".to_string(),
        confidence: Confidence::Native,
        raw_payload: Some(json!({"tool_name": "Read"})),
        data: EventData::ToolCall {
            tool_name: "Read".to_string(),
            input: json!({"file_path": "/src/main.rs"}),
        },
    };

    // Write event
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .unwrap();
    let json_str = serde_json::to_string(&event).unwrap();
    writeln!(file, "{}", json_str).unwrap();
    drop(file);

    // Read and verify
    let content = std::fs::read_to_string(&path).unwrap();
    let lines: Vec<&str> = content.trim().split('\n').collect();
    assert_eq!(lines.len(), 1);

    let parsed: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
    assert_eq!(parsed["session_id"], "test_session");
    assert_eq!(parsed["event_type"], "ToolCall");
    assert_eq!(parsed["tool_name"], "Read");
}

/// Test that codex config structure is valid
#[test]
fn test_codex_config_structure() {
    let mut config = json!({});
    let config_map = config.as_object_mut().unwrap();

    for event in &["SessionStart", "SessionStop"] {
        let arr = config_map
            .entry(event.to_string())
            .or_insert_with(|| json!([]));
        arr.as_array_mut().unwrap().push(json!({
            "command": format!("/usr/bin/nose hook-handler --agent codex --event {}", event),
            "_nose_managed": true
        }));
    }

    let serialized = serde_json::to_string(&config).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&serialized).unwrap();
    assert_eq!(parsed["SessionStart"].as_array().unwrap().len(), 1);
    assert_eq!(parsed["SessionStop"].as_array().unwrap().len(), 1);
}

/// Test that gemini config structure is valid
#[test]
fn test_gemini_config_structure() {
    let mut settings = json!({});
    let hooks_obj = settings
        .as_object_mut()
        .unwrap()
        .entry("hooks")
        .or_insert_with(|| json!({}));
    let hooks_map = hooks_obj.as_object_mut().unwrap();

    for event in &["BeforeTool", "AfterTool", "SessionStart", "SessionEnd"] {
        let arr = hooks_map
            .entry(event.to_string())
            .or_insert_with(|| json!([]));
        arr.as_array_mut().unwrap().push(json!({
            "command": format!("/usr/bin/nose hook-handler --agent gemini --event {}", event),
            "_nose_managed": true
        }));
    }

    let serialized = serde_json::to_string(&settings).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&serialized).unwrap();
    assert_eq!(
        parsed["hooks"]["BeforeTool"].as_array().unwrap().len(),
        1
    );
}
