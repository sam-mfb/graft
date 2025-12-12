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
    /// Hash-related operations
    Hash {
        #[command(subcommand)]
        command: HashCommands,
    },
    Patch {
        #[command(subcommand)]
        command: PatchCommands,
    },
}

#[derive(Subcommand)]
enum PatchCommands {
    /// Create a patch from two files
    Create {
        /// Original file
        orig: PathBuf,
        /// Modified file
        new: PathBuf,
        /// Path to write patch file to
        patch: PathBuf,
    },
    /// Apply a patch to a file
    Apply {
        /// Original file
        orig: PathBuf,
        /// Patch file
        patch: PathBuf,
        /// Path to write output file to
        output: PathBuf,
    },
}

#[derive(Subcommand)]
enum HashCommands {
    /// Calculate the SHA-256 hash of a file
    Calculate {
        /// File to hash
        file: PathBuf,
    },
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
    },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Patch { command } => match command {
            PatchCommands::Create { orig, new, patch } => {
                match game_localizer::commands::patch_create::run(&orig, &new, &patch) {
                    Ok(()) => {
                        println!("Patch written to {}", patch.display());
                    }
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        process::exit(2);
                    }
                }
            }
            PatchCommands::Apply { orig, patch, output } => {
                match game_localizer::commands::patch_apply::run(&orig, &patch, &output) {
                    Ok(()) => {
                        println!("Patched file written to {}", output.display());
                    }
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        process::exit(2);
                    }
                }
            }
        }
        Commands::Hash { command } => match command {
            HashCommands::Calculate { file } => {
                match game_localizer::commands::calculate::run(&file) {
                    Ok(result) => {
                        println!("Hash for file {}: {}", file.display(), result);
                    }
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        process::exit(2);
                    }
                }
            }
            HashCommands::Compare { file1, file2 } => {
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
            HashCommands::Check { hash, file } => {
                match game_localizer::commands::check::run(&hash, &file) {
                    Ok(result) => match result {
                        CheckResult::Match => {
                            println!("Hash match");
                        }
                        CheckResult::NoMatch { actual } => {
                            println!("Hashes differ");
                            println!("Expected hash: {}", hash);
                            println!("Actual hash: {}", actual);
                        }
                    },
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        process::exit(2);
                    }
                }
            }
        },
    }
}
