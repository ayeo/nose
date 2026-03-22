use nose::event::{AgentType, Confidence, Event, EventData};
use chrono::Utc;
use uuid::Uuid;

#[test]
fn test_session_start_serializes_to_jsonl() {
    let event = Event {
        event_id: Uuid::nil(),
        session_id: "sess_01".to_string(),
        timestamp: chrono::DateTime::parse_from_rfc3339("2026-03-22T10:00:00Z")
            .unwrap()
            .with_timezone(&Utc),
        agent_type: AgentType::Claude,
        workspace: "/project".to_string(),
        confidence: Confidence::Native,
        raw_payload: None,
        data: EventData::SessionStart {
            environment: Some("cli".to_string()),
            args: vec!["--model".to_string(), "opus".to_string()],
            config: serde_json::json!({}),
        },
    };

    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains("\"event_type\":\"SessionStart\""));
    assert!(json.contains("\"agent_type\":\"claude\""));
    assert!(json.contains("\"confidence\":\"native\""));
    assert!(json.contains("\"session_id\":\"sess_01\""));
}

#[test]
fn test_tool_call_serializes() {
    let event = Event {
        event_id: Uuid::nil(),
        session_id: "sess_01".to_string(),
        timestamp: Utc::now(),
        agent_type: AgentType::Claude,
        workspace: "/project".to_string(),
        confidence: Confidence::Native,
        raw_payload: None,
        data: EventData::ToolCall {
            tool_name: "Read".to_string(),
            input: serde_json::json!({"file_path": "/src/main.rs"}),
        },
    };

    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains("\"event_type\":\"ToolCall\""));
    assert!(json.contains("\"tool_name\":\"Read\""));
}

#[test]
fn test_event_roundtrip() {
    let event = Event {
        event_id: Uuid::nil(),
        session_id: "sess_01".to_string(),
        timestamp: chrono::DateTime::parse_from_rfc3339("2026-03-22T10:00:00Z")
            .unwrap()
            .with_timezone(&Utc),
        agent_type: AgentType::Claude,
        workspace: "/project".to_string(),
        confidence: Confidence::Inferred,
        raw_payload: Some(serde_json::json!({"original": "data"})),
        data: EventData::FileRead {
            path: "/src/main.rs".to_string(),
        },
    };

    let json = serde_json::to_string(&event).unwrap();
    let deserialized: Event = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.session_id, "sess_01");
}
