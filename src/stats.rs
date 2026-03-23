use std::collections::{HashMap, HashSet};
use crate::event::{Event, EventData};

fn format_number(n: u64) -> String {
    let s = n.to_string();
    let mut result = String::new();
    for (i, ch) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(ch);
    }
    result.chars().rev().collect()
}

fn format_duration(total_ms: u64) -> String {
    let total_secs = total_ms / 1000;
    let hours = total_secs / 3600;
    let mins = (total_secs % 3600) / 60;
    if hours > 0 {
        format!("{}h {}m", hours, mins)
    } else if mins > 0 {
        format!("{}m", mins)
    } else {
        format!("{}s", total_secs)
    }
}

#[derive(Default)]
pub struct Stats {
    pub sessions: u64,
    pub total_events: u64,
    pub total_duration_ms: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub commands_run: u64,
    pub event_counts: HashMap<String, u64>,
    pub model_counts: HashMap<String, u64>,
    pub tool_counts: HashMap<String, u64>,
    pub files_touched: HashSet<String>,
}

impl Stats {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_event(&mut self, event: &Event) {
        self.total_events += 1;

        let type_name = match &event.data {
            EventData::SessionStart { .. } => "SessionStart",
            EventData::SessionEnd { .. } => "SessionEnd",
            EventData::ModelRequest { .. } => "ModelRequest",
            EventData::ModelResponse { .. } => "ModelResponse",
            EventData::ToolCall { .. } => "ToolCall",
            EventData::ToolResult { .. } => "ToolResult",
            EventData::FileRead { .. } => "FileRead",
            EventData::FileWrite { .. } => "FileWrite",
            EventData::FileDelete { .. } => "FileDelete",
            EventData::CommandExec { .. } => "CommandExec",
            EventData::SubagentStart { .. } => "SubagentStart",
            EventData::SubagentEnd { .. } => "SubagentEnd",
            EventData::NetworkCall { .. } => "NetworkCall",
            EventData::McpCall { .. } => "McpCall",
            EventData::Artifact { .. } => "Artifact",
            EventData::Error { .. } => "Error",
        };
        *self.event_counts.entry(type_name.to_string()).or_insert(0) += 1;

        match &event.data {
            EventData::SessionStart { .. } => {
                self.sessions += 1;
            }
            EventData::SessionEnd { duration_ms, .. } => {
                self.total_duration_ms += duration_ms;
            }
            EventData::ModelRequest {
                model,
                input_tokens,
                ..
            } => {
                *self.model_counts.entry(model.clone()).or_insert(0) += 1;
                if let Some(tokens) = input_tokens {
                    self.input_tokens += tokens;
                }
            }
            EventData::ModelResponse {
                output_tokens: Some(tokens),
                ..
            } => {
                self.output_tokens += tokens;
            }
            EventData::ModelResponse { .. } => {}
            EventData::ToolCall { tool_name, .. } => {
                *self.tool_counts.entry(tool_name.clone()).or_insert(0) += 1;
            }
            EventData::FileRead { path } => {
                self.files_touched.insert(path.clone());
            }
            EventData::FileWrite { path, .. } => {
                self.files_touched.insert(path.clone());
            }
            EventData::FileDelete { path } => {
                self.files_touched.insert(path.clone());
            }
            EventData::CommandExec { .. } => {
                self.commands_run += 1;
            }
            _ => {}
        }
    }

    pub fn display(&self, workspace: &str) {
        println!("Nose Stats for {}", workspace);
        println!();
        println!("Sessions: {}", self.sessions);
        println!("Total events: {}", format_number(self.total_events));
        println!("Duration: {}", format_duration(self.total_duration_ms));
        println!();

        println!("Events by type:");
        let mut event_counts: Vec<(&String, &u64)> = self.event_counts.iter().collect();
        event_counts.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
        for (name, count) in &event_counts {
            println!("  {:<16} {}", name, format_number(**count));
        }
        println!();

        let total_tokens = self.input_tokens + self.output_tokens;
        println!("Tokens:");
        println!("  Input:  {:>12}", format_number(self.input_tokens));
        println!("  Output: {:>12}", format_number(self.output_tokens));
        println!("  Total:  {:>12}", format_number(total_tokens));
        println!();

        if !self.model_counts.is_empty() {
            println!("Models used:");
            let mut model_counts: Vec<(&String, &u64)> = self.model_counts.iter().collect();
            model_counts.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
            for (model, count) in &model_counts {
                println!("  {:<28} {} requests", model, format_number(**count));
            }
            println!();
        }

        if !self.tool_counts.is_empty() {
            println!("Top tools:");
            let mut tool_counts: Vec<(&String, &u64)> = self.tool_counts.iter().collect();
            tool_counts.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
            for (tool, count) in tool_counts.iter().take(10) {
                println!("  {:<16} {}", tool, format_number(**count));
            }
            println!();
        }

        println!("Files touched: {}", self.files_touched.len());
        println!("Commands run: {}", format_number(self.commands_run));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_format_number_tests() {
        assert_eq!(format_number(0), "0");
        assert_eq!(format_number(999), "999");
        assert_eq!(format_number(1000), "1,000");
        assert_eq!(format_number(1234567), "1,234,567");
    }

    #[test]
    fn test_format_number() {
        make_format_number_tests();
    }

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(0), "0s");
        assert_eq!(format_duration(30_000), "30s");
        assert_eq!(format_duration(60_000), "1m");
        assert_eq!(format_duration(3_600_000), "1h 0m");
        assert_eq!(format_duration(16_320_000), "4h 32m");
    }
}
