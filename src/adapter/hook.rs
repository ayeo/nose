use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use crate::adapter::Adapter;
use crate::error::AdapterError;
use crate::event::Event;

pub struct HookAdapter;

impl Adapter for HookAdapter {
    fn name(&self) -> &'static str {
        "hook"
    }

    fn discovery_paths(&self, _cwd: &Path) -> Vec<PathBuf> {
        // Return ~/.nose/events/ - NOT scoped to cwd since hooks write here globally
        if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
            let path = home.join(".nose").join("events");
            if path.exists() {
                vec![path]
            } else {
                vec![]
            }
        } else {
            vec![]
        }
    }

    fn detect(&self, path: &Path) -> bool {
        path.to_string_lossy().contains(".nose/events/")
            && path.extension().is_some_and(|ext| ext == "jsonl")
    }

    fn parse(&self, reader: &mut dyn Read, _session_id: &str, _workspace: &str) -> Result<Vec<Event>, AdapterError> {
        // Passthrough - events are already in unified format
        let buf_reader = BufReader::new(reader);
        let mut events = Vec::new();
        for line in buf_reader.lines() {
            let line_str = line?;
            if line_str.trim().is_empty() {
                continue;
            }
            match serde_json::from_str::<Event>(&line_str) {
                Ok(event) => events.push(event),
                Err(_) => continue, // skip unparseable lines
            }
        }
        Ok(events)
    }
}
