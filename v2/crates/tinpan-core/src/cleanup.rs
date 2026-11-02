//! Clean-up with sunscreen (built in and pinned to one version, as in Tin Pan 1): it removes the haze and
//! buzz in Suno songs and saves "<name> (clean).wav" next to the download. sunscreen reads MP3 and WAV,
//! so other files (Suno's free M4A) are decoded to a WAV first, by the caller, which has the decoder.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use crate::config::CleanupConfig;
use crate::error::{Error, Result};
use crate::i18n::{t, tf};

pub const VERSION: &str = "0.7.1";

/// SHA-256 of sunscreen-v0.7.1-x86_64-pc-windows-msvc.zip's sunscreen.exe, from its GitHub release.
pub const PINNED_SHA256: &str = "BFEF0C7CDD22EF3C0BDCC669A3F9E58E6BCCE130573BF028C92E7E2EE406B6E3";

/// SHA-256 of the sunscreen that comes next to Tin Pan elsewhere: in Tin Pan.app on macOS (from
/// sunscreen-v0.7.1-aarch64-apple-darwin.tar.gz), in the AppImage on Linux (from the x86_64-unknown-linux-gnu one).
#[cfg(not(windows))]
const PINNED_SHA256_BESIDE: &str = if cfg!(target_os = "macos") {
    "6F3B180A1005F4BEB5A675ED8BD2A107DBCC601129E03B78E9A0D5FE90A1FEE6"
} else {
    "24EC1FFA40B53C1E8D233B1A030665EB8297765364815B74235F81452B966535"
};

const TITLE: &str = "song";
const LIMIT: Duration = Duration::from_secs(15 * 60);

#[cfg(windows)]
const SUNSCREEN: &[u8] = include_bytes!("../../../../deps/sunscreen/sunscreen.exe");

/// "<name> (clean).wav" next to the download.
pub fn cleaned_path(audio: &Path) -> PathBuf {
    let stem = audio.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    audio.with_file_name(format!("{stem} (clean).wav"))
}

/// sunscreen's command line for these options. Numbers always use a dot.
pub fn arguments(input: &Path, out_dir: &Path, o: &CleanupConfig) -> Vec<String> {
    let n = |v: f64| {
        let text = format!("{v:.3}");
        text.trim_end_matches('0').trim_end_matches('.').to_string()
    };
    let mut args = vec![input.display().to_string(), out_dir.display().to_string(), TITLE.to_string()];
    if !o.debuzz {
        args.push("--no-debuzz".into());
    }
    if !o.gate {
        args.push("--no-gate".into());
    }
    if o.smooth {
        args.push(format!("--smooth={}", n(o.smooth_ms)));
    }
    if o.widen {
        args.push(format!("--widen={}", n(o.widen_amount)));
    }
    if o.excite {
        args.push(format!("--excite={}", n(o.excite_db)));
    }
    if o.matching {
        args.push(if o.match_reference.trim().is_empty() { "--match".into() } else { format!("--match={}", o.match_reference) });
    }
    if o.master {
        args.push(format!("--master={}", n(o.master_lufs)));
    }
    args
}

/// The built-in sunscreen, unpacked where Tin Pan 1 unpacks it (so both share one copy) and checked
/// against the pinned hash.
pub fn unpack(native_dir: &Path) -> Result<PathBuf> {
    #[cfg(windows)]
    {
        use sha2::{Digest, Sha256};
        let hash = hex::encode_upper(Sha256::digest(SUNSCREEN));
        if hash != PINNED_SHA256 {
            return Err(Error::api(t("This copy of Tin Pan has no clean-up tool built in.")));
        }
        let path = native_dir.join(&hash[..16]).join("sunscreen.exe");
        let current = std::fs::read(&path).ok().is_some_and(|bytes| hex::encode_upper(Sha256::digest(&bytes)) == hash);
        if !current {
            let written = path.parent().map_or(Ok(()), std::fs::create_dir_all).and_then(|()| std::fs::write(&path, SUNSCREEN));
            if let Err(e) = written {
                log::warn!("Couldn't unpack sunscreen: {e}");
                return Err(Error::api(t("Couldn't unpack the clean-up tool.")));
            }
        }
        Ok(path)
    }
    // Elsewhere it comes next to Tin Pan (inside Tin Pan.app, or the AppImage), checked against its pinned hash too.
    #[cfg(not(windows))]
    {
        use sha2::{Digest, Sha256};
        let _ = native_dir;
        let path = std::env::current_exe().ok().and_then(|exe| exe.parent().map(|dir| dir.join("sunscreen")));
        match path {
            Some(path) if std::fs::read(&path).is_ok_and(|bytes| hex::encode_upper(Sha256::digest(&bytes)) == PINNED_SHA256_BESIDE) => Ok(path),
            _ => Err(Error::api(t("This copy of Tin Pan has no clean-up tool built in."))),
        }
    }
}

