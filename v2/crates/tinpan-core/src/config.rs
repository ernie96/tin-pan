//! Tin Pan's settings: config.json in %APPDATA%\Tin Pan on Windows (the same file Tin Pan 1 uses, so
//! both read the same settings and sign-in), ~/Library/Application Support/Tin Pan on macOS and
//! ~/.config/tin-pan on Linux. Field names are Tin Pan 1's, and anything 2.0 doesn't know yet is kept
//! as it is, so saving from one version never loses what the other wrote.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

type Extra = BTreeMap<String, Value>;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase", default)]
pub struct AppConfig {
    pub audio: AudioConfig,
    pub user: UserConfig,
    pub library: LibraryConfig,
    /// "" follows the system; otherwise "en", "ro", "uk" or "es".
    pub language: String,
    /// Everything, AskedFor or ErrorsOnly.
    pub speech_level: String,
    /// Click, Say, ClickAndSay or Off: how a song list marks the playing song.
    pub playing_cue: String,
    pub resume: ResumeConfig,
    /// The "mv" key of the model last picked to make a song.
    pub last_model: String,
    pub update: UpdateConfig,
    pub cleanup: CleanupConfig,
    #[serde(flatten)]
    pub extra: Extra,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            audio: AudioConfig::default(),
            user: UserConfig::default(),
            library: LibraryConfig::default(),
            language: String::new(),
            speech_level: "Everything".into(),
            playing_cue: "Click".into(),
            resume: ResumeConfig::default(),
            last_model: String::new(),
            update: UpdateConfig::default(),
            cleanup: CleanupConfig::default(),
            extra: Extra::new(),
        }
    }
}

/// Clean-up with sunscreen. The steps and values are sunscreen's own options; the defaults are its
/// recommended chain and built-in values, as in Tin Pan 1.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase", default)]
pub struct CleanupConfig {
    /// Clean up after Download MP3, Download WAV and Download all. Plain Download asks instead.
    pub after_download: bool,
    /// Keep the downloaded file next to the cleaned-up WAV.
    pub keep_original: bool,
    pub debuzz: bool,
    pub gate: bool,
    pub smooth: bool,
    pub smooth_ms: f64,
    pub widen: bool,
    pub widen_amount: f64,
    pub excite: bool,
    pub excite_db: f64,
    #[serde(rename = "Match")]
    pub matching: bool,
    pub match_reference: String,
    pub master: bool,
    pub master_lufs: f64,
    #[serde(flatten)]
    pub extra: Extra,
}

