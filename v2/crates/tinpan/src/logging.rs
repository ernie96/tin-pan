//! A small append-only log next to Tin Pan 1's: tinpan2.log in the "logs" folder of Tin Pan's local data
//! (%LOCALAPPDATA%\Tin Pan on Windows), or TINPAN_LOG when set. Over 1 MB at start, it becomes .old.
//! Never log tokens, cookies, or paths with the user name in them.

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;

use log::{Level, LevelFilter, Log, Metadata, Record};

const MAX_BYTES: u64 = 1024 * 1024;

struct FileLog(Mutex<Option<File>>);

impl Log for FileLog {
    fn enabled(&self, metadata: &Metadata) -> bool {
        // Only Tin Pan's own lines; libraries' chatter stays out.
        metadata.level() <= Level::Info && metadata.target().starts_with("tinpan")
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        if let Ok(mut file) = self.0.lock()
            && let Some(file) = file.as_mut()
        {
            // One line per entry, so text from Suno can't fake log lines.
            let message = record.args().to_string().replace(['\r', '\n'], " ");
            let _ = writeln!(file, "{} {} {message}", chrono::Local::now().format("%Y-%m-%d %H:%M:%S"), record.level());
        }
    }

    fn flush(&self) {
        if let Ok(mut file) = self.0.lock()
            && let Some(file) = file.as_mut()
        {
            let _ = file.flush();
        }
    }
}

pub fn path() -> PathBuf {
    match std::env::var("TINPAN_LOG") {
        Ok(path) if !path.is_empty() => PathBuf::from(path),
        _ => dirs::data_local_dir()
            .unwrap_or_default()
            .join(if cfg!(any(windows, target_os = "macos")) { "Tin Pan" } else { "tin-pan" })
            .join("logs")
            .join("tinpan2.log"),
    }
}

/// Starts logging. Logging never stops the app: if the file can't be opened, nothing is written.
pub fn init() {
    let path = path();
    if let Some(dir) = path.parent() {
        let _ = fs::create_dir_all(dir);
    }
    if fs::metadata(&path).is_ok_and(|m| m.len() > MAX_BYTES) {
        let _ = fs::rename(&path, path.with_extension("log.old"));
    }
    static LOGGER: FileLog = FileLog(Mutex::new(None));
    if let Ok(mut file) = LOGGER.0.lock() {
        *file = OpenOptions::new().create(true).append(true).open(&path).ok();
    }
    if log::set_logger(&LOGGER).is_ok() {
        log::set_max_level(LevelFilter::Info);
    }
}
