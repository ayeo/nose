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
