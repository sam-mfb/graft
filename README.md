# graft

Binary patching toolkit for creating and applying patches to files.

## Quick Start

```bash
# 1. Create a patch from original and modified directories
graft patch create original/ modified/ my-patch/ --title "My Game Patcher"

# 2. Create self-contained patchers for distribution
graft build create my-patch/ --target linux-x64 -o my-patcher-linux
graft build create my-patch/ --target windows-x64 -o my-patcher.exe
graft build create my-patch/ --target macos-arm64 -o my-patcher-macos

# 3. End users just run the patcher
./my-patcher-linux                              # GUI mode
./my-patcher-linux headless apply /target -y   # CLI mode
```

## Project Structure

```
graft/
├── crates/
│   ├── graft-core/      # Shared library (patching logic)
│   ├── graft/           # CLI tool
│   └── graft-gui/       # GUI patcher application (also serves as stub for patchers)
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

### Path Restrictions

By default, patches are blocked from modifying sensitive locations to prevent misuse:

**Blocked paths:**
- Path traversal (`../` sequences)
- System directories (`/usr`, `/bin`, `/etc`, `C:\Windows`, etc.)
- macOS `.app` bundles
- Executable files (`.exe`, `.dll`, `.so`, `.dylib`, `.sh`, `.bin`, etc.)

To create a patch that can target these locations (for trusted use cases):
```bash
graft patch create original/ modified/ my-patch/ --allow-restricted
```

This sets `"allow_restricted": true` in the manifest. Without this flag, patches default to `allow_restricted: false` and will be rejected if they attempt to modify restricted paths.

## GUI Patcher

The `graft-gui` crate provides a graphical patcher application.

### Demo Mode

The GUI automatically runs in demo mode with mock data when no patch data is embedded/appended:
```
cargo run -p graft-gui
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

**Windows Note:** When the patcher is double-clicked, stdout/stderr are not connected (Windows GUI subsystem). For scripted use, run from a terminal or use the main `graft` CLI.

**macOS Note:** For .app bundles, the binary is inside the bundle:
```
./MyPatcher.app/Contents/MacOS/MyPatcher headless apply /path/to/game
```

### Features

- **Pre-validation**: Validates target files before applying (both GUI and headless)
- **Already-patched detection**: Detects if folder was previously patched and offers rollback
- **Automatic rollback**: On apply failure, automatically restores from backup
- **Backup management**: After rollback, option to delete or keep backup files

## Building Self-Contained Patchers

The `graft build` command creates standalone patcher executables by appending patch data to pre-built stub binaries. No Rust toolchain required!

### Usage

```bash
# List available target platforms
graft build targets

# Create a patcher for the current platform
graft build create <patch-dir> [-o <output-file>]

# Create a patcher for a specific target
graft build create <patch-dir> --target <target> [-o <output-file>]
```

### Example

```bash
# Create a patch with a custom window title
graft patch create original/ modified/ my-patch/ --title "My Game Patcher"

# Build a self-contained patcher
graft build create my-patch/ -o my-patcher

# The resulting binary can be distributed and run:
./my-patcher                              # GUI mode
./my-patcher headless apply /target -y   # CLI mode (apply)
./my-patcher headless rollback /target   # CLI mode (rollback)
```

### Available Targets

| Name | Platform |
|------|----------|
| `linux-x64` | Linux x86_64 |
| `linux-arm64` | Linux ARM64 |
| `windows-x64` | Windows x86_64 |
| `macos-x64` | macOS x86_64 |
| `macos-arm64` | macOS ARM64 (Apple Silicon) |

### Cross-Platform Example

```bash
# Create patchers for multiple platforms
graft build create my-patch/ --target linux-x64 -o my-patcher-linux
graft build create my-patch/ --target windows-x64 -o my-patcher.exe
graft build create my-patch/ --target macos-arm64 -o my-patcher-macos
```

### How It Works

Patchers are created using a "self-appending binary" approach:

1. The `graft` CLI includes pre-built stub binaries for all supported platforms
2. When you run `graft patcher create`, your patch data (tar.gz archive) is appended to the appropriate stub
3. At runtime, the patcher reads the appended data from itself

This means you can create patchers for any platform from any platform - no cross-compilation needed!

### Customizing Patchers

#### Window Title

Set a custom window title when creating the patch:
```bash
graft patch create original/ modified/ my-patch/ --title "My Game Patcher"
```

Or edit `my-patch/manifest.json` directly to change the `"title"` field.

#### Custom Icon

Replace the default icon by placing your own PNG file in the patch folder:
```
my-patch/
  manifest.json
  diffs/
  files/
  .graft_assets/
    icon.png          # Your custom icon (1024x1024 recommended)
```

The icon is automatically embedded when building patchers:
- **Windows**: Embedded as the application icon (visible in Explorer)
- **macOS**: Converted to .icns and included in the .app bundle

If no custom icon is provided, a default graft icon is used.

## Development

### Building from Source

```bash
cargo build --release -p graft
```

When building from source without the `embedded-stubs` feature, stubs are downloaded from GitHub releases on first use and cached in `~/.cache/graft/stubs/`.

### Build Features

| Feature | Description |
|---------|-------------|
| (default) | Downloads stubs from GitHub releases on demand |
| `native-stub` | Embeds only the current platform's stub (Linux only) |
| `embedded-stubs` | Embeds all platform stubs (used for releases) |

**Note:** The `native-stub` feature only works on Linux. On macOS and Windows, stubs are downloaded from GitHub releases on first use. This is because macOS requires `.app` bundles which are more complex to embed during local development.

### Stub Version (Development)

When using downloaded stubs, they come from the latest release by default. To pin to a specific version:

```bash
GRAFT_STUB_VERSION=0.1.0 graft patcher create my-patch/ -o my-patcher
```
