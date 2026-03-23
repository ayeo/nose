use clap::{Parser, Subcommand};
use std::fs::File;
use std::io::stdout;

use nose::adapter::all_adapters;
use nose::discovery::discover_sessions;
use nose::event::Event;
use nose::hooks::handler::run_hook_handler;
use nose::hooks::install::run_install;
use nose::hooks::uninstall::run_uninstall;
use nose::offset::{load_offsets, parse_file_from_offset, save_offsets};
use nose::output::write_events_jsonl;
use nose::stats::Stats;
use nose::watch::run_watch;

#[derive(Parser)]
#[command(name = "nose", about = "Agent Activity Observability")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Parse agent sessions and emit unified JSONL events
    Parse {
        /// Only emit events not yet seen (uses ~/.nose/offsets.json as bookmark)
        #[arg(long)]
        new: bool,
    },
    /// Show a statistics summary of agent activity in the current directory
    Stats,
    /// Stream events in real-time, watching for new agent activity
    Watch,
    /// Manage agent hook configuration
    Hooks {
        #[command(subcommand)]
        action: HookAction,
    },
    /// Handle an agent hook event (called by agents, reads JSON from stdin)
    HookHandler {
        #[arg(long)]
        agent: String,
        #[arg(long)]
        event: String,
    },
}

#[derive(Subcommand)]
enum HookAction {
    /// Install nose hooks into all detected agents
    Install,
    /// Remove nose hooks from all detected agents
    Uninstall,
}

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Commands::Parse { new } => run_parse(new),
        Commands::Stats => run_stats(),
        Commands::Watch => run_watch(),
        Commands::Hooks { action } => match action {
            HookAction::Install => run_install(),
            HookAction::Uninstall => run_uninstall(),
        },
        Commands::HookHandler { agent, event } => run_hook_handler(&agent, &event),
    }
}

/// Iterate over all discovered session events and call a callback for each batch.
fn for_each_session_events<F>(mut callback: F)
where
    F: FnMut(&[Event]),
{
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let adapters = all_adapters();

    for adapter in &adapters {
        let search_paths = adapter.discovery_paths(&cwd);
        let sessions = discover_sessions(&search_paths, adapter.as_ref());

        for session in sessions {
            let file = match File::open(&session.path) {
                Ok(f) => f,
                Err(e) => {
                    eprintln!(
                        "nose: warning: could not open {}: {}",
                        session.path.display(),
                        e
                    );
                    continue;
                }
            };

            let mut reader = file;
            match adapter.parse(&mut reader, &session.session_id, &session.workspace) {
                Ok(events) => callback(&events),
                Err(e) => {
                    eprintln!(
                        "nose: warning: failed to parse {}: {}",
                        session.path.display(),
                        e
                    );
                }
            }
        }
    }
}

fn run_parse(incremental: bool) {
    let mut out = stdout().lock();

    if !incremental {
        for_each_session_events(|events| {
            if let Err(e) = write_events_jsonl(events, &mut out) {
                eprintln!("nose: warning: write error: {}", e);
            }
        });
        return;
    }

    // Incremental mode: only emit events since last bookmark.
    let mut offsets = load_offsets();
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let adapters = all_adapters();

    for adapter in &adapters {
        let search_paths = adapter.discovery_paths(&cwd);
        let sessions = discover_sessions(&search_paths, adapter.as_ref());

        for session in sessions {
            let offset = *offsets.get(&session.path).unwrap_or(&0);

            match parse_file_from_offset(
                &session.path,
                offset,
                adapter.as_ref(),
                &session.session_id,
                &session.workspace,
            ) {
                Ok((events, new_pos)) => {
                    if !events.is_empty() {
                        if let Err(e) = write_events_jsonl(&events, &mut out) {
                            eprintln!("nose: warning: write error: {}", e);
                        }
                    }
                    offsets.insert(session.path.clone(), new_pos);
                }
                Err(e) => {
                    eprintln!(
                        "nose: warning: failed to parse {}: {}",
                        session.path.display(),
                        e
                    );
                }
            }
        }
    }

    save_offsets(&offsets);
}

fn run_stats() {
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let workspace = cwd.display().to_string();
    let mut stats = Stats::new();

    for_each_session_events(|events| {
        for event in events {
            stats.add_event(event);
        }
    });

    stats.display(&workspace);
}
