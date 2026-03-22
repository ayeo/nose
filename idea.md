# OOS — concise summary

## Problem

The real problem is not token telemetry, but **observability of agent actions**:

- which tools the agent used,
- which files it read, created, or modified,
- which commands it executed,
- which external systems it touched,
- what side effects and artifacts it produced.

This cannot be solved at the model-provider level alone, because the LLM usually does not know what happened in the local filesystem, shell, IDE, sandbox, or runtime.

## Core conclusion

OOS should be an abstraction over **agent activity and execution provenance**, not just over LLM token usage.

The abstraction must normalize what the agent actually did across different environments:
- local CLI / IDE agents,
- cloud-hosted agents,
- custom orchestrators,
- provider API wrappers.

## What OOS should capture

A common event model should include:

- **Session / Run** — start, end, runtime, environment
- **Tool activity** — tool called, completed, failed, inputs, outputs
- **File activity** — read, create, update, delete, path, change summary
- **Command activity** — command, cwd, exit code, duration, output summary
- **Orchestration activity** — subagent start/finish, handoff, approvals
- **External side effects** — MCP calls, HTTP calls, git actions, deploy actions

## Recommended model

Minimal canonical structure:

- `Run`
- `Step`
- `ToolCall`
- `FileOperation`
- `CommandExec`
- `NetworkCall`
- `MCPCall`
- `ModelGeneration`
- `Artifact`

Each event should contain:
- `event_id`
- `session_id`
- `run_id`
- `timestamp`
- `source`
- `agent_name`
- `workspace`
- `confidence`
- `raw_payload`
- `normalized_payload`

## Integration levels

OOS should support three levels of observability:

### 1. Native
Runtime emits structured events directly.
Example: hooks / built-in telemetry.

### 2. Wrapped
A wrapper captures process execution, tool calls, outputs, and diffs.

### 3. Observed
Host-level monitoring captures filesystem, process, and network activity when no native integration exists.

## Honest product position

Do not claim “full visibility everywhere”.

The correct position is:

**OOS normalizes and correlates agent actions across tools, files, commands, and sessions.  
The level of completeness depends on the integration type.**

## Best product framing

This is not primarily “LLM observability”.

It is:

**Agent Activity Observability**  
or  
**Agent Execution Provenance**

That is the real abstraction layer worth building.
