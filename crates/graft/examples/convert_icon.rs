//! Convert default_icon.png to AppIcon.icns for graft-gui stub bundles.

use graft::commands::macos_bundle::convert_png_to_icns;
use std::path::Path;

fn main() {
    let png_path = Path::new("crates/graft/assets/default_icon.png");
    let icns_path = Path::new("crates/graft-gui/assets/AppIcon.icns");

    match convert_png_to_icns(png_path, icns_path) {
        Ok(()) => println!("Created {}", icns_path.display()),
        Err(e) => eprintln!("Error: {}", e),
    }
}
