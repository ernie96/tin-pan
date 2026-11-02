use embed_manifest::manifest::{ActiveCodePage, DpiAwareness, Setting, SupportedOS::*};
use embed_manifest::{embed_manifest, new_manifest};

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    // prism, built static for background speech, leaves its Orca backend's libraries (glibmm and giomm, over
    // D-Bus) to the final link. Without them installed, prism builds without that backend and needs nothing.
    if std::env::var("TARGET").unwrap_or_default().contains("linux") && std::env::var_os("CARGO_FEATURE_BACKGROUND_SPEECH").is_some() {
        for library in ["giomm-2.68", "glibmm-2.68"] {
            if let Err(e) = pkg_config::Config::new().atleast_version("2.68").probe(library) {
                println!("cargo:warning=No {library}, so speech can't reach Orca while Tin Pan is in the background: {e}");
            }
        }
    }
    if std::env::var("TARGET").unwrap_or_default().contains("windows") {
        // Common Controls 6 (themed, and what screen readers expect), UTF-8, per-monitor DPI, long paths.
        let manifest = new_manifest("TinPan")
            .supported_os(Windows7..=Windows10)
            .active_code_page(ActiveCodePage::Utf8)
            .dpi_awareness(DpiAwareness::PerMonitorV2)
            .long_path_aware(Setting::Enabled);
        if let Err(e) = embed_manifest(manifest) {
            println!("cargo:warning=Couldn't embed the Windows manifest: {e}");
        }
    }
}
