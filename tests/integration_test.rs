use std::process::Command;
use std::fs;
use tempfile::tempdir;

#[test]
fn test_nose_parse_with_claude_fixture() {
    // Use a directory under the real home to avoid macOS /var → /private/var symlink issues
    let dir = tempdir().unwrap();
    let home = dir.path().canonicalize().unwrap();

    // Create a fake project directory
    let project_dir = home.join("workspace").join("myproject");
    fs::create_dir_all(&project_dir).unwrap();

    // Claude Code encodes cwd: /path/to/project → -path-to-project
    let encoded = project_dir.to_string_lossy().replace('/', "-");
    let claude_dir = home.join(".claude").join("projects").join(&encoded);
    fs::create_dir_all(&claude_dir).unwrap();

    let fixture = include_str!("fixtures/claude/tool_use_session.jsonl");
    fs::write(claude_dir.join("sess_02.jsonl"), fixture).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nose"))
        .arg("parse")
        .env("HOME", &home)
        .current_dir(&project_dir)
        .output()
        .expect("Failed to run nose");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = stdout.trim().split('\n').filter(|l| !l.is_empty()).collect();

    assert!(!lines.is_empty(), "Expected JSONL output, got nothing. stderr: {}", String::from_utf8_lossy(&output.stderr));

    for line in &lines {
        let parsed: serde_json::Value = serde_json::from_str(line)
            .unwrap_or_else(|e| panic!("Invalid JSON line: {}\nError: {}", line, e));
        assert!(parsed["event_type"].is_string(), "Missing event_type in: {}", line);
        assert!(parsed["agent_type"].is_string(), "Missing agent_type in: {}", line);
        assert!(parsed["session_id"].is_string(), "Missing session_id in: {}", line);
    }

    let event_types: Vec<String> = lines.iter()
        .map(|l| {
            let v: serde_json::Value = serde_json::from_str(l).unwrap();
            v["event_type"].as_str().unwrap().to_string()
        })
        .collect();

    assert!(event_types.contains(&"ModelRequest".to_string()));
    assert!(event_types.contains(&"ToolCall".to_string()));
    assert!(event_types.contains(&"FileRead".to_string()));
    assert!(event_types.contains(&"CommandExec".to_string()));
}

#[test]
fn test_nose_parse_empty_home() {
    let dir = tempdir().unwrap();
    let home = dir.path().canonicalize().unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nose"))
        .arg("parse")
        .env("HOME", &home)
        .current_dir(&home)
        .output()
        .expect("Failed to run nose");

    assert!(output.status.success());
    assert!(output.stdout.is_empty() || String::from_utf8_lossy(&output.stdout).trim().is_empty());
}
