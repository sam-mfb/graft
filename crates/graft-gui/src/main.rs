#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

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
//! ## 2. Patcher Stub / Self-Appending Binary
//!
//! This binary can be used as a "stub" for creating patchers. Patch data
//! can be appended to the end of the executable, and the patcher will
//! read it at runtime. This allows creating patchers without recompilation.
//!
//! The binary format for self-appending:
//! ```text
//! [executable] + [patch.tar.gz] + [size: u64 LE] + [magic: "GRAFTPCH"]
//! ```
//!
//! Alternatively, the `embedded_patch` feature can be used for compile-time
//! embedding via `include_bytes!`.
//!
//! ## Modes
//!
//! - **GUI mode** (default): graphical interface with embedded/appended patch data
//! - **Demo mode** (automatic): if no patch data is found, runs with mock data
//! - **Headless apply**: `graft-gui headless apply <path>` - CLI-only for scripting
//! - **Headless rollback**: `graft-gui headless rollback <path>` - undo a patch

mod cli;
mod gui;
mod runner;
mod self_read;
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
        Some(Command::Headless { action }) => match action {
            HeadlessAction::Apply { path, yes } => run_headless(&path, yes),
            HeadlessAction::Rollback { path, force } => run_rollback(&path, force),
        },
        None => run_gui(),
    }
}

/// Get patch data from compile-time embedding or runtime self-reading.
///
/// Priority:
/// 1. Compile-time embedded data (if `embedded_patch` feature is enabled)
/// 2. Runtime self-reading (appended data at end of executable)
fn get_patch_data() -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    // Try compile-time embedded data first
    #[cfg(feature = "embedded_patch")]
    {
        const PATCH_DATA: &[u8] = include_bytes!(env!("GRAFT_PATCH_ARCHIVE"));
        return Ok(PATCH_DATA.to_vec());
    }

    // Fall back to runtime self-reading
    #[cfg(not(feature = "embedded_patch"))]
    {
        self_read::read_appended_data().map_err(|e| e.into())
    }
}

/// Run the GUI application
///
/// If no patch data is embedded/appended, automatically runs in demo mode.
fn run_gui() -> Result<(), Box<dyn std::error::Error>> {
    match get_patch_data() {
        Ok(data) => gui::run(Some(&data)).map_err(|e| e.into()),
        Err(_) => {
            // No patch data - run in demo mode
            gui::run(None).map_err(|e| e.into())
        }
    }
}

/// Run in headless (CLI) mode
fn run_headless(target_path: &PathBuf, skip_confirm: bool) -> Result<(), Box<dyn std::error::Error>> {
    match get_patch_data() {
        Ok(data) => cli::run_headless(&data, target_path, skip_confirm),
        Err(e) => {
            eprintln!("Error: {}", e);
            eprintln!("Headless mode requires patch data.");
            std::process::exit(1);
        }
    }
}

/// Run rollback in headless (CLI) mode
fn run_rollback(target_path: &PathBuf, force: bool) -> Result<(), Box<dyn std::error::Error>> {
    match get_patch_data() {
        Ok(data) => cli::run_rollback(&data, target_path, force),
        Err(e) => {
            eprintln!("Error: {}", e);
            eprintln!("Rollback mode requires patch data.");
            std::process::exit(1);
        }
    }
}
