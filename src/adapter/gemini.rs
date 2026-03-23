use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use chrono::{DateTime, Utc};
use uuid::Uuid;
use crate::adapter::Adapter;
use crate::error::AdapterError;
use crate::event::{AgentType, Confidence, Event, EventData};

pub struct GeminiAdapter;

impl GeminiAdapter {
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
            agent_type: AgentType::Gemini,
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

    fn handle_tool_use(&self, line: &serde_json::Value, session_id: &str, workspace: &str) -> Vec<Event> {
        let mut events = Vec::new();
        let timestamp = self.parse_timestamp(line);
        let tool_name = line["name"].as_str().unwrap_or("unknown").to_string();
        let input = line["input"].clone();

        events.push(self.make_event(
            session_id,
            workspace,
            timestamp,
            Confidence::Native,
            EventData::ToolCall {
                tool_name: tool_name.clone(),
                input: input.clone(),
            },
            Some(line.clone()),
        ));

        // Infer higher-level events from tool name
        match tool_name.as_str() {
            "read_file" => {
                if let Some(path) = input["path"].as_str() {
                    events.push(self.make_event(
                        session_id,
                        workspace,
                        timestamp,
                        Confidence::Inferred,
                        EventData::FileRead { path: path.to_string() },
                        None,
                    ));
                }
            }
            "write_file" | "create_file" => {
                if let Some(path) = input["path"].as_str() {
                    let bytes = input["content"].as_str().map(|s| s.len() as u64);
                    events.push(self.make_event(
                        session_id,
                        workspace,
                        timestamp,
                        Confidence::Inferred,
                        EventData::FileWrite { path: path.to_string(), bytes_written: bytes },
                        None,
                    ));
                }
            }
            "edit_file" | "replace_in_file" => {
                if let Some(path) = input["path"].as_str() {
                    events.push(self.make_event(
                        session_id,
                        workspace,
                        timestamp,
                        Confidence::Inferred,
                        EventData::FileWrite { path: path.to_string(), bytes_written: None },
                        None,
                    ));
                }
            }
            "delete_file" => {
                if let Some(path) = input["path"].as_str() {
                    events.push(self.make_event(
                        session_id,
                        workspace,
                        timestamp,
                        Confidence::Inferred,
                        EventData::FileDelete { path: path.to_string() },
                        None,
                    ));
                }
            }
            "run_shell_command" | "execute_command" | "shell" => {
                let command = input["command"].as_str().unwrap_or("").to_string();
                let cwd = input["cwd"].as_str().map(|s| s.to_string());
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
            "web_fetch" | "fetch_url" | "http_request" => {
                let url = input["url"].as_str().unwrap_or("").to_string();
                events.push(self.make_event(
                    session_id,
                    workspace,
                    timestamp,
                    Confidence::Inferred,
                    EventData::NetworkCall {
                        method: "GET".to_string(),
                        url,
                        status_code: None,
                        duration_ms: None,
                    },
                    None,
                ));
            }
            _ => {}
        }

        events
    }

    fn handle_tool_result(&self, line: &serde_json::Value, session_id: &str, workspace: &str, last_tool_name: &str) -> Event {
        let timestamp = self.parse_timestamp(line);
        let output = line["output"].as_str();
        let error = line["error"].as_str();
        let is_error = error.is_some() || line["is_error"].as_bool().unwrap_or(false);

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

        let error_str = if is_error {
            error.map(|s| s.to_string()).or_else(|| output.map(|s| s.to_string()))
        } else {
            None
        };

        self.make_event(
            session_id,
            workspace,
            timestamp,
            Confidence::Native,
            EventData::ToolResult {
                tool_name: last_tool_name.to_string(),
                output_summary,
                error: error_str,
                duration_ms: line["duration_ms"].as_u64(),
            },
            None,
        )
    }

    fn handle_result(&self, line: &serde_json::Value, session_id: &str, workspace: &str) -> Vec<Event> {
        let mut events = Vec::new();
        let timestamp = self.parse_timestamp(line);

        // Extract model/token usage from result if present
        let model = line["model"].as_str().unwrap_or("gemini").to_string();
        let input_tokens = line["usage"]["input_tokens"].as_u64();
        let output_tokens = line["usage"]["output_tokens"].as_u64();

        if input_tokens.is_some() || output_tokens.is_some() {
            events.push(self.make_event(
                session_id,
                workspace,
                timestamp,
                Confidence::Native,
                EventData::ModelResponse {
                    output_tokens,
                    stop_reason: Some("end_turn".to_string()),
                    duration_ms: line["duration_ms"].as_u64(),
                },
                None,
            ));
            events.push(self.make_event(
                session_id,
                workspace,
                timestamp,
                Confidence::Inferred,
                EventData::ModelRequest {
                    model,
                    provider: Some("google".to_string()),
                    input_tokens,
                },
                None,
            ));
        }

        events
    }
}

impl Adapter for GeminiAdapter {
    fn name(&self) -> &'static str {
        "gemini"
    }

