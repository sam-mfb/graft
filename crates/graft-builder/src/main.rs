use std::path::PathBuf;
use std::process;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "graft-builder")]
#[command(about = "Build self-contained GUI patchers from graft patches")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Build a GUI patcher executable from a patch directory
    Build {
        /// Path to the patch directory (containing manifest.json)
        patch_dir: PathBuf,

        /// Output directory for the built executable
        #[arg(short, long, default_value = "./dist")]
        output: PathBuf,

        /// Name for the patcher executable (without extension)
        #[arg(short, long)]
        name: Option<String>,
    },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Build {
            patch_dir,
            output,
            name,
        } => match graft_builder::build(&patch_dir, &output, name.as_deref()) {
            Ok(output_path) => {
                println!("Built patcher: {}", output_path.display());
            }
            Err(e) => {
                eprintln!("Error: {}", e);
                process::exit(1);
            }
        },
    }
}
