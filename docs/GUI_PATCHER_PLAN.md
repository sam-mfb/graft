# Plan: GUI Patcher Builder

## Overview

Create a tool that takes a patch directory and produces self-contained GUI executables for macOS, Windows, and Linux.

**Workflow:**
1. `game-localizer patch create ...` - Create patch (existing)
2. `game-localizer patch apply ...` - Test patch (existing)
3. `patch-gui-builder build <patch-dir> -o dist/` - Build GUI executables (new)

**User choices:**
- GUI framework: egui/eframe (pure Rust, ~5MB binaries)
- Build approach: Separate `patch-gui-builder` tool
- Cross-platform: Local cross-compile using `cross` (Docker)

## Project Structure

Convert to Cargo workspace:

```
game-localizer/
├── Cargo.toml                    # Workspace root
├── crates/
│   ├── patch-core/               # Shared library (extracted)
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── patch/            # apply.rs, verify.rs, mod.rs
│   │       └── utils/            # manifest.rs, diff.rs, hash.rs, file_ops.rs
│   │
│   ├── game-localizer/           # Existing CLI (moved)
│   │   └── src/
│   │       ├── main.rs
│   │       └── commands/
│   │
│   ├── patch-gui-builder/        # Builder tool (new)
│   │   └── src/
│   │       ├── main.rs           # CLI: build subcommand
│   │       ├── builder.rs        # Orchestrates build process
│   │       ├── archive.rs        # Creates tar.gz of patch
│   │       └── template.rs       # Generates Rust project
│   │
│   └── patcher-gui/              # GUI app template (new)
│       └── src/
│           ├── main.rs           # Entry point with embedded data
│           └── app.rs            # egui application
```

## Embedding Strategy

Embed patch as compressed tar archive:

```rust
// Generated in patcher-gui/src/main.rs
const PATCH_DATA: &[u8] = include_bytes!("../patch_data.tar.gz");

fn main() -> eframe::Result<()> {
    patcher_gui::run(PATCH_DATA)
}
```

At runtime: extract to temp dir, load manifest, apply patch.

## GUI App Design

**States:**
1. **Welcome** - Show patch info, "Select Folder" button
2. **FolderSelected** - Show path, "Apply Patch" button
3. **Applying** - Progress bar, current file
4. **Success** - Green checkmark, done message
5. **Error** - Red X, error details, "Show Details" expander

**Demo mode** (`cargo run -p patcher-gui -- --demo`):
- Uses mock manifest data (no real patch embedded)
- Simulates state transitions without touching filesystem
- For UI development and testing appearance of all states

**Headless mode** (for generated patchers):
```bash
./my-patcher                        # Launch GUI (default)
./my-patcher --headless <target>    # CLI mode, no GUI
./my-patcher --headless <target> -y # Skip confirmation prompt
```
- Same binary supports both GUI and CLI
- Useful for advanced users and E2E testing
- Outputs progress and errors to stdout/stderr

**Localization:**
- UI strings stored in locale files (`locales/en.json`, `locales/ja.json`, etc.)
- Builder embeds selected locales into the patcher
- Runtime auto-detects system locale, falls back to English
- Strings: button labels, status messages, error descriptions

**Key dependencies:**
- `eframe` / `egui` - GUI framework
- `rfd` - Native file dialogs
- `tar` / `flate2` - Archive extraction
- `patch-core` - Patching logic

## CLI Interface

```
patch-gui-builder build <PATCH_DIR> [OPTIONS]

OPTIONS:
    -o, --output <DIR>       Output directory [default: ./dist]
    -n, --name <NAME>        Patcher name [default: from manifest]
    --targets <TARGETS>      linux-x64,linux-arm64,windows,macos-x64,macos-arm64
                             [default: linux-x64,linux-arm64,windows]
    --locales <LOCALES>      Locales to include [default: en]
                             Example: --locales en,ja,es
```

## Implementation Phases

