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
        if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
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
                Err(_) => continue,
            };

            match line["type"].as_str() {
                Some("assistant") => {
                    events.extend(self.parse_assistant_message(&line, session_id, workspace));
                }
                // user, progress, file-history-snapshot, system — skip for v1
                _ => {}
            }
        }

        Ok(events)
    }
}
