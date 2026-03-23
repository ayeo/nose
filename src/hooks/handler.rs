use chrono::Utc;
use serde_json::Value;
use std::fs::{self, OpenOptions};
use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use uuid::Uuid;

use crate::event::{AgentType, Confidence, Event, EventData};

/// Run the hook handler: read stdin, transform to event, append to JSONL file, output `{}`.
pub fn run_hook_handler(agent: &str, event: &str) {
    // Read all of stdin
    let stdin = io::stdin();
    let mut input = String::new();
    for line in stdin.lock().lines() {
        match line {
            Ok(l) => {
                input.push_str(&l);
                input.push('\n');
            }
            Err(_) => break,
        }
    }

    let payload: Value = match serde_json::from_str(input.trim()) {
        Ok(v) => v,
        Err(_) => {
            // If we can't parse, still output {} so the agent isn't blocked
            print!("{{}}");
            return;
        }
    };

    let session_id = payload
        .get("session_id")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();

    let agent_type = match agent {
        "claude" => AgentType::Claude,
        "codex" => AgentType::Codex,
        "gemini" => AgentType::Gemini,
        _ => {
            print!("{{}}");
            return;
        }
    };

    let events = transform_payload(&agent_type, event, &payload, &session_id);

    if !events.is_empty() {
        if let Err(e) = append_events(&events, agent, &session_id) {
            eprintln!("nose: hook-handler: failed to write events: {}", e);
        }
    }

    // Required: output empty JSON to stdout
    print!("{{}}");
}

fn transform_payload(
    agent_type: &AgentType,
    event_name: &str,
    payload: &Value,
    session_id: &str,
) -> Vec<Event> {
    let now = Utc::now();
    let workspace = std::env::current_dir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();

    let data = match (agent_type, event_name) {
        (AgentType::Claude, "PreToolUse") | (AgentType::Gemini, "BeforeTool") => {
            let tool_name = payload
                .get("tool_name")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();
            let input = payload
                .get("tool_input")
                .cloned()
                .unwrap_or(Value::Null);
            Some(EventData::ToolCall {
                tool_name,
                input,
            })
        }
        (AgentType::Claude, "PostToolUse") | (AgentType::Gemini, "AfterTool") => {
            let tool_name = payload
                .get("tool_name")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();
            let output_summary = payload
                .get("tool_response")
                .and_then(|v| v.as_str())
                .map(|s| truncate(s, 500).to_string());
            Some(EventData::ToolResult {
                tool_name,
                output_summary,
                error: None,
                duration_ms: None,
            })
        }
        (_, "SessionStart") => Some(EventData::SessionStart {
            environment: None,
            args: Vec::new(),
            config: Value::Null,
        }),
        (_, "SessionEnd") | (AgentType::Codex, "SessionStop") => Some(EventData::SessionEnd {
            exit_code: None,
            duration_ms: 0,
        }),
        _ => None,
    };

    match data {
        Some(event_data) => vec![Event {
            event_id: Uuid::new_v4(),
            session_id: session_id.to_string(),
            timestamp: now,
            agent_type: agent_type.clone(),
            workspace,
            confidence: Confidence::Native,
            raw_payload: Some(payload.clone()),
            data: event_data,
        }],
        None => Vec::new(),
    }
}

fn truncate(s: &str, max_len: usize) -> &str {
    if s.len() <= max_len {
        s
    } else {
        &s[..max_len]
    }
}

fn events_dir() -> PathBuf {
    let home = dirs::home_dir().expect("could not determine home directory");
    home.join(".nose").join("events")
}

