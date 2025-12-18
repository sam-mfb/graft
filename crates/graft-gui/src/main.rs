mod cli;
mod gui;
mod runner;

use clap::Parser;
use std::path::PathBuf;

/// Graft Patcher - GUI application for applying patches
#[derive(Parser, Debug)]
#[command(name = "graft-patcher")]
#[command(about = "Apply patches with a graphical interface")]
struct Args {
    /// Run in demo mode with mock data (for development/testing)
    #[arg(long)]
    demo: bool,

    /// Run in headless (CLI) mode instead of GUI
    #[arg(long)]
    headless: Option<PathBuf>,

    /// Skip confirmation prompt in headless mode
    #[arg(short = 'y', long)]
    yes: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    if let Some(target_path) = args.headless {
        run_headless(&target_path, args.yes)
    } else if args.demo {
        gui::run(None).map_err(|e| e.into())
    } else {
        run_gui()
    }
}

/// Run the GUI with embedded patch data
fn run_gui() -> Result<(), Box<dyn std::error::Error>> {
    // In a real generated patcher, this would be:
    // const PATCH_DATA: &[u8] = include_bytes!("../patch_data.tar.gz");
    // gui::run(Some(PATCH_DATA))

    // For development, just run in demo mode
    eprintln!("No embedded patch data. Running in demo mode.");
    eprintln!("Use --demo to explicitly run demo mode, or build with graft-builder to embed a patch.");
    gui::run(None).map_err(|e| e.into())
}

/// Run in headless (CLI) mode
fn run_headless(target_path: &PathBuf, skip_confirm: bool) -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(feature = "embedded_patch")]
    {
        const PATCH_DATA: &[u8] = include_bytes!("../patch_data.tar.gz");
        return cli::run_headless(PATCH_DATA, target_path, skip_confirm);
    }

    #[cfg(not(feature = "embedded_patch"))]
    {
        let _ = (target_path, skip_confirm); // Suppress unused warnings
        eprintln!("Error: No embedded patch data available.");
        eprintln!("Headless mode requires a patcher built with graft-builder.");
        std::process::exit(1);
    }
}
