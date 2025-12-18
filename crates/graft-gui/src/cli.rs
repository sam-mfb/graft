use crate::runner::{PatchRunner, ProgressEvent};
use std::io::{self, Write};
use std::path::Path;

/// Run in headless (CLI) mode with embedded patch data
#[allow(dead_code)] // Only used when embedded_patch feature is enabled
pub fn run_headless(
    patch_data: &[u8],
    target_path: &Path,
    skip_confirm: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("Graft Patcher - Headless Mode");
    println!("==============================");

    // Extract patch
    print!("Extracting patch data... ");
    io::stdout().flush()?;

    let runner = PatchRunner::extract(patch_data)?;
    println!("done");

    // Show patch info
    let info = runner.info();
    println!("\nPatch Information:");
    println!("  Version: {}", info.version);
    println!("  Operations: {}", info.entry_count);
    println!("    - {} patches", info.patches);
    println!("    - {} additions", info.additions);
    println!("    - {} deletions", info.deletions);
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

    // Apply patch with progress callback
    println!("\nApplying patch...");

    let result = runner.apply(target_path, |event| {
        match event {
            ProgressEvent::Processing { file, index, total } => {
                print!("  [{}/{}] {}... ", index + 1, total, file);
                let _ = io::stdout().flush();
            }
            ProgressEvent::Done { .. } => {
                // Final "ok" is printed after the last Processing event
            }
            ProgressEvent::Error { .. } => {
                println!("FAILED");
            }
        }
    });

    match result {
        Ok(()) => {
            println!("ok"); // For the last file
            println!("\nPatch applied successfully!");
            Ok(())
        }
        Err(e) => {
            eprintln!("\nError: {}", e);
            std::process::exit(1);
        }
    }
}
