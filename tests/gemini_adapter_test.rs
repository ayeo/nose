use nose::adapter::gemini::GeminiAdapter;
use nose::adapter::Adapter;
use nose::event::{AgentType, Confidence, EventData};
use std::io::Cursor;

#[test]
fn test_gemini_parse_simple_session() {
    let fixture = include_str!("fixtures/gemini/simple_session.jsonl");
    let adapter = GeminiAdapter;
    let mut reader = Cursor::new(fixture.as_bytes());
    let events = adapter.parse(&mut reader, "sess_g01", "/project").unwrap();

    assert!(events.len() >= 2, "Expected at least 2 events, got {}", events.len());

    let model_req = events.iter().find(|e| matches!(&e.data, EventData::ModelRequest { .. }));
    assert!(model_req.is_some(), "Expected ModelRequest event");

    let model_resp = events.iter().find(|e| matches!(&e.data, EventData::ModelResponse { .. }));
    assert!(model_resp.is_some(), "Expected ModelResponse event");

    for event in &events {
        assert_eq!(event.agent_type, AgentType::Gemini);
        assert_eq!(event.workspace, "/project");
    }
}

#[test]
fn test_gemini_parse_tool_use_session() {
    let fixture = include_str!("fixtures/gemini/tool_use_session.jsonl");
    let adapter = GeminiAdapter;
    let mut reader = Cursor::new(fixture.as_bytes());
    let events = adapter.parse(&mut reader, "sess_g02", "/project").unwrap();

    let tool_calls: Vec<_> = events
        .iter()
        .filter(|e| matches!(&e.data, EventData::ToolCall { .. }))
        .collect();
    assert_eq!(tool_calls.len(), 2, "Expected 2 ToolCall events");

    let file_reads: Vec<_> = events
        .iter()
        .filter(|e| matches!(&e.data, EventData::FileRead { .. }))
        .collect();
    assert_eq!(file_reads.len(), 1, "Expected 1 FileRead event");
    assert_eq!(file_reads[0].confidence, Confidence::Inferred);

    let cmd_execs: Vec<_> = events
        .iter()
        .filter(|e| matches!(&e.data, EventData::CommandExec { .. }))
        .collect();
    assert_eq!(cmd_execs.len(), 1, "Expected 1 CommandExec event");
    assert_eq!(cmd_execs[0].confidence, Confidence::Inferred);

    let tool_results: Vec<_> = events
        .iter()
        .filter(|e| matches!(&e.data, EventData::ToolResult { .. }))
        .collect();
    assert_eq!(tool_results.len(), 2, "Expected 2 ToolResult events");
}

#[test]
fn test_gemini_parse_emits_session_lifecycle() {
    let fixture = include_str!("fixtures/gemini/simple_session.jsonl");
    let adapter = GeminiAdapter;
    let mut reader = Cursor::new(fixture.as_bytes());
    let events = adapter.parse(&mut reader, "sess_g01", "/project").unwrap();

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
fn test_gemini_parse_error_session() {
    let fixture = include_str!("fixtures/gemini/error_session.jsonl");
    let adapter = GeminiAdapter;
    let mut reader = Cursor::new(fixture.as_bytes());
    let events = adapter.parse(&mut reader, "sess_g03", "/project").unwrap();

    let tool_results: Vec<_> = events
        .iter()
        .filter(|e| matches!(&e.data, EventData::ToolResult { .. }))
        .collect();
    assert_eq!(tool_results.len(), 2, "Expected 2 ToolResult events");

    // First tool_result should have an error
    let error_result = &tool_results[0];
    if let EventData::ToolResult { error, .. } = &error_result.data {
        assert!(error.is_some(), "Expected error in first tool result");
    } else {
        panic!("Expected ToolResult");
    }

    // FileWrite should be inferred from write_file tool
    let file_writes: Vec<_> = events
        .iter()
        .filter(|e| matches!(&e.data, EventData::FileWrite { .. }))
        .collect();
    assert_eq!(file_writes.len(), 1, "Expected 1 FileWrite event");
}

#[test]
fn test_gemini_detect() {
    let adapter = GeminiAdapter;
    assert!(adapter.detect(std::path::Path::new(
        "/Users/test/.gemini/projects/foo/session.jsonl"
    )));
    assert!(!adapter.detect(std::path::Path::new(
        "/Users/test/.claude/projects/foo/abc123.jsonl"
    )));
    assert!(!adapter.detect(std::path::Path::new(
        "/Users/test/.gemini/projects/foo/session.json"
    )));
}

#[test]
fn test_gemini_discovery_paths() {
    let adapter = GeminiAdapter;
    let cwd = std::path::Path::new("/Users/test/workspace/myproject");
    let paths = adapter.discovery_paths(cwd);
    assert!(!paths.is_empty());
    assert!(paths[0].to_string_lossy().contains(".gemini"));
    assert!(paths[0].to_string_lossy().contains("-Users-test-workspace-myproject"));
}
