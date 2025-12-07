use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "game-localizer")]
#[command(about = "Tools for binary patching game files")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Compare two files by their SHA-256 hash
    Compare {
        /// First file to compare
        file1: PathBuf,
        /// Second file to compare
        file2: PathBuf,
    },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Compare { file1, file2 } => {
            game_localizer::commands::compare::run(file1, file2);
        }
    }
}
