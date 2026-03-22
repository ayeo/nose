use std::collections::HashMap;
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

    fn parse_assistant_message(&self, line: &serde_json::Value, session_id: &str, workspace: &str, tool_id_to_name: &mut HashMap<String, String>) -> Vec<Event> {
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
                if let Some("tool_use") = block["type"].as_str() {
                    let tool_name = block["name"].as_str().unwrap_or("unknown").to_string();
                    let input = block["input"].clone();

                    // Record tool_use_id → tool_name mapping for later ToolResult lookup
                    if let Some(tool_id) = block["id"].as_str() {
                        tool_id_to_name.insert(tool_id.to_string(), tool_name.clone());
                    }

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

    fn parse_user_message(&self, line: &serde_json::Value, session_id: &str, workspace: &str, tool_id_to_name: &HashMap<String, String>) -> Vec<Event> {
        let mut events = Vec::new();
        let msg = &line["message"];
        let timestamp = self.parse_timestamp(line);

        if let Some(content) = msg["content"].as_array() {
            for block in content {
                if let Some("tool_result") = block["type"].as_str() {
                    let tool_use_id = block["tool_use_id"].as_str().unwrap_or("");
                    let tool_name = tool_id_to_name.get(tool_use_id).cloned().unwrap_or_else(|| "unknown".to_string());
                    let is_error = block["is_error"].as_bool().unwrap_or(false);

                    let content_str = block["content"].as_str();

                    let output_summary = if is_error {
                        None
                    } else {
                        content_str.map(|s| {
                            s.char_indices()
                                .take_while(|&(i, _)| i < 200)
                                .map(|(_, c)| c)
                                .collect::<String>()
                        })
                    };

                    let error = if is_error {
                        content_str.map(|s| s.to_string())
                    } else {
                        None
                    };

                    events.push(self.make_event(
                        session_id, workspace, timestamp, Confidence::Native,
                        EventData::ToolResult {
                            tool_name,
                            output_summary,
                            error,
                            duration_ms: None,
                        },
                        None,
                    ));
                }
            }
        }

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
        if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
            vec![home.join(".claude").join("projects")]
        } else {
            vec![]
        }
    }

    fn detect(&self, path: &Path) -> bool {
        path.to_string_lossy().contains(".claude/projects/")
            && path.extension().is_some_and(|ext| ext == "jsonl")
    }

    fn parse(&self, reader: &mut dyn Read, session_id: &str, workspace: &str) -> Result<Vec<Event>, AdapterError> {
        let buf_reader = BufReader::new(reader);
        let mut events = Vec::new();
        let mut tool_id_to_name: HashMap<String, String> = HashMap::new();
        let mut first_timestamp: Option<DateTime<Utc>> = None;
        let mut last_timestamp: Option<DateTime<Utc>> = None;
        let mut session_start_emitted = false;

        for line_result in buf_reader.lines() {
            let line_str = line_result?;
            if line_str.trim().is_empty() {
                continue;
            }

            let line: serde_json::Value = match serde_json::from_str(&line_str) {
                Ok(v) => v,
                Err(_) => continue,
            };

            let line_type = line["type"].as_str();
            if !matches!(line_type, Some("user") | Some("assistant")) {
                continue;
            }

            let ts = self.parse_timestamp(&line);
            if first_timestamp.is_none() {
                first_timestamp = Some(ts);
            }
            last_timestamp = Some(ts);

            // Emit SessionStart from the first user or assistant line
            if !session_start_emitted {
                session_start_emitted = true;
                events.push(self.make_event(
                    session_id, workspace, ts, Confidence::Inferred,
                    EventData::SessionStart {
                        environment: None,
                        args: vec![],
                        config: serde_json::Value::Null,
                    },
                    None,
                ));
            }

            match line_type {
                Some("assistant") => {
                    events.extend(self.parse_assistant_message(&line, session_id, workspace, &mut tool_id_to_name));
                }
                Some("user") => {
                    events.extend(self.parse_user_message(&line, session_id, workspace, &tool_id_to_name));
                }
                _ => {}
            }
        }

        // Emit SessionEnd after processing all lines
        if let (Some(first_ts), Some(last_ts)) = (first_timestamp, last_timestamp) {
            let duration_ms = (last_ts - first_ts).num_milliseconds().max(0) as u64;
            events.push(self.make_event(
                session_id, workspace, last_ts, Confidence::Inferred,
                EventData::SessionEnd {
                    exit_code: 0,
                    duration_ms,
                },
                None,
            ));
        }

        Ok(events)
    }
}
