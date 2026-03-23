use std::collections::HashMap;
use std::io::Write;
use std::path::PathBuf;
use tempfile::NamedTempFile;

use nose::watch::parse_file_from_offset;

/// Returns a simple JSONL line that Claude adapter can parse.
/// We use the hook adapter format since it's simpler to construct.
fn make_hook_event_line() -> String {
    serde_json::json!({
        "event_id": "00000000-0000-0000-0000-000000000001",
        "session_id": "test-session",
        "timestamp": "2024-01-01T00:00:00Z",
        "agent_type": "claude",
        "workspace": "/tmp/test",
        "confidence": "native",
        "event_type": "ToolCall",
        "tool_name": "Read",
        "input": {}
    })
    .to_string()
}

#[test]
fn test_parse_file_from_offset_zero() {
    let mut file = NamedTempFile::new().unwrap();
    let line = make_hook_event_line();
    writeln!(file, "{}", line).unwrap();
    file.flush().unwrap();

    let adapter = nose::adapter::hook::HookAdapter;
    let path = file.path();

    let (events, new_pos) =
        parse_file_from_offset(path, 0, &adapter, "test-session", "/tmp/test").unwrap();

    assert!(!events.is_empty(), "Expected events from offset 0");
    assert!(new_pos > 0, "Position should advance");
}

#[test]
fn test_parse_file_from_offset_at_eof() {
    let mut file = NamedTempFile::new().unwrap();
    let line = make_hook_event_line();
    writeln!(file, "{}", line).unwrap();
    file.flush().unwrap();

    let file_len = std::fs::metadata(file.path()).unwrap().len();
    let adapter = nose::adapter::hook::HookAdapter;
    let path = file.path();

    let (events, new_pos) =
        parse_file_from_offset(path, file_len, &adapter, "test-session", "/tmp/test").unwrap();

    assert!(events.is_empty(), "No new events when offset is at EOF");
    assert_eq!(new_pos, file_len, "Position should remain at EOF");
}

#[test]
fn test_parse_file_incremental_append() {
    let mut file = NamedTempFile::new().unwrap();
    let line = make_hook_event_line();
    writeln!(file, "{}", line).unwrap();
    file.flush().unwrap();

    let first_len = std::fs::metadata(file.path()).unwrap().len();
    // Copy the path to avoid borrow conflicts with `file`
    let path = PathBuf::from(file.path());

    // Parse the first batch
    let adapter = nose::adapter::hook::HookAdapter;
    let (events1, pos1) =
        parse_file_from_offset(&path, 0, &adapter, "test-session", "/tmp/test").unwrap();
    assert!(!events1.is_empty());
    assert_eq!(pos1, first_len);

    // Append a second event
    let line2 = make_hook_event_line();
    writeln!(file, "{}", line2).unwrap();
    file.flush().unwrap();

    // Parse only the new content
    let (events2, pos2) =
        parse_file_from_offset(&path, pos1, &adapter, "test-session", "/tmp/test").unwrap();
    assert!(!events2.is_empty(), "Should parse newly appended event");
    assert!(pos2 > pos1, "Position should advance past the appended content");
}

#[test]
fn test_position_tracking_with_hashmap() {
    let mut positions: HashMap<PathBuf, u64> = HashMap::new();

    let mut file = NamedTempFile::new().unwrap();
    let path = PathBuf::from(file.path());

    // File starts empty
    nose::watch::record_file_position(&mut positions, &path);
    assert_eq!(*positions.get(&path).unwrap(), 0u64);

    // Write some content
    let line = make_hook_event_line();
    writeln!(file, "{}", line).unwrap();
    file.flush().unwrap();

    // Record new position
    nose::watch::record_file_position(&mut positions, &path);
    let recorded = *positions.get(&path).unwrap();
    let actual_len = std::fs::metadata(&path).unwrap().len();
    assert_eq!(recorded, actual_len);
}
