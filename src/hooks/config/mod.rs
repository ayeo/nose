pub mod claude;
pub mod codex;
pub mod gemini;

use std::path::PathBuf;

/// Represents an agent whose hooks we can manage.
pub trait AgentConfig {
    /// Human-readable agent name.
    fn name(&self) -> &'static str;

    /// Path to the agent's config file for hooks.
    fn config_path(&self) -> PathBuf;

    /// Whether the agent appears to be installed (config dir exists).
    fn is_installed(&self) -> bool {
        self.config_path().parent().is_some_and(|p| p.exists())
    }

    /// Install nose-managed hooks into the agent config.
    /// `nose_bin` is the absolute path to the nose binary.
    /// Returns a description of what was installed.
    fn install_hooks(&self, nose_bin: &str) -> Result<String, String>;

    /// Remove nose-managed hooks from the agent config.
    /// Returns a description of what was removed.
    fn uninstall_hooks(&self) -> Result<String, String>;
}

pub fn all_agents() -> Vec<Box<dyn AgentConfig>> {
    vec![
        Box::new(claude::ClaudeConfig),
        Box::new(codex::CodexConfig),
        Box::new(gemini::GeminiConfig),
    ]
}
