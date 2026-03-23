use super::config::all_agents;

pub fn run_uninstall() {
    let agents = all_agents();
    for agent in &agents {
        if !agent.is_installed() {
            println!("Skipping {} (not installed)", agent.name());
            continue;
        }

        match agent.uninstall_hooks() {
            Ok(msg) => println!("{}", msg),
            Err(e) => eprintln!("nose: warning: {}: {}", agent.name(), e),
        }
    }

    println!("Done. (event files in ~/.nose/events/ were not removed)");
}
