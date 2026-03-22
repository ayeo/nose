use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use chrono::{DateTime, Utc};
use uuid::Uuid;
use crate::adapter::Adapter;
use crate::error::AdapterError;
use crate::event::{AgentType, Confidence, Event, EventData};

pub struct CodexAdapter;

impl CodexAdapter {
    fn make_event(
        &self,
        session_id: &str,
        workspace: &str,
        timestamp: DateTime<Utc>,
        confidence: Confidence,
        data: EventData,
        raw: Option<serde_json::Value>,
    ) -> Event {
        Event {
            event_id: Uuid::new_v4(),
            session_id: session_id.to_string(),
            timestamp,
            agent_type: AgentType::Codex,
            workspace: workspace.to_string(),
            confidence,
            raw_payload: raw,
            data,
        }
    }

    fn parse_timestamp(&self, line: &serde_json::Value) -> DateTime<Utc> {
        line["timestamp"]
            .as_str()
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(Utc::now)
    }

    /// Parse a `function_call` line into ToolCall and higher-level inferred events.
    fn parse_function_call(
        &self,
        line: &serde_json::Value,
        session_id: &str,
        workspace: &str,
    ) -> Vec<Event> {
        let mut events = Vec::new();
        let timestamp = self.parse_timestamp(line);

        let name = line["name"].as_str().unwrap_or("unknown").to_string();
        let arguments: serde_json::Value = line["arguments"]
            .as_str()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_else(|| line["arguments"].clone());

        events.push(self.make_event(
            session_id,
            workspace,
            timestamp,
            Confidence::Native,
            EventData::ToolCall {
                tool_name: name.clone(),
                input: arguments.clone(),
            },
            Some(line.clone()),
        ));

        // Infer higher-level events from function name
        match name.as_str() {
            "shell" => {
                let command = arguments["command"].as_str().unwrap_or("").to_string();
                let cwd = arguments["workdir"].as_str().map(|s| s.to_string());
                events.push(self.make_event(
                    session_id,
                    workspace,
                    timestamp,
                    Confidence::Inferred,
                    EventData::CommandExec {
                        command,
                        cwd,
                        exit_code: None,
                        duration_ms: None,
                    },
                    None,
                ));
            }
            "read" => {
                if let Some(path) = arguments["path"].as_str() {
                    events.push(self.make_event(
                        session_id,
                        workspace,
                        timestamp,
                        Confidence::Inferred,
                        EventData::FileRead {
                            path: path.to_string(),
                        },
                        None,
                    ));
                }
            }
            "write" => {
                if let Some(path) = arguments["path"].as_str() {
                    let bytes = arguments["content"].as_str().map(|s| s.len() as u64);
                    events.push(self.make_event(
                        session_id,
                        workspace,
                        timestamp,
                        Confidence::Inferred,
                        EventData::FileWrite {
                            path: path.to_string(),
                            bytes_written: bytes,
                        },
                        None,
                    ));
                }
            }
            "delete" => {
                if let Some(path) = arguments["path"].as_str() {
                    events.push(self.make_event(
                        session_id,
                        workspace,
                        timestamp,
                        Confidence::Inferred,
                        EventData::FileDelete {
                            path: path.to_string(),
                        },
                        None,
                    ));
                }
            }
            _ => {}
        }

        events
    }

    /// Parse a `function_call_output` line into a ToolResult event.
    fn parse_function_call_output(
        &self,
        line: &serde_json::Value,
        session_id: &str,
        workspace: &str,
        last_tool_name: &str,
    ) -> Vec<Event> {
        let timestamp = self.parse_timestamp(line);
        let output = line["output"].as_str();
        let is_error = line["is_error"].as_bool().unwrap_or(false);

        let output_summary = if is_error {
            None
        } else {
            output.map(|s| {
                s.char_indices()
                    .take_while(|&(i, _)| i < 200)
                    .map(|(_, c)| c)
                    .collect::<String>()
            })
        };

        let error = if is_error {
            output.map(|s| s.to_string())
        } else {
            None
        };

        vec![self.make_event(
            session_id,
            workspace,
            timestamp,
            Confidence::Native,
            EventData::ToolResult {
                tool_name: last_tool_name.to_string(),
                output_summary,
                error,
                duration_ms: None,
            },
            None,
        )]
    }

