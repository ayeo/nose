use nose::adapter::cursor::CursorAdapter;
use nose::adapter::Adapter;
use nose::event::{AgentType, Confidence, EventData};
use std::io::Cursor;

#[test]
fn test_cursor_parse_simple_session() {
    let fixture = include_str!("fixtures/cursor/simple_session.jsonl");
    let adapter = CursorAdapter;
    let mut reader = Cursor::new(fixture.as_bytes());
    let events = adapter.parse(&mut reader, "sess_01", "/project").unwrap();

    assert!(events.len() >= 2, "Expected at least 2 events, got {}", events.len());

    let cmd_execs: Vec<_> = events
        .iter()
        .filter(|e| matches!(&e.data, EventData::CommandExec { .. }))
        .collect();
    assert_eq!(cmd_execs.len(), 1, "Expected 1 CommandExec event");
    assert_eq!(cmd_execs[0].confidence, Confidence::Native);

    if let EventData::CommandExec { command, cwd, .. } = &cmd_execs[0].data {
        assert_eq!(command, "cargo build");
        assert_eq!(cwd.as_deref(), Some("/project"));
    }

    let session_ends: Vec<_> = events
        .iter()
        .filter(|e| matches!(&e.data, EventData::SessionEnd { .. }))
        .collect();
    assert_eq!(session_ends.len(), 1, "Expected 1 SessionEnd event");
    assert_eq!(session_ends[0].confidence, Confidence::Native);

    for event in &events {
        assert_eq!(event.agent_type, AgentType::Cursor);
        assert_eq!(event.workspace, "/project");
        assert_eq!(event.session_id, "sess_01");
    }
}

#[test]
fn test_cursor_parse_tool_use_session() {
    let fixture = include_str!("fixtures/cursor/tool_use_session.jsonl");
    let adapter = CursorAdapter;
    let mut reader = Cursor::new(fixture.as_bytes());
    let events = adapter.parse(&mut reader, "sess_02", "/project").unwrap();

    let file_reads: Vec<_> = events
        .iter()
        .filter(|e| matches!(&e.data, EventData::FileRead { .. }))
        .collect();
    assert_eq!(file_reads.len(), 1, "Expected 1 FileRead event");
    assert_eq!(file_reads[0].confidence, Confidence::Native);
    if let EventData::FileRead { path } = &file_reads[0].data {
        assert_eq!(path, "/project/src/main.rs");
    }

    let file_writes: Vec<_> = events
        .iter()
        .filter(|e| matches!(&e.data, EventData::FileWrite { .. }))
        .collect();
    assert_eq!(file_writes.len(), 1, "Expected 1 FileWrite event");
    assert_eq!(file_writes[0].confidence, Confidence::Native);
    if let EventData::FileWrite { path, .. } = &file_writes[0].data {
        assert_eq!(path, "/project/src/main.rs");
    }

    let cmd_execs: Vec<_> = events
        .iter()
        .filter(|e| matches!(&e.data, EventData::CommandExec { .. }))
        .collect();
    assert_eq!(cmd_execs.len(), 1, "Expected 1 CommandExec event");
    if let EventData::CommandExec { command, .. } = &cmd_execs[0].data {
        assert_eq!(command, "cargo test");
    }
}

#[test]
fn test_cursor_parse_mcp_session() {
    let fixture = include_str!("fixtures/cursor/mcp_session.jsonl");
    let adapter = CursorAdapter;
    let mut reader = Cursor::new(fixture.as_bytes());
    let events = adapter.parse(&mut reader, "sess_03", "/project").unwrap();

    let mcp_calls: Vec<_> = events
        .iter()
        .filter(|e| matches!(&e.data, EventData::McpCall { .. }))
        .collect();
    assert_eq!(mcp_calls.len(), 2, "Expected 2 McpCall events");

    if let EventData::McpCall { server_name, method, params } = &mcp_calls[0].data {
        assert_eq!(server_name, "github");
        assert_eq!(method, "list_issues");
        assert!(params.is_some());
    }

    if let EventData::McpCall { server_name, method, .. } = &mcp_calls[1].data {
        assert_eq!(server_name, "filesystem");
        assert_eq!(method, "read_file");
    }

    for call in &mcp_calls {
        assert_eq!(call.confidence, Confidence::Native);
    }
}

#[test]
fn test_cursor_parse_emits_session_lifecycle() {
    let fixture = include_str!("fixtures/cursor/tool_use_session.jsonl");
    let adapter = CursorAdapter;
    let mut reader = Cursor::new(fixture.as_bytes());
    let events = adapter.parse(&mut reader, "sess_04", "/project").unwrap();

    let starts: Vec<_> = events
        .iter()
        .filter(|e| matches!(&e.data, EventData::SessionStart { .. }))
        .collect();
    assert_eq!(starts.len(), 1, "Expected 1 SessionStart event");
    assert_eq!(starts[0].confidence, Confidence::Inferred);

    let ends: Vec<_> = events
        .iter()
        .filter(|e| matches!(&e.data, EventData::SessionEnd { .. }))
        .collect();
    assert_eq!(ends.len(), 1, "Expected 1 SessionEnd event");
}

#[test]
fn test_cursor_parse_infers_session_end_without_stop() {
    let fixture = include_str!("fixtures/cursor/no_stop_session.jsonl");
    let adapter = CursorAdapter;
    let mut reader = Cursor::new(fixture.as_bytes());
    let events = adapter.parse(&mut reader, "sess_05", "/project").unwrap();

    let ends: Vec<_> = events
        .iter()
        .filter(|e| matches!(&e.data, EventData::SessionEnd { .. }))
        .collect();
    assert_eq!(ends.len(), 1, "Expected 1 SessionEnd event even without stop line");
    assert_eq!(ends[0].confidence, Confidence::Inferred);
}

#[test]
fn test_cursor_detect() {
    let adapter = CursorAdapter;
    assert!(adapter.detect(std::path::Path::new(
        "/Users/test/Library/Application Support/Cursor/User/workspaceStorage/abc123/history.jsonl"
    )));
    assert!(adapter.detect(std::path::Path::new(
        "/home/test/.config/cursor/User/workspaceStorage/abc123/history.jsonl"
    )));
    assert!(!adapter.detect(std::path::Path::new(
        "/Users/test/.claude/projects/foo/session.jsonl"
    )));
    assert!(!adapter.detect(std::path::Path::new(
        "/Users/test/Cursor/User/workspaceStorage/abc123/history.json"
    )));
}

#[test]
fn test_cursor_discovery_paths_macos() {
    let adapter = CursorAdapter;
    let cwd = std::path::Path::new("/Users/test/workspace/myproject");
    let paths = adapter.discovery_paths(cwd);
    // On macOS, should return the Cursor workspaceStorage path
    // On Linux, should return cursor config path
    // Either way, should not panic and the path should reference Cursor
    for path in &paths {
        let s = path.to_string_lossy().to_lowercase();
        assert!(s.contains("cursor"), "Expected path to contain 'cursor', got: {}", path.display());
    }
}
