use std::path::PathBuf;
use std::process;

use clap::{Parser, Subcommand};
use graft::commands::check::CheckResult;

#[derive(Parser)]
#[command(name = "graft")]
#[command(about = "Binary patching toolkit")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Diff operations for single files
    Diff {
        #[command(subcommand)]
        command: DiffCommands,
    },
    /// Hash-related operations
    Hash {
        #[command(subcommand)]
        command: HashCommands,
    },
    /// Patch operations for directories
    Patch {
        #[command(subcommand)]
        command: PatchCommands,
    },
    /// Build standalone patcher executables
    Build {
        #[command(subcommand)]
        command: BuildCommands,
    },
}

#[derive(Subcommand)]
enum BuildCommands {
    /// Create a self-contained patcher executable
    Create {
        /// Path to the patch directory (containing manifest.json)
        patch_dir: PathBuf,

        /// Target platform (linux-x64, linux-arm64, windows-x64, macos-x64, macos-arm64)
        #[arg(short, long)]
        target: Option<String>,

        /// Output file path
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// List available target platforms
    Targets,
}

#[derive(Subcommand)]
enum DiffCommands {
    /// Create a diff from two files
    Create {
        /// Original file
        orig: PathBuf,
        /// Modified file
        new: PathBuf,
        /// Path to write diff file to
        diff: PathBuf,
    },
    /// Apply a diff to a file
    Apply {
        /// Original file
        orig: PathBuf,
        /// Diff file
        diff: PathBuf,
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

#[derive(Subcommand)]
enum PatchCommands {
    /// Create a patch from two directories
    Create {
        /// Original directory
        orig: PathBuf,
        /// Modified directory
        new: PathBuf,
        /// Output directory for patch files
        output: PathBuf,
        /// Manifest version number
        #[arg(long, default_value = "1")]
        version: u32,
        /// Window title for the patcher application
        #[arg(long)]
        title: Option<String>,
        /// Allow patching restricted paths (system dirs, executables)
        #[arg(long)]
        allow_restricted: bool,
    },
    /// Apply a patch to a target directory
    Apply {
        /// Target directory to patch
        target: PathBuf,
        /// Directory containing patch files
        patch: PathBuf,
    },
    /// Rollback a previously applied patch using backup
    Rollback {
        /// Target directory to restore
        target: PathBuf,
        /// Path to manifest.json (from original patch)
        manifest: PathBuf,
        /// Skip validation of patched files (use when files have been modified)
        #[arg(long, short)]
        force: bool,
    },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Diff { command } => match command {
            DiffCommands::Create { orig, new, diff } => {
                match graft::commands::diff_create::run(&orig, &new, &diff) {
                    Ok(()) => {
                        println!("Diff written to {}", diff.display());
                    }
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        process::exit(2);
                    }
                }
            }
            DiffCommands::Apply { orig, diff, output } => {
                match graft::commands::diff_apply::run(&orig, &diff, &output) {
                    Ok(()) => {
                        println!("Output written to {}", output.display());
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
                match graft::commands::calculate::run(&file) {
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
                match graft::commands::compare::run(&file1, &file2) {
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
                match graft::commands::check::run(&hash, &file) {
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
        }
        Commands::Patch { command } => match command {
            PatchCommands::Create {
                orig,
                new,
                output,
                version,
                title,
                allow_restricted,
            } => {
                match graft::commands::patch_create::run(&orig, &new, &output, version, title.as_deref(), allow_restricted) {
                    Ok(()) => {
                        println!("Patch created at {}", output.display());
                    }
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        process::exit(2);
                    }
                }
            }
            PatchCommands::Apply { target, patch } => {
                match graft::commands::patch_apply::run(&target, &patch) {
                    Ok(()) => {
                        println!("Patch applied successfully");
                    }
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        process::exit(2);
                    }
                }
            }
            PatchCommands::Rollback { target, manifest, force } => {
                match graft::commands::patch_rollback::run(&target, &manifest, force) {
                    Ok(()) => {
                        println!("Rollback complete");
                    }
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        process::exit(2);
                    }
                }
            }
        },
        Commands::Build { command } => match command {
            BuildCommands::Create {
                patch_dir,
                target,
                output,
            } => {
                match graft::commands::build::run(
                    &patch_dir,
                    target.as_deref(),
                    output.as_deref(),
                ) {
                    Ok(()) => {}
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        process::exit(2);
                    }
                }
            }
            BuildCommands::Targets => {
                println!("Available targets:");
                for target in graft::targets::ALL_TARGETS {
                    let current = graft::targets::current_target()
                        .map(|t| t.name == target.name)
                        .unwrap_or(false);
                    let marker = if current { " (current)" } else { "" };
                    println!("  {}{}", target.name, marker);
                }
            }
        },
    }
}
