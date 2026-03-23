use std::fs;

use super::config::all_agents;

pub fn run_install() {
    // 1. Create events directory
    let home = dirs::home_dir().expect("could not determine home directory");
    let events_dir = home.join(".nose").join("events");
    if let Err(e) = fs::create_dir_all(&events_dir) {
        eprintln!("nose: failed to create {}: {}", events_dir.display(), e);
        return;
    }
    println!("Created {}", events_dir.display());

    // 2. Resolve nose binary path
    let nose_bin = match std::env::current_exe() {
        Ok(p) => p.to_string_lossy().to_string(),
        Err(e) => {
            eprintln!("nose: could not determine binary path: {}", e);
            return;
        }
    };

    // 3. Install hooks for each detected agent
    let agents = all_agents();
    for agent in &agents {
        if !agent.is_installed() {
            println!("Skipping {} (not installed)", agent.name());
            continue;
        }

        match agent.install_hooks(&nose_bin) {
            Ok(msg) => println!("{}", msg),
            Err(e) => eprintln!("nose: warning: {}: {}", agent.name(), e),
        }
    }

    println!("Done.");
}

#[cfg(test)]
mod tests {
    use crate::hooks::config::all_agents;

    #[test]
    fn test_all_agents_returns_five() {
        let agents = all_agents();
        assert_eq!(agents.len(), 5);
    }

    #[test]
    fn test_agent_names() {
        let agents = all_agents();
        let names: Vec<&str> = agents.iter().map(|a| a.name()).collect();
        assert!(names.contains(&"Claude Code"));
        assert!(names.contains(&"Codex CLI"));
        assert!(names.contains(&"Gemini CLI"));
        assert!(names.contains(&"Cursor"));
        assert!(names.contains(&"GitHub Copilot"));
    }
}
