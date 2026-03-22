# Nose Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a Rust CLI tool that auto-discovers AI coding agent sessions and emits unified JSONL events to stdout.

**Architecture:** `nose parse` discovers agent session files at known paths, selects the right adapter per agent, parses raw logs into unified Event structs, and serializes them as JSONL to stdout. Each adapter implements the same trait. Error handling is graceful — skip and warn.

**Tech Stack:** Rust, clap, serde/serde_json, uuid, chrono, walkdir

**Spec:** `docs/superpowers/specs/2026-03-22-nose-design.md`

---

## File Structure

```
nose/
├── Cargo.toml
├── src/
│   ├── main.rs                # CLI entry: `nose parse`
│   ├── event.rs               # Event enum + common fields + serde
│   ├── error.rs               # AdapterError, NoseError types
│   ├── discovery.rs           # Scan known paths, return found session files
│   ├── adapter/
│   │   ├── mod.rs             # Adapter trait + registry
│   │   └── claude.rs          # Claude Code adapter
│   └── output.rs              # JSONL serializer to stdout
├── tests/
│   ├── fixtures/
│   │   └── claude/
│   │       ├── simple_session.jsonl    # Minimal Claude transcript
│   │       └── tool_use_session.jsonl  # Session with tool calls
│   ├── event_test.rs
│   ├── claude_adapter_test.rs
│   ├── discovery_test.rs
│   └── integration_test.rs
```

Note: Codex, Gemini, Cursor, and Copilot adapters follow the same pattern as Claude. They are not included in this plan — each gets its own plan after Claude adapter is working end-to-end.

---

### Task 1: Project Scaffold

**Files:**
- Create: `Cargo.toml`
- Create: `src/main.rs`

- [ ] **Step 1: Initialize Cargo project**

```bash
cd /Users/michalgrabowski/workspace/nose
cargo init --name nose
```

- [ ] **Step 2: Add dependencies to Cargo.toml**

Replace the generated `Cargo.toml` with:

```toml
[package]
name = "nose"
version = "0.1.0"
edition = "2021"

[dependencies]
clap = { version = "4", features = ["derive"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
uuid = { version = "1", features = ["v4", "serde"] }
chrono = { version = "0.4", features = ["serde"] }
walkdir = "2"
```

- [ ] **Step 3: Write minimal main.rs with clap**

```rust
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "nose", about = "Agent Activity Observability")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Parse agent sessions and emit unified JSONL events
    Parse,
}

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Commands::Parse => {
            eprintln!("nose: parse not yet implemented");
        }
    }
}
```

- [ ] **Step 4: Verify it compiles and runs**

Run: `cargo build`
Expected: compiles without errors

Run: `cargo run -- parse`
Expected: prints "nose: parse not yet implemented" to stderr

- [ ] **Step 5: Create .gitignore**

Create `.gitignore`:

```
/target
```

- [ ] **Step 6: Commit**

```bash
git init
git add .gitignore Cargo.toml Cargo.lock src/main.rs README.md idea.md docs/
git commit -m "feat: scaffold nose project with clap CLI"
```

---

### Task 2: Event Model

**Files:**
- Create: `src/event.rs`
- Create: `src/error.rs`
- Create: `tests/event_test.rs`

- [ ] **Step 1: Write the failing test for Event serialization**

Create `tests/event_test.rs`:

```rust
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
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test event_test`
Expected: FAIL — module `nose::event` does not exist

- [ ] **Step 3: Create error.rs**

Create `src/error.rs`:

```rust
use std::fmt;

#[derive(Debug)]
pub enum AdapterError {
    /// File could not be read
    Io(std::io::Error),
    /// JSON parsing failed
    Parse(serde_json::Error),
    /// Unknown or unsupported format
    UnsupportedFormat(String),
}

impl fmt::Display for AdapterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AdapterError::Io(e) => write!(f, "IO error: {}", e),
            AdapterError::Parse(e) => write!(f, "Parse error: {}", e),
            AdapterError::UnsupportedFormat(msg) => write!(f, "Unsupported format: {}", msg),
        }
    }
}

impl std::error::Error for AdapterError {}

impl From<std::io::Error> for AdapterError {
    fn from(e: std::io::Error) -> Self {
        AdapterError::Io(e)
    }
}

impl From<serde_json::Error> for AdapterError {
    fn from(e: serde_json::Error) -> Self {
        AdapterError::Parse(e)
    }
}
```

