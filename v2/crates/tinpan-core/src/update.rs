//! Updates from GitHub Releases, the same way Tin Pan 1 does them, so Tin Pan 1 can update to 2.0 and 2.0
//! keeps updating itself: the latest release's version tag against ours, one asset (TinPan.exe on Windows)
//! with an Ed25519 ".sig" made with the same key and message, and a helper that swaps the exe once Tin Pan
//! has closed and starts it again. From ship-shape: a mark on the relaunched app so it can come to the front.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use reqwest::blocking::Client;
use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::i18n::{t, tf};

pub const REPOSITORY: &str = "ernie96/tin-pan";

/// The release signing public key, Tin Pan 1's. Its private half is the UPDATE_SIGNING_KEY secret.
pub const PUBLIC_KEY: &str = "dAJzkGbji8/wUluGvr2CG/4IOW7SobcI5WCtrUPoWyw=";

/// Signed messages start with this, so a signature for anything else can't pass.
const DOMAIN: &str = "TinPan-update-v1";

/// The release asset this build updates from. Windows uses Tin Pan 1's, so 1.x and 2.x take the same file.
pub const ASSET_NAME: &str = if cfg!(windows) {
    "TinPan.exe"
} else if cfg!(target_os = "macos") {
    "TinPan.dmg"
} else {
    "TinPan.AppImage"
};

/// Set by the update helper on the Tin Pan it starts, as ship-shape marks its relaunch.
pub const RELAUNCH_ENV: &str = "TINPAN_RELAUNCHED";

/// A version as tags write it: "v2.0.0", "2.0", "2.0.0-alpha.1".
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
    /// A pre-release comes before the release itself ("2.0.0-alpha.1" < "2.0.0"); None sorts after Some.
    released: bool,
    pre: String,
}

impl Version {
    pub fn parse(text: &str) -> Option<Self> {
        let text = text.trim().trim_start_matches(['v', 'V']);
        let (core, pre) = match text.split_once(['-', '+', ' ']) {
            Some((core, rest)) => (core, rest.split('+').next().unwrap_or("").to_string()),
            None => (text, String::new()),
        };
        let mut parts = core.split('.').map(|p| p.parse::<u64>());
        let major = parts.next()?.ok()?;
        let minor = parts.next().unwrap_or(Ok(0)).ok()?;
        let patch = parts.next().unwrap_or(Ok(0)).ok()?;
        Some(Self { major, minor, patch, released: pre.is_empty(), pre })
    }

    /// "2.0.0", the way Tin Pan says it.
    pub fn short(&self) -> String {
        format!("{}.{}.{}", self.major, self.minor, self.patch)
    }
}

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.pre.is_empty() { write!(f, "{}", self.short()) } else { write!(f, "{}-{}", self.short(), self.pre) }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateInfo {
    pub version: Version,
    pub notes: String,
    pub download_url: String,
    pub signature_url: String,
    pub page_url: String,
}

#[derive(Deserialize)]
struct Release {
    tag_name: Option<String>,
    body: Option<String>,
    html_url: Option<String>,
    #[serde(default)]
    assets: Vec<Asset>,
}

#[derive(Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
}

/// The client for GitHub: no 30-second limit, since the exe is big; checks and downloads set their own.
pub fn client() -> Client {
    let _ = rustls::crypto::ring::default_provider().install_default();
    Client::builder().user_agent(format!("TinPan/{}", crate::version())).build().expect("the HTTP client builds")
}

