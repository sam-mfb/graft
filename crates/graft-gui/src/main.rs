mod app;

use clap::Parser;
use graft_core::patch::{apply::apply_entry, MANIFEST_FILENAME};
use graft_core::utils::manifest::Manifest;
use std::io::{self, Write};
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
        run_headless(target_path, args.yes)
    } else if args.demo {
        run_gui_demo()
    } else {
        run_gui()
    }
}

/// Run the GUI in demo mode
fn run_gui_demo() -> Result<(), Box<dyn std::error::Error>> {
    app::run(None).map_err(|e| e.into())
}

/// Run the GUI with embedded patch data
fn run_gui() -> Result<(), Box<dyn std::error::Error>> {
    // In a real generated patcher, this would be:
    // const PATCH_DATA: &[u8] = include_bytes!("../patch_data.tar.gz");
    // app::run(Some(PATCH_DATA))

    // For development, just run in demo mode
    eprintln!("No embedded patch data. Running in demo mode.");
    eprintln!("Use --demo to explicitly run demo mode, or build with graft-builder to embed a patch.");
    app::run(None).map_err(|e| e.into())
}

/// Run in headless (CLI) mode
fn run_headless(
    #[allow(unused_variables)] target_path: PathBuf,
    #[allow(unused_variables)] skip_confirm: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    // In a real generated patcher, this would use embedded data
    // For now, we'll show an error since there's no embedded patch

    #[cfg(feature = "embedded_patch")]
    {
        const PATCH_DATA: &[u8] = include_bytes!("../patch_data.tar.gz");
        return run_headless_with_data(PATCH_DATA, target_path, skip_confirm);
    }

    #[cfg(not(feature = "embedded_patch"))]
    {
        eprintln!("Error: No embedded patch data available.");
        eprintln!("Headless mode requires a patcher built with graft-builder.");
        std::process::exit(1);
    }
}

#[allow(dead_code)]
fn run_headless_with_data(
    patch_data: &[u8],
    target_path: PathBuf,
    skip_confirm: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    use flate2::read::GzDecoder;
    use tar::Archive;

    println!("Graft Patcher - Headless Mode");
    println!("==============================");

    // Extract patch to temp directory
    print!("Extracting patch data... ");
    io::stdout().flush()?;

    let temp_dir = tempfile::tempdir()?;
    let decoder = GzDecoder::new(patch_data);
    let mut archive = Archive::new(decoder);
    archive.unpack(temp_dir.path())?;

    println!("done");

    // Load manifest
    let manifest_path = temp_dir.path().join(MANIFEST_FILENAME);
    let manifest = Manifest::load(&manifest_path)?;

    println!("\nPatch Information:");
    println!("  Version: {}", manifest.version);
    println!("  Operations: {}", manifest.entries.len());
    println!("\nTarget: {}", target_path.display());

    // Confirm unless -y flag
    if !skip_confirm {
        print!("\nApply patch? [y/N] ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Aborted.");
            return Ok(());
        }
    }

    // Apply patch
    println!("\nApplying patch...");
    let total = manifest.entries.len();
    for (i, entry) in manifest.entries.iter().enumerate() {
        let file = entry.file();
        print!("  [{}/{}] {}... ", i + 1, total, file);
        io::stdout().flush()?;

        match apply_entry(entry, &target_path, temp_dir.path()) {
            Ok(()) => println!("ok"),
            Err(e) => {
                println!("FAILED");
                eprintln!("\nError: {}", e);
                std::process::exit(1);
            }
        }
    }

    println!("\nPatch applied successfully!");
    Ok(())
}