- [ ] **Step 4: Create event.rs with all 16 event types**

Create `src/event.rs`:

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AgentType {
    Claude,
    Codex,
    Gemini,
    Cursor,
    Copilot,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Confidence {
    Native,
    Inferred,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub event_id: Uuid,
    pub session_id: String,
    pub timestamp: DateTime<Utc>,
    pub agent_type: AgentType,
    pub workspace: String,
    pub confidence: Confidence,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_payload: Option<serde_json::Value>,
    #[serde(flatten)]
    pub data: EventData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event_type")]
pub enum EventData {
    SessionStart {
        environment: Option<String>,
        args: Vec<String>,
        config: serde_json::Value,
    },
    SessionEnd {
        exit_code: i32,
        duration_ms: u64,
    },
    ModelRequest {
        model: String,
        provider: Option<String>,
        input_tokens: Option<u64>,
    },
    ModelResponse {
        output_tokens: Option<u64>,
        stop_reason: Option<String>,
        duration_ms: Option<u64>,
    },
    ToolCall {
        tool_name: String,
        input: serde_json::Value,
    },
    ToolResult {
        tool_name: String,
        output_summary: Option<String>,
        error: Option<String>,
        duration_ms: Option<u64>,
    },
    FileRead {
        path: String,
    },
    FileWrite {
        path: String,
        bytes_written: Option<u64>,
    },
    FileDelete {
        path: String,
    },
    CommandExec {
        command: String,
        cwd: Option<String>,
        exit_code: Option<i32>,
        duration_ms: Option<u64>,
    },
    SubagentStart {
        subagent_name: String,
        task: Option<String>,
    },
    SubagentEnd {
        subagent_name: String,
        exit_code: Option<i32>,
        duration_ms: Option<u64>,
    },
    NetworkCall {
        method: String,
        url: String,
        status_code: Option<u16>,
        duration_ms: Option<u64>,
    },
    McpCall {
        server_name: String,
        method: String,
        params: Option<serde_json::Value>,
    },
    Artifact {
        artifact_type: String,
        path: Option<String>,
        description: Option<String>,
    },
    Error {
        error_type: String,
        message: String,
        context: Option<String>,
    },
}
```

- [ ] **Step 5: Expose modules from lib.rs**

Create `src/lib.rs`:

```rust
pub mod event;
pub mod error;
```

- [ ] **Step 6: Run tests to verify they pass**

Run: `cargo test --test event_test`
Expected: all 3 tests PASS

- [ ] **Step 7: Commit**

```bash
git add src/event.rs src/error.rs src/lib.rs tests/event_test.rs
git commit -m "feat: add unified event model with 16 event types and serde serialization"
```

---

### Task 3: JSONL Output

**Files:**
- Create: `src/output.rs`
- Modify: `src/lib.rs`

- [ ] **Step 1: Write failing test**

Add to `tests/event_test.rs`:

```rust
use nose::output::write_events_jsonl;

#[test]
fn test_write_events_jsonl() {
    let events = vec![
        Event {
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
                args: vec![],
                config: serde_json::json!({}),
            },
        },
        Event {
            event_id: Uuid::nil(),
            session_id: "sess_01".to_string(),
            timestamp: chrono::DateTime::parse_from_rfc3339("2026-03-22T10:00:01Z")
                .unwrap()
                .with_timezone(&Utc),
            agent_type: AgentType::Claude,
            workspace: "/project".to_string(),
            confidence: Confidence::Native,
            raw_payload: None,
            data: EventData::SessionEnd {
                exit_code: 0,
                duration_ms: 1000,
            },
        },
    ];

    let mut buf = Vec::new();
    write_events_jsonl(&events, &mut buf).unwrap();
    let output = String::from_utf8(buf).unwrap();
    let lines: Vec<&str> = output.trim().split('\n').collect();
    assert_eq!(lines.len(), 2);
    assert!(lines[0].contains("SessionStart"));
    assert!(lines[1].contains("SessionEnd"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test event_test test_write_events_jsonl`
Expected: FAIL — `nose::output` does not exist

- [ ] **Step 3: Implement output.rs**

Create `src/output.rs`:

```rust
use std::io::Write;
use crate::event::Event;

pub fn write_events_jsonl(events: &[Event], writer: &mut impl Write) -> std::io::Result<()> {
    for event in events {
        serde_json::to_writer(&mut *writer, event)?;
        writeln!(writer)?;
    }
    Ok(())
}
```

- [ ] **Step 4: Add to lib.rs**

Add `pub mod output;` to `src/lib.rs`.

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test --test event_test test_write_events_jsonl`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add src/output.rs src/lib.rs tests/event_test.rs
git commit -m "feat: add JSONL output writer"
```

---

### Task 4: Adapter Trait + Registry

**Files:**
- Create: `src/adapter/mod.rs`
- Modify: `src/lib.rs`

- [ ] **Step 1: Create adapter trait and registry**

Create `src/adapter/mod.rs`:

```rust
pub mod claude;

use std::io::Read;
use std::path::{Path, PathBuf};
use crate::error::AdapterError;
use crate::event::Event;

pub trait Adapter {
    fn name(&self) -> &'static str;
    fn discovery_paths(&self) -> Vec<PathBuf>;
    fn detect(&self, path: &Path) -> bool;
    fn parse(&self, reader: &mut dyn Read, session_id: &str, workspace: &str) -> Result<Vec<Event>, AdapterError>;
}

pub fn all_adapters() -> Vec<Box<dyn Adapter>> {
    vec![
        Box::new(claude::ClaudeAdapter),
    ]
}
```

- [ ] **Step 2: Add to lib.rs**

Add `pub mod adapter;` to `src/lib.rs`.

- [ ] **Step 3: Verify it compiles**

Run: `cargo check`
Expected: compiles (claude module will be created in next task)

Note: this will fail until Task 5 creates `claude.rs`. If building incrementally, create a placeholder `src/adapter/claude.rs`:

```rust
use std::io::Read;
use std::path::{Path, PathBuf};
use crate::adapter::Adapter;
use crate::error::AdapterError;
use crate::event::Event;

pub struct ClaudeAdapter;

impl Adapter for ClaudeAdapter {
    fn name(&self) -> &'static str { "claude" }
    fn discovery_paths(&self) -> Vec<PathBuf> { vec![] }
    fn detect(&self, _path: &Path) -> bool { false }
    fn parse(&self, _reader: &mut dyn Read, _session_id: &str, _workspace: &str) -> Result<Vec<Event>, AdapterError> {
        Ok(vec![])
    }
}
```

- [ ] **Step 4: Verify it compiles**

Run: `cargo check`
Expected: compiles without errors

- [ ] **Step 5: Commit**

```bash
git add src/adapter/ src/lib.rs
git commit -m "feat: add adapter trait and registry"
```

---

### Task 5: Claude Code Adapter

**Files:**
- Modify: `src/adapter/claude.rs`
- Create: `tests/fixtures/claude/simple_session.jsonl`
- Create: `tests/fixtures/claude/tool_use_session.jsonl`
- Create: `tests/claude_adapter_test.rs`

The Claude Code transcript format (from `~/.claude/projects/*/`):
- JSONL files, one JSON object per line
- Each line has a `type` field: `user`, `assistant`, `progress`, `tool_result`, `file-history-snapshot`
- `assistant` messages have `message.content[]` array with blocks of type `text`, `thinking`, `tool_use`
- `tool_use` blocks have `name` and `input` fields
- `tool_result` / `progress` messages have `message.content[]` with `tool_result` blocks
- Common fields per line: `sessionId`, `cwd`, `timestamp`, `uuid`
- Assistant messages include `message.model` and `message.usage` (input_tokens, output_tokens)

- [ ] **Step 1: Create test fixture — simple session**

Create `tests/fixtures/claude/simple_session.jsonl`:

```jsonl
{"type":"user","sessionId":"sess_01","cwd":"/project","timestamp":"2026-03-22T10:00:00Z","uuid":"u1","parentUuid":null,"isSidechain":false,"version":"2.1.56","message":{"role":"user","content":"hello"}}
{"type":"assistant","sessionId":"sess_01","cwd":"/project","timestamp":"2026-03-22T10:00:01Z","uuid":"u2","parentUuid":"u1","isSidechain":false,"version":"2.1.56","message":{"model":"claude-opus-4-6","id":"msg_01","type":"message","role":"assistant","content":[{"type":"text","text":"Hello!"}],"stop_reason":"end_turn","usage":{"input_tokens":100,"output_tokens":50}}}
```

- [ ] **Step 2: Create test fixture — tool use session**

Create `tests/fixtures/claude/tool_use_session.jsonl`:

```jsonl
{"type":"user","sessionId":"sess_02","cwd":"/project","timestamp":"2026-03-22T10:00:00Z","uuid":"u1","parentUuid":null,"isSidechain":false,"version":"2.1.56","message":{"role":"user","content":"read main.rs"}}
{"type":"assistant","sessionId":"sess_02","cwd":"/project","timestamp":"2026-03-22T10:00:01Z","uuid":"u2","parentUuid":"u1","isSidechain":false,"version":"2.1.56","message":{"model":"claude-opus-4-6","id":"msg_01","type":"message","role":"assistant","content":[{"type":"tool_use","id":"tool_01","name":"Read","input":{"file_path":"/src/main.rs"}},{"type":"tool_use","id":"tool_02","name":"Bash","input":{"command":"cargo test","description":"run tests"}}],"stop_reason":"tool_use","usage":{"input_tokens":200,"output_tokens":100}}}
{"type":"user","sessionId":"sess_02","cwd":"/project","timestamp":"2026-03-22T10:00:02Z","uuid":"u3","parentUuid":"u2","isSidechain":false,"version":"2.1.56","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"tool_01","content":"fn main() {}","is_error":false}]}}
```

Note: In real Claude Code transcripts, tool_result lines have `"type":"user"` at the top level. The `"type":"tool_result"` only appears inside `message.content[]`. The adapter also encounters `progress`, `file-history-snapshot`, `system`, and other line types which are intentionally skipped in v1.


- [ ] **Step 3: Write failing tests**

Create `tests/claude_adapter_test.rs`:

```rust
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

    // Should produce: ModelRequest + ModelResponse (from assistant message)
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

    // Should produce: ModelRequest, ToolCall(Read), ToolCall(Bash), FileRead(inferred), CommandExec(inferred), ModelResponse
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
```

- [ ] **Step 4: Run tests to verify they fail**

Run: `cargo test --test claude_adapter_test`
Expected: FAIL — ClaudeAdapter::parse returns empty vec

- [ ] **Step 5: Implement Claude adapter**

Replace `src/adapter/claude.rs`:

```rust
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use chrono::{DateTime, Utc};
use uuid::Uuid;
use crate::adapter::Adapter;
use crate::error::AdapterError;
use crate::event::{AgentType, Confidence, Event, EventData};

pub struct ClaudeAdapter;

impl ClaudeAdapter {
    fn make_event(&self, session_id: &str, workspace: &str, timestamp: DateTime<Utc>, confidence: Confidence, data: EventData, raw: Option<serde_json::Value>) -> Event {
        Event {
            event_id: Uuid::new_v4(),
            session_id: session_id.to_string(),
            timestamp,
            agent_type: AgentType::Claude,
            workspace: workspace.to_string(),
            confidence,
            raw_payload: raw,
            data,
        }
    }

    fn parse_assistant_message(&self, line: &serde_json::Value, session_id: &str, workspace: &str) -> Vec<Event> {
        let mut events = Vec::new();
        let msg = &line["message"];
        let timestamp = self.parse_timestamp(line);

        // ModelRequest
        let model = msg["model"].as_str().unwrap_or("unknown").to_string();
        let input_tokens = msg["usage"]["input_tokens"].as_u64();
        events.push(self.make_event(
            session_id, workspace, timestamp, Confidence::Native,
            EventData::ModelRequest {
                model,
                provider: Some("anthropic".to_string()),
                input_tokens,
            },
            None,
        ));

        // Process content blocks
        if let Some(content) = msg["content"].as_array() {
            for block in content {
                match block["type"].as_str() {
                    Some("tool_use") => {
                        let tool_name = block["name"].as_str().unwrap_or("unknown").to_string();
                        let input = block["input"].clone();

                        events.push(self.make_event(
                            session_id, workspace, timestamp, Confidence::Native,
                            EventData::ToolCall {
                                tool_name: tool_name.clone(),
                                input: input.clone(),
                            },
                            Some(block.clone()),
                        ));

                        // Infer higher-level events from tool name
                        match tool_name.as_str() {
                            "Read" => {
                                if let Some(path) = input["file_path"].as_str() {
                                    events.push(self.make_event(
                                        session_id, workspace, timestamp, Confidence::Inferred,
                                        EventData::FileRead { path: path.to_string() },
                                        None,
                                    ));
                                }
                            }
                            "Write" => {
                                if let Some(path) = input["file_path"].as_str() {
                                    let bytes = input["content"].as_str().map(|s| s.len() as u64);
                                    events.push(self.make_event(
                                        session_id, workspace, timestamp, Confidence::Inferred,
                                        EventData::FileWrite { path: path.to_string(), bytes_written: bytes },
                                        None,
                                    ));
                                }
                            }
                            "Edit" => {
                                if let Some(path) = input["file_path"].as_str() {
                                    events.push(self.make_event(
                                        session_id, workspace, timestamp, Confidence::Inferred,
                                        EventData::FileWrite { path: path.to_string(), bytes_written: None },
                                        None,
                                    ));
                                }
                            }
                            "Bash" => {
                                let command = input["command"].as_str().unwrap_or("").to_string();
                                let cwd = input["cwd"].as_str().map(|s| s.to_string());
                                events.push(self.make_event(
                                    session_id, workspace, timestamp, Confidence::Inferred,
                                    EventData::CommandExec {
                                        command,
                                        cwd,
                                        exit_code: None,
                                        duration_ms: None,
                                    },
                                    None,
                                ));
                            }
                            name if name.starts_with("mcp__") => {
                                let parts: Vec<&str> = name.splitn(3, "__").collect();
                                let server = parts.get(1).unwrap_or(&"unknown").to_string();
                                let method = parts.get(2).unwrap_or(&"unknown").to_string();
                                events.push(self.make_event(
                                    session_id, workspace, timestamp, Confidence::Inferred,
                                    EventData::McpCall {
                                        server_name: server,
                                        method,
                                        params: Some(input),
                                    },
                                    None,
                                ));
                            }
                            "WebFetch" | "WebSearch" => {
                                let url = input["url"].as_str().unwrap_or("").to_string();
                                events.push(self.make_event(
                                    session_id, workspace, timestamp, Confidence::Inferred,
                                    EventData::NetworkCall {
                                        method: "GET".to_string(),
                                        url,
                                        status_code: None,
                                        duration_ms: None,
                                    },
                                    None,
                                ));
                            }
                            "Agent" => {
                                let subagent_name = input["subagent_type"].as_str().unwrap_or("agent").to_string();
                                let task = input["description"].as_str().map(|s| s.to_string());
                                events.push(self.make_event(
                                    session_id, workspace, timestamp, Confidence::Inferred,
                                    EventData::SubagentStart {
                                        subagent_name,
                                        task,
                                    },
                                    None,
                                ));
                            }
                            _ => {}
                        }
                    }
                    _ => {}
                }
            }
        }

        // ModelResponse
        let output_tokens = msg["usage"]["output_tokens"].as_u64();
        let stop_reason = msg["stop_reason"].as_str().map(|s| s.to_string());
        events.push(self.make_event(
            session_id, workspace, timestamp, Confidence::Native,
            EventData::ModelResponse {
                output_tokens,
                stop_reason,
                duration_ms: None,
            },
            None,
        ));

        events
    }

    fn parse_timestamp(&self, line: &serde_json::Value) -> DateTime<Utc> {
        line["timestamp"]
            .as_str()
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(Utc::now)
    }
}

impl Adapter for ClaudeAdapter {
    fn name(&self) -> &'static str {
        "claude"
    }

    fn discovery_paths(&self) -> Vec<PathBuf> {
        if let Some(home) = dirs_home() {
            vec![home.join(".claude").join("projects")]
        } else {
            vec![]
        }
    }

    fn detect(&self, path: &Path) -> bool {
        path.to_string_lossy().contains(".claude/projects/")
            && path.extension().map_or(false, |ext| ext == "jsonl")
    }

    fn parse(&self, reader: &mut dyn Read, session_id: &str, workspace: &str) -> Result<Vec<Event>, AdapterError> {
        let buf_reader = BufReader::new(reader);
        let mut events = Vec::new();

        for line_result in buf_reader.lines() {
            let line_str = line_result?;
            if line_str.trim().is_empty() {
                continue;
            }

            let line: serde_json::Value = match serde_json::from_str(&line_str) {
                Ok(v) => v,
                Err(_) => continue, // skip unparseable lines
            };

            match line["type"].as_str() {
                Some("assistant") => {
                    events.extend(self.parse_assistant_message(&line, session_id, workspace));
                }
                // user, progress, file-history-snapshot — skip for now
                _ => {}
            }
        }

        Ok(events)
    }
}

fn dirs_home() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}
```

- [ ] **Step 6: Run tests to verify they pass**

Run: `cargo test --test claude_adapter_test`
Expected: all 4 tests PASS

- [ ] **Step 7: Commit**

```bash
git add src/adapter/claude.rs tests/fixtures/claude/ tests/claude_adapter_test.rs
git commit -m "feat: implement Claude Code adapter with tool inference"
```

---

### Task 6: Discovery

**Files:**
- Create: `src/discovery.rs`
- Create: `tests/discovery_test.rs`
- Modify: `src/lib.rs`

- [ ] **Step 1: Write failing test**

Create `tests/discovery_test.rs`:

```rust
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
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test discovery_test`
Expected: FAIL — `nose::discovery` does not exist

- [ ] **Step 3: Add tempfile dev-dependency**

Add to `Cargo.toml` under a new section:

```toml
[dev-dependencies]
tempfile = "3"
```

- [ ] **Step 4: Implement discovery.rs**

Create `src/discovery.rs`:

```rust
use std::path::{Path, PathBuf};
use walkdir::WalkDir;
use crate::adapter::Adapter;

pub struct SessionFile {
    pub path: PathBuf,
    pub session_id: String,
    pub workspace: String,
}

/// Extracts session_id and workspace from the first line of a Claude transcript.
/// Falls back to filename-based session_id and "unknown" workspace if parsing fails.
fn extract_session_metadata(path: &Path) -> (String, String) {
    let fallback_id = path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    if let Ok(file) = std::fs::File::open(path) {
        let reader = std::io::BufReader::new(file);
        use std::io::BufRead;
        if let Some(Ok(first_line)) = reader.lines().next() {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&first_line) {
                let session_id = v["sessionId"]
                    .as_str()
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| fallback_id.clone());
                let workspace = v["cwd"]
                    .as_str()
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| "unknown".to_string());
                return (session_id, workspace);
            }
        }
    }

    (fallback_id, "unknown".to_string())
}

pub fn discover_sessions(search_paths: &[PathBuf], adapter: &dyn Adapter) -> Vec<SessionFile> {
    let mut sessions = Vec::new();

    for base_path in search_paths {
        if !base_path.exists() {
            continue;
        }

        for entry in WalkDir::new(base_path)
            .follow_links(false)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if path.is_file() && adapter.detect(path) {
                let (session_id, workspace) = extract_session_metadata(path);

                sessions.push(SessionFile {
                    path: path.to_path_buf(),
                    session_id,
                    workspace,
                });
            }
        }
    }

    sessions
}
```

- [ ] **Step 5: Add to lib.rs**

Add `pub mod discovery;` to `src/lib.rs`.

- [ ] **Step 6: Run tests to verify they pass**

Run: `cargo test --test discovery_test`
Expected: all 2 tests PASS

- [ ] **Step 7: Commit**

```bash
git add src/discovery.rs src/lib.rs tests/discovery_test.rs Cargo.toml
git commit -m "feat: add session file discovery with walkdir"
```

---

### Task 7: Wire It All Together in main.rs

**Files:**
- Modify: `src/main.rs`
- Create: `tests/integration_test.rs`

- [ ] **Step 1: Write integration test**

Create `tests/integration_test.rs`:

```rust
use std::process::Command;
use std::fs;
use tempfile::tempdir;

#[test]
fn test_nose_parse_with_claude_fixture() {
    // Create a fake .claude directory structure
    let dir = tempdir().unwrap();
    let home = dir.path();
    let claude_dir = home.join(".claude").join("projects").join("test-project");
    fs::create_dir_all(&claude_dir).unwrap();

    let fixture = include_str!("fixtures/claude/tool_use_session.jsonl");
    fs::write(claude_dir.join("sess_02.jsonl"), fixture).unwrap();

    // Run nose parse with HOME overridden
    let output = Command::new(env!("CARGO_BIN_EXE_nose"))
        .arg("parse")
        .env("HOME", home)
        .output()
        .expect("Failed to run nose");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = stdout.trim().split('\n').filter(|l| !l.is_empty()).collect();

    assert!(!lines.is_empty(), "Expected JSONL output, got nothing. stderr: {}", String::from_utf8_lossy(&output.stderr));

    // Verify each line is valid JSON with event_type
    for line in &lines {
        let parsed: serde_json::Value = serde_json::from_str(line)
            .unwrap_or_else(|e| panic!("Invalid JSON line: {}\nError: {}", line, e));
        assert!(parsed["event_type"].is_string(), "Missing event_type in: {}", line);
        assert!(parsed["agent_type"].is_string(), "Missing agent_type in: {}", line);
        assert!(parsed["session_id"].is_string(), "Missing session_id in: {}", line);
    }

    // Verify we got the expected event types
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

    let output = Command::new(env!("CARGO_BIN_EXE_nose"))
        .arg("parse")
        .env("HOME", dir.path())
        .output()
        .expect("Failed to run nose");

    // Should succeed with no output (no agents found)
    assert!(output.status.success());
    assert!(output.stdout.is_empty() || String::from_utf8_lossy(&output.stdout).trim().is_empty());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test integration_test`
Expected: FAIL — nose parse prints to stderr, not stdout

- [ ] **Step 3: Wire up main.rs**

Replace `src/main.rs`:

```rust
use clap::{Parser, Subcommand};
use std::fs::File;
use std::io::stdout;

use nose::adapter::all_adapters;
use nose::discovery::discover_sessions;
use nose::output::write_events_jsonl;

#[derive(Parser)]
#[command(name = "nose", about = "Agent Activity Observability")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Parse agent sessions and emit unified JSONL events
    Parse,
}

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Commands::Parse => run_parse(),
    }
}

fn run_parse() {
    let adapters = all_adapters();
    let mut out = stdout().lock();

    for adapter in &adapters {
        let search_paths = adapter.discovery_paths();
        let sessions = discover_sessions(&search_paths, adapter.as_ref());

        for session in sessions {
            let file = match File::open(&session.path) {
                Ok(f) => f,
                Err(e) => {
                    eprintln!("nose: warning: could not open {}: {}", session.path.display(), e);
                    continue;
                }
            };

            let mut reader = std::io::BufReader::new(file);
            match adapter.parse(&mut reader, &session.session_id, &session.workspace) {
                Ok(events) => {
                    if let Err(e) = write_events_jsonl(&events, &mut out) {
                        eprintln!("nose: warning: write error: {}", e);
                    }
                }
                Err(e) => {
                    eprintln!("nose: warning: failed to parse {}: {}", session.path.display(), e);
                }
            }
        }
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --test integration_test`
Expected: all 2 tests PASS

- [ ] **Step 5: Run all tests**

Run: `cargo test`
Expected: all tests PASS

- [ ] **Step 6: Test manually against real Claude data**

Run: `cargo run -- parse | head -5`
Expected: JSONL lines from your actual Claude Code sessions

- [ ] **Step 7: Commit**

```bash
git add src/main.rs tests/integration_test.rs
git commit -m "feat: wire up nose parse end-to-end pipeline"
```

---

### Task 8: Verify and Polish

- [ ] **Step 1: Run clippy**

Run: `cargo clippy -- -D warnings`
Expected: no warnings

- [ ] **Step 2: Fix any clippy issues**

Fix whatever clippy reports.

- [ ] **Step 3: Run all tests one final time**

Run: `cargo test`
Expected: all tests PASS

- [ ] **Step 4: Test against real data**

Run: `cargo run -- parse 2>/dev/null | wc -l`
Expected: non-zero number of events from your Claude sessions

Run: `cargo run -- parse 2>/dev/null | head -1 | python3 -m json.tool`
Expected: pretty-printed JSON with all expected fields

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "chore: clippy fixes and polish"
```
