use nose::discovery::discover_sessions;
use nose::adapter::claude::ClaudeAdapter;
use nose::adapter::Adapter;
use std::fs;
use tempfile::tempdir;

#[test]
fn test_discover_finds_jsonl_files() {
    let dir = tempdir().unwrap();
    let claude_dir = dir.path().join(".claude").join("projects").join("test-project");
    fs::create_dir_all(&claude_dir).unwrap();

    let line = r#"{"type":"user","sessionId":"sess_01","cwd":"/my/project","timestamp":"2026-03-22T10:00:00Z"}"#;
    fs::write(claude_dir.join("session1.jsonl"), format!("{}\n", line)).unwrap();
    fs::write(claude_dir.join("session2.jsonl"), format!("{}\n", line)).unwrap();
    fs::write(claude_dir.join("not-a-session.txt"), "nope").unwrap();

    let adapter = ClaudeAdapter;
    let sessions = discover_sessions(&[claude_dir.clone()], &adapter);
    assert_eq!(sessions.len(), 2);

    // Verify session metadata is extracted from file content
    assert_eq!(sessions[0].session_id, "sess_01");
    assert_eq!(sessions[0].workspace, "/my/project");
}

#[test]
fn test_discover_skips_missing_paths() {
    let adapter = ClaudeAdapter;
    let sessions = discover_sessions(&[std::path::PathBuf::from("/nonexistent/path")], &adapter);
    assert_eq!(sessions.len(), 0);
}
