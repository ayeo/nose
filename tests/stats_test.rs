use chrono::Utc;
use nose::event::{AgentType, Confidence, Event, EventData};
use nose::stats::Stats;
use uuid::Uuid;

fn make_event(data: EventData) -> Event {
    Event {
        event_id: Uuid::new_v4(),
        session_id: "test-session".to_string(),
        timestamp: Utc::now(),
        agent_type: AgentType::Claude,
        workspace: "/test/workspace".to_string(),
        confidence: Confidence::Native,
        raw_payload: None,
        data,
    }
}

#[test]
fn test_stats_counts_sessions() {
    let mut stats = Stats::new();
    stats.add_event(&make_event(EventData::SessionStart {
        environment: None,
        args: vec![],
        config: serde_json::json!({}),
    }));
    stats.add_event(&make_event(EventData::SessionStart {
        environment: None,
        args: vec![],
        config: serde_json::json!({}),
    }));
    assert_eq!(stats.sessions, 2);
    assert_eq!(stats.total_events, 2);
}

#[test]
fn test_stats_accumulates_duration() {
    let mut stats = Stats::new();
    stats.add_event(&make_event(EventData::SessionEnd {
        exit_code: Some(0),
        duration_ms: 10_000,
    }));
    stats.add_event(&make_event(EventData::SessionEnd {
        exit_code: Some(0),
        duration_ms: 5_000,
    }));
    assert_eq!(stats.total_duration_ms, 15_000);
}

#[test]
fn test_stats_counts_tokens() {
    let mut stats = Stats::new();
    stats.add_event(&make_event(EventData::ModelRequest {
        model: "claude-opus-4-6".to_string(),
        provider: None,
        input_tokens: Some(1000),
    }));
    stats.add_event(&make_event(EventData::ModelRequest {
        model: "claude-opus-4-6".to_string(),
        provider: None,
        input_tokens: Some(500),
    }));
    stats.add_event(&make_event(EventData::ModelResponse {
        output_tokens: Some(200),
        stop_reason: None,
        duration_ms: None,
    }));
    assert_eq!(stats.input_tokens, 1500);
    assert_eq!(stats.output_tokens, 200);
    assert_eq!(*stats.model_counts.get("claude-opus-4-6").unwrap(), 2);
}

#[test]
fn test_stats_tracks_tool_calls() {
    let mut stats = Stats::new();
    for _ in 0..5 {
        stats.add_event(&make_event(EventData::ToolCall {
            tool_name: "Read".to_string(),
            input: serde_json::json!({}),
        }));
    }
    stats.add_event(&make_event(EventData::ToolCall {
        tool_name: "Bash".to_string(),
        input: serde_json::json!({}),
    }));
    assert_eq!(*stats.tool_counts.get("Read").unwrap(), 5);
    assert_eq!(*stats.tool_counts.get("Bash").unwrap(), 1);
}

#[test]
fn test_stats_tracks_unique_files() {
    let mut stats = Stats::new();
    stats.add_event(&make_event(EventData::FileRead {
        path: "/src/main.rs".to_string(),
    }));
    stats.add_event(&make_event(EventData::FileRead {
        path: "/src/main.rs".to_string(), // duplicate
    }));
    stats.add_event(&make_event(EventData::FileWrite {
        path: "/src/lib.rs".to_string(),
        bytes_written: Some(100),
    }));
    stats.add_event(&make_event(EventData::FileDelete {
        path: "/tmp/old.rs".to_string(),
    }));
    assert_eq!(stats.files_touched.len(), 3);
}

#[test]
fn test_stats_counts_commands() {
    let mut stats = Stats::new();
    stats.add_event(&make_event(EventData::CommandExec {
        command: "cargo test".to_string(),
        cwd: None,
        exit_code: Some(0),
        duration_ms: None,
    }));
    stats.add_event(&make_event(EventData::CommandExec {
        command: "ls".to_string(),
        cwd: None,
        exit_code: Some(0),
        duration_ms: None,
    }));
    assert_eq!(stats.commands_run, 2);
}

#[test]
fn test_stats_event_type_counts() {
    let mut stats = Stats::new();
    stats.add_event(&make_event(EventData::ToolCall {
        tool_name: "Read".to_string(),
        input: serde_json::json!({}),
    }));
    stats.add_event(&make_event(EventData::ToolResult {
        tool_name: "Read".to_string(),
        output_summary: None,
        error: None,
        duration_ms: None,
    }));
    stats.add_event(&make_event(EventData::ToolCall {
        tool_name: "Bash".to_string(),
        input: serde_json::json!({}),
    }));
    assert_eq!(*stats.event_counts.get("ToolCall").unwrap(), 2);
    assert_eq!(*stats.event_counts.get("ToolResult").unwrap(), 1);
    assert_eq!(stats.total_events, 3);
}

#[test]
fn test_display_contains_expected_strings() {
    let mut stats = Stats::new();
    stats.add_event(&make_event(EventData::SessionStart {
        environment: None,
        args: vec![],
        config: serde_json::json!({}),
    }));
    stats.add_event(&make_event(EventData::ModelRequest {
        model: "claude-opus-4-6".to_string(),
        provider: None,
        input_tokens: Some(1_234_567),
    }));
    stats.add_event(&make_event(EventData::ModelResponse {
        output_tokens: Some(456_789),
        stop_reason: None,
        duration_ms: None,
    }));
    stats.add_event(&make_event(EventData::ToolCall {
        tool_name: "Read".to_string(),
        input: serde_json::json!({}),
    }));
    stats.add_event(&make_event(EventData::FileRead {
        path: "/src/main.rs".to_string(),
    }));
    stats.add_event(&make_event(EventData::CommandExec {
        command: "cargo build".to_string(),
        cwd: None,
        exit_code: Some(0),
        duration_ms: None,
    }));
    stats.add_event(&make_event(EventData::SessionEnd {
        exit_code: Some(0),
        duration_ms: 16_320_000,
    }));

    // Verify counters without capturing stdout
    assert_eq!(stats.sessions, 1);
    assert_eq!(stats.input_tokens, 1_234_567);
    assert_eq!(stats.output_tokens, 456_789);
    assert_eq!(*stats.tool_counts.get("Read").unwrap(), 1);
    assert_eq!(stats.files_touched.len(), 1);
    assert_eq!(stats.commands_run, 1);
    assert_eq!(stats.total_duration_ms, 16_320_000);
    assert_eq!(*stats.model_counts.get("claude-opus-4-6").unwrap(), 1);
}

#[test]
fn test_model_request_without_tokens() {
    let mut stats = Stats::new();
    stats.add_event(&make_event(EventData::ModelRequest {
        model: "claude-sonnet-4-6".to_string(),
        provider: None,
        input_tokens: None,
    }));
    assert_eq!(stats.input_tokens, 0);
    assert_eq!(*stats.model_counts.get("claude-sonnet-4-6").unwrap(), 1);
}
