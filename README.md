# Nose

Unified event format for AI coding agent actions. Nose auto-discovers agent sessions on your machine, parses their logs, and emits a consistent JSONL event stream.

**Lang:** Rust | **Output:** JSONL | **Platform:** macOS, Linux

## Installation

```bash
cargo install --path .
```

Or build locally:

```bash
cargo build --release
# binary at target/release/nose
```

## Usage

Run from within any project directory where you've used an AI coding agent:

```bash
cd my-project/
nose parse
```

Nose scopes to the **current working directory**. It only parses sessions from the project you're in.

Output goes to stdout as JSONL (one JSON event per line):

```bash
# Save to file
nose parse > events.jsonl

# Count events
nose parse | wc -l

# Filter for tool calls only
nose parse | jq 'select(.event_type == "ToolCall")'

# See what files the agent touched
nose parse | jq 'select(.event_type == "FileWrite") | .path'

# Get all shell commands the agent ran
nose parse | jq 'select(.event_type == "CommandExec") | .command'

# Count events by type
nose parse | jq -r '.event_type' | sort | uniq -c | sort -rn
```

Nose reads files only. It does not install hooks or run agents.

## Supported Agents

| Agent | Data Sources |
|---|---|
| Claude Code | JSONL transcripts (`~/.claude/projects/`) |
| Codex CLI | JSON log files (`~/.codex/sessions/`) |
| Gemini CLI | Stream-JSON output (`~/.gemini/`) |
| Cursor | Hook output files (planned) |
| GitHub Copilot | Hook output files (planned) |

## Event Model

All events share common fields:

| Field | Type | Description |
|---|---|---|
| `event_id` | UUID | Unique event identifier |
| `session_id` | string | Agent session identifier |
| `timestamp` | ISO 8601 | When the event occurred |
| `agent_type` | enum | `claude`, `codex`, `gemini`, `cursor`, `copilot` |
| `workspace` | string | Working directory path |
| `confidence` | enum | `native`, `inferred` |
| `raw_payload` | object? | Original agent-specific payload (optional) |

## Event Types

| Event | Description |
|---|---|
| SessionStart | Agent started a session |
| SessionEnd | Agent ended a session |
| ModelRequest | Prompt sent to LLM |
| ModelResponse | Response received from LLM |
| ToolCall | Agent invoked a tool |
| ToolResult | Tool returned a result |
| FileRead | File read operation |
| FileWrite | File write/create operation |
| FileDelete | File delete operation |
| CommandExec | Shell command execution |
| SubagentStart | Sub-agent spawned |
| SubagentEnd | Sub-agent finished |
| NetworkCall | HTTP/API call |
| McpCall | MCP server call |
| Artifact | Agent produced an artifact |
| Error | Error in agent session |

## Architecture

```
cwd -> Adapter.discovery_paths() -> known agent paths
                                        |
                                    walkdir scan
                                        |
                                  adapter.detect()
                                        |
Claude adapter --\
Codex adapter  ---+-- adapter.parse() -- Unified Events -- JSONL (stdout)
Gemini adapter --/
```