/// The newer release, or None when up to date (or no release yet). Errors are what to say.
pub fn check(http: &Client, current: &Version) -> Result<Option<UpdateInfo>, String> {
    let url = format!("https://api.github.com/repos/{REPOSITORY}/releases/latest");
    let response = http
        .get(&url)
        .header("Accept", "application/vnd.github+json")
        .timeout(Duration::from_secs(15))
        .send()
        .map_err(|e| if e.is_timeout() { t("Checking for updates timed out.") } else { t("Couldn't check for updates. Check your internet connection.") })?;
    if response.status().as_u16() == 404 {
        return Ok(None);
    }
    if !response.status().is_success() {
        return Err(tf("GitHub answered {0} when checking for updates.", &[&response.status().as_u16()]));
    }
    let release: Release = response.json().map_err(|_| t("Couldn't check for updates. Check your internet connection."))?;
    let latest = release.tag_name.as_deref().and_then(Version::parse).ok_or_else(|| t("The latest release has no usable version number."))?;
    log::info!("Update check: running {current}, latest release {latest}");
    if latest <= *current {
        return Ok(None);
    }
    pick(&release, latest.clone()).map(Some).ok_or_else(|| tf("Tin Pan {0} is out, but its signed download is missing. Get it from the release page.", &[&latest.short()]))
}

fn pick(release: &Release, version: Version) -> Option<UpdateInfo> {
    let find = |name: &str| release.assets.iter().find(|a| a.name.eq_ignore_ascii_case(name)).map(|a| a.browser_download_url.clone());
    Some(UpdateInfo {
        version,
        notes: release.body.clone().unwrap_or_default(),
        download_url: find(ASSET_NAME)?,
        signature_url: find(&format!("{ASSET_NAME}.sig"))?,
        page_url: release.html_url.clone().unwrap_or_else(|| format!("https://github.com/{REPOSITORY}/releases/latest")),
    })
}

/// Where downloads wait to be installed: the folder Tin Pan 1 uses on Windows. On Linux it's the user's own cache
/// folder, as ship-shape does, since a fixed path in the shared /tmp could be taken by another user first.
pub fn folder() -> PathBuf {
    if cfg!(target_os = "linux") {
        let cache = std::env::var_os("XDG_CACHE_HOME").map(PathBuf::from).or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")));
        if let Some(cache) = cache {
            return cache.join("tin-pan").join("update");
        }
    }
    std::env::temp_dir().join("TinPanUpdate")
}

/// Written by the helper when the exe couldn't be replaced; read once at the next start.
pub fn failure_note() -> PathBuf {
    folder().join("install-failed.txt")
}

/// Downloads the asset and checks its signature; the verified file, or what to say. `progress` gets 0 to 1.
pub fn download(http: &Client, update: &UpdateInfo, progress: &dyn Fn(f64)) -> Result<PathBuf, String> {
    let target = folder().join(ASSET_NAME);
    let fail = |target: &Path, text: String| {
        let _ = std::fs::remove_file(target);
        text
    };
    std::fs::create_dir_all(folder()).map_err(|_| t("Couldn't download the update. Check your internet connection and try again."))?;
    let offline = || t("Couldn't download the update. Check your internet connection and try again.");
    let signature = http.get(&update.signature_url).timeout(Duration::from_secs(60)).send().and_then(|r| r.error_for_status()).and_then(|r| r.text()).map_err(|_| offline())?;
    // A download that stalls still ends.
    let mut response = http.get(&update.download_url).timeout(Duration::from_secs(15 * 60)).send().and_then(|r| r.error_for_status()).map_err(|_| offline())?;
    let total = response.content_length().unwrap_or(0);
    let mut file = std::fs::File::create(&target).map_err(|_| offline())?;
    let mut buffer = vec![0u8; 81_920];
    let mut received = 0u64;
    loop {
        let read = match response.read(&mut buffer) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) => {
                let stalled = e.to_string().contains("timed out");
                return Err(fail(&target, if stalled { t("The update download stalled. Check your internet connection and try again.") } else { offline() }));
            }
        };
        std::io::Write::write_all(&mut file, &buffer[..read]).map_err(|_| fail(&target, offline()))?;
        received += read as u64;
        if total > 0 {
            progress(received as f64 / total as f64);
        }
    }
    drop(file);
    if !verify(PUBLIC_KEY, &target, ASSET_NAME, &signature) {
        log::error!("Update {} failed signature verification; discarded", update.version);
        return Err(fail(&target, t("The downloaded update isn't signed by Tin Pan's publisher, so it wasn't installed.")));
    }
    log::info!("Update {} downloaded and verified", update.version);
    Ok(target)
}

