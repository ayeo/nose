pub mod claude;
pub mod codex;
pub mod gemini;
pub mod hook;

use std::io::Read;
use std::path::{Path, PathBuf};
use crate::error::AdapterError;
use crate::event::Event;

pub trait Adapter {
    fn name(&self) -> &'static str;
    /// Return discovery paths scoped to the given working directory
    fn discovery_paths(&self, cwd: &Path) -> Vec<PathBuf>;
    fn detect(&self, path: &Path) -> bool;
    fn parse(&self, reader: &mut dyn Read, session_id: &str, workspace: &str) -> Result<Vec<Event>, AdapterError>;
}

pub fn all_adapters() -> Vec<Box<dyn Adapter>> {
    vec![
        Box::new(claude::ClaudeAdapter),
        Box::new(codex::CodexAdapter),
        Box::new(gemini::GeminiAdapter),
        Box::new(hook::HookAdapter),
    ]
}
