use crate::runner::{PatchRunner, ProgressAction, ProgressEvent};
use crate::validator::PatchValidator;
use std::io::{self, Write};
use std::path::Path;

fn format_action(action: ProgressAction) -> &'static str {
    match action {
        ProgressAction::Validating => "Validating",
        ProgressAction::CheckingNotExists => "Checking",
        ProgressAction::BackingUp => "Backing up",
        ProgressAction::Skipping => "Skipping",
        ProgressAction::Patching => "Patching",
        ProgressAction::Adding => "Adding",
        ProgressAction::Deleting => "Deleting",
        ProgressAction::Restoring => "Restoring",
        ProgressAction::Removing => "Removing",
    }
}

/// Run in headless (CLI) mode with embedded patch data
pub fn run_headless(
    patch_data: &[u8],
    target_path: &Path,
    skip_confirm: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("Graft Patcher - Headless Mode");
    println!("==============================");

    // Validate patch and get info
    print!("Validating patch data... ");
    io::stdout().flush()?;

    let info = PatchValidator::validate(patch_data)?;
    println!("done");

    // Show patch info
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

    // Create runner and apply patch
    println!("\nApplying patch...");

    let runner = PatchRunner::new(patch_data)?;
    let result = runner.apply(target_path, |event| match event {
        ProgressEvent::PhaseStarted { phase } => {
            println!("\n{}...", phase);
        }
        ProgressEvent::Operation {
            file,
            index,
            total,
            action,
        } => {
            println!("  [{}/{}] {}: {}", index + 1, total, format_action(action), file);
        }
        ProgressEvent::Done { files_patched } => {
            println!("\n{} files processed.", files_patched);
        }
        ProgressEvent::Error { .. } => {
            // Error details will be printed by the result handler below
        }
    });

    match result {
        Ok(()) => {
            println!("\nPatch applied successfully!");
            Ok(())
        }
        Err(e) => {
            eprintln!("\nError: {}", e);
            std::process::exit(1);
        }
    }
}