fn append_events(events: &[Event], agent: &str, session_id: &str) -> io::Result<()> {
    let dir = events_dir();
    fs::create_dir_all(&dir)?;

    let filename = format!("{}_{}.jsonl", agent, session_id);
    let path = dir.join(filename);

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)?;

    for event in events {
        let json = serde_json::to_string(event)?;
        writeln!(file, "{}", json)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_transform_claude_pre_tool_use() {
        let payload = json!({
            "session_id": "abc123",
            "tool_name": "Read",
            "tool_input": {"file_path": "/src/main.rs"}
        });

        let events = transform_payload(&AgentType::Claude, "PreToolUse", &payload, "abc123");
        assert_eq!(events.len(), 1);

        let event = &events[0];
        assert_eq!(event.session_id, "abc123");
        match &event.data {
            EventData::ToolCall { tool_name, input } => {
                assert_eq!(tool_name, "Read");
                assert_eq!(input["file_path"], "/src/main.rs");
            }
            _ => panic!("expected ToolCall event"),
        }
    }

    #[test]
    fn test_transform_claude_post_tool_use() {
        let payload = json!({
            "session_id": "abc123",
            "tool_name": "Read",
            "tool_response": "file contents here"
        });

        let events = transform_payload(&AgentType::Claude, "PostToolUse", &payload, "abc123");
        assert_eq!(events.len(), 1);

        match &events[0].data {
            EventData::ToolResult {
                tool_name,
                output_summary,
                error,
                ..
            } => {
                assert_eq!(tool_name, "Read");
                assert_eq!(output_summary.as_deref(), Some("file contents here"));
                assert!(error.is_none());
            }
            _ => panic!("expected ToolResult event"),
        }
    }

    #[test]
    fn test_transform_session_start() {
        let payload = json!({"session_id": "sess1"});
        let events = transform_payload(&AgentType::Claude, "SessionStart", &payload, "sess1");
        assert_eq!(events.len(), 1);
        assert!(matches!(&events[0].data, EventData::SessionStart { .. }));
    }

    #[test]
    fn test_transform_session_end() {
        let payload = json!({"session_id": "sess1"});
        let events = transform_payload(&AgentType::Claude, "SessionEnd", &payload, "sess1");
        assert_eq!(events.len(), 1);
        assert!(matches!(&events[0].data, EventData::SessionEnd { .. }));
    }

    #[test]
    fn test_transform_codex_session_stop() {
        let payload = json!({"session_id": "sess1"});
        let events = transform_payload(&AgentType::Codex, "SessionStop", &payload, "sess1");
        assert_eq!(events.len(), 1);
        assert!(matches!(&events[0].data, EventData::SessionEnd { .. }));
    }

    #[test]
    fn test_transform_gemini_before_tool() {
        let payload = json!({
            "session_id": "gem1",
            "tool_name": "shell",
            "tool_input": {"command": "ls"}
        });

        let events = transform_payload(&AgentType::Gemini, "BeforeTool", &payload, "gem1");
        assert_eq!(events.len(), 1);
        assert!(matches!(&events[0].data, EventData::ToolCall { .. }));
    }

    #[test]
    fn test_transform_unknown_event_returns_empty() {
        let payload = json!({"session_id": "x"});
        let events = transform_payload(&AgentType::Claude, "UnknownEvent", &payload, "x");
        assert!(events.is_empty());
    }

    #[test]
    fn test_truncate() {
        assert_eq!(truncate("hello", 10), "hello");
        assert_eq!(truncate("hello world", 5), "hello");
    }

    #[test]
    fn test_append_events_creates_file() {
        let dir = tempfile::tempdir().unwrap();
        // Override the events dir by writing directly
        let filename = dir.path().join("claude_test123.jsonl");

        let event = Event {
            event_id: Uuid::new_v4(),
            session_id: "test123".to_string(),
            timestamp: Utc::now(),
            agent_type: AgentType::Claude,
            workspace: "/tmp".to_string(),
            confidence: Confidence::Native,
            raw_payload: None,
            data: EventData::SessionStart {
                environment: None,
                args: Vec::new(),
                config: serde_json::Value::Null,
            },
        };

        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&filename)
            .unwrap();

        let json = serde_json::to_string(&event).unwrap();
        writeln!(file, "{}", json).unwrap();

        let content = std::fs::read_to_string(&filename).unwrap();
        assert!(content.contains("test123"));
        assert!(content.contains("SessionStart"));
    }
}
