# Nose — Agent Activity Observability — Design Spec

## Problem

Observability of AI coding agents is fragmented. Each agent (Claude Code, Codex CLI, Gemini CLI, Cursor, Copilot) produces different formats, logs, and events. There is no unified way to answer: what did the agent do, which files it touched, what commands it ran, what side effects it produced.

## Solution

Nose is a CLI tool (Rust) that auto-discovers agent sessions on the local machine, parses their logs/transcripts, and emits a unified JSONL event stream.

```bash
nose parse
```

No flags, no config. Nose detects installed agents, finds sessions, and outputs unified events to stdout.

**Nose reads files only.** It does not install hooks or run agents. It parses the log/transcript files that agents leave on disk. The "hooks" referenced in the support matrix describe what data the agent *writes* — Nose reads those files post-hoc.

**Platform:** macOS and Linux only.

## Architecture

```
Known agent paths ──→ Discovery ──→ Session files
                                        │
                              Format detection
                                        │
                              Adapter selection
                                        │
Claude adapter ───┐
Codex adapter ────┤
Gemini adapter ───┤──→ Unified Events ──→ JSONL (stdout)
Cursor adapter ───┤
Copilot adapter ──┘
```

### Pipeline

1. **Discovery** — scan known paths for each agent type
2. **Detection** — identify agent type from file structure/content
3. **Parsing** — agent-specific adapter reads raw data
4. **Normalization** — adapter emits unified events
5. **Output** — JSONL to stdout

### Adapter trait

Each agent implements a single trait:

```rust
trait Adapter {
    /// Known paths where this agent stores session data
    fn discovery_paths() -> Vec<PathBuf>;
    /// Check if a file/directory belongs to this agent
    fn detect(path: &Path) -> bool;
    /// Parse raw session data into unified events
    fn parse(reader: impl Read) -> Result<Vec<Event>, AdapterError>;
}
```

Adapters are stateless. `object` fields in the event model map to `serde_json::Value`.

## Agent Discovery Paths

| Agent | Known paths | Raw format |
|---|---|---|
| Claude Code | `~/.claude/projects/*/` | JSONL transcripts — one JSON object per line, each with `type` field (e.g. `tool_use`, `tool_result`, `text`) |
| Codex CLI | `~/.codex/log/` | JSON log files — structured events with `type` field (e.g. `thread.started`, `turn.completed`, `item`) |
| Gemini CLI | `~/.gemini/` | Stream-JSON — JSONL with event types like `tool_use`, `tool_result`, `result`, `error` |
| Cursor | `~/Library/Application Support/Cursor/` (macOS), `~/.config/cursor/` (Linux) | Hook output JSON files |
| Copilot | `~/.github-copilot/` | Hook output JSON files |

Note: Exact paths and formats will be verified during implementation against actual agent installations.

## Unified Event Model

### Common Fields

Every event contains:

| Field | Type | Description |
|---|---|---|
| `event_id` | UUID | Unique event identifier |
| `session_id` | string | Agent session identifier |
| `timestamp` | ISO 8601 | When the event occurred |
| `agent_type` | enum | `claude` \| `codex` \| `gemini` \| `cursor` \| `copilot` |
| `workspace` | string | Working directory path |
| `confidence` | enum | `native` \| `inferred` — whether event was directly reported or inferred from parsing |
| `raw_payload` | object? | Optional original agent-specific payload, preserved for lossless round-tripping |

### JSONL Output Format

Events are serialized as one JSON object per line, using a `"event_type"` tag to discriminate. Common fields are top-level, event-specific fields are flattened alongside them.

