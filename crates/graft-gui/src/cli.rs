use crate::runner::{PatchRunner, ProgressAction, ProgressEvent, RollbackEvent};
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

    // Create runner for validation checks
    let runner = PatchRunner::new(patch_data)?;

    // Check if already patched (backup exists)
    if PatchRunner::has_backup(target_path) {
        eprintln!("\nError: This folder appears to already be patched.");
        eprintln!("A backup directory (.patch-backup) was found.");
        eprintln!();
        eprintln!("To rollback the patch, run:");
        eprintln!("  {} headless rollback {}", std::env::args().next().unwrap_or_default(), target_path.display());
        std::process::exit(1);
    }

    // Pre-validate target folder
    print!("\nValidating target folder... ");
    io::stdout().flush()?;

    if let Err(e) = runner.validate_target(target_path) {
        println!("failed");
        eprintln!("\nError: Target folder cannot be patched.");
        eprintln!("{}", e);
        std::process::exit(1);
    }
    println!("done");

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
            println!();
            println!("To rollback later, run:");
            println!("  {} headless rollback {}", std::env::args().next().unwrap_or_default(), target_path.display());
            Ok(())
        }
        Err(e) => {
            eprintln!("\nError: {}", e);
            std::process::exit(1);
        }
    }
}

/// Run rollback in headless (CLI) mode
pub fn run_rollback(
    patch_data: &[u8],
    target_path: &Path,
    force: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("Graft Patcher - Headless Rollback");
    println!("==================================");
    println!("\nTarget: {}", target_path.display());

    // Create runner
    let runner = PatchRunner::new(patch_data)?;

    // Check if backup exists
    if !PatchRunner::has_backup(target_path) {
        eprintln!("\nError: No backup directory found.");
        eprintln!("Cannot rollback without .patch-backup directory.");
        std::process::exit(1);
    }

    println!("\nRolling back...");

    let mut error_occurred = false;
    let result = runner.rollback(target_path, force, |event| match event {
        RollbackEvent::ValidatingTarget => {
            print!("Validating target files... ");
            let _ = io::stdout().flush();
        }
        RollbackEvent::ValidatingBackup => {
            println!("done");
            print!("Validating backup... ");
            let _ = io::stdout().flush();
        }
        RollbackEvent::TargetModified { reason } => {
            println!("failed");
            eprintln!("\nError: Target files have been modified since patching.");
            eprintln!("{}", reason);
            eprintln!();
            eprintln!("To force rollback anyway, run:");
            eprintln!("  {} headless rollback --force {}", std::env::args().next().unwrap_or_default(), target_path.display());
            error_occurred = true;
        }
        RollbackEvent::Rolling { file, index, total, action } => {
            if index == 0 {
                println!("done\n");
            }
            println!("  [{}/{}] {}: {}", index + 1, total, format_action(action), file);
        }
        RollbackEvent::Done { files_restored } => {
            println!("\n{} files restored.", files_restored);
        }
        RollbackEvent::Error { message } => {
            eprintln!("\nError: {}", message);
            error_occurred = true;
        }
    });

    if error_occurred {
        std::process::exit(1);
    }

    match result {
        Ok(()) => {
            println!("\nRollback complete!");

            // Ask about deleting backup
            print!("\nDelete backup directory? [y/N] ");
            io::stdout().flush()?;

            let mut input = String::new();
            io::stdin().read_line(&mut input)?;
            if input.trim().eq_ignore_ascii_case("y") {
                if let Err(e) = PatchRunner::delete_backup(target_path) {
                    eprintln!("Warning: Failed to delete backup: {}", e);
                } else {
                    println!("Backup deleted.");
                }
            } else {
                println!("Backup preserved.");
            }

            Ok(())
        }
        Err(e) => {
            eprintln!("\nError: {}", e);
            std::process::exit(1);
        }
    }
}