    /// Parse an assistant `message` line with content blocks (model response).
    fn parse_assistant_message(
        &self,
        line: &serde_json::Value,
        session_id: &str,
        workspace: &str,
    ) -> Vec<Event> {
        let mut events = Vec::new();
        let timestamp = self.parse_timestamp(line);

        let model = line["model"].as_str().unwrap_or("unknown").to_string();
        let input_tokens = line["usage"]["input_tokens"].as_u64();
        events.push(self.make_event(
            session_id,
            workspace,
            timestamp,
            Confidence::Native,
            EventData::ModelRequest {
                model,
                provider: Some("openai".to_string()),
                input_tokens,
            },
            None,
        ));

        let output_tokens = line["usage"]["output_tokens"].as_u64();
        events.push(self.make_event(
            session_id,
            workspace,
            timestamp,
            Confidence::Native,
            EventData::ModelResponse {
                output_tokens,
                stop_reason: None,
                duration_ms: None,
            },
            None,
        ));

        events
    }
}

impl Adapter for CodexAdapter {
    fn name(&self) -> &'static str {
        "codex"
    }

    fn discovery_paths(&self, cwd: &Path) -> Vec<PathBuf> {
        if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
            // Codex CLI encodes cwd as directory name similar to Claude Code: /Users/foo/bar → -Users-foo-bar
            let encoded = cwd.to_string_lossy().replace('/', "-");
            vec![home.join(".codex").join("sessions").join(encoded)]
        } else {
            vec![]
        }
    }

    fn detect(&self, path: &Path) -> bool {
        path.to_string_lossy().contains(".codex/")
            && path.extension().is_some_and(|ext| ext == "jsonl")
    }

    fn parse(
        &self,
        reader: &mut dyn Read,
        session_id: &str,
        workspace: &str,
    ) -> Result<Vec<Event>, AdapterError> {
        let buf_reader = BufReader::new(reader);
        let mut events = Vec::new();
        let mut first_timestamp: Option<DateTime<Utc>> = None;
        let mut last_timestamp: Option<DateTime<Utc>> = None;
        let mut session_start_emitted = false;
        let mut last_tool_name = String::from("unknown");

        for line_result in buf_reader.lines() {
            let line_str = line_result?;
            if line_str.trim().is_empty() {
                continue;
            }

            let line: serde_json::Value = match serde_json::from_str(&line_str) {
                Ok(v) => v,
                Err(_) => continue,
            };

            let line_type = match line["type"].as_str() {
                Some(t) => t,
                None => continue,
            };

            let ts = self.parse_timestamp(&line);
            if first_timestamp.is_none() {
                first_timestamp = Some(ts);
            }
            last_timestamp = Some(ts);

            if !session_start_emitted {
                session_start_emitted = true;
                events.push(self.make_event(
                    session_id,
                    workspace,
                    ts,
                    Confidence::Inferred,
                    EventData::SessionStart {
                        environment: None,
                        args: vec![],
                        config: serde_json::Value::Null,
                    },
                    None,
                ));
            }

            match line_type {
                "message" => {
                    let role = line["role"].as_str().unwrap_or("");
                    if role == "assistant" {
                        events.extend(self.parse_assistant_message(&line, session_id, workspace));
                    }
                    // user messages are tracked for session lifecycle but don't produce extra events
                }
                "function_call" => {
                    // Track the tool name for the subsequent function_call_output
                    if let Some(name) = line["name"].as_str() {
                        last_tool_name = name.to_string();
                    }
                    events.extend(self.parse_function_call(&line, session_id, workspace));
                }
                "function_call_output" => {
                    events.extend(self.parse_function_call_output(
                        &line,
                        session_id,
                        workspace,
                        &last_tool_name,
                    ));
                }
                _ => {}
            }
        }

        // Emit SessionEnd after processing all lines
        if let (Some(first_ts), Some(last_ts)) = (first_timestamp, last_timestamp) {
            let duration_ms = (last_ts - first_ts).num_milliseconds().max(0) as u64;
            events.push(self.make_event(
                session_id,
                workspace,
                last_ts,
                Confidence::Inferred,
                EventData::SessionEnd {
                    exit_code: None,
                    duration_ms,
                },
                None,
            ));
        }

        Ok(events)
    }
}