```jsonl
{"event_id":"a1b2c3","session_id":"sess_01","timestamp":"2026-03-22T10:00:00Z","agent_type":"claude","workspace":"/project","confidence":"native","event_type":"SessionStart","environment":"cli","args":["--model","opus"],"config":{}}
{"event_id":"d4e5f6","session_id":"sess_01","timestamp":"2026-03-22T10:00:01Z","agent_type":"claude","workspace":"/project","confidence":"native","event_type":"ToolCall","tool_name":"Read","input":{"file_path":"/src/main.rs"}}
{"event_id":"g7h8i9","session_id":"sess_01","timestamp":"2026-03-22T10:00:02Z","agent_type":"claude","workspace":"/project","confidence":"inferred","event_type":"FileRead","path":"/src/main.rs"}
{"event_id":"j0k1l2","session_id":"sess_01","timestamp":"2026-03-22T10:00:05Z","agent_type":"claude","workspace":"/project","confidence":"native","event_type":"CommandExec","command":"cargo test","cwd":"/project","exit_code":0,"duration_ms":3200}
```

Serde representation: `#[serde(tag = "event_type")]` on the Event enum.

### Event Types (16)

#### 1. SessionStart
Agent started a session.

| Field | Type |
|---|---|
| `environment` | string |
| `args` | string[] |
| `config` | object |

#### 2. SessionEnd
Agent ended a session.

| Field | Type |
|---|---|
| `exit_code` | i32 |
| `duration_ms` | u64 |

#### 3. ModelRequest
Prompt sent to LLM.

| Field | Type |
|---|---|
| `model` | string |
| `provider` | string |
| `input_tokens` | u64 |

#### 4. ModelResponse
Response received from LLM.

| Field | Type |
|---|---|
| `output_tokens` | u64 |
| `stop_reason` | string |
| `duration_ms` | u64 |

#### 5. ToolCall
Agent invoked a tool.

| Field | Type |
|---|---|
| `tool_name` | string |
| `input` | object |

#### 6. ToolResult
Tool returned a result.

| Field | Type |
|---|---|
| `tool_name` | string |
| `output_summary` | string |
| `error` | string? |
| `duration_ms` | u64 |

#### 7. FileRead
File read operation.

| Field | Type |
|---|---|
| `path` | string |

#### 8. FileWrite
File write/create operation.

| Field | Type |
|---|---|
| `path` | string |
| `bytes_written` | u64 |

#### 9. FileDelete
File delete operation.

| Field | Type |
|---|---|
| `path` | string |

#### 10. CommandExec
Shell command execution.

| Field | Type |
|---|---|
| `command` | string |
| `cwd` | string |
| `exit_code` | i32 |
| `duration_ms` | u64 |

#### 11. SubagentStart
Sub-agent spawned.

| Field | Type |
|---|---|
| `subagent_name` | string |
| `task` | string |

#### 12. SubagentEnd
Sub-agent finished.

| Field | Type |
|---|---|
| `subagent_name` | string |
| `exit_code` | i32 |
| `duration_ms` | u64 |

#### 13. NetworkCall
HTTP/API call.

| Field | Type |
|---|---|
| `method` | string |
| `url` | string |
| `status_code` | u16 |
| `duration_ms` | u64 |

#### 14. McpCall
MCP server call.

| Field | Type |
|---|---|
| `server_name` | string |
| `method` | string |
| `params` | object |

#### 15. Artifact
Agent produced an artifact.

| Field | Type |
|---|---|
| `artifact_type` | string |
| `path` | string |
| `description` | string |

#### 16. Error
Error in agent session.

| Field | Type |
|---|---|
| `error_type` | string |
| `message` | string |
| `context` | string |

## Event Support Matrix

✅ = natively available  ⚠️ = requires parsing/inference  ❌ = not available

