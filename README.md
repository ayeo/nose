# Nose — Agent Activity Observability

Unified abstraction layer for observing AI coding agent actions. Nose auto-discovers agent sessions on your machine, parses their logs, and emits a consistent JSONL event stream.

**Lang:** Rust · **Output:** JSONL

## Usage

```bash
nose parse
```

No flags, no config. Nose detects installed agents, finds their session files, and outputs unified events to stdout.

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