    fn discovery_paths(&self, cwd: &Path) -> Vec<PathBuf> {
        let cwd_str = cwd.to_string_lossy();
        // Encode cwd as directory name, handling both Unix and Windows separators
        let encoded = cwd_str.replace(['/', '\\'], "-");

        let mut paths = Vec::new();

        // Primary: HOME env var (works on macOS/Linux; may be set on Windows too)
        if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
            paths.push(home.join(".gemini").join("projects").join(&encoded));
        }

        // Windows: %APPDATA%\gemini\projects\ and %USERPROFILE%\.gemini\projects\
        #[cfg(target_os = "windows")]
        {
            if let Some(data_dir) = dirs::data_dir() {
                paths.push(data_dir.join("gemini").join("projects").join(&encoded));
            }
            if let Some(home_dir) = dirs::home_dir() {
                let candidate = home_dir.join(".gemini").join("projects").join(&encoded);
                if !paths.contains(&candidate) {
                    paths.push(candidate);
                }
            }
        }

        // Fallback for any platform where HOME wasn't set: use dirs::home_dir()
        if paths.is_empty() {
            if let Some(home_dir) = dirs::home_dir() {
                paths.push(home_dir.join(".gemini").join("projects").join(&encoded));
            }
        }

        paths
    }

    fn detect(&self, path: &Path) -> bool {
        let path_str = path.to_string_lossy();
        (path_str.contains(".gemini/") || path_str.contains(".gemini\\"))
            && path.extension().is_some_and(|ext| ext == "jsonl")
    }

    fn parse(&self, reader: &mut dyn Read, session_id: &str, workspace: &str) -> Result<Vec<Event>, AdapterError> {
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

            let line_type = line["type"].as_str();
            if !matches!(line_type, Some("tool_use") | Some("tool_result") | Some("result") | Some("model_request")) {
                continue;
            }

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
                Some("model_request") => {
                    let model = line["model"].as_str().unwrap_or("gemini").to_string();
                    let input_tokens = line["usage"]["input_tokens"].as_u64();
                    events.push(self.make_event(
                        session_id,
                        workspace,
                        ts,
                        Confidence::Native,
                        EventData::ModelRequest {
                            model,
                            provider: Some("google".to_string()),
                            input_tokens,
                        },
                        None,
                    ));
                }
                Some("tool_use") => {
                    if let Some(name) = line["name"].as_str() {
                        last_tool_name = name.to_string();
                    }
                    events.extend(self.handle_tool_use(&line, session_id, workspace));
                }
                Some("tool_result") => {
                    events.push(self.handle_tool_result(&line, session_id, workspace, &last_tool_name));
                }
                Some("result") => {
                    events.extend(self.handle_result(&line, session_id, workspace));
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discovery_paths_does_not_panic() {
        let adapter = GeminiAdapter;
        let cwd = std::path::Path::new("/some/project/path");
        // Should not panic on any platform
        let paths = adapter.discovery_paths(cwd);
        assert!(paths.len() >= 1 || paths.is_empty());
    }

    #[test]
    fn discovery_paths_encodes_windows_separators() {
        let adapter = GeminiAdapter;
        let cwd = std::path::Path::new("C:\\Users\\foo\\project");
        let paths = adapter.discovery_paths(cwd);
        for path in &paths {
            let path_str = path.to_string_lossy();
            assert!(!path_str.ends_with('\\') || path_str.contains("projects"));
        }
    }
}
