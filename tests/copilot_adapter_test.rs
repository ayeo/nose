use nose::adapter::copilot::CopilotAdapter;
use nose::adapter::Adapter;
use nose::event::{AgentType, Confidence, EventData};
use std::io::Cursor;

#[test]
fn test_copilot_parse_simple_session() {
    let fixture = include_str!("fixtures/copilot/simple_session.jsonl");
    let adapter = CopilotAdapter;
    let mut reader = Cursor::new(fixture.as_bytes());
    let events = adapter.parse(&mut reader, "sess_01", "/project").unwrap();

    assert!(events.len() >= 2, "Expected at least 2 events, got {}", events.len());

    let session_start = events.iter().find(|e| matches!(&e.data, EventData::SessionStart { .. }));
    assert!(session_start.is_some(), "Expected SessionStart event");

    let session_end = events.iter().find(|e| matches!(&e.data, EventData::SessionEnd { .. }));
    assert!(session_end.is_some(), "Expected SessionEnd event");

    for event in &events {
        assert_eq!(event.agent_type, AgentType::Copilot);
        assert_eq!(event.workspace, "/project");
    }
}

#[test]
fn test_copilot_parse_session_start_is_native() {
    let fixture = include_str!("fixtures/copilot/simple_session.jsonl");
    let adapter = CopilotAdapter;
    let mut reader = Cursor::new(fixture.as_bytes());
    let events = adapter.parse(&mut reader, "sess_01", "/project").unwrap();

    let session_start = events.iter().find(|e| matches!(&e.data, EventData::SessionStart { .. })).unwrap();
    assert_eq!(session_start.confidence, Confidence::Native);

    let session_end = events.iter().find(|e| matches!(&e.data, EventData::SessionEnd { .. })).unwrap();
    assert_eq!(session_end.confidence, Confidence::Native);
}

#[test]
fn test_copilot_parse_tool_use_session() {
    let fixture = include_str!("fixtures/copilot/tool_use_session.jsonl");
    let adapter = CopilotAdapter;
    let mut reader = Cursor::new(fixture.as_bytes());
    let events = adapter.parse(&mut reader, "sess_02", "/project").unwrap();

    let tool_calls: Vec<_> = events
        .iter()
        .filter(|e| matches!(&e.data, EventData::ToolCall { .. }))
        .collect();
    assert_eq!(tool_calls.len(), 2, "Expected 2 ToolCall events (read + bash)");

    let cmd_execs: Vec<_> = events
        .iter()
        .filter(|e| matches!(&e.data, EventData::CommandExec { .. }))
        .collect();
    assert_eq!(cmd_execs.len(), 1, "Expected 1 CommandExec event");
    assert_eq!(cmd_execs[0].confidence, Confidence::Inferred);

    let file_reads: Vec<_> = events
        .iter()
        .filter(|e| matches!(&e.data, EventData::FileRead { .. }))
        .collect();
    assert_eq!(file_reads.len(), 1, "Expected 1 FileRead event");
    assert_eq!(file_reads[0].confidence, Confidence::Inferred);

    let tool_results: Vec<_> = events
        .iter()
        .filter(|e| matches!(&e.data, EventData::ToolResult { .. }))
        .collect();
    assert_eq!(tool_results.len(), 2, "Expected 2 ToolResult events");
}

#[test]
fn test_copilot_parse_file_ops_session() {
    let fixture = include_str!("fixtures/copilot/file_ops_session.jsonl");
    let adapter = CopilotAdapter;
    let mut reader = Cursor::new(fixture.as_bytes());
    let events = adapter.parse(&mut reader, "sess_03", "/project").unwrap();

    let file_writes: Vec<_> = events
        .iter()
        .filter(|e| matches!(&e.data, EventData::FileWrite { .. }))
        .collect();
    assert_eq!(file_writes.len(), 1, "Expected 1 FileWrite event");
    assert_eq!(file_writes[0].confidence, Confidence::Inferred);

    if let EventData::FileWrite { path, bytes_written } = &file_writes[0].data {
        assert_eq!(path, "/project/hello.txt");
        assert_eq!(*bytes_written, Some(13)); // "Hello, world!" = 13 bytes
    }
}

#[test]
fn test_copilot_parse_error_session() {
    let fixture = include_str!("fixtures/copilot/error_session.jsonl");
    let adapter = CopilotAdapter;
    let mut reader = Cursor::new(fixture.as_bytes());
    let events = adapter.parse(&mut reader, "sess_04", "/project").unwrap();

    let errors: Vec<_> = events
        .iter()
        .filter(|e| matches!(&e.data, EventData::Error { .. }))
        .collect();
    assert_eq!(errors.len(), 1, "Expected 1 Error event");
    assert_eq!(errors[0].confidence, Confidence::Native);

    if let EventData::Error { error_type, message, .. } = &errors[0].data {
        assert_eq!(error_type, "timeout");
        assert_eq!(message, "Request timed out");
    }
}

#[test]
fn test_copilot_detect() {
    let adapter = CopilotAdapter;
    assert!(adapter.detect(std::path::Path::new("/Users/test/.github-copilot/hooks/session.jsonl")));
    assert!(!adapter.detect(std::path::Path::new("/Users/test/.claude/projects/foo/abc123.jsonl")));
    assert!(!adapter.detect(std::path::Path::new("/Users/test/.github-copilot/hooks/session.json")));
}

#[test]
fn test_copilot_name() {
    let adapter = CopilotAdapter;
    assert_eq!(adapter.name(), "copilot");
}
