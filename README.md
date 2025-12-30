# graft

Binary patching toolkit for creating and applying patches to files.

## Project Structure

```
graft/
├── crates/
│   ├── graft-core/      # Shared library (patching logic)
│   ├── graft/           # CLI tool
│   ├── graft-gui/       # GUI patcher application
│   └── graft-builder/   # Builder for self-contained patchers
```

## Installation

```
cargo install --path crates/graft
```

## Commands

### Diff

Create a diff:
```
graft diff create <original> <modified> <diff-output>
```

Apply a diff:
```
graft diff apply <original> <diff-file> <output>
```

### Hash

Calculate SHA-256 hash of a file:
```
graft hash calculate <file>
```

Compare two files by hash:
```
graft hash compare <file1> <file2>
```

Check if a file matches an expected hash:
```
graft hash check <hash> <file>
```

### Patch

Create a patch from two directories:
```
graft patch create <original-dir> <modified-dir> <patch-output-dir>
```

This compares the directories and generates:
- `manifest.json` - lists all operations with SHA-256 hashes
- `diffs/` - binary diffs for modified files
- `files/` - copies of newly added files

Apply a patch to a target directory:
```
graft patch apply <target-dir> <patch-dir>
```

This will:
1. Validate all files exist and match expected hashes
2. Backup modified/deleted files to `.patch-backup/`
3. Apply all changes (patch, add, delete)
4. Verify results match expected hashes
5. Rollback automatically on any failure

Rollback a previously applied patch:
```
graft patch rollback <target-dir> <manifest-path> [--force]
```

This restores files from `.patch-backup/` to their original state. The `--force` flag skips validation of target files (use when files have been modified since patching).

## GUI Patcher

The `graft-gui` crate provides a graphical patcher application.

### Demo Mode

Run the GUI with mock data for development/testing:
```
cargo run -p graft-gui -- demo
```

### Headless Mode

When built with embedded patch data, supports CLI mode for scripting:

Apply a patch:
```
./patcher headless apply <target-dir> [-y]
```

Rollback a previously applied patch:
```
./patcher headless rollback <target-dir> [--force]
```

The `--force` flag skips validation of target files (use when files have been modified since patching).

### Features

- **Pre-validation**: Validates target files before applying (both GUI and headless)
- **Already-patched detection**: Detects if folder was previously patched and offers rollback
- **Automatic rollback**: On apply failure, automatically restores from backup
- **Backup management**: After rollback, option to delete or keep backup files

## Building Self-Contained Patchers

The `graft-builder` tool creates standalone patcher executables with embedded patch data.

### Usage

```
graft-builder build <patch-dir> [OPTIONS]

OPTIONS:
    -o, --output <DIR>    Output directory [default: ./dist]
    -n, --name <NAME>     Patcher name [default: patcher]
```

### Example

```bash
# Create a patch
graft patch create original/ modified/ my-patch/

# Build a self-contained patcher
graft-builder build my-patch/ -o dist/ -n my-patcher

# The resulting binary can be distributed and run:
./dist/my-patcher                              # GUI mode
./dist/my-patcher headless apply /target -y   # CLI mode (apply)
./dist/my-patcher headless rollback /target   # CLI mode (rollback)
```
