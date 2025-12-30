//! # graft-gui
//!
//! This crate serves two purposes:
//!
//! ## 1. Development & Demo
//!
//! Run directly for UI development and testing:
//! ```bash
//! cargo run -p graft-gui -- demo
//! ```
//!
//! ## 2. Template for Generated Patchers
//!
//! The `graft-builder` tool uses this crate as a template to generate standalone
//! patcher executables. It compiles this crate with:
//! - The `embedded_patch` feature enabled
//! - The `GRAFT_PATCH_ARCHIVE` env var pointing to the patch archive
//!
//! The generated patcher is a self-contained executable that users can run to
//! apply a specific patch - no separate patch files needed.
//!
//! ## Modes
//!
//! - **GUI mode** (default): `graft-gui` - graphical interface
//! - **Demo mode**: `graft-gui demo` - GUI with mock data for development
//! - **Headless apply**: `graft-gui headless apply <path>` - CLI-only for scripting
//! - **Headless rollback**: `graft-gui headless rollback <path>` - undo a patch

#[cfg(feature = "embedded_patch")]
mod cli;
mod gui;
mod runner;
mod validator;

use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "graft-gui")]
#[command(about = "GUI/CLI patcher application")]
struct Args {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Run in demo mode with mock data (for development/testing)
    Demo,

    /// Run in headless (CLI) mode instead of GUI
    Headless {
        #[command(subcommand)]
        action: HeadlessAction,
    },
}

#[derive(Subcommand, Debug)]
enum HeadlessAction {
    /// Apply the patch to a target directory
    Apply {
        /// Target directory to apply the patch to
        path: PathBuf,

        /// Skip confirmation prompt
        #[arg(short = 'y', long)]
        yes: bool,
    },

    /// Rollback a previously applied patch
    Rollback {
        /// Target directory to rollback
        path: PathBuf,

        /// Force rollback even if files have been modified
        #[arg(short, long)]
        force: bool,
    },
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    match args.command {
        Some(Command::Demo) => run_gui(true),
        Some(Command::Headless { action }) => match action {
            HeadlessAction::Apply { path, yes } => run_headless(&path, yes),
            HeadlessAction::Rollback { path, force } => run_rollback(&path, force),
        },
        None => run_gui(false),
    }
}

/// Run the GUI application
fn run_gui(is_demo: bool) -> Result<(), Box<dyn std::error::Error>> {
    if is_demo {
        return gui::run(None).map_err(|e| e.into());
    }

    #[cfg(feature = "embedded_patch")]
    {
        const PATCH_DATA: &[u8] = include_bytes!(env!("GRAFT_PATCH_ARCHIVE"));
        return gui::run(Some(PATCH_DATA)).map_err(|e| e.into());
    }

    #[cfg(not(feature = "embedded_patch"))]
    {
        eprintln!("Error: No embedded patch data available.");
        eprintln!("This binary was not built with an embedded patch.");
        eprintln!();
        eprintln!("Options:");
        eprintln!("  - Use 'demo' subcommand for testing: graft-gui demo");
        eprintln!("  - Build a patcher with graft-builder to embed a patch");
        std::process::exit(1);
    }
}

/// Run in headless (CLI) mode
fn run_headless(target_path: &PathBuf, skip_confirm: bool) -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(feature = "embedded_patch")]
    {
        const PATCH_DATA: &[u8] = include_bytes!(env!("GRAFT_PATCH_ARCHIVE"));
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

/// Run rollback in headless (CLI) mode
fn run_rollback(target_path: &PathBuf, force: bool) -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(feature = "embedded_patch")]
    {
        const PATCH_DATA: &[u8] = include_bytes!(env!("GRAFT_PATCH_ARCHIVE"));
        return cli::run_rollback(PATCH_DATA, target_path, force);
    }

    #[cfg(not(feature = "embedded_patch"))]
    {
        let _ = (target_path, force); // Suppress unused warnings
        eprintln!("Error: No embedded patch data available.");
        eprintln!("Headless mode requires a patcher built with graft-builder.");
        std::process::exit(1);
    }
}