/// Cleans up a download. `decode` turns a file sunscreen can't read (M4A) into a WAV at the path it's given.
/// Removes the download afterwards unless the options keep it. Returns the cleaned file's path.
pub fn clean(audio: &Path, options: &CleanupConfig, native_dir: &Path, decode: impl FnOnce(&Path, &Path) -> std::result::Result<(), String>) -> Result<PathBuf> {
    let name = audio.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    if !audio.exists() {
        return Err(Error::api(tf("Couldn't find the downloaded file for {0}.", &[&name])));
    }
    if options.matching && !options.match_reference.trim().is_empty() && !Path::new(options.match_reference.trim()).exists() {
        return Err(Error::api(t("The reference song for tone matching isn't there any more. Choose another in Settings, or clear it.")));
    }
    let exe = unpack(native_dir)?;
    let work = std::env::temp_dir().join("TinPanCleanup").join(uuid::Uuid::new_v4().simple().to_string());
    let result = (|| {
        std::fs::create_dir_all(&work).map_err(|_| Error::Io(t("Couldn't save the cleaned-up file. Check the download folder in Settings.")))?;
        let extension = audio.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
        let input = if extension == "mp3" || extension == "wav" {
            audio.to_path_buf()
        } else {
            let wav = work.join("input.wav");
            decode(audio, &wav).map_err(|e| {
                log::warn!("Couldn't decode .{extension} for clean-up: {e}");
                Error::api(tf("Windows couldn't read this {0} file to clean it up.", &[&extension.to_uppercase()]))
            })?;
            wav
        };
        let out_dir = work.join("out");
        run(&exe, &arguments(&input, &out_dir, options), &name)?;
        // The last stage's file is the one with the most "+" in its name.
        let finished = std::fs::read_dir(&out_dir)
            .ok()
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.file_name().is_some_and(|n| n.to_string_lossy().starts_with(&format!("{TITLE} - Clean")) && n.to_string_lossy().ends_with(".wav")))
            .max_by_key(|p| p.file_name().map_or(0, |n| n.to_string_lossy().matches('+').count()))
            .ok_or_else(|| Error::api(tf("sunscreen didn't write a cleaned-up file for {0}.", &[&name])))?;
        let target = cleaned_path(audio);
        move_file(&finished, &target).map_err(|_| Error::Io(t("Couldn't save the cleaned-up file. Check the download folder in Settings.")))?;
        if !options.keep_original && target != audio {
            let _ = std::fs::remove_file(audio);
        }
        log::info!("Cleaned up with sunscreen {VERSION}");
        Ok(target)
    })();
    if let Err(e) = std::fs::remove_dir_all(&work) {
        log::warn!("Couldn't remove a clean-up work folder: {e}");
    }
    result
}

/// A rename, or a copy when the work folder and the download folder are on different drives.
fn move_file(from: &Path, to: &Path) -> std::io::Result<()> {
    let _ = std::fs::remove_file(to);
    std::fs::rename(from, to).or_else(|_| std::fs::copy(from, to).map(drop))
}

fn run(exe: &Path, args: &[String], name: &str) -> Result<()> {
    let mut command = Command::new(exe);
    command.args(args).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // No console window flashing up.
        command.creation_flags(0x0800_0000);
    }
    let mut child = command.spawn().map_err(|e| {
        log::warn!("Couldn't start sunscreen: {e}");
        Error::api(t("Couldn't start the clean-up tool. Your antivirus may have blocked it."))
    })?;
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if started.elapsed() > LIMIT => {
                let _ = child.kill();
                return Err(Error::api(tf("Cleaning up {0} took too long, so it was stopped.", &[&name])));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(200)),
            Err(e) => {
                log::warn!("Waiting for sunscreen: {e}");
                break;
            }
        }
    }
    let output = child.wait_with_output().map_err(|_| Error::api(tf("sunscreen couldn't clean up {0}. Details are in the log.", &[&name])))?;
    if !output.status.success() {
        let text = String::from_utf8_lossy(&output.stdout).to_string() + &String::from_utf8_lossy(&output.stderr);
        let lines: Vec<&str> = text.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
        log::warn!("sunscreen exited with {}: {}", output.status, lines[lines.len().saturating_sub(3)..].join(" | "));
        return Err(Error::api(tf("sunscreen couldn't clean up {0}. Details are in the log.", &[&name])));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_command_line_is_sunscreens_own() {
        let mut o = CleanupConfig::default();
        let args = arguments(Path::new("in.wav"), Path::new("out"), &o);
        assert_eq!(args, ["in.wav", "out", "song", "--smooth=85", "--widen=1", "--excite=-3", "--match", "--master=-14"]);
        o.debuzz = false;
        o.gate = false;
        o.smooth = false;
        o.widen_amount = 0.5;
        o.match_reference = "ref.mp3".into();
        let args = arguments(Path::new("in.wav"), Path::new("out"), &o);
        assert_eq!(&args[3..], ["--no-debuzz", "--no-gate", "--widen=0.5", "--excite=-3", "--match=ref.mp3", "--master=-14"]);
    }

    #[test]
    fn the_cleaned_file_sits_next_to_the_download() {
        assert_eq!(cleaned_path(Path::new("dir/Song (1a2b).m4a")), Path::new("dir/Song (1a2b) (clean).wav"));
    }

    #[cfg(windows)]
    #[test]
    fn the_built_in_sunscreen_is_the_pinned_one() {
        use sha2::{Digest, Sha256};
        assert_eq!(hex::encode_upper(Sha256::digest(SUNSCREEN)), PINNED_SHA256);
    }
}