impl Default for CleanupConfig {
    fn default() -> Self {
        Self {
            after_download: false,
            keep_original: true,
            debuzz: true,
            gate: true,
            smooth: true,
            smooth_ms: 85.0,
            widen: true,
            widen_amount: 1.0,
            excite: true,
            excite_db: -3.0,
            matching: true,
            match_reference: String::new(),
            master: true,
            master_lufs: -14.0,
            extra: Extra::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase", default)]
pub struct AudioConfig {
    /// "mp3" or "wav".
    pub output_format: String,
    pub download_dir: String,
    pub default_volume: f32,
    /// The output songs play on, by its system name; empty is the default output.
    pub output_device: String,
    pub second_output_device: String,
    /// Read timed lyrics aloud as they're sung.
    pub read_lyrics_live: bool,
    /// Play songs in Tin Pan itself, which starts them sooner; Suno's own player takes over when that fails.
    /// Off, every song plays through Suno's player.
    pub play_directly: bool,
    /// When a list ends, keep playing songs Suno picks to follow the last one.
    pub autoplay: bool,
    pub save_lyrics: bool,
    pub save_timed_lyrics: bool,
    pub save_details: bool,
    pub save_cover: bool,
    #[serde(flatten)]
    pub extra: Extra,
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            output_format: "mp3".into(),
            download_dir: String::new(),
            default_volume: 0.8,
            output_device: String::new(),
            second_output_device: String::new(),
            read_lyrics_live: false,
            play_directly: true,
            autoplay: false,
            save_lyrics: false,
            save_timed_lyrics: false,
            save_details: false,
            save_cover: false,
            extra: Extra::new(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase", default)]
pub struct UserConfig {
    /// Suno's cookies, encrypted (see secrets).
    pub protected_cookies: String,
    pub device_id: String,
    /// Set after Sign out, so the next sign-in starts with no saved browser session.
    pub clear_browser_on_next_sign_in: bool,
    #[serde(flatten)]
    pub extra: Extra,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase", default)]
pub struct LibraryConfig {
    /// Empty is every workspace.
    pub workspace_id: String,
    pub workspace_name: String,
    /// "False" hides disliked songs, "Any" shows them, "True" shows only them.
    pub disliked: String,
    /// The other filters, by Suno's field name: "Any", "True" or "False".
    pub show: BTreeMap<String, String>,
    /// newest, oldest, most-liked or least-liked.
    pub sort: Option<String>,
    #[serde(flatten)]
    pub extra: Extra,
}

impl Default for LibraryConfig {
    fn default() -> Self {
        Self {
            workspace_id: String::new(),
            workspace_name: String::new(),
            disliked: "False".into(),
            show: BTreeMap::new(),
            sort: Some("newest".into()),
            extra: Extra::new(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase", default)]
pub struct ResumeConfig {
    pub clip_id: String,
    pub title: String,
    pub seconds: f64,
    #[serde(flatten)]
    pub extra: Extra,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase", default)]
pub struct UpdateConfig {
    pub check_at_startup: bool,
    pub skipped_version: String,
    #[serde(flatten)]
    pub extra: Extra,
}

impl Default for UpdateConfig {
    fn default() -> Self {
        Self { check_at_startup: true, skipped_version: String::new(), extra: Extra::new() }
    }
}

/// The settings folder for this platform.
pub fn settings_dir() -> PathBuf {
    let base = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    base.join(if cfg!(any(windows, target_os = "macos")) { "Tin Pan" } else { "tin-pan" })
}

/// Settings read from and saved to one file.
pub struct ConfigStore {
    path: PathBuf,
    pub current: AppConfig,
}

impl ConfigStore {
    /// The settings in the usual place.
    pub fn open() -> Self {
        Self::at(settings_dir().join("config.json"))
    }

    /// The settings in `path`; defaults if it's missing. A damaged file is set aside, not overwritten.
    pub fn at(path: PathBuf) -> Self {
        let mut current = match fs::read_to_string(&path) {
            Ok(text) => serde_json::from_str(&text).unwrap_or_else(|e| {
                log::error!("Settings file was unreadable ({e}); started with defaults");
                set_aside(&path);
                AppConfig::default()
            }),
            Err(_) => AppConfig::default(),
        };
        if current.user.device_id.is_empty() {
            current.user.device_id = uuid::Uuid::new_v4().to_string();
        }
        Self { path, current }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Writes the settings to a temporary file, then moves it into place, so a crash never leaves half a file.
    pub fn save(&self) -> std::io::Result<()> {
        if let Some(dir) = self.path.parent() {
            fs::create_dir_all(dir)?;
        }
        let temp = self.path.with_extension("json.tmp");
        fs::write(&temp, serde_json::to_string_pretty(&self.current)?)?;
        fs::rename(&temp, &self.path)
    }
}

fn set_aside(path: &Path) {
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let broken = path.with_extension(format!("json.broken-{stamp}"));
    if let Err(e) = fs::rename(path, &broken) {
        log::warn!("Couldn't set the damaged settings aside: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_file(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("tinpan-test-{}-{name}", uuid::Uuid::new_v4()))
    }

    #[test]
    fn reads_tin_pan_1_settings_and_keeps_what_it_doesnt_know() {
        let path = temp_file("config.json");
        fs::write(
            &path,
            r#"{"Audio":{"OutputFormat":"wav","DefaultVolume":0.5,"PlayDirectly":false},"User":{"DeviceId":"d1"},
               "Language":"ro","Cleanup":{"Gate":false},"LastModel":"chirp-fenix"}"#,
        )
        .unwrap();
        let store = ConfigStore::at(path.clone());
        assert_eq!((store.current.audio.output_format.as_str(), store.current.audio.default_volume), ("wav", 0.5));
        assert_eq!((store.current.user.device_id.as_str(), store.current.language.as_str()), ("d1", "ro"));
        store.save().unwrap();
        let saved: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(saved["Cleanup"]["Gate"], Value::Bool(false));
        assert!(!store.current.cleanup.gate && store.current.cleanup.matching, "Tin Pan 1's clean-up settings are read");
        assert_eq!(saved["Audio"]["PlayDirectly"], Value::Bool(false));
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn a_damaged_file_is_set_aside_and_defaults_are_used() {
        let path = temp_file("config.json");
        fs::write(&path, "{ not json").unwrap();
        let store = ConfigStore::at(path.clone());
        assert_eq!(store.current.speech_level, "Everything");
        assert!(!store.current.user.device_id.is_empty());
        assert!(!path.exists(), "the damaged file is moved, not overwritten");
        if let Some(dir) = path.parent() {
            let prefix = path.file_name().unwrap().to_string_lossy().to_string();
            for entry in fs::read_dir(dir).unwrap().flatten() {
                if entry.file_name().to_string_lossy().starts_with(&prefix) {
                    let _ = fs::remove_file(entry.path());
                }
            }
        }
    }
}
