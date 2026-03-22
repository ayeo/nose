use nose::adapter::claude::ClaudeAdapter;
use nose::adapter::Adapter;
use nose::event::{AgentType, Confidence, EventData};
use std::io::Cursor;

#[test]
fn test_claude_parse_simple_session() {
    let fixture = include_str!("fixtures/claude/simple_session.jsonl");
    let adapter = ClaudeAdapter;
    let mut reader = Cursor::new(fixture.as_bytes());
    let events = adapter.parse(&mut reader, "sess_01", "/project").unwrap();

    assert!(events.len() >= 2, "Expected at least 2 events, got {}", events.len());

    let model_req = events.iter().find(|e| matches!(&e.data, EventData::ModelRequest { .. }));
    assert!(model_req.is_some(), "Expected ModelRequest event");

    let model_resp = events.iter().find(|e| matches!(&e.data, EventData::ModelResponse { .. }));
    assert!(model_resp.is_some(), "Expected ModelResponse event");

    for event in &events {
        assert_eq!(event.agent_type, AgentType::Claude);
        assert_eq!(event.workspace, "/project");
    }
}

#[test]
fn test_claude_parse_tool_use_session() {
    let fixture = include_str!("fixtures/claude/tool_use_session.jsonl");
    let adapter = ClaudeAdapter;
    let mut reader = Cursor::new(fixture.as_bytes());
    let events = adapter.parse(&mut reader, "sess_02", "/project").unwrap();

    let tool_calls: Vec<_> = events.iter().filter(|e| matches!(&e.data, EventData::ToolCall { .. })).collect();
    assert_eq!(tool_calls.len(), 2, "Expected 2 ToolCall events");

    let file_reads: Vec<_> = events.iter().filter(|e| matches!(&e.data, EventData::FileRead { .. })).collect();
    assert_eq!(file_reads.len(), 1, "Expected 1 FileRead event");
    assert_eq!(file_reads[0].confidence, Confidence::Inferred);

    let cmd_execs: Vec<_> = events.iter().filter(|e| matches!(&e.data, EventData::CommandExec { .. })).collect();
    assert_eq!(cmd_execs.len(), 1, "Expected 1 CommandExec event");
    assert_eq!(cmd_execs[0].confidence, Confidence::Inferred);
}

#[test]
fn test_claude_detect() {
    let adapter = ClaudeAdapter;
    assert!(adapter.detect(std::path::Path::new("/Users/test/.claude/projects/foo/abc123.jsonl")));
    assert!(!adapter.detect(std::path::Path::new("/Users/test/.codex/log/foo.json")));
}

#[test]
fn test_claude_discovery_paths() {
    let adapter = ClaudeAdapter;
    let paths = adapter.discovery_paths();
    assert!(!paths.is_empty());
    assert!(paths[0].to_string_lossy().contains(".claude"));
}
