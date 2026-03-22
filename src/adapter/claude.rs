use std::io::Read;
use std::path::{Path, PathBuf};
use crate::adapter::Adapter;
use crate::error::AdapterError;
use crate::event::Event;

pub struct ClaudeAdapter;

impl Adapter for ClaudeAdapter {
    fn name(&self) -> &'static str { "claude" }
    fn discovery_paths(&self) -> Vec<PathBuf> { vec![] }
    fn detect(&self, _path: &Path) -> bool { false }
    fn parse(&self, _reader: &mut dyn Read, _session_id: &str, _workspace: &str) -> Result<Vec<Event>, AdapterError> {
        Ok(vec![])
    }
}
