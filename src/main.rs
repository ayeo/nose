use clap::{Parser, Subcommand};

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
        Commands::Parse => {
            eprintln!("nose: parse not yet implemented");
        }
    }
}
