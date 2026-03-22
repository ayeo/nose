use clap::{Parser, Subcommand};
use std::fs::File;
use std::io::stdout;

use nose::adapter::all_adapters;
use nose::discovery::discover_sessions;
use nose::output::write_events_jsonl;

#[derive(Parser)]
#[command(name = "nose", about = "Agent Activity Observability")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Parse agent sessions and emit unified JSONL events
    Parse,
}

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Commands::Parse => run_parse(),
    }
}

fn run_parse() {
    let adapters = all_adapters();
    let mut out = stdout().lock();

    for adapter in &adapters {
        let search_paths = adapter.discovery_paths();
        let sessions = discover_sessions(&search_paths, adapter.as_ref());

        for session in sessions {
            let file = match File::open(&session.path) {
                Ok(f) => f,
                Err(e) => {
                    eprintln!("nose: warning: could not open {}: {}", session.path.display(), e);
                    continue;
                }
            };

            let mut reader = file;
            match adapter.parse(&mut reader, &session.session_id, &session.workspace) {
                Ok(events) => {
                    if let Err(e) = write_events_jsonl(&events, &mut out) {
                        eprintln!("nose: warning: write error: {}", e);
                    }
                }
                Err(e) => {
                    eprintln!("nose: warning: failed to parse {}: {}", session.path.display(), e);
                }
            }
        }
    }
}