/// "TinPan-update-v1:TinPan.exe:<sha256 of the file, lower-case hex>", as Tin Pan 1 and build/Signer sign it.
pub fn message(asset_name: &str, sha256: &[u8]) -> String {
    format!("{DOMAIN}:{asset_name}:{}", hex::encode(sha256))
}

fn hash_file(path: &Path) -> Option<Vec<u8>> {
    let mut file = std::fs::File::open(path).ok()?;
    let mut hasher = Sha256::new();
    std::io::copy(&mut file, &mut hasher).ok()?;
    Some(hasher.finalize().to_vec())
}

/// Whether `signature` (base64) is the publisher's for this file under this asset name.
pub fn verify(public_key: &str, path: &Path, asset_name: &str, signature: &str) -> bool {
    let (Ok(key), Ok(signature), Some(hash)) = (BASE64.decode(public_key.trim()), BASE64.decode(signature.trim()), hash_file(path)) else { return false };
    let (Ok(key), Ok(signature)) = (<[u8; 32]>::try_from(key.as_slice()), <[u8; 64]>::try_from(signature.as_slice())) else { return false };
    let Ok(key) = VerifyingKey::from_bytes(&key) else { return false };
    key.verify(message(asset_name, &hash).as_bytes(), &Signature::from_bytes(&signature)).is_ok()
}

/// The changelog part of a release's notes: the release workflow starts them with a "Download
/// **TinPan.exe** below" paragraph for the web page, which isn't news, so it's skipped.
pub fn changes_only(notes: &str) -> String {
    let text = notes.replace("\r\n", "\n");
    let text = text.trim();
    if !text.starts_with("Download **TinPan.exe**") {
        return text.to_string();
    }
    text.split_once("\n\n").map(|(_, rest)| rest.trim().to_string()).unwrap_or_default()
}

/// Release notes are Markdown; read them as plain lines, at most 1500 characters, as Tin Pan 1 does.
pub fn plain_text(markdown: &str) -> String {
    let mut lines = Vec::new();
    for line in markdown.replace('\r', "").lines() {
        let mut line = line.trim_end().to_string();
        let trimmed = line.trim_start();
        if trimmed.starts_with('#') {
            line = trimmed.trim_start_matches('#').trim_start().to_string();
        } else if let Some(rest) = trimmed.strip_prefix("* ").or_else(|| trimmed.strip_prefix("+ ")).or_else(|| trimmed.strip_prefix("- ")) {
            line = format!("- {rest}");
        }
        lines.push(strip_inline(&line));
    }
    let mut text = lines.join("\n");
    while text.contains("\n\n\n") {
        text = text.replace("\n\n\n", "\n\n");
    }
    let text = text.trim().to_string();
    if text.chars().count() > 1500 { format!("{}...", text.chars().take(1500).collect::<String>().trim_end()) } else { text }
}

