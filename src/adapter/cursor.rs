use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use chrono::{DateTime, Utc};
use uuid::Uuid;
use crate::adapter::Adapter;
use crate::error::AdapterError;
use crate::event::{AgentType, Confidence, Event, EventData};

pub struct CursorAdapter;

impl CursorAdapter {
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
            agent_type: AgentType::Cursor,
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

    fn handle_before_shell_execution(
        &self,
        line: &serde_json::Value,
        session_id: &str,
        workspace: &str,
    ) -> Vec<Event> {
        let timestamp = self.parse_timestamp(line);
        let command = line["command"].as_str().unwrap_or("").to_string();
        let cwd = line["cwd"].as_str().map(|s| s.to_string());

        vec![self.make_event(
            session_id,
            workspace,
            timestamp,
            Confidence::Native,
            EventData::CommandExec {
                command,
                cwd,
                exit_code: None,
                duration_ms: None,
            },
            Some(line.clone()),
        )]
    }

    fn handle_after_file_edit(
        &self,
        line: &serde_json::Value,
        session_id: &str,
        workspace: &str,
    ) -> Vec<Event> {
        let timestamp = self.parse_timestamp(line);
        let path = line["file_path"].as_str().unwrap_or("").to_string();

        vec![self.make_event(
            session_id,
            workspace,
            timestamp,
            Confidence::Native,
            EventData::FileWrite {
                path,
                bytes_written: None,
            },
            Some(line.clone()),
        )]
    }

    fn handle_before_read_file(
        &self,
        line: &serde_json::Value,
        session_id: &str,
        workspace: &str,
    ) -> Vec<Event> {
        let timestamp = self.parse_timestamp(line);
        let path = line["file_path"].as_str().unwrap_or("").to_string();

        vec![self.make_event(
            session_id,
            workspace,
            timestamp,
            Confidence::Native,
            EventData::FileRead { path },
            Some(line.clone()),
        )]
    }

    fn handle_before_mcp_execution(
        &self,
        line: &serde_json::Value,
        session_id: &str,
        workspace: &str,
    ) -> Vec<Event> {
        let timestamp = self.parse_timestamp(line);
        let server_name = line["server_name"].as_str().unwrap_or("unknown").to_string();
        let method = line["tool_name"].as_str().unwrap_or("unknown").to_string();
        let params = if line["tool_input"].is_null() {
            None
        } else {
            Some(line["tool_input"].clone())
        };

        vec![self.make_event(
            session_id,
            workspace,
            timestamp,
            Confidence::Native,
            EventData::McpCall {
                server_name,
                method,
                params,
            },
            Some(line.clone()),
        )]
    }

    fn handle_stop(
        &self,
        line: &serde_json::Value,
        session_id: &str,
        workspace: &str,
        first_timestamp: Option<DateTime<Utc>>,
    ) -> Vec<Event> {
        let timestamp = self.parse_timestamp(line);
        let duration_ms = first_timestamp
            .map(|ft| (timestamp - ft).num_milliseconds().max(0) as u64)
            .unwrap_or(0);

        vec![self.make_event(
            session_id,
            workspace,
            timestamp,
            Confidence::Native,
            EventData::SessionEnd {
                exit_code: None,
                duration_ms,
            },
            Some(line.clone()),
        )]
    }
}

impl Adapter for CursorAdapter {
    fn name(&self) -> &'static str {
        "cursor"
    }

    fn discovery_paths(&self, _cwd: &Path) -> Vec<PathBuf> {
        let base = match std::env::consts::OS {
            "macos" => dirs::data_dir().map(|d| d.join("Cursor").join("User").join("workspaceStorage")),
            "linux" => dirs::config_dir().map(|d| d.join("cursor").join("User").join("workspaceStorage")),
            "windows" => dirs::data_dir().map(|d| d.join("Cursor").join("User").join("workspaceStorage")),
            _ => None,
        };
        base.into_iter().collect()
    }

    fn detect(&self, path: &Path) -> bool {
        let path_str = path.to_string_lossy();
        (path_str.contains("Cursor/User/workspaceStorage")
            || path_str.contains("cursor/User/workspaceStorage")
            || path_str.contains("Cursor\\User\\workspaceStorage"))
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
        let mut stop_emitted = false;

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

            // Emit SessionStart from the first event
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
                "beforeShellExecution" => {
                    events.extend(self.handle_before_shell_execution(&line, session_id, workspace));
                }
                "afterFileEdit" => {
                    events.extend(self.handle_after_file_edit(&line, session_id, workspace));
                }
                "beforeReadFile" => {
                    events.extend(self.handle_before_read_file(&line, session_id, workspace));
                }
                "beforeMCPExecution" => {
                    events.extend(self.handle_before_mcp_execution(&line, session_id, workspace));
                }
                "stop" => {
                    stop_emitted = true;
                    events.extend(self.handle_stop(&line, session_id, workspace, first_timestamp));
                }
                _ => {}
            }
        }

        // Emit SessionEnd if no explicit stop event was seen
        if !stop_emitted {
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
        }

        Ok(events)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discovery_paths_does_not_panic() {
        let adapter = CursorAdapter;
        let cwd = std::path::Path::new("/some/project/path");
        // Should not panic on any platform
        let paths = adapter.discovery_paths(cwd);
        assert!(paths.len() >= 1 || paths.is_empty());
    }
}
