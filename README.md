# Nose — Agent Activity Observability

Unified abstraction layer for observing AI coding agent actions. Nose auto-discovers agent sessions on your machine, parses their logs, and emits a consistent JSONL event stream.

**Lang:** Rust · **Output:** JSONL

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

Nose scopes to the **current working directory** — it only parses sessions from the project you're in, not all projects on your machine.

Output goes to stdout as JSONL (one JSON event per line). Pipe it however you need:

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

Nose reads files only — it does not install hooks or run agents.

## Supported Agents

| Agent | Data Sources (files read by Nose) |
|---|---|
| Claude Code | JSONL transcripts (`~/.claude/projects/*/`) |
| Codex CLI | JSON log files (`~/.codex/log/`) |
| Gemini CLI | Stream-JSON output (`~/.gemini/`) |
| Cursor | Hook output files (`~/Library/Application Support/Cursor/`) |
| GitHub Copilot | Hook output files (`~/.github-copilot/`) |

## Unified Event Model

All events share common fields:

| Field | Type | Description |
|---|---|---|
| `event_id` | UUID | Unique event identifier |
| `session_id` | string | Agent session identifier |
| `timestamp` | ISO 8601 | When the event occurred |
| `agent_type` | enum | `claude` \| `codex` \| `gemini` \| `cursor` \| `copilot` |
| `workspace` | string | Working directory path |
| `confidence` | enum | `native` \| `inferred` |
| `raw_payload` | object? | Original agent-specific payload (optional) |

## Event Types

| # | Event | Description |
|---|---|---|
| 1 | **SessionStart** | Agent started a session |
| 2 | **SessionEnd** | Agent ended a session |
| 3 | **ModelRequest** | Prompt sent to LLM |
| 4 | **ModelResponse** | Response received from LLM |
| 5 | **ToolCall** | Agent invoked a tool |
| 6 | **ToolResult** | Tool returned a result |
| 7 | **FileRead** | File read operation |
| 8 | **FileWrite** | File write/create operation |
| 9 | **FileDelete** | File delete operation |
| 10 | **CommandExec** | Shell command execution |
| 11 | **SubagentStart** | Sub-agent spawned |
| 12 | **SubagentEnd** | Sub-agent finished |
| 13 | **NetworkCall** | HTTP/API call |
| 14 | **McpCall** | MCP server call |
| 15 | **Artifact** | Agent produced an artifact |
| 16 | **Error** | Error in agent session |

## Event Support Matrix

✅ = natively available  ⚠️ = requires parsing/inference  ❌ = not available

| Event | Claude Code | Codex CLI | Gemini CLI | Cursor | Copilot |
|---|---|---|---|---|---|
| **SessionStart** | ✅ hooks | ✅ hooks+json | ✅ hooks | ⚠️ | ✅ hooks |
| **SessionEnd** | ✅ hooks | ✅ hooks+json | ✅ hooks | ⚠️ stop hook | ✅ hooks |
| **ModelRequest** | ⚠️ transcript | ⚠️ json stream | ✅ BeforeModel | ❌ | ❌ |
| **ModelResponse** | ⚠️ transcript | ⚠️ json stream | ✅ AfterModel | ❌ | ❌ |
| **ToolCall** | ✅ PreToolUse | ⚠️ json stream | ✅ BeforeTool | ⚠️ | ✅ preToolUse |
| **ToolResult** | ✅ PostToolUse | ⚠️ json stream | ✅ AfterTool | ⚠️ | ✅ postToolUse |
| **FileRead** | ✅ via tools | ⚠️ via json | ✅ via tools | ✅ hook | ⚠️ via tools |
| **FileWrite** | ✅ via tools | ⚠️ via json | ✅ via tools | ✅ hook | ⚠️ via tools |
| **FileDelete** | ✅ via tools | ⚠️ via json | ✅ via tools | ⚠️ | ⚠️ via tools |
| **CommandExec** | ✅ Bash tool | ⚠️ json stream | ✅ via tools | ✅ hook | ⚠️ via tools |
| **SubagentStart** | ✅ hooks | ❌ | ✅ BeforeAgent | ❌ | ❌ |
| **SubagentEnd** | ✅ hooks | ❌ | ✅ AfterAgent | ❌ | ❌ |
| **NetworkCall** | ⚠️ WebFetch | ❌ | ⚠️ via tools | ❌ | ❌ |
| **McpCall** | ✅ mcp__* | ❌ | ✅ via tools | ✅ hook | ❌ |
| **Artifact** | ⚠️ file writes | ⚠️ output flag | ⚠️ result event | ❌ | ❌ |
| **Error** | ✅ StopFailure | ⚠️ json stream | ✅ Notification | ⚠️ stop hook | ✅ errorOccurred |

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
