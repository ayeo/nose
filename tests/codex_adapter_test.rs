use nose::adapter::codex::CodexAdapter;
use nose::adapter::Adapter;
use nose::event::{AgentType, Confidence, EventData};
use std::io::Cursor;

#[test]
fn test_codex_parse_simple_session() {
    let fixture = include_str!("fixtures/codex/simple_session.jsonl");
    let adapter = CodexAdapter;
    let mut reader = Cursor::new(fixture.as_bytes());
    let events = adapter.parse(&mut reader, "sess_01", "/project").unwrap();

    assert!(events.len() >= 2, "Expected at least 2 events, got {}", events.len());

    let model_req = events.iter().find(|e| matches!(&e.data, EventData::ModelRequest { .. }));
    assert!(model_req.is_some(), "Expected ModelRequest event");

    let model_resp = events.iter().find(|e| matches!(&e.data, EventData::ModelResponse { .. }));
    assert!(model_resp.is_some(), "Expected ModelResponse event");

    for event in &events {
        assert_eq!(event.agent_type, AgentType::Codex);
        assert_eq!(event.workspace, "/project");
    }
}

#[test]
fn test_codex_parse_tool_use_session() {
    let fixture = include_str!("fixtures/codex/tool_use_session.jsonl");
    let adapter = CodexAdapter;
    let mut reader = Cursor::new(fixture.as_bytes());
    let events = adapter.parse(&mut reader, "sess_02", "/project").unwrap();

    let tool_calls: Vec<_> = events
        .iter()
        .filter(|e| matches!(&e.data, EventData::ToolCall { .. }))
        .collect();
    assert_eq!(tool_calls.len(), 2, "Expected 2 ToolCall events (shell + read)");

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
fn test_codex_parse_file_ops_session() {
    let fixture = include_str!("fixtures/codex/file_ops_session.jsonl");
    let adapter = CodexAdapter;
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

    let file_deletes: Vec<_> = events
        .iter()
        .filter(|e| matches!(&e.data, EventData::FileDelete { .. }))
        .collect();
    assert_eq!(file_deletes.len(), 1, "Expected 1 FileDelete event");

    if let EventData::FileDelete { path } = &file_deletes[0].data {
        assert_eq!(path, "/project/old.txt");
    }
}

#[test]
fn test_codex_detect() {
    let adapter = CodexAdapter;
    assert!(adapter.detect(std::path::Path::new("/Users/test/.codex/sessions/foo/abc123.jsonl")));
    assert!(!adapter.detect(std::path::Path::new("/Users/test/.claude/projects/foo/abc123.jsonl")));
    assert!(!adapter.detect(std::path::Path::new("/Users/test/.codex/sessions/foo/session.json")));
}

#[test]
fn test_codex_discovery_paths() {
    let adapter = CodexAdapter;
    let cwd = std::path::Path::new("/Users/test/workspace/myproject");
    let paths = adapter.discovery_paths(cwd);
    assert!(!paths.is_empty());
    assert!(paths[0].to_string_lossy().contains(".codex"));
    assert!(paths[0].to_string_lossy().contains("-Users-test-workspace-myproject"));
}

#[test]
fn test_codex_parse_emits_session_lifecycle() {
    let fixture = include_str!("fixtures/codex/simple_session.jsonl");
    let adapter = CodexAdapter;
    let mut reader = Cursor::new(fixture.as_bytes());
    let events = adapter.parse(&mut reader, "sess_01", "/project").unwrap();

    let starts: Vec<_> = events
        .iter()
        .filter(|e| matches!(&e.data, EventData::SessionStart { .. }))
        .collect();
    assert_eq!(starts.len(), 1, "Expected 1 SessionStart event");

    let ends: Vec<_> = events
        .iter()
        .filter(|e| matches!(&e.data, EventData::SessionEnd { .. }))
        .collect();
    assert_eq!(ends.len(), 1, "Expected 1 SessionEnd event");
}

#[test]
fn test_codex_tool_result_tracks_tool_name() {
    let fixture = include_str!("fixtures/codex/tool_use_session.jsonl");
    let adapter = CodexAdapter;
    let mut reader = Cursor::new(fixture.as_bytes());
    let events = adapter.parse(&mut reader, "sess_02", "/project").unwrap();

    let tool_results: Vec<_> = events
        .iter()
        .filter(|e| matches!(&e.data, EventData::ToolResult { .. }))
        .collect();

    // First result should be from 'shell', second from 'read'
    if let EventData::ToolResult { tool_name, .. } = &tool_results[0].data {
        assert_eq!(tool_name, "shell");
    }
    if let EventData::ToolResult { tool_name, .. } = &tool_results[1].data {
        assert_eq!(tool_name, "read");
    }
}
