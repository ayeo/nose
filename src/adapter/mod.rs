pub mod claude;

use std::io::Read;
use std::path::{Path, PathBuf};
use crate::error::AdapterError;
use crate::event::Event;

pub trait Adapter {
    fn name(&self) -> &'static str;
    fn discovery_paths(&self) -> Vec<PathBuf>;
    fn detect(&self, path: &Path) -> bool;
    fn parse(&self, reader: &mut dyn Read, session_id: &str, workspace: &str) -> Result<Vec<Event>, AdapterError>;
}

pub fn all_adapters() -> Vec<Box<dyn Adapter>> {
    vec![
        Box::new(claude::ClaudeAdapter),
    ]
}
