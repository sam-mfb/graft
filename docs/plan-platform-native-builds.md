# Plan: Platform-Native Patcher Builds

## Overview

Make graft produce proper platform-native applications:
- Windows: GUI apps (no console window)
- macOS: .app bundles
- Rename `patcher` subcommand to `build`
- Auto-demo mode for bare stubs

---

## A. Auto-Demo Mode for Bare Stubs

**Goal:** When graft-gui runs without embedded patch data, automatically launch demo mode instead of requiring `demo` subcommand.

### Changes

**`crates/graft-gui/src/main.rs`:**
- Modify startup logic:
  1. Try to read embedded patch data
  2. If found → run patcher
  3. If not found → run demo mode automatically
- Remove `demo` subcommand from CLI (no longer needed)

**Benefits:**
- Double-click bare stub → see demo UI
- Double-click patcher (stub + patch) → run patcher
- No CLI needed for basic usage

---

## B. Headless Mode Caveats

**Goal:** Keep `headless` subcommand but document platform limitations.

### README Documentation

Add note:
```
### Headless Mode (CLI)

The `headless` subcommand applies patches without GUI:

    ./patcher headless --target /path/to/game

**Windows Note:** When built as a GUI application (default for releases),
stdout/stderr are not connected when run from a terminal. For scripted
use on Windows, run from PowerShell or use the main `graft` CLI tool.

**macOS Note:** For .app bundles, run the binary inside the bundle directly:

    ./MyPatcher.app/Contents/MacOS/MyPatcher headless --target /path/to/game
```

No code changes required.

---

## C. Platform-Native Builds

### C1. Windows GUI Subsystem

**Goal:** Windows patchers launch without console window.

**`crates/graft-gui/src/main.rs`:**
```rust
#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
```

This is a compile-time attribute applied to the stub. When users double-click the .exe, no console window appears.

### C2. macOS .app Bundles

**Goal:** `graft build` creates .app bundles for macOS targets.

**New module: `crates/graft/src/commands/build/macos_bundle.rs`:**

```rust
pub fn create_app_bundle(
    binary_path: &Path,
    output_path: &Path,
    app_name: &str,
    version: &str,
) -> Result<(), Error>
```

Creates:
```
MyPatcher.app/
  Contents/
    MacOS/
      MyPatcher          (the patched binary)
    Info.plist           (generated from template)
    Resources/
      (optional: icon.icns)
```

**Info.plist template:**
```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "...">
<plist version="1.0">
<dict>
    <key>CFBundleExecutable</key>
    <string>{app_name}</string>
    <key>CFBundleIdentifier</key>
    <string>com.graft.patcher.{app_name}</string>
    <key>CFBundleName</key>
    <string>{app_name}</string>
    <key>CFBundleVersion</key>
    <string>{version}</string>
    <key>CFBundleShortVersionString</key>
    <string>{version}</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>LSMinimumSystemVersion</key>
    <string>10.13</string>
    <key>NSHighResolutionCapable</key>
    <true/>
</dict>
</plist>
```

### C3. Update Build Command Logic

**`crates/graft/src/commands/build.rs`** (renamed from patcher_create.rs):

```rust
pub fn run(patch_dir: &Path, target: &Target, output: &Path) -> Result<()> {
    // 1. Get stub for target
    let stub = stubs::get_stub(target)?;

    // 2. Create patched binary (stub + patch data)
    let patched = create_patched_binary(stub, patch_dir)?;

    // 3. Platform-specific output
    match target.os {
        "macos" => {
            // Create .app bundle
            let app_name = output.file_stem().unwrap_or("Patcher");
            macos_bundle::create_app_bundle(&patched, output, app_name, version)?;
        }
        _ => {
            // Windows/Linux: just write the binary
            fs::write(output, patched)?;
        }
    }
}
```

---

## D. Rename `patcher` to `build`

### Changes

**`crates/graft/src/main.rs`:**
```rust
enum Commands {
    // ... existing commands ...

    /// Build a self-contained patcher application
    Build {
        /// Directory containing patch files
        patch_dir: PathBuf,
        /// Target platform (e.g., windows-x64, macos-arm64)
        target: String,
        /// Output path
        #[arg(short, long)]
        output: PathBuf,
    },
}
```

**Rename files:**
- `commands/patcher_create.rs` → `commands/build.rs`

**Update mod.rs and imports accordingly.**

---

## Files to Modify/Create

| File | Action |
|------|--------|
| `crates/graft-gui/src/main.rs` | Add windows_subsystem, auto-demo logic |
| `crates/graft/src/main.rs` | Rename patcher → build |
| `crates/graft/src/commands/patcher_create.rs` | Rename to build.rs, add bundle logic |
| `crates/graft/src/commands/build/macos_bundle.rs` | New: .app bundle creation |
| `README.md` | Document headless caveats |

---

## Output Conventions

| Target | Output |
|--------|--------|
| `windows-x64` | `MyPatcher.exe` (GUI subsystem) |
| `linux-x64` | `MyPatcher` (executable) |
| `macos-arm64` | `MyPatcher.app/` (bundle) |
| `macos-x64` | `MyPatcher.app/` (bundle) |

---

## Future Considerations (Not in Scope)

- Code signing / notarization (separate CI workflow)
- Custom icons for .app bundles
- Unpatch/restore functionality
