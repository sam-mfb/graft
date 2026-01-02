# graft

Binary patching toolkit for creating and applying patches to files.

## Quick Start

```bash
# 1. Create a patch from original and modified directories
graft patch create original/ modified/ my-patch/ -v 1 --name MyPatcher --title "My Game Patcher"

# 2. Create self-contained patchers for distribution
#    -o specifies the OUTPUT DIRECTORY (not filename)
graft build my-patch/ -o ./output                             # All available platforms
graft build my-patch/ -o ./output --target linux-x64          # Single platform
graft build my-patch/ -o ./output -t linux-x64 -t windows-x64 # Multiple platforms

# 3. End users just run the patcher (filenames derived from --name)
./output/MyPatcher-linux-x64                              # GUI mode
./output/MyPatcher-linux-x64 headless apply /target -y   # CLI mode
```

## Project Structure

```
graft/
├── crates/
│   ├── graft-core/      # Shared library (patching logic)
│   ├── graft/           # CLI tool
│   ├── graft-gui/       # GUI patcher application (also serves as stub for patchers)
│   └── graft-icon/      # Icon conversion utility (PNG to ICNS/ICO)
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
graft patch create <original-dir> <modified-dir> <patch-output-dir> -v <version> --name <patcher-name>
```

The `--name` argument specifies the base name for the patcher executable (e.g., "MyPatcher" produces "MyPatcher-linux-x64").

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
- Executable files (`.exe`, `.dll`, `.so`, `.dylib`, `.sh`, etc.)

To create a patch that can target these locations (for trusted use cases):
```bash
graft patch create original/ modified/ my-patch/ -v 1 --allow-restricted
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
./MyPatcher-macos-arm64.app/Contents/MacOS/graft-gui headless apply /path/to/game
```

### Features

- **Pre-validation**: Validates target files before applying (both GUI and headless)
- **Already-patched detection**: Detects if folder was previously patched and offers rollback
- **Automatic rollback**: On apply failure, automatically restores from backup
- **Backup management**: After rollback, option to delete or keep backup files

## Building Self-Contained Patchers

The `graft build` command creates standalone patcher executables by appending patch data to pre-built stub binaries. No Rust toolchain required!

Check your graft version to see if you're in production or development mode:

```bash
graft --version
# graft 0.5.0 (production)  - has embedded stubs
# graft 0.5.0 (development) - requires --stub-dir
```

### Production (with embedded stubs)

When built with `--features embedded-stubs`, graft has all platform stubs embedded:

```bash
graft build ./my-patch -o ./output                              # Build for all platforms
graft build ./my-patch -o ./output --target linux-x64           # Single target
graft build ./my-patch -o ./output -t linux-x64 -t windows-x64  # Multiple targets
graft build ./my-patch -o ./output --stub-dir ./custom          # Override with custom stubs
```

**Note:** The `-o` option specifies an output **directory**, not a filename. Patcher files are created inside this directory with names derived from the `--name` specified during patch creation:
- `./output/MyPatcher-linux-x64`
- `./output/MyPatcher-windows-x64.exe`
- `./output/MyPatcher-macos-arm64.app/`

### Development (without embedded stubs)

Development builds require `--stub-dir` pointing to stub binaries:

```bash
# First, build the GUI stub
cargo build -p graft-gui --release
mkdir -p ./stubs
cp target/release/graft-gui ./stubs/graft-gui-stub-linux-x64

# Then build patchers
graft build ./my-patch -o ./output --stub-dir ./stubs
graft build ./my-patch -o ./output --stub-dir ./stubs --target linux-x64
```

Stub files must follow the naming convention:
- `graft-gui-stub-linux-x64`
- `graft-gui-stub-linux-arm64`
- `graft-gui-stub-windows-x64.exe`
- `graft-gui-stub-macos-x64.app.zip`
- `graft-gui-stub-macos-arm64.app.zip`

### Example

```bash
# Create a patch with a custom patcher name and window title
graft patch create original/ modified/ my-patch/ -v 1 --name MyPatcher --title "My Game Patcher"

# Build self-contained patchers (production mode)
graft build my-patch/ -o ./output

# The resulting binaries can be distributed and run:
./output/MyPatcher-linux-x64                              # GUI mode
./output/MyPatcher-linux-x64 headless apply /target -y   # CLI mode (apply)
./output/MyPatcher-linux-x64 headless rollback /target   # CLI mode (rollback)
```

### Available Targets

| Name | Platform |
|------|----------|
| `linux-x64` | Linux x86_64 |
| `linux-arm64` | Linux ARM64 |
| `windows-x64` | Windows x86_64 |
| `macos-x64` | macOS x86_64 |
| `macos-arm64` | macOS ARM64 (Apple Silicon) |

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
graft patch create original/ modified/ my-patch/ -v 1 --title "My Game Patcher"
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

When building from source without the `embedded-stubs` feature (development mode), you must provide stub binaries via `--stub-dir`.

### Build Features

| Feature | Description |
|---------|-------------|
| (default) | Development mode - requires `--stub-dir` argument |
| `embedded-stubs` | Embeds all platform stubs (used for releases) |

### Building with Embedded Stubs (CI/Release)

To build graft with embedded stubs for distribution:

```bash
# Prepare stubs directory with all platform stubs
mkdir -p ./stubs
# ... copy stub binaries from CI artifacts ...

# Build graft with embedded stubs
GRAFT_STUBS_DIR=./stubs cargo build -p graft --release --features embedded-stubs
```

The resulting binary will show "production" in version output and won't require `--stub-dir`.
