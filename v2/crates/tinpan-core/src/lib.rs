//! Everything in Tin Pan that isn't the window: Suno's API and sign-in, song files, settings and
//! translations. No user interface here, so it can be tested on its own and used on any platform.

pub mod api;
pub mod billing;
pub mod cleanup;
pub mod clerk;
pub mod clip;
pub mod config;
pub mod cookies;
pub mod create;
pub mod discover;
pub mod drm;
pub mod error;
pub mod files;
pub mod i18n;
pub mod json;
pub mod link;
pub mod lyrics;
pub mod personas;
pub mod playlist;
pub mod report;
pub mod secrets;
pub mod update;
pub mod upload;

pub use error::{Error, Result};

/// Tin Pan's version, as in Cargo.toml.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
