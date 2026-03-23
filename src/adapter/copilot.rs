use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use chrono::{DateTime, Utc};
use uuid::Uuid;
use crate::adapter::Adapter;
use crate::error::AdapterError;
use crate::event::{AgentType, Confidence, Event, EventData};

pub struct CopilotAdapter;

impl CopilotAdapter {
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
            agent_type: AgentType::Copilot,
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

    fn parse_pre_tool_use(
        &self,
        line: &serde_json::Value,
        session_id: &str,
        workspace: &str,
    ) -> Vec<Event> {
        let mut events = Vec::new();
        let timestamp = self.parse_timestamp(line);

        let tool_name = line["toolName"].as_str().unwrap_or("unknown").to_string();
        let tool_args = line["toolArgs"].clone();

        events.push(self.make_event(
            session_id,
            workspace,
            timestamp,
            Confidence::Native,
            EventData::ToolCall {
                tool_name: tool_name.clone(),
                input: tool_args.clone(),
            },
            Some(line.clone()),
        ));

        // Infer higher-level events from tool name
        match tool_name.as_str() {
            "read" => {
                if let Some(path) = tool_args["file"].as_str() {
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
            "edit" | "write" => {
                if let Some(path) = tool_args["file"].as_str() {
                    let bytes = tool_args["content"].as_str().map(|s| s.len() as u64);
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
            "bash" | "shell" | "run" => {
                let command = tool_args["command"].as_str().unwrap_or("").to_string();
                let cwd = tool_args["cwd"].as_str().map(|s| s.to_string());
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
            _ => {}
        }

        events
    }

    fn parse_post_tool_use(
        &self,
        line: &serde_json::Value,
        session_id: &str,
        workspace: &str,
    ) -> Vec<Event> {
        let timestamp = self.parse_timestamp(line);
        let tool_name = line["toolName"].as_str().unwrap_or("unknown").to_string();
        let tool_result = line["toolResult"].as_str();
        let is_error = line["isError"].as_bool().unwrap_or(false);

        let output_summary = if is_error {
            None
        } else {
            tool_result.map(|s| {
                s.char_indices()
                    .take_while(|&(i, _)| i < 200)
                    .map(|(_, c)| c)
                    .collect::<String>()
            })
        };

        let error = if is_error {
            tool_result.map(|s| s.to_string())
        } else {
            None
        };

        vec![self.make_event(
            session_id,
            workspace,
            timestamp,
            Confidence::Native,
            EventData::ToolResult {
                tool_name,
                output_summary,
                error,
                duration_ms: None,
            },
            None,
        )]
    }
}

impl Adapter for CopilotAdapter {
    fn name(&self) -> &'static str {
        "copilot"
    }

    fn discovery_paths(&self, _cwd: &Path) -> Vec<PathBuf> {
        let mut paths = Vec::new();

        // ~/.github-copilot/ (works on both macOS and Linux)
        if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
            let dotfile = home.join(".github-copilot");
            if dotfile.exists() {
                paths.push(dotfile);
            }
        }

        // Platform-specific paths
        match std::env::consts::OS {
            "macos" => {
                if let Some(data_dir) = dirs::data_dir() {
                    let path = data_dir.join("github-copilot");
                    if path.exists() {
                        paths.push(path);
                    }
                }
            }
            "linux" => {
                if let Some(config_dir) = dirs::config_dir() {
                    let path = config_dir.join("github-copilot");
                    if path.exists() {
                        paths.push(path);
                    }
                }
            }
            _ => {}
        }

        paths
    }

    fn detect(&self, path: &Path) -> bool {
        let s = path.to_string_lossy();
        (s.contains("github-copilot"))
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

        for line_result in buf_reader.lines() {
            let line_str = line_result?;
            if line_str.trim().is_empty() {
                continue;
            }

            let line: serde_json::Value = match serde_json::from_str(&line_str) {
                Ok(v) => v,
                Err(_) => continue,
            };

            let event_type = match line["event"].as_str() {
                Some(t) => t,
                None => continue,
            };

            let ts = self.parse_timestamp(&line);
            if first_timestamp.is_none() {
                first_timestamp = Some(ts);
            }

            match event_type {
                "sessionStart" => {
                    let initial_prompt = line["initialPrompt"].as_str().map(|s| s.to_string());
                    let args = initial_prompt
                        .map(|p| vec![p])
                        .unwrap_or_default();
                    events.push(self.make_event(
                        session_id,
                        workspace,
                        ts,
                        Confidence::Native,
                        EventData::SessionStart {
                            environment: None,
                            args,
                            config: serde_json::Value::Null,
                        },
                        Some(line.clone()),
                    ));
                }
                "sessionEnd" => {
                    let duration_ms = if let Some(first_ts) = first_timestamp {
                        (ts - first_ts).num_milliseconds().max(0) as u64
                    } else {
                        0
                    };
                    events.push(self.make_event(
                        session_id,
                        workspace,
                        ts,
                        Confidence::Native,
                        EventData::SessionEnd {
                            exit_code: None,
                            duration_ms,
                        },
                        Some(line.clone()),
                    ));
                }
                "preToolUse" => {
                    events.extend(self.parse_pre_tool_use(&line, session_id, workspace));
                }
                "postToolUse" => {
                    events.extend(self.parse_post_tool_use(&line, session_id, workspace));
                }
                "errorOccurred" => {
                    let error_type = line["error"].as_str().unwrap_or("unknown").to_string();
                    let message = line["message"].as_str().unwrap_or("").to_string();
                    events.push(self.make_event(
                        session_id,
                        workspace,
                        ts,
                        Confidence::Native,
                        EventData::Error {
                            error_type,
                            message,
                            context: None,
                        },
                        Some(line.clone()),
                    ));
                }
                _ => {}
            }
        }

        Ok(events)
    }
}