| Event | Claude Code | Codex CLI | Gemini CLI | Cursor | Copilot |
|---|---|---|---|---|---|
| SessionStart | ✅ hooks | ✅ hooks+json | ✅ hooks | ⚠️ | ✅ hooks |
| SessionEnd | ✅ hooks | ✅ hooks+json | ✅ hooks | ⚠️ stop hook | ✅ hooks |
| ModelRequest | ⚠️ transcript | ⚠️ json stream | ✅ BeforeModel | ❌ | ❌ |
| ModelResponse | ⚠️ transcript | ⚠️ json stream | ✅ AfterModel | ❌ | ❌ |
| ToolCall | ✅ PreToolUse | ⚠️ json stream | ✅ BeforeTool | ⚠️ | ✅ preToolUse |
| ToolResult | ✅ PostToolUse | ⚠️ json stream | ✅ AfterTool | ⚠️ | ✅ postToolUse |
| FileRead | ✅ via tools | ⚠️ via json | ✅ via tools | ✅ hook | ⚠️ via tools |
| FileWrite | ✅ via tools | ⚠️ via json | ✅ via tools | ✅ hook | ⚠️ via tools |
| FileDelete | ✅ via tools | ⚠️ via json | ✅ via tools | ⚠️ | ⚠️ via tools |
| CommandExec | ✅ Bash tool | ⚠️ json stream | ✅ via tools | ✅ hook | ⚠️ via tools |
| SubagentStart | ✅ hooks | ❌ | ✅ BeforeAgent | ❌ | ❌ |
| SubagentEnd | ✅ hooks | ❌ | ✅ AfterAgent | ❌ | ❌ |
| NetworkCall | ⚠️ WebFetch | ❌ | ⚠️ via tools | ❌ | ❌ |
| McpCall | ✅ mcp__* | ❌ | ✅ via tools | ✅ hook | ❌ |
| Artifact | ⚠️ file writes | ⚠️ output flag | ⚠️ result event | ❌ | ❌ |
| Error | ✅ StopFailure | ⚠️ json stream | ✅ Notification | ⚠️ stop hook | ✅ errorOccurred |

## Project Structure

```
nose/
├── src/
│   ├── main.rs              # CLI entry (clap)
│   ├── event.rs             # Unified event model + serde
│   ├── discovery.rs         # Agent discovery (known paths)
│   ├── detect.rs            # Format auto-detection
│   ├── adapter/
│   │   ├── mod.rs           # Adapter trait
│   │   ├── claude.rs        # Claude Code adapter
│   │   ├── codex.rs         # Codex CLI adapter
│   │   ├── gemini.rs        # Gemini CLI adapter
│   │   ├── cursor.rs        # Cursor adapter
│   │   └── copilot.rs       # GitHub Copilot adapter
│   └── output.rs            # JSONL writer to stdout
├── Cargo.toml
└── tests/
    └── fixtures/            # Sample agent outputs for each agent
```

## Dependencies (Rust crates)

- `clap` — CLI argument parsing
- `serde` + `serde_json` — JSON serialization/deserialization
- `uuid` — event ID generation
- `chrono` — timestamps
- `glob` / `walkdir` — file discovery

## Error Handling

- Corrupted/truncated session files: skip file, log warning to stderr, continue with next
- Unknown event types in raw data: skip event, continue parsing
- Missing discovery paths: silently skip agent (not installed)
- Changed log formats: adapter returns `AdapterError`, Nose logs warning and skips

Nose never fails hard. It emits what it can and warns about what it can't.

## Design Decisions

- **No `agent_name` field** — `agent_type` enum is sufficient. If needed later, it can be derived.
- **`confidence` restored from idea.md** — inferred events (e.g. FileRead parsed from a ToolCall) are marked `inferred`, natively reported events are `native`.
- **`raw_payload` restored from idea.md** — optional, allows lossless round-tripping. Adapters include it when available.
- **No `run_id`** — idea.md distinguished `session_id` from `run_id`. For v1, `session_id` is sufficient. Can be added later if agents expose distinct run semantics.
- **File-based only** — Nose reads files post-hoc. No runtime hooks, no agent wrapping. This keeps it simple and non-invasive.

## Out of Scope (for now)

- Real-time / streaming mode (watch for new sessions)
- Filtering by time range, event type, agent
- Remote agent support (cloud-hosted)
- Any UI or dashboard
- Python bindings
- Config file
