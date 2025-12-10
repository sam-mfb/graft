use std::path::PathBuf;
use std::process;

use clap::{Parser, Subcommand};
use game_localizer::commands::check::CheckResult;

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
    /// Check if a file matches a SHA-256 hash
    Check {
        /// Hash to reference
        hash: String,
        /// File to compare
        file: PathBuf,
    }
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Compare { file1, file2 } => {
            match game_localizer::commands::compare::run(&file1, &file2) {
                Ok(result) => {
                    println!("{}: {}", file1.display(), result.hash1);
                    println!("{}: {}", file2.display(), result.hash2);
                    if result.matches {
                        println!("Files match");
                    } else {
                        println!("Files differ");
                        process::exit(1);
                    }
                }
                Err(e) => {
                    eprintln!("Error: {}", e);
                    process::exit(2);
                }
            }
        }
        Commands::Check { hash, file } => {
            match game_localizer::commands::check::run(&hash,&file) {
                Ok(result) => {
                    match result {
                        CheckResult::Match() => {
                        println!("Hash match");
                        }
                        CheckResult::NoMatch {actual} => {
                        println!("Hashes differ");
                        println!("Expected hash: {}",hash);
                        println!("Actual Hash: {}",actual);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Error: {}", e);
                    process::exit(2);
                }
      }
    }
    }
}