### Phase 1: Workspace Restructure
- Convert to Cargo workspace
- Create `crates/patch-core/` - extract `src/patch/` and `src/utils/`
- Move CLI to `crates/game-localizer/`
- Update imports, verify tests pass

### Phase 2: GUI Runtime (`patcher-gui`)
- Create egui app with state machine
- Implement demo mode with mock data
- Implement patch extraction from embedded tar.gz
- Folder selection with `rfd`
- Progress display during apply
- Success/error views
- Headless mode (`--headless <target>`)

### Phase 3: Builder Tool (`patch-gui-builder`)
- CLI with clap
- Archive creation (tar.gz patch data)
- Template project generation
- Local `cargo build --release` integration

### Phase 4: Cross-Compilation (Linux + Windows)
- Add Cross.toml configuration
- Build orchestration for x86_64 Linux, ARM64 Linux, Windows
- Copy artifacts to output directory

### Phase 5: macOS Support
- Document native build on Mac (`cargo build --release`)
- Add `--targets` flag to select platforms
- Optional: GitHub Actions workflow for all platforms

### Phase 6: Localization
- Add localization framework (e.g., `rust-i18n` or `fluent`)
- Extract all UI strings to locale files
- Support locale selection in builder (`--locale en,ja,es`)
- Auto-detect system locale at runtime
- Fallback to English if locale unavailable

### Phase 7: Polish
- Customization options (name, window title, icon)
- Better error messages
- Documentation

## Key Files to Modify/Create

**Extract to patch-core:**
- `src/patch/mod.rs` → `crates/patch-core/src/patch/mod.rs`
- `src/patch/apply.rs` → `crates/patch-core/src/patch/apply.rs`
- `src/patch/verify.rs` → `crates/patch-core/src/patch/verify.rs`
- `src/utils/*` → `crates/patch-core/src/utils/*`

**New files:**
- `Cargo.toml` - workspace root
- `crates/patch-core/Cargo.toml`
- `crates/game-localizer/Cargo.toml`
- `crates/patch-gui-builder/src/main.rs`
- `crates/patch-gui-builder/src/builder.rs`
- `crates/patch-gui-builder/src/archive.rs`
- `crates/patcher-gui/src/main.rs`
- `crates/patcher-gui/src/app.rs`

## Dependencies

**patch-core:**
```toml
bsdiff = "0.2.1"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
sha2 = "0.10"
```

**patcher-gui:**
```toml
eframe = "0.29"
rfd = "0.15"
tar = "0.4"
flate2 = "1.0"
tempfile = "3"
rust-i18n = "3"                   # Localization
sys-locale = "0.3"                # Detect system locale
patch-core = { path = "../patch-core" }
```

**patch-gui-builder:**
```toml
clap = { version = "4", features = ["derive"] }
tar = "0.4"
flate2 = "1.0"
tempfile = "3"
patch-core = { path = "../patch-core" }
```

## Cross-Compilation Targets

**Phase 1 - via `cross` (Docker):**
| Target | Output Name | Notes |
|--------|-------------|-------|
| x86_64-unknown-linux-gnu | patcher-linux-x64 | Default |
| aarch64-unknown-linux-gnu | patcher-linux-arm64 | ARM64 Linux |
| x86_64-pc-windows-gnu | patcher-windows.exe | Default |

**Phase 2 - macOS:**
| Target | Output Name | Notes |
|--------|-------------|-------|
| x86_64-apple-darwin | patcher-macos-x64 | Intel Mac |
| aarch64-apple-darwin | patcher-macos-arm64 | Apple Silicon |

macOS options:
- Build natively on Mac hardware (`cargo build --release`)
- GitHub Actions (free macOS runners)
- osxcross (complex setup, not recommended)

## Notes

- Start with Linux + Windows via `cross` (works great from Docker)
- Add macOS via native build on Mac or GitHub Actions
- ARM Linux (aarch64) included for Raspberry Pi, AWS Graviton, etc.
