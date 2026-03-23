use nose::adapter::hook::HookAdapter;
use nose::adapter::Adapter;
use nose::event::{AgentType, Confidence, Event, EventData};
use std::io::Cursor;
use std::path::Path;
use uuid::Uuid;
use chrono::Utc;

fn make_test_event() -> Event {
    Event {
        event_id: Uuid::new_v4(),
        session_id: "test-session-123".to_string(),
        timestamp: Utc::now(),
        agent_type: AgentType::Claude,
        workspace: "/test/workspace".to_string(),
        confidence: Confidence::Native,
        raw_payload: None,
        data: EventData::FileRead {
            path: "/test/workspace/src/main.rs".to_string(),
        },
    }
}

#[test]
fn test_hook_adapter_parse_single_event() {
    let event = make_test_event();
    let line = serde_json::to_string(&event).unwrap();
    let mut reader = Cursor::new(line.as_bytes());

    let adapter = HookAdapter;
    let events = adapter.parse(&mut reader, "ignored", "ignored").unwrap();

    assert_eq!(events.len(), 1);
    assert_eq!(events[0].session_id, "test-session-123");
    assert_eq!(events[0].workspace, "/test/workspace");
    assert_eq!(events[0].agent_type, AgentType::Claude);
    assert_eq!(events[0].confidence, Confidence::Native);
    assert!(matches!(&events[0].data, EventData::FileRead { path } if path == "/test/workspace/src/main.rs"));
}

#[test]
fn test_hook_adapter_parse_multiple_events() {
    let event1 = Event {
        event_id: Uuid::new_v4(),
        session_id: "sess-abc".to_string(),
        timestamp: Utc::now(),
        agent_type: AgentType::Claude,
        workspace: "/project".to_string(),
        confidence: Confidence::Native,
        raw_payload: None,
        data: EventData::SessionStart {
            environment: Some("test".to_string()),
            args: vec!["--flag".to_string()],
            config: serde_json::Value::Null,
        },
    };
    let event2 = Event {
        event_id: Uuid::new_v4(),
        session_id: "sess-abc".to_string(),
        timestamp: Utc::now(),
        agent_type: AgentType::Claude,
        workspace: "/project".to_string(),
        confidence: Confidence::Native,
        raw_payload: None,
        data: EventData::SessionEnd {
            exit_code: Some(0),
            duration_ms: 1234,
        },
    };

    let jsonl = format!(
        "{}\n{}\n",
        serde_json::to_string(&event1).unwrap(),
        serde_json::to_string(&event2).unwrap()
    );
    let mut reader = Cursor::new(jsonl.as_bytes());

    let adapter = HookAdapter;
    let events = adapter.parse(&mut reader, "ignored", "ignored").unwrap();

    assert_eq!(events.len(), 2);
    assert!(matches!(&events[0].data, EventData::SessionStart { .. }));
    assert!(matches!(&events[1].data, EventData::SessionEnd { .. }));
}

#[test]
fn test_hook_adapter_skips_empty_lines_and_invalid_json() {
    let event = make_test_event();
    let jsonl = format!(
        "\n{}\nnot-valid-json\n\n",
        serde_json::to_string(&event).unwrap()
    );
    let mut reader = Cursor::new(jsonl.as_bytes());

    let adapter = HookAdapter;
    let events = adapter.parse(&mut reader, "ignored", "ignored").unwrap();

    assert_eq!(events.len(), 1, "Expected only the valid event to be parsed");
}

#[test]
fn test_hook_adapter_detect() {
    let adapter = HookAdapter;
    assert!(adapter.detect(Path::new("/home/user/.nose/events/claude_abc123.jsonl")));
    assert!(adapter.detect(Path::new("/Users/foo/.nose/events/codex_xyz.jsonl")));
    assert!(!adapter.detect(Path::new("/home/user/.claude/projects/foo/bar.jsonl")));
    assert!(!adapter.detect(Path::new("/home/user/.nose/events/somefile.json")));
    assert!(!adapter.detect(Path::new("/home/user/.nose/other/file.jsonl")));
}

#[test]
fn test_hook_adapter_name() {
    let adapter = HookAdapter;
    assert_eq!(adapter.name(), "hook");
}

#[test]
fn test_hook_adapter_parse_preserves_event_fields() {
    let original = Event {
        event_id: Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap(),
        session_id: "preserve-me".to_string(),
        timestamp: Utc::now(),
        agent_type: AgentType::Codex,
        workspace: "/preserved/workspace".to_string(),
        confidence: Confidence::Inferred,
        raw_payload: Some(serde_json::json!({"key": "value"})),
        data: EventData::CommandExec {
            command: "cargo test".to_string(),
            cwd: Some("/preserved/workspace".to_string()),
            exit_code: Some(0),
            duration_ms: Some(5000),
        },
    };

    let line = serde_json::to_string(&original).unwrap();
    let mut reader = Cursor::new(line.as_bytes());

    let adapter = HookAdapter;
    let events = adapter.parse(&mut reader, "unused-session", "unused-workspace").unwrap();

    assert_eq!(events.len(), 1);
    let parsed = &events[0];
    assert_eq!(parsed.event_id, original.event_id);
    assert_eq!(parsed.session_id, "preserve-me");
    assert_eq!(parsed.workspace, "/preserved/workspace");
    assert_eq!(parsed.agent_type, AgentType::Codex);
    assert_eq!(parsed.confidence, Confidence::Inferred);
    assert!(parsed.raw_payload.is_some());
    assert!(matches!(
        &parsed.data,
        EventData::CommandExec { command, exit_code: Some(0), .. } if command == "cargo test"
    ));
}