/// Links become their words; **, __ and ` go.
fn strip_inline(line: &str) -> String {
    let mut out = String::new();
    let mut rest = line;
    while let Some(open) = rest.find('[') {
        let after = &rest[open + 1..];
        match after.find("](").and_then(|close| after[close + 2..].find(')').map(|end| (close, close + 2 + end))) {
            Some((close, end)) => {
                out.push_str(&rest[..open]);
                out.push_str(&after[..close]);
                rest = &after[end + 1..];
            }
            None => {
                out.push_str(&rest[..=open]);
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out.replace("**", "").replace("__", "").replace('`', "")
}

/// A PowerShell single-quoted string. PowerShell also ends such strings at the curly quotes U+2018 to
/// U+201B (common in names like O’Brien), so each is doubled as well, as in Tin Pan 1.
pub fn quote(value: &str) -> String {
    let mut out = String::from("'");
    for c in value.chars() {
        out.push(c);
        if matches!(c, '\'' | '\u{2018}' | '\u{2019}' | '\u{201A}' | '\u{201B}') {
            out.push(c);
        }
    }
    out.push('\'');
    out
}

/// Tin Pan 1's helper: wait for Tin Pan to exit, move the old exe aside, copy the new one in (retrying while the
/// old one is still locked), put the old one back if that fails and leave a note saying so, then start Tin Pan,
/// marked as relaunched. The old copy is removed at the next start.
pub fn install_script(pid: u32, new_exe: &Path, exe: &Path) -> String {
    let (new_exe, exe) = (new_exe.display().to_string(), exe.display().to_string());
    let old = format!("{exe}.old");
    format!(
        "Wait-Process -Id {pid} -Timeout 60 -ErrorAction SilentlyContinue; $ok = $false; \
         for ($i = 0; $i -lt 15 -and -not $ok; $i++) {{ try {{ \
         Move-Item -LiteralPath {exe_q} -Destination {old_q} -Force -ErrorAction Stop; \
         try {{ Copy-Item -LiteralPath {new_q} -Destination {exe_q} -Force -ErrorAction Stop; $ok = $true }} \
         catch {{ Move-Item -LiteralPath {old_q} -Destination {exe_q} -Force -ErrorAction SilentlyContinue; throw }} \
         }} catch {{ Start-Sleep -Seconds 1 }} }}; \
         if (-not $ok) {{ Set-Content -LiteralPath {note_q} -Value 'failed' }}; \
         Remove-Item -LiteralPath {new_q} -Force -ErrorAction SilentlyContinue; \
         $env:{RELAUNCH_ENV}='1'; Start-Process -FilePath {exe_q}",
        exe_q = quote(&exe),
        old_q = quote(&old),
        new_q = quote(&new_exe),
        note_q = quote(&failure_note().display().to_string()),
    )
}

/// A shell single-quoted string, with any ' in it closed, escaped and reopened.
pub fn sh_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', r"'\''"))
}

/// ship-shape's Linux helper: wait for Tin Pan to exit, make the new AppImage runnable and move it over the old
/// one, then start it, marked as relaunched. `;` between steps, so a failed move still starts the old one, which
/// then says so, from the note the failed move leaves (as the Windows helper does).
pub fn appimage_script(pid: u32, new_appimage: &Path, current_appimage: &Path) -> String {
    let (new_q, cur_q) = (sh_quote(&new_appimage.display().to_string()), sh_quote(&current_appimage.display().to_string()));
    let note_q = sh_quote(&failure_note().display().to_string());
    format!("sleep 1; while kill -0 {pid} 2>/dev/null; do sleep 0.2; done; chmod +x {new_q}; mv -f {new_q} {cur_q} || echo failed > {note_q}; {RELAUNCH_ENV}=1 nohup {cur_q} >/dev/null 2>&1 &")
}

/// At startup: removes the previous exe an update left, and says once (true) if the last update couldn't be installed.
pub fn tidy_after_update() -> bool {
    if let Ok(exe) = std::env::current_exe() {
        let old = PathBuf::from(format!("{}.old", exe.display()));
        if old.exists()
            && let Err(e) = std::fs::remove_file(&old)
        {
            log::warn!("Couldn't remove {}: {e}", old.display());
        }
    }
    let note = failure_note();
    if !note.exists() {
        return false;
    }
    let _ = std::fs::remove_file(note);
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};

    #[test]
    fn versions_compare_with_pre_releases_first() {
        let v = |s: &str| Version::parse(s).unwrap();
        assert!(v("v2.0.0") > v("1.12.0"));
        assert!(v("2.0.0") > v("2.0.0-alpha.1"));
        assert!(v("2.0.0-alpha.2") > v("2.0.0-alpha.1"));
        assert_eq!(v("1.2"), v("1.2.0"));
        assert_eq!(v("v2.0.0-alpha.1").short(), "2.0.0");
        assert!(Version::parse("latest").is_none());
    }

    #[test]
    fn signatures_bind_the_file_to_its_asset_name() {
        let key = SigningKey::from_bytes(&[7u8; 32]);
        let public = BASE64.encode(key.verifying_key().to_bytes());
        let dir = std::env::temp_dir().join(format!("tinpan-sig-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("TinPan.exe");
        std::fs::write(&file, b"the new Tin Pan").unwrap();
        let signed = BASE64.encode(key.sign(message("TinPan.exe", &hash_file(&file).unwrap()).as_bytes()).to_bytes());
        assert!(verify(&public, &file, "TinPan.exe", &signed));
        assert!(!verify(&public, &file, "Other.exe", &signed));
        std::fs::write(&file, b"something else").unwrap();
        assert!(!verify(&public, &file, "TinPan.exe", &signed));
        assert!(!verify(&public, &file, "TinPan.exe", "not base64"));
        let _ = std::fs::remove_dir_all(dir);
    }

    /// Checks a file Tin Pan 1's build/Signer signed: TINPAN_SIGNED_FILE (with its .sig beside it) and
    /// TINPAN_SIGNER_PUBLIC_KEY. Run with --ignored when the signing scheme or the Signer changes.
    #[test]
    #[ignore]
    fn verifies_what_tin_pan_1s_signer_signed() {
        let file = PathBuf::from(std::env::var("TINPAN_SIGNED_FILE").expect("TINPAN_SIGNED_FILE"));
        let key = std::env::var("TINPAN_SIGNER_PUBLIC_KEY").expect("TINPAN_SIGNER_PUBLIC_KEY");
        let signature = std::fs::read_to_string(format!("{}.sig", file.display())).unwrap();
        let name = file.file_name().unwrap().to_string_lossy().to_string();
        assert!(verify(&key, &file, &name, &signature));
    }

    #[test]
    fn notes_lose_the_download_paragraph_and_markdown() {
        let notes = "Download **TinPan.exe** below and run it.\r\n\r\n### Added\n* **Tabs**: see [the guide](https://x).\n\n\n\n- `F6` moves";
        assert_eq!(plain_text(&changes_only(notes)), "Added\n- Tabs: see the guide.\n\n- F6 moves");
        assert_eq!(changes_only("Just notes"), "Just notes");
    }

    #[test]
    fn the_linux_helper_waits_replaces_and_relaunches() {
        assert_eq!(sh_quote("/home/o'brien/TinPan.AppImage"), r"'/home/o'\''brien/TinPan.AppImage'");
        let script = appimage_script(7, Path::new("/c/new.AppImage"), Path::new("/a/TinPan.AppImage"));
        assert!(script.starts_with("sleep 1; while kill -0 7 2>/dev/null"));
        assert!(script.contains("mv -f '/c/new.AppImage' '/a/TinPan.AppImage' || echo failed > "));
        assert!(script.ends_with("; TINPAN_RELAUNCHED=1 nohup '/a/TinPan.AppImage' >/dev/null 2>&1 &"));
    }

    #[test]
    fn the_helper_quotes_paths_and_marks_the_relaunch() {
        assert_eq!(quote("C:\\Users\\O’Brien\\TinPan.exe"), "'C:\\Users\\O’’Brien\\TinPan.exe'");
        let script = install_script(42, Path::new("C:\\t\\TinPan.exe"), Path::new("C:\\a\\TinPan.exe"));
        assert!(script.starts_with("Wait-Process -Id 42"));
        assert!(script.contains("-Destination 'C:\\a\\TinPan.exe.old'") && script.ends_with("$env:TINPAN_RELAUNCHED='1'; Start-Process -FilePath 'C:\\a\\TinPan.exe'"));
    }
}
