# Multi-File Patch System

## Overview

Implement a "patch" system for creating and applying patches across collections of binary files. This builds on our existing single-file "diff" operations.

**Design choices:**

- In-place modification when applying (with backup/rollback)
- Flat directories only (no recursion)
- All files processed (no filtering)

## CLI Commands

```
game-localizer patch create <original-dir> <new-dir> <patch-output-dir>
game-localizer patch apply <target-dir> <patch-dir>
```

## Patch Asset Structure

```
<patch-output-dir>/
├── manifest.json
├── diffs/
│   ├── game.bin.diff
│   └── config.bin.diff
└── files/
    └── new_asset.bin
```

## Manifest Schema

```json
{
  "version": 1,
  "entries": [
    {
      "file": "game.bin",
      "operation": "patch",
      "original_hash": "abc123...",
      "diff_hash": "def456...",
      "final_hash": "ghi789..."
    },
    {
      "file": "new_asset.bin",
      "operation": "add",
      "final_hash": "jkl012..."
    },
    {
      "file": "old_asset.bin",
      "operation": "delete",
      "original_hash": "mno345..."
    }
  ]
}
```

## Create Workflow

1. Scan both directories for files
2. Categorize each file:
   - **patch**: exists in both, hashes differ
   - **add**: only in new directory
   - **delete**: only in original directory
   - (unchanged files: same hash → skip, no manifest entry)
3. For each "patch" file: create diff, write to `diffs/<filename>.diff`
4. For each "add" file: copy to `files/<filename>`
5. Write manifest.json with all entries and hashes

## Apply Workflow

### Validation Phase

- All files to patch must exist in target and match `original_hash`
- Files to add must NOT already exist in target
- Files to delete: if present, must match `original_hash` (no deleting unexpected files); if absent, skip silently

### Backup Phase

- Create `<target-dir>/.patch-backup/` directory
- Copy all files to patch and delete into backup

### Apply Phase (with inline verification)

For each entry, in order:
1. Apply the operation (patch/add/delete)
2. Immediately verify the result (hash check for patch/add)
3. If verification fails → rollback and abort

This ensures we stop at first failure rather than applying all patches before discovering an error.

### Rollback (on any failure)

- Restore all files from backup
- Remove any newly added files
- Return error (backup directory preserved for debugging)

### Success

- Keep backup directory (enables future patch uninstallation)
- Return success

## Module Structure

```
src/
├── commands/           # CLI wrappers
├── utils/              # low-level utilities
│   ├── diff.rs         # existing: create_diff, apply_diff
│   ├── hash.rs         # existing: hash_bytes
│   ├── manifest.rs     # types + JSON serialization
│   ├── dir_scan.rs     # directory listing and comparison
│   └── file_ops.rs     # backup_file, restore_file
├── patch/              # patch orchestration
│   ├── mod.rs
│   ├── apply.rs        # apply_entry
│   └── verify.rs       # verify_entry
```

### `src/utils/manifest.rs`

Types:
```rust
pub enum Operation { Patch, Add, Delete }

pub struct ManifestEntry {
    pub file: String,
    pub operation: Operation,
    pub original_hash: Option<String>,  // present for Patch, Delete
    pub diff_hash: Option<String>,      // present for Patch only
    pub final_hash: Option<String>,     // present for Patch, Add
}

pub struct Manifest {
    pub version: u32,
    pub entries: Vec<ManifestEntry>,
}
```

Functions:
- `Manifest::load(path: &Path) -> io::Result<Manifest>`
- `Manifest::save(&self, path: &Path) -> io::Result<()>`

### `src/utils/dir_scan.rs`

Functions:
- `list_files(dir: &Path) -> io::Result<Vec<String>>` - list filenames (not paths) in directory
- `categorize_files(orig_dir: &Path, new_dir: &Path) -> io::Result<Vec<ManifestEntry>>` - compare directories and return categorized entries with hashes

### `src/utils/file_ops.rs`

Low-level file operations:
- `backup_file(file: &Path, backup_dir: &Path) -> Result<(), PatchError>` - copy file to backup
- `restore_file(file: &Path, backup_dir: &Path) -> Result<(), PatchError>` - restore file from backup

### `src/patch/apply.rs`

Orchestrates applying a single manifest entry:
- `apply_entry(entry: &ManifestEntry, target_dir: &Path, patch_dir: &Path) -> Result<(), PatchError>`

### `src/patch/verify.rs`

Orchestrates verifying a single manifest entry:
- `verify_entry(entry: &ManifestEntry, target_dir: &Path) -> Result<(), PatchError>`

### `src/commands/patch_create.rs`

- `pub fn run(orig: &Path, new: &Path, output: &Path) -> io::Result<()>`

Orchestrates:
1. `dir_scan::categorize_files()` to get entries
2. For each patch entry: `diff::create_diff()` and write to diffs/
3. For each add entry: copy file to files/
4. `Manifest::save()` to write manifest.json

### `src/commands/patch_apply.rs`

- `pub fn run(target: &Path, patch: &Path) -> Result<(), PatchError>`

Orchestrates:
1. `Manifest::load()` to read manifest
2. Validate all entries against target directory
3. Backup all files to be modified/deleted
4. For each entry: `apply_entry()` then `verify_entry()`
5. On failure: rollback using `restore_file()`

## Files to Modify

### `src/utils/mod.rs`

```rust
pub mod diff;
pub mod dir_scan;
pub mod file_ops;
pub mod hash;
pub mod manifest;
```

### `src/patch/mod.rs` (new)

