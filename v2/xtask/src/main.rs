//! Packaging tasks. `cargo xtask macos`, after `cargo build --release -p tinpan`, makes Tin Pan.app and
//! TinPan.dmg in target/release, the way fedra packages its macOS build.

use std::error::Error;
use std::path::{Path, PathBuf};

fn main() -> Result<(), Box<dyn Error>> {
    match std::env::args().nth(1).as_deref() {
        Some("macos") => macos(),
        _ => {
            println!("Tasks:");
            println!("    macos    Tin Pan.app and TinPan.dmg from the release build (on macOS)");
            Ok(())
        }
    }
}

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().map(Path::to_path_buf).unwrap_or_default()
}

#[cfg(target_os = "macos")]
fn macos() -> Result<(), Box<dyn Error>> {
    use shipfitter::macos::{DeveloperId, MacApp, dmg, sign_bundle};

    let release = root().join("target/release");
    let binary = release.join("tinpan");
    if !binary.exists() {
        return Err("Build it first: cargo build --release -p tinpan".into());
    }
    let app = MacApp {
        name: "Tin Pan",
        identifier: "io.github.ernie96.tinpan",
        executable: "tinpan",
        version: env!("CARGO_PKG_VERSION"),
        // macOS 13 or later, as the build targets.
        extra: "\t<key>LSMinimumSystemVersion</key>\n\t<string>13.0</string>\n",
        ..MacApp::default()
    };
    let bundle = app.bundle(&release, &binary, &[], &[])?;
    // Signed with a Developer ID when one is set up (MACOS_SIGN_IDENTITY); otherwise the bundle keeps its ad hoc
    // signature, and macOS asks people to allow Tin Pan in System Settings the first time.
    match DeveloperId::from_env() {
        Some(signer) => sign_bundle(&bundle, &[], &signer)?,
        None => println!("No signing identity, so Tin Pan.app keeps its ad hoc signature."),
    }
    println!("Built {}", bundle.display());
    let image = release.join("TinPan.dmg");
    dmg(&bundle, &image)?;
    println!("Built {}", image.display());
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn macos() -> Result<(), Box<dyn Error>> {
    let _ = root();
    Err("The macOS app can only be packaged on macOS.".into())
}