```rust
pub mod apply;
pub mod verify;

// Constants for patch directory structure
pub const DIFFS_DIR: &str = "diffs";
pub const FILES_DIR: &str = "files";
pub const DIFF_EXTENSION: &str = ".diff";
pub const MANIFEST_FILENAME: &str = "manifest.json";
pub const BACKUP_DIR: &str = ".patch-backup";
```

### `src/commands/mod.rs`

```rust
pub mod calculate;
pub mod check;
pub mod compare;
pub mod diff_apply;
pub mod diff_create;
pub mod patch_apply;
pub mod patch_create;
```

### `src/lib.rs`

```rust
pub mod commands;
pub mod patch;
pub mod utils;
```

### `src/main.rs`

Add `Commands::Patch` with `PatchCommands::Create` and `PatchCommands::Apply`

### `Cargo.toml`

Add dependencies:
- `serde = { version = "1", features = ["derive"] }`
- `serde_json = "1"`

## Error Handling

All errors use String reasons for consistency and human-readable CLI output:

```rust
pub enum PatchError {
    ValidationFailed { file: String, reason: String },
    BackupFailed { file: String, reason: String },
    ApplyFailed { file: String, reason: String },
    VerificationFailed { file: String, expected: String, actual: String },
    RollbackFailed { reason: String },
    ManifestError { reason: String },
}
```

## Implementation Phases

### Phase 1: Foundation (Types + Serialization)

**Build:**
- Add serde/serde_json dependencies to Cargo.toml
- Create `src/utils/manifest.rs` with `Operation`, `ManifestEntry`, `Manifest` types
- Implement `Manifest::load()` and `Manifest::save()`
- Update `src/utils/mod.rs`

**Test:**
- Roundtrip serialization (save then load)
- Load from valid JSON string
- Handle missing/malformed JSON

**Review:** Verify JSON schema matches spec above

---

### Phase 2: Directory Scanning

**Build:**
- Create `src/utils/dir_scan.rs`
- Implement `list_files(dir: &Path) -> io::Result<Vec<String>>`
- Implement `categorize_files(orig_dir: &Path, new_dir: &Path) -> io::Result<Vec<ManifestEntry>>`

**Test:**
- `list_files` returns only files (not directories)
- `categorize_files` correctly identifies: patch, add, delete, unchanged (skipped)
- Empty directories
- Non-existent directories error

**Review:** Verify categorization logic matches spec

---

### Phase 3: File Operations

**Build:**
- Create `src/utils/file_ops.rs`
- Implement `backup_file(file: &Path, backup_dir: &Path) -> Result<(), PatchError>`
- Implement `restore_file(file: &Path, backup_dir: &Path) -> Result<(), PatchError>`

**Test:**
- Backup copies file correctly
- Restore replaces file correctly
- Backup creates directory if needed
- Error on missing source file

**Review:** Verify error messages are clear

---

### Phase 4: Patch Creation Command

**Build:**
- Create `src/commands/patch_create.rs`
- Implement `run(orig: &Path, new: &Path, output: &Path) -> io::Result<()>`
- Creates `diffs/` and `files/` subdirectories
- Writes manifest.json
- Update `src/commands/mod.rs`

**Test:**
- Creates correct directory structure
- Diffs are valid (can be applied)
- Added files are copied correctly
- Manifest contains all expected entries with correct hashes

**Review:** Manual test with sample directories

---

### Phase 5: Patch Orchestration

**Build:**
- Create `src/patch/mod.rs` with `PatchError` and constants
- Create `src/patch/apply.rs` with `apply_entry()`
- Create `src/patch/verify.rs` with `verify_entry()`
- Update `src/lib.rs`

**Test:**
- `apply_entry` for patch operation
- `apply_entry` for add operation
- `apply_entry` for delete operation
- `verify_entry` passes on correct hash
- `verify_entry` fails on incorrect hash

**Review:** Verify error types are used correctly

---

### Phase 5b: Patch Orchestration Fixes

**Build:**
- Add constants to `src/patch/mod.rs`: `DIFFS_DIR`, `FILES_DIR`, `DIFF_EXTENSION`, `MANIFEST_FILENAME`, `BACKUP_DIR`
- Update `src/patch/apply.rs` to use constants instead of hardcoded strings
- Update `src/patch/apply.rs` to validate file existence before operations:
  - Use `ValidationFailed` for missing files (precondition not met)
  - Use `ApplyFailed` for operation errors (permission denied, disk full, etc.)
- Update `src/commands/patch_create.rs` to use constants

**Test:**
- Verify `ValidationFailed` returned when file missing
- Verify `ApplyFailed` returned when operation fails (not precondition)

---

### Phase 6: Patch Application Command

**Build:**
- Create `src/commands/patch_apply.rs`
- Implement `run(target: &Path, patch: &Path) -> Result<(), PatchError>`
- Validation phase
- Backup phase
- Apply+verify loop
- Rollback on failure
- Update `src/commands/mod.rs`

**Test:**
- Successful apply modifies target correctly
- Validation rejects missing files
- Validation rejects hash mismatch
- Rollback restores original state on failure
- Already-deleted files don't cause error

**Review:** Manual end-to-end test

---

### Phase 7: CLI Integration

**Build:**
- Update `src/main.rs` with `Commands::Patch` and `PatchCommands`

**Test:**
- `cargo run -- patch create` works
- `cargo run -- patch apply` works
- Error messages display correctly

**Review:** Verify help text is clear

---

### Phase 8: Documentation

**Build:**
- Update README.md with patch commands

**Review:** Verify examples work
