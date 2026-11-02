//! Suno's API, as Tin Pan 1's SunoApiService talks to it: the same hosts, headers, retries and replies.
//! Blocking; it runs on Tin Pan's network thread.

use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};

use base64::Engine;
use reqwest::Method;
use reqwest::blocking::Client;
use serde_json::{Value, json};

use crate::clerk::ClerkSession;
use crate::clip::SunoClip;
use crate::config::LibraryConfig;
use crate::cookies::SunoCookies;
use crate::create::{GenerateRequest, LyricsDraft, LyricsResult, Persona, PersonaInfo, RemasterRequest};
use crate::discover::{self, Comment, Contest, CreatorPage, ExploreShelf, Notification};
use crate::drm;
use crate::error::{Error, Result};
use crate::i18n::{t, tf};
use crate::json::{self as sj, FeedPage};
use crate::lyrics::AlignedWord;
use crate::playlist::{self, Playlist};

/// Where the API answers, in order. Suno's site moved to suno.com/a/api on 2026-10-24; the old host is the fallback.
pub const HOSTS: [&str; 2] = ["https://suno.com/a", "https://studio-api-prod.suno.com"];

/// After the new host fails and the old one answers, the old one goes first for this long.
const PREFER_OLD_FOR: Duration = Duration::from_secs(10 * 60);
const RATE_LIMIT_RETRIES: u32 = 2;

/// The signed-in account, as the API needs it.
pub struct Account {
    session: ClerkSession,
    device_id: String,
}

impl Account {
    pub fn new(session: ClerkSession, fallback_device_id: &str) -> Self {
        let device_id = session.cookies.anonymous_id().unwrap_or(fallback_device_id).to_string();
        Self { session, device_id }
    }

    pub fn user_name(&self) -> Option<&str> {
        self.session.user_name.as_deref()
    }

    pub fn cookies(&self) -> &SunoCookies {
        &self.session.cookies
    }

    /// Whether Clerk rotated the cookies since the last call; they should be saved again.
    pub fn take_cookies_changed(&mut self) -> bool {
        std::mem::take(&mut self.session.cookies_changed)
    }
}

pub struct SunoApi {
    http: Client,
    account: Mutex<Option<Account>>,
    old_first_until: Mutex<Option<Instant>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Workspace {
    pub id: String,
    pub name: String,
    pub songs: Option<i64>,
}

impl std::fmt::Display for Workspace {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.songs {
            Some(n) => write!(f, "{}, {}", self.name, crate::i18n::plural(n, "{0} song", "{0} songs")),
            None => write!(f, "{}", self.name),
        }
    }
}

/// What the library asks for: a page after `cursor`, optionally a search, with the chosen filters.
#[derive(Debug, Clone, Default)]
pub struct LibraryQuery {
    pub cursor: Option<String>,
    pub search: Option<String>,
    pub library: LibraryConfig,
}

impl SunoApi {
    pub fn new(http: Client) -> Self {
        Self { http, account: Mutex::new(None), old_first_until: Mutex::new(None) }
    }

    /// Signs in with saved or fresh cookies. SignInRequired when they no longer work.
    pub fn sign_in(&self, cookies: &str, device_id: &str) -> Result<String> {
        let mut session = ClerkSession::new(self.http.clone(), SunoCookies::parse(Some(cookies)));
        session.initialize()?;
        session.token(false)?;
        let name = session.user_name.clone().unwrap_or_default();
        *self.account.lock().unwrap() = Some(Account::new(session, device_id));
        Ok(name)
    }

    pub fn sign_out(&self) {
        *self.account.lock().unwrap() = None;
    }

    pub fn is_signed_in(&self) -> bool {
        self.account.lock().unwrap().is_some()
    }

    /// The cookies to save, when Clerk has rotated them since this was last asked.
    pub fn rotated_cookies(&self) -> Option<String> {
        let mut guard = self.account.lock().unwrap();
        let account = guard.as_mut()?;
        account.take_cookies_changed().then(|| account.cookies().to_header())
    }

    fn token(&self, force_refresh: bool) -> Result<(String, String)> {
        let mut guard = self.account.lock().unwrap();
        let account = guard.as_mut().ok_or_else(|| Error::SignInRequired(t("Not signed in. Sign in from Settings.")))?;
        match account.session.token(force_refresh) {
            Ok(token) => Ok((token, account.device_id.clone())),
            Err(e @ Error::SignInRequired(_)) => {
                *guard = None;
                Err(e)
            }
            Err(e) => Err(e),
        }
    }

    fn host_order(&self) -> [&'static str; 2] {
        match *self.old_first_until.lock().unwrap() {
            Some(until) if Instant::now() < until => [HOSTS[1], HOSTS[0]],
            _ => HOSTS,
        }
    }

    fn request(&self, method: Method, url: &str, token: &str, device_id: &str, site_host: bool) -> reqwest::blocking::RequestBuilder {
        let mut request = self
            .http
            .request(method, url)
            .header("Authorization", format!("Bearer {token}"))
            .header("Device-Id", device_id)
            .header("Browser-Token", browser_token(chrono::Utc::now().timestamp_millis()))
            .header("Origin", "https://suno.com")
            .header("Referer", "https://suno.com/")
            .header("Accept", "application/json");
        // The site host reads the signed-in session as a browser sends it, a __session cookie.
        if site_host {
            request = request.header("Cookie", format!("__session={token}"));
        }
        request
    }

    /// Sends a request to Suno and returns the reply's text, trying the other host when this one didn't
    /// take it, a fresh token when Suno calls this one stale, and waiting out "too many requests".
    pub fn send(&self, method: Method, path: &str, body: Option<&Value>) -> Result<String> {
        self.send_with(method, path, body, true, 0)
    }

    fn send_with(&self, method: Method, path: &str, body: Option<&Value>, retry_auth: bool, rate_limited: u32) -> Result<String> {
        let (token, device_id) = self.token(!retry_auth)?;
        let hosts = self.host_order();
        for (i, host) in hosts.iter().enumerate() {
            let last = i == hosts.len() - 1;
            let mut request = self.request(method.clone(), &format!("{host}{path}"), &token, &device_id, *host == HOSTS[0]);
            if let Some(body) = body {
                request = request.header("Content-Type", "application/json").body(body.to_string());
            }
            let response = match request.send() {
                Ok(r) => r,
                // No connection to this host. Only reads move on: a POST may already have arrived.
                Err(e) if !last && method == Method::GET && !e.is_timeout() => {
                    log::warn!("{method} {} couldn't reach {host}; trying the other host", queryless(path));
                    continue;
                }
                Err(e) if e.is_timeout() => {
                    return Err(Error::api(if method == Method::GET {
                        t("Suno didn't answer in time. Try again.")
                    } else {
                        t("Suno didn't answer in time. It may still have done it, so refresh to check before trying again.")
                    }));
                }
                Err(e) => {
                    log::error!("{method} {} failed: {e}", queryless(path));
                    return Err(Error::api(t("Can't reach Suno. Check your internet connection.")));
                }
            };
            let status = response.status().as_u16();
            let retry_after = response.headers().get("retry-after").and_then(|v| v.to_str().ok()).and_then(|v| v.trim().parse::<u64>().ok());
            let text = response.text().unwrap_or_default();
            if !last && not_taken_here(&method, status, &text, *host == HOSTS[0]) {
                log::warn!("{method} {} on {host} answered {status}; trying the other host", queryless(path));
                continue;
            }
            if i > 0 && *host == HOSTS[1] && (200..300).contains(&status) {
                *self.old_first_until.lock().unwrap() = Some(Instant::now() + PREFER_OLD_FOR);
            }
            if (200..300).contains(&status) {
                return Ok(text);
            }
            log::warn!("{method} {} returned {status}: {}", queryless(path), truncate(&text.replace(['\r', '\n'], " "), 300));
            // Suno rejects tokens it considers stale even before they expire; one fresh token fixes it.
            if retry_auth && (status == 401 || status == 403 || looks_like_stale_token(&text)) {
                return self.send_with(method, path, body, false, rate_limited);
            }
            if status == 429 && rate_limited < RATE_LIMIT_RETRIES && (retryable_rate_limit(&text) || retry_after.is_some()) {
                let wait = retry_after.filter(|s| *s <= 10).map_or(Duration::from_secs(if rate_limited == 0 { 2 } else { 5 }), Duration::from_secs);
                thread::sleep(wait);
                return self.send_with(method, path, body, retry_auth, rate_limited + 1);
            }
            return Err(Error::api_status(describe_failure(status, &text), status));
        }
        Err(Error::api(t("Can't reach Suno. Check your internet connection.")))
    }

    // ── Account ─────────────────────────────────────────────────────────

    pub fn billing_info(&self) -> Result<crate::billing::BillingInfo> {
        crate::billing::BillingInfo::read(&self.send(Method::GET, "/api/billing/info/", None)?)
    }

    /// Whether Suno shapes style ideas and boosts to your taste (GET /api/personalization/settings); None if unknown.
    pub fn style_personalization(&self) -> Result<Option<bool>> {
        let reply = self.send(Method::GET, "/api/personalization/settings", None)?;
        Ok(serde_json::from_str::<Value>(&reply).ok().and_then(|v| v.get("styles_augmentation").and_then(Value::as_bool)))
    }

    // ── Songs ───────────────────────────────────────────────────────────

    /// A page of the library (POST /api/feed/v3, as Suno's web library sends it).
    pub fn library_page(&self, query: &LibraryQuery) -> Result<FeedPage> {
        let mut body = json!({ "limit": 100, "filters": feed_filters(&query.library, query.search.as_deref()) });
        if let Some(cursor) = &query.cursor {
            body["cursor"] = json!(cursor);
        }
        sj::read_feed_page(&self.send(Method::POST, "/api/feed/v3", Some(&body))?)
    }

    /// Songs by id, two at a time (Suno only answers reliably for two).
    pub fn clips(&self, ids: &[&str]) -> Result<Vec<SunoClip>> {
        let mut result = Vec::new();
        for chunk in ids.chunks(2) {
            let list = chunk.iter().map(|id| escape(id)).collect::<Vec<_>>().join(",");
            result.extend(sj::read_feed_page(&self.send(Method::GET, &format!("/api/feed/?ids={list}"), None)?)?.clips);
        }
        Ok(result)
    }

    /// Your workspaces (GET /api/project/me), with "My Workspace" (id "default") first when Suno leaves it out.
    pub fn workspaces(&self) -> Result<Vec<Workspace>> {
        let root = sj::parse(&self.send(Method::GET, "/api/project/me", None)?)?;
        let mut list: Vec<Workspace> = root
            .get("projects")
            .and_then(Value::as_array)
            .map(|projects| {
                projects
                    .iter()
                    .filter_map(|p| {
                        let id = match p.get("id")? {
                            Value::String(s) => s.clone(),
                            other => other.to_string(),
                        };
                        let name = p.get("name").and_then(Value::as_str).unwrap_or("").to_string();
                        let name = if !name.is_empty() {
                            name
                        } else if id == "default" {
                            t("My Workspace")
                        } else {
                            t("Untitled workspace")
                        };
                        (!id.is_empty()).then(|| Workspace { id, name, songs: p.get("clip_count").and_then(sj::as_int) })
                    })
                    .collect()
            })
            .unwrap_or_default();
        if list.iter().all(|w| w.id != "default") {
            list.insert(0, Workspace { id: "default".into(), name: t("My Workspace"), songs: None });
        }
        Ok(list)
    }

    /// Likes ("LIKE"), dislikes ("DISLIKE") or clears (None) the song: POST /api/gen/{id}/update_reaction_type/.
    /// Returns Suno's state now: "L", "D", or None once cleared.
    pub fn set_reaction(&self, clip_id: &str, reaction: Option<&str>) -> Result<Option<String>> {
        let body = json!({ "reaction": reaction, "recommendation_metadata": {} });
        let reply = self.send(Method::POST, &format!("/api/gen/{}/update_reaction_type/", escape(clip_id)), Some(&body))?;
        Ok(sj::try_read_string(&reply, "reaction_type"))
    }

    /// Changes what Suno shows for the song: its title, its lyrics, or both (POST /api/gen/{id}/set_metadata/).
    pub fn set_details(&self, clip_id: &str, title: Option<&str>, lyrics: Option<&str>) -> Result<()> {
        let mut body = serde_json::Map::new();
        if let Some(title) = title {
            body.insert("title".into(), json!(title));
        }
        if let Some(lyrics) = lyrics {
            body.insert("lyrics".into(), json!(lyrics));
        }
        if body.is_empty() {
            return Ok(());
        }
        self.send(Method::POST, &format!("/api/gen/{}/set_metadata/", escape(clip_id)), Some(&Value::Object(body))).map(drop)
    }

    pub fn set_public(&self, clip_id: &str, public: bool) -> Result<()> {
        self.send(Method::POST, &format!("/api/gen/{}/set_visibility/", escape(clip_id)), Some(&json!({ "is_public": public }))).map(drop)
    }

    /// Whether others may cover and remix the song.
    pub fn set_remix_allowed(&self, clip_id: &str, allowed: bool) -> Result<()> {
        self.send(Method::POST, &format!("/api/clips/{}/toggle_remixes/", escape(clip_id)), Some(&json!({ "can_remix": allowed }))).map(drop)
    }

    /// Suno's own share link for the song (POST /api/share/link).
    pub fn share_link(&self, clip_id: &str) -> Result<String> {
        let reply = self.send(Method::POST, "/api/share/link", Some(&json!({ "content_type": "clip", "content_id": clip_id, "source": "web" })))?;
        sj::try_read_string(&reply, "link").filter(|l| !l.is_empty()).ok_or_else(|| Error::api(t("Suno gave no share link.")))
    }

    /// Tells Suno's recommendations to stop (or start again) suggesting songs like this one.
    pub fn set_not_interested(&self, clip_id: &str, not_interested: bool) -> Result<()> {
        let method = if not_interested { Method::POST } else { Method::DELETE };
        self.send(method, &format!("/api/recommend/feedback/song/{}", escape(clip_id)), Some(&json!({ "feedback_type": "not_interested" }))).map(drop)
    }

    /// Deletes songs for good (POST /api/clips/delete/); there's no undo.
    pub fn delete_forever(&self, clip_ids: &[&str]) -> Result<()> {
        for chunk in clip_ids.chunks(20) {
            self.send(Method::POST, "/api/clips/delete/", Some(&json!({ "ids": chunk })))?;
        }
        Ok(())
    }

    /// Moves the song to Suno's trash, or back out of it.
    pub fn set_trashed(&self, clip_id: &str, trashed: bool) -> Result<()> {
        self.send(Method::POST, "/api/gen/trash", Some(&json!({ "clip_ids": [clip_id], "trash": trashed }))).map(drop)
    }

    /// The song's sung words with their times (GET /api/gen/{id}/aligned_lyrics/v2/); empty when Suno has none.
    pub fn aligned_words(&self, clip_id: &str) -> Result<Vec<AlignedWord>> {
        let reply = match self.send(Method::GET, &format!("/api/gen/{}/aligned_lyrics/v2/", escape(clip_id)), None) {
            Ok(reply) => reply,
            Err(e) if e.status() == Some(404) => return Ok(Vec::new()),
            Err(e) => return Err(e),
        };
        let root = sj::parse(&reply)?;
        Ok(root
            .get("aligned_words")
            .and_then(Value::as_array)
            .map(|words| {
                words
                    .iter()
                    .filter_map(|w| {
                        let time = |name: &str| w.get(name).and_then(Value::as_f64).unwrap_or(0.0);
                        Some(AlignedWord { word: w.get("word")?.as_str()?.to_string(), start: time("start_s"), end: time("end_s") })
                    })
                    .collect()
            })
            .unwrap_or_default())
    }

    /// The song's timed words: Suno's stored ones, or, when it has none but the song has lyrics, a fresh timing
    /// of those lyrics with the newer route (v3), as Tin Pan 1 does.
    pub fn timed_words(&self, clip_id: &str) -> Result<Vec<AlignedWord>> {
        let stored = self.aligned_words(clip_id)?;
        if !stored.is_empty() {
            return Ok(stored);
        }
        let lyrics = self.clips(&[clip_id])?.into_iter().find(|c| c.id == clip_id).and_then(|c| c.lyrics().map(|l| l.trim().to_string())).unwrap_or_default();
        if lyrics.is_empty() {
            return Ok(stored);
        }
        match self.align_lyrics(clip_id, &lyrics) {
            Ok(words) => Ok(words),
            Err(e @ Error::SignInRequired(_)) => Err(e),
            Err(e) => {
                log::warn!("Timing the lyrics of {clip_id} (v3) didn't work: {e}");
                Ok(stored)
            }
        }
    }

    /// POST /api/gen/{id}/aligned_lyrics/v3 with the lyrics, then the same path until it answers with
    /// "alignment" (or state "error"), every 2 seconds for up to 40 tries.
    fn align_lyrics(&self, clip_id: &str, lyrics: &str) -> Result<Vec<AlignedWord>> {
        let path = format!("/api/gen/{}/aligned_lyrics/v3", escape(clip_id));
        let mut reply = self.send(Method::POST, &path, Some(&json!({ "lyrics": lyrics, "enable_augmentation": false })))?;
        for attempt in 0.. {
            let root = sj::parse(&reply)?;
            if let Some(words) = root.get("alignment").and_then(Value::as_array) {
                return Ok(words
                    .iter()
                    .filter_map(|w| {
                        let time = |name: &str| w.get(name).and_then(Value::as_f64).unwrap_or(0.0);
                        Some(AlignedWord { word: w.get("word")?.as_str()?.to_string(), start: time("start_s"), end: time("end_s") })
                    })
                    .filter(|w| w.end < 6000.0)
                    .collect());
            }
            if root.get("state").and_then(Value::as_str) == Some("error") || attempt >= 40 {
                return Ok(Vec::new());
            }
            thread::sleep(Duration::from_secs(2));
            reply = self.send(Method::GET, &path, None)?;
        }
        unreachable!("the loop returns")
    }

    // ── Downloads ───────────────────────────────────────────────────────

    /// Saves a file from Suno's file host to `path`: first as path.part, then renamed, so a broken
    /// download never looks finished. Only https addresses are used.
    pub fn download_to(&self, url: &str, path: &std::path::Path) -> Result<()> {
        if !url.to_ascii_lowercase().starts_with("https://") {
            return Err(Error::api(t("Suno gave a download address this app won't use.")));
        }
        let mut response = self.http.get(url).timeout(Duration::from_secs(30 * 60)).send().map_err(|e| {
            log::warn!("Download from {}: {e}", url.split('/').nth(2).unwrap_or(""));
            Error::api(t("Can't reach Suno. Check your internet connection."))
        })?;
        let status = response.status().as_u16();
        if !response.status().is_success() {
            return Err(Error::api_status(tf("The download failed ({0}).", &[&status]), status));
        }
        write_file(path, |file| response.copy_to(file).map(drop).map_err(std::io::Error::other))
    }

    /// The song's own file, through the free route Tin Pan 1's downloads use: the unencrypted file as it
    /// is, or the encrypted one decrypted here. Uses none of the account's downloads.
    pub fn download_free(&self, clip: &SunoClip, path: &std::path::Path) -> Result<()> {
        let data = self.song_audio(clip)?;
        write_file(path, |file| std::io::Write::write_all(file, &data))
    }

    /// Suno's WAV of the song: asks for it to be made, then waits until it's there (up to 3 minutes).
    pub fn wav_url(&self, clip_id: &str) -> Result<String> {
        let id = escape(clip_id);
        let mut requested = false;
        for _ in 0..90 {
            let reply = self.send(Method::GET, &format!("/api/gen/{id}/wav_file/"), None)?;
            if let Some(url) = sj::try_read_string(&reply, "wav_file_url").filter(|u| !u.is_empty()) {
                return Ok(url);
            }
            if !requested {
                self.send(Method::POST, &format!("/api/gen/{id}/convert_wav/"), Some(&json!({})))?;
                requested = true;
            }
            thread::sleep(Duration::from_secs(2));
        }
        Err(Error::api(t("Suno took too long making the WAV file. Try again later.")))
    }

    /// Spends one of the plan's downloads on the song (POST /api/download/authorize), for Suno's own download.
    pub fn authorize_download(&self, clip_id: &str) -> Result<()> {
        let reply = self.send(Method::POST, "/api/download/authorize", Some(&json!({ "item_id": clip_id, "item_type": "clip" })))?;
        let ok = serde_json::from_str::<Value>(&reply).ok().and_then(|v| v.get("ok").and_then(Value::as_bool)) == Some(true);
        if ok { Ok(()) } else { Err(Error::api(t("Suno didn't confirm the download. Check your downloads left with Shift+F2."))) }
    }

    /// Suno's own download of the song in `format`: waits while Suno prepares it, then gives its address.
    /// Suno Studio accounts use Studio's route.
    pub fn prepare_download(&self, clip_id: &str, format: &str, studio: bool) -> Result<String> {
        let path = if studio {
            format!("/api/studio/clip/{}/download?format={}", escape(clip_id), escape(format))
        } else {
            format!("/api/download/clip/{}?format={}", escape(clip_id), escape(format))
        };
        for _ in 0..90 {
            let reply = self.send(Method::GET, &path, None)?;
            let status = sj::try_read_string(&reply, "status");
            let reason = sj::try_read_string(&reply, "reason");
            if let (Some("ready"), Some(url)) = (status.as_deref(), sj::try_read_string(&reply, "download_url").filter(|u| !u.is_empty())) {
                return Ok(url);
            }
            let waiting = matches!(status.as_deref(), Some("processing" | "rate_limited")) || reason.as_deref() == Some("rate_limited");
            if !waiting {
                let detail = sj::try_read_string(&reply, "detail").or(reason).filter(|d| !d.trim().is_empty());
                return Err(Error::api(match detail {
                    Some(detail) => tf("Suno couldn't prepare that download: {0}", &[&detail]),
                    None => t("Suno couldn't prepare that download."),
                }));
            }
            thread::sleep(Duration::from_secs(2));
        }
        Err(Error::api(t("Suno took too long preparing the download. Try again later.")))
    }

    /// Trending, as Suno's Explore page asks for it (POST /api/unified/feed with feed_id "trending").
    pub fn trending(&self) -> Result<Vec<SunoClip>> {
        let body = json!({ "feed_id": "trending", "cursor": null, "page_size": 50 });
        sj::find_clips(&self.send(Method::POST, "/api/unified/feed", Some(&body))?)
    }

    // ── Making songs ────────────────────────────────────────────────────

    pub fn personas(&self) -> Result<Vec<Persona>> {
        Ok(crate::create::read_personas(&sj::parse(&self.send(Method::GET, "/api/persona/get-personas/", None)?)?))
    }

    /// None when Suno needs no captcha; otherwise 1 (hCaptcha) or 2 (Turnstile).
    pub fn captcha_needed(&self) -> Result<Option<i32>> {
        let check = match self.send(Method::POST, "/api/c/check", Some(&json!({ "ctype": "generation" }))) {
            Ok(check) => sj::parse(&check)?,
            // No captcha gate on this API version; just try.
            Err(Error::Api { status: Some(404 | 405), .. }) => return Ok(None),
            Err(e) => return Err(e),
        };
        if check.get("required").and_then(Value::as_bool) != Some(true) {
            return Ok(None);
        }
        Ok(Some(if check.get("captcha_version").and_then(sj::as_int) == Some(2) { 2 } else { 1 }))
    }

    /// Reads the persona and sets the request's task and source song for it. A persona whose details don't load
    /// is sent as it is, as Tin Pan 1 does.
    fn apply_persona(&self, request: &mut GenerateRequest, model_takes_voice: Option<bool>) -> Result<()> {
        let Some(id) = request.persona_id.clone() else { return Ok(()) };
        let mut persona = match self.send(Method::GET, &format!("/api/persona/get-persona/{}/", escape(&id)), None).and_then(|r| sj::parse(&r)) {
            Ok(root) => PersonaInfo::read(&root),
            Err(e @ Error::SignInRequired(_)) => return Err(e),
            Err(e) => {
                log::warn!("Persona {id} details didn't load ({e}); sending it without its source song");
                return Ok(());
            }
        };
        if let (Some(root), None) = (persona.root_clip_id.clone(), persona.root_duration) {
            match self.clips(&[root.as_str()]) {
                Ok(clips) => persona.root_duration = clips.iter().find(|c| c.id == root).and_then(SunoClip::duration),
                Err(e @ Error::SignInRequired(_)) => return Err(e),
                Err(e) => log::warn!("Persona source song length didn't load: {e}"),
            }
        }
        request.apply_persona(&persona, model_takes_voice)
    }

    /// POST /api/generate/v2-web/: the songs Suno started. The captcha token, when one is needed, is already on the request.
    pub fn generate(&self, mut request: GenerateRequest, model_takes_voice: Option<bool>) -> Result<Vec<SunoClip>> {
        self.apply_persona(&mut request, model_takes_voice)?;
        let reply = self.send(Method::POST, "/api/generate/v2-web/", Some(&request.body()))?;
        if sj::try_read_string(&reply, "status").is_some_and(|s| s.eq_ignore_ascii_case("error")) {
            return Err(Error::api(t("Suno turned this request down. Check the style and lyrics, then try again.")));
        }
        let clips = sj::read_feed_page(&reply)?.clips;
        if clips.is_empty() {
            return Err(Error::api(t("Suno didn't say whether it started. Refresh the library before trying again, so you don't pay twice.")));
        }
        Ok(clips)
    }

    /// POST /api/generate/upsample: the remastered songs Suno started.
    pub fn remaster(&self, request: &RemasterRequest) -> Result<Vec<SunoClip>> {
        let clips = sj::find_clips_by_id(&self.send(Method::POST, "/api/generate/upsample", Some(&request.body()))?)?;
        if clips.is_empty() {
            return Err(Error::api(t("Suno didn't say whether the remaster started. Refresh the library before trying again.")));
        }
        Ok(clips)
    }

    // ── Editing into a new song ─────────────────────────────────────────
    // Speed and reverse answer with the new clip; crop and fade with action_clip_id, the new clip's id, so a
    // stand-in follows it until the real one is read, as in Tin Pan 1.

    pub fn change_speed(&self, clip_id: &str, multiplier: f64, keep_pitch: bool, title: &str) -> Result<SunoClip> {
        let body = json!({ "clip_id": clip_id, "speed_multiplier": multiplier, "keep_pitch": keep_pitch, "title": title });
        new_clip(&self.send(Method::POST, "/api/clips/adjust-speed/", Some(&body))?)
    }

    pub fn reverse(&self, clip_id: &str, title: &str) -> Result<SunoClip> {
        new_clip(&self.send(Method::POST, "/api/clips/reverse-clip/", Some(&json!({ "clip_id": clip_id, "title": title })))?)
    }

    pub fn crop(&self, clip_id: &str, start: f64, end: f64, remove: bool, title: &str) -> Result<SunoClip> {
        let body = json!({ "crop_start_s": start, "crop_end_s": end, "is_crop_remove": remove, "title": title, "ui_surface": "song_actions" });
        action_clip(&self.send(Method::POST, &format!("/api/edit/crop/{}/", escape(clip_id)), Some(&body))?, title)
    }

    pub fn fade(&self, clip_id: &str, fade_in: Option<f64>, fade_out: Option<f64>, title: &str) -> Result<SunoClip> {
        let mut body = json!({ "title": title });
        if let Some(s) = fade_in.filter(|s| *s > 0.0) {
            body["fade_in_time"] = json!(s);
        }
        if let Some(s) = fade_out.filter(|s| *s > 0.0) {
            body["fade_out_time"] = json!(s);
        }
        action_clip(&self.send(Method::POST, &format!("/api/edit/fade/{}/", escape(clip_id)), Some(&body))?, title)
    }

    /// Make a video: Suno starts it; `video_status` says how it's going ("complete" brings its address).
    pub fn start_video(&self, clip_id: &str) -> Result<()> {
        self.send(Method::POST, &format!("/api/video/generate/{}/", escape(clip_id)), None).map(drop)
    }

    pub fn video_status(&self, clip_id: &str) -> Result<(String, Option<String>)> {
        let reply = self.send(Method::GET, &format!("/api/video/generate/{}/status/", escape(clip_id)), None)?;
        Ok((sj::try_read_string(&reply, "status").unwrap_or_default(), sj::try_read_string(&reply, "video_url").filter(|u| !u.is_empty())))
    }

    /// Make a persona: it keeps this song's voice and style for new songs.
    pub fn create_persona(&self, root_clip_id: &str, name: &str, description: &str, public: bool) -> Result<()> {
        let body = json!({ "root_clip_id": root_clip_id, "name": name, "description": description, "is_public": public });
        match sj::try_read_string(&self.send(Method::POST, "/api/persona/create/", Some(&body))?, "id") {
            Some(id) if !id.is_empty() => Ok(()),
            _ => Err(Error::api(t("Suno didn't say whether the persona was made. Check your personas."))),
        }
    }

    /// Shows or hides the list of remixes on the song's page.
    pub fn set_show_remixes(&self, clip_id: &str, show: bool) -> Result<()> {
        self.send(Method::POST, &format!("/api/clips/{}/toggle_show_remixes", escape(clip_id)), Some(&json!({ "show_remix": show }))).map(drop)
    }

    /// The songs this one credits, and how many remixes it has (with whether that number is Suno's cap).
    pub fn credits(&self, clip_id: &str) -> Result<(Vec<discover::CreditedSong>, Option<i64>, bool)> {
        let id = escape(clip_id);
        let sources = discover::read_credits(&sj::parse(&self.send(Method::GET, &format!("/api/clips/{id}/attribution"), None)?)?);
        let (count, capped) = match self.send(Method::GET, &format!("/api/clips/remixes/count?clip_id={id}"), None).and_then(|r| sj::parse(&r)) {
            Ok(root) => (root.get("count").and_then(sj::as_int), root.get("is_capped").and_then(Value::as_bool) == Some(true)),
            Err(e @ Error::SignInRequired(_)) => return Err(e),
            Err(e) => {
                log::warn!("Remix count didn't load: {e}");
                (None, false)
            }
        };
        Ok((sources, count, capped))
    }

    /// Stems Suno already made from this song, page by page (20 pages at most).
    pub fn existing_stems(&self, clip_id: &str) -> Result<Vec<SunoClip>> {
        let id = escape(clip_id);
        let pages = sj::parse(&self.send(Method::GET, &format!("/api/clip/{id}/stems/pages"), None)?)?.get("pages").and_then(sj::as_int).unwrap_or(0);
        let mut ids: Vec<String> = Vec::new();
        for page in 0..pages.min(20) {
            let root = sj::parse(&self.send(Method::GET, &format!("/api/clip/{id}/stems?page={page}"), None)?)?;
            for stem in root.get("stems").and_then(Value::as_array).into_iter().flatten() {
                if let Some(sid) = stem.get("id").and_then(Value::as_str).filter(|s| !s.is_empty())
                    && !ids.iter().any(|i| i == sid)
                {
                    ids.push(sid.to_string());
                }
            }
        }
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let refs: Vec<&str> = ids.iter().map(String::as_str).collect();
        let clips = self.clips(&refs)?;
        Ok(ids.iter().filter_map(|i| clips.iter().find(|c| c.id == *i).cloned()).collect())
    }

    /// Sets a song's cover: a picture uploaded to Suno, one at an address, or none (Suno's own again).
    pub fn set_song_cover(&self, clip_id: &str, uploaded: Option<&str>, url: Option<&str>) -> Result<()> {
        let body = match (uploaded, url) {
            (Some(id), _) => json!({ "image_s3_id": format!("image_{id}") }),
            (None, Some(url)) => json!({ "image_url": url }),
            (None, None) => json!({ "remove_image_cover": true }),
        };
        self.send(Method::POST, &format!("/api/gen/{}/set_metadata/", escape(clip_id)), Some(&body)).map(drop)
    }

    /// Suno draws a picture from a description; its address.
    pub fn make_picture(&self, prompt: &str) -> Result<String> {
        sj::try_read_string(&self.send(Method::POST, "/api/gen/prompt_image/", Some(&json!({ "prompt": prompt })))?, "image_url")
            .filter(|u| !u.is_empty())
            .ok_or_else(|| Error::api(t("Suno didn't make a picture. Try another description.")))
    }

    pub fn set_playlist_cover(&self, playlist_id: &str, uploaded: &str) -> Result<()> {
        let body = json!({ "metadata": { "cover_image_s3_id": format!("image_{uploaded}") } });
        self.send(Method::PATCH, &format!("/api/playlist/v2/{}", escape(playlist_id)), Some(&body)).map(drop)
    }

    pub fn set_playlist_liked(&self, playlist_id: &str, liked: bool) -> Result<()> {
        let body = json!({ "reaction": if liked { Some("LIKE") } else { None } });
        self.send(Method::POST, &format!("/api/playlist_reaction/{}/update_reaction_type/", escape(playlist_id)), Some(&body)).map(drop)
    }

    pub fn set_playlist_saved(&self, playlist_id: &str, saved: bool) -> Result<()> {
        self.send(if saved { Method::POST } else { Method::DELETE }, &format!("/api/playlist/v2/{}/save", escape(playlist_id)), None).map(drop)
    }

    /// A draft album ("lp" or "ep") from a playlist's songs; its id.
    pub fn create_album(&self, name: &str, description: &str, kind: &str, from_playlist: &str) -> Result<String> {
        let body = json!({
            "name": name, "description": description, "playlist_type": "album", "album_type": kind, "album_status": "draft",
            "creation_source": "playlist_make_album", "source_playlist_id": from_playlist,
        });
        let root = sj::parse(&self.send(Method::POST, "/api/playlist/create/", Some(&body))?)?;
        let id = |v: Option<&Value>| v.and_then(|v| v.get("id")).and_then(Value::as_str).filter(|s| !s.is_empty()).map(str::to_string);
        id(Some(&root))
            .or_else(|| id(root.get("metadata")))
            .or_else(|| id(root.get("playlist")))
            .ok_or_else(|| Error::api(t("Suno didn't say whether the album was made. Refresh playlists to check.")))
    }

    pub fn publish_album(&self, album_id: &str) -> Result<()> {
        self.send(Method::POST, &format!("/api/playlist/v2/{}/album/publish", escape(album_id)), Some(&json!({ "use_album_cover": true }))).map(drop)
    }

    /// Get whole song: joins an extended song's parts into one.
    pub fn whole_song(&self, clip_id: &str) -> Result<Vec<SunoClip>> {
        Ok(new_clip(&self.send(Method::POST, "/api/generate/concat/v2/", Some(&json!({ "clip_id": clip_id })))?).into_iter().collect())
    }

    /// Splits a song into its stems (vocals, instruments).
    pub fn stems(&self, clip_id: &str) -> Result<Vec<SunoClip>> {
        sj::find_clips_by_id(&self.send(Method::POST, &format!("/api/edit/stems/{}", escape(clip_id)), Some(&json!({})))?)
    }

    /// Notifications: v3, or v2 where v3 isn't there.
    pub fn notifications(&self) -> Result<Vec<Notification>> {
        let reply = match self.send(Method::GET, "/api/notification/v3?include_hooks=true", None) {
            Err(Error::Api { status: Some(404 | 405), .. }) => {
                log::warn!("notification/v3 not available; using v2");
                self.send(Method::GET, "/api/notification/v2", None)?
            }
            other => other?,
        };
        Ok(discover::read_notifications(&sj::parse(&reply)?))
    }

    /// Marks every notification up to now as read, then clears the unread count.
    pub fn mark_notifications_read(&self) -> Result<()> {
        let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string();
        self.send(Method::POST, "/api/notification/v2/read", Some(&json!({ "all": true, "before_datetime_utc": now })))?;
        if let Err(e) = self.send(Method::POST, "/api/notification/v2/clear-badge", None) {
            log::warn!("Notification count not cleared: {e}");
        }
        Ok(())
    }

    /// Suno writes lyrics from a topic: POST /api/generate/lyrics/, then asked every few seconds until done.
    pub fn write_lyrics(&self, topic: &str) -> Result<LyricsResult> {
        let started: LyricsResult = serde_json::from_str(&self.send(Method::POST, "/api/generate/lyrics/", Some(&json!({ "prompt": topic })))?).unwrap_or_default();
        let Some(id) = started.id.filter(|i| !i.is_empty()) else { return Err(Error::api(t("Suno didn't start writing the lyrics."))) };
        for _ in 0..30 {
            thread::sleep(Duration::from_secs(2));
            let result: LyricsResult = serde_json::from_str(&self.send(Method::GET, &format!("/api/generate/lyrics/{}", escape(&id)), None)?).unwrap_or_default();
            if let Some(why) = result.error_message.as_deref().filter(|m| !m.trim().is_empty()) {
                return Err(Error::api(tf("Suno couldn't write those lyrics: {0}", &[&why])));
            }
            match result.status.as_deref() {
                Some("complete") => return Ok(result),
                Some("error") => return Err(Error::api(t("Suno couldn't write lyrics for that topic."))),
                _ => {}
            }
        }
        Err(Error::api(t("Suno took too long writing the lyrics. Try again.")))
    }

    /// Style boost: Suno rewrites the style into a fuller description, using the lyrics as a hint.
    pub fn boost_style(&self, style: &str, lyrics: Option<&str>, instrumental: bool) -> Result<String> {
        let mut body = json!({ "original_tags": style, "is_instrumental": instrumental });
        if let Some(lyrics) = lyrics.filter(|l| !l.trim().is_empty()) {
            body["lyrics"] = json!(lyrics);
        }
        let boosted = sj::try_read_string(&self.send(Method::POST, "/api/prompts/upsample", Some(&body))?, "upsampled").unwrap_or_default();
        if boosted.trim().is_empty() {
            return Err(Error::api(t("Suno gave no boosted style.")));
        }
        Ok(boosted.trim().to_string())
    }

    pub fn style_ideas(&self) -> Result<Vec<String>> {
        Ok(crate::create::read_style_ideas(&sj::parse(&self.send(Method::GET, "/api/prompts/suggestions", None)?)?))
    }

    /// Change my lyrics: the same body Suno's lyrics editor sends when you ask it to change your lyrics.
    pub fn cowrite_lyrics(&self, lyrics: &str, instruction: &str, title: &str, style: &str) -> Result<String> {
        let body = json!({
            "selected": lyrics, "context_before": "", "context_after": "", "instruction": instruction,
            "title": title.chars().take(100).collect::<String>(), "style": style, "mode": "apply_user_request",
            "references": [], "num_variants": null, "lyricist_id": null,
            "metadata": { "lyrics_model": "default", "enable_thinking": false },
            "create_session_token": null, "lyrics_project_id": null,
        });
        let edited = sj::try_read_string(&self.send(Method::POST, "/api/generate/cowrite-lyrics/", Some(&body))?, "edited_lyrics").unwrap_or_default();
        if edited.trim().is_empty() {
            return Err(Error::api(t("Suno sent back no lyrics.")));
        }
        Ok(edited.trim().to_string())
    }

    /// GET /api/lyrics-projects?limit=50&sort=updated_at, following next_cursor, up to 10 pages.
    pub fn lyrics_drafts(&self) -> Result<Vec<LyricsDraft>> {
        let mut drafts = Vec::new();
        let mut cursor: Option<String> = None;
        for _ in 0..10 {
            let path = match &cursor {
                Some(c) => format!("/api/lyrics-projects?limit=50&sort=updated_at&cursor={}", escape(c)),
                None => "/api/lyrics-projects?limit=50&sort=updated_at".to_string(),
            };
            let (more, next) = crate::create::read_lyrics_drafts(&sj::parse(&self.send(Method::GET, &path, None)?)?);
            drafts.extend(more);
            if next.is_none() || next == cursor {
                break;
            }
            cursor = next;
        }
        Ok(drafts)
    }

    pub fn lyrics_draft_text(&self, draft_id: &str) -> Result<Option<String>> {
        Ok(crate::create::read_lyrics_draft_text(&sj::parse(&self.send(Method::GET, &format!("/api/lyrics-projects/{}", escape(draft_id)), None)?)?))
    }

    /// A new draft holding `lyrics`: made, then its words saved (Suno's flush takes the whole lyrics).
    pub fn new_lyrics_draft(&self, title: &str, lyrics: &str) -> Result<()> {
        let name = crate::create::draft_title(title);
        let reply = self.send(Method::POST, "/api/lyrics-projects", Some(&json!({ "title": name })))?;
        let id = sj::try_read_string(&reply, "id").filter(|i| !i.is_empty()).ok_or_else(|| Error::api(t("Suno didn't make the draft.")))?;
        self.save_lyrics_draft(&id, lyrics)
    }

    pub fn save_lyrics_draft(&self, draft_id: &str, lyrics: &str) -> Result<()> {
        self.send(Method::POST, &format!("/api/lyrics-projects/{}/flush", escape(draft_id)), Some(&json!({ "lyrics": lyrics }))).map(drop)
    }

    pub fn rename_lyrics_draft(&self, draft_id: &str, title: &str) -> Result<()> {
        self.send(Method::PATCH, &format!("/api/lyrics-projects/{}", escape(draft_id)), Some(&json!({ "title": crate::create::draft_title(title) }))).map(drop)
    }

    pub fn delete_lyrics_draft(&self, draft_id: &str) -> Result<()> {
        self.send(Method::DELETE, &format!("/api/lyrics-projects/{}", escape(draft_id)), None).map(drop)
    }

    // ── Discover ────────────────────────────────────────────────────────

    /// GET /api/profiles/{handle}, 20 songs a page, newest first. Both sort fields are required (422 without them).
    pub fn creator(&self, handle: &str, page: u32) -> Result<CreatorPage> {
        let handle = handle.trim().trim_start_matches('@');
        let reply = self.send(Method::GET, &format!("/api/profiles/{}?clips_sort_by=created_at&playlists_sort_by=created_at&page={page}", escape(handle)), None)?;
        Ok(CreatorPage::read(&sj::parse(&reply)?, handle))
    }

    /// A creator's page with up to five pages of their songs, as Tin Pan 1 loads it.
    pub fn creator_songs(&self, handle: &str) -> Result<CreatorPage> {
        let mut first: Option<CreatorPage> = None;
        let mut songs: Vec<SunoClip> = Vec::new();
        for page in 1..=5 {
            let p = self.creator(handle, page)?;
            let (count, before) = (p.songs.len(), songs.len());
            for song in &p.songs {
                if !songs.iter().any(|s| s.id == song.id) {
                    songs.push(song.clone());
                }
            }
            let total = p.total_songs;
            first.get_or_insert(p);
            if count < 20 || songs.len() == before || songs.len() as i64 >= total {
                break;
            }
        }
        let mut page = first.unwrap_or_default();
        page.songs = songs;
        Ok(page)
    }

    /// POST /api/social/following-feed, newest first.
    pub fn following(&self) -> Result<Vec<SunoClip>> {
        let body = json!({ "page_size": 50, "ranking_method": "most_recent", "result_type": "all" });
        sj::find_clips(&self.send(Method::POST, "/api/social/following-feed", Some(&body))?)
    }

    /// GET /api/clips/get_similar/?id=, without the song itself.
    pub fn similar(&self, clip_id: &str) -> Result<Vec<SunoClip>> {
        let clips = sj::find_clips(&self.send(Method::GET, &format!("/api/clips/get_similar/?id={}", escape(clip_id)), None)?)?;
        Ok(clips.into_iter().filter(|c| c.id != clip_id).collect())
    }

    /// GET /api/clips/remixes?clip_id=: covers and remixes others made of this song.
    pub fn remixes(&self, clip_id: &str) -> Result<Vec<SunoClip>> {
        let clips = sj::find_clips(&self.send(Method::GET, &format!("/api/clips/remixes?clip_id={}", escape(clip_id)), None)?)?;
        Ok(clips.into_iter().filter(|c| c.id != clip_id).collect())
    }

    /// The song this one was made from: its own details say so for covers, extensions and whole songs;
    /// otherwise GET /api/clips/parent?clip_id= (404 when there's none).
    pub fn original(&self, clip: &SunoClip) -> Result<Option<SunoClip>> {
        let id = match clip.metadata.as_ref().and_then(crate::clip::ClipMetadata::original_id) {
            Some(id) => Some(id),
            None => match self.send(Method::GET, &format!("/api/clips/parent?clip_id={}", escape(&clip.id)), None) {
                Ok(reply) => discover::read_parent_id(&sj::parse(&reply)?, &clip.id),
                Err(Error::Api { status: Some(404), .. }) => None,
                Err(e) => return Err(e),
            },
        };
        match id {
            Some(id) => Ok(self.clips(&[id.as_str()])?.into_iter().next()),
            None => Ok(None),
        }
    }

    /// POST /api/unified/explore, up to 3 pages of shelves.
    pub fn explore(&self) -> Result<Vec<ExploreShelf>> {
        let mut shelves = Vec::new();
        let mut cursor: Option<String> = None;
        for _ in 0..3 {
            let (more, next) = discover::read_explore(&sj::parse(&self.send(Method::POST, "/api/unified/explore", Some(&json!({ "cursor": cursor })))?)?);
            shelves.extend(more);
            if next.is_none() || next == cursor {
                break;
            }
            cursor = next;
        }
        Ok(shelves)
    }

    /// GET /api/contests/, with each contest's own song where it has one.
    pub fn contests(&self) -> Result<(Vec<Contest>, Vec<SunoClip>)> {
        let contests = discover::read_contests(&sj::parse(&self.send(Method::GET, "/api/contests/", None)?)?);
        let ids: Vec<&str> = contests.iter().filter_map(|c| c.base_clip_id.as_deref()).collect();
        let songs = if ids.is_empty() { Vec::new() } else { self.clips(&ids)? };
        Ok((contests, songs))
    }

    /// GET /api/gen/{id}/comments, up to 10 pages.
    pub fn comments(&self, clip_id: &str) -> Result<Vec<Comment>> {
        let id = escape(clip_id);
        let mut result = Vec::new();
        let mut cursor: Option<String> = None;
        for _ in 0..10 {
            let path = match &cursor {
                Some(c) => format!("/api/gen/{id}/comments?cursor={}", escape(c)),
                None => format!("/api/gen/{id}/comments"),
            };
            let (comments, next) = discover::read_comments(&sj::parse(&self.send(Method::GET, &path, None)?)?);
            let none = comments.is_empty();
            result.extend(comments);
            if none || next.as_deref().is_none_or(str::is_empty) || next == cursor {
                break;
            }
            cursor = next;
        }
        Ok(result)
    }

    pub fn post_comment(&self, clip_id: &str, text: &str) -> Result<()> {
        let body = json!({ "content": text, "parent_id": null, "entity_type": "clip", "track_timestamp": null });
        self.send(Method::POST, &format!("/api/gen/{}/comment", escape(clip_id)), Some(&body)).map(drop)
    }

    // ── Playlists ───────────────────────────────────────────────────────

    /// Your playlists (GET /api/playlist/me), every page.
    pub fn playlists(&self) -> Result<Vec<Playlist>> {
        let mut result = Vec::new();
        for page in 1..=20 {
            let root = sj::parse(&self.send(Method::GET, &format!("/api/playlist/me?page={page}&show_trashed=false&show_sharelist=false"), None)?)?;
            let Some(list) = root.get("playlists").and_then(Value::as_array) else { break };
            let before = result.len();
            result.extend(list.iter().filter(|p| p.is_object()).map(Playlist::read));
            let total = root.get("num_total_results").and_then(sj::as_int).map_or(result.len(), |n| n as usize);
            if result.len() == before || result.len() >= total {
                break;
            }
        }
        Ok(result)
    }

    /// A playlist's songs (GET /api/playlist/{id}/), every page; without a total, until a page brings nothing new.
    pub fn playlist_songs(&self, playlist_id: &str) -> Result<Vec<SunoClip>> {
        let id = escape(playlist_id);
        let mut result: Vec<SunoClip> = Vec::new();
        for page in 1..=50 {
            let root = sj::parse(&self.send(Method::GET, &format!("/api/playlist/{id}/?page={page}"), None)?)?;
            let total = root.get("num_total_results").and_then(sj::as_int);
            let clips = root.get("playlist_clips").and_then(Value::as_array).map(|list| {
                list.iter().filter_map(|entry| SunoClip::from_value(entry.get("clip").unwrap_or(entry))).collect::<Vec<_>>()
            });
            let known: std::collections::HashSet<String> = result.iter().map(|c| c.id.clone()).collect();
            let added: Vec<SunoClip> = clips.unwrap_or_default().into_iter().filter(|c| !known.contains(&c.id)).collect();
            let none = added.is_empty();
            result.extend(added);
            if none || total.is_some_and(|t| result.len() as i64 >= t) {
                break;
            }
        }
        Ok(result)
    }

    pub fn create_playlist(&self, name: &str) -> Result<Playlist> {
        let root = sj::parse(&self.send(Method::POST, "/api/playlist/create/", Some(&json!({ "name": name })))?)?;
        let mut made = Playlist::read(&root);
        if made.id.is_empty() {
            return Err(Error::api(t("Suno didn't say whether the playlist was made. Refresh playlists to check.")));
        }
        if made.name.is_empty() {
            made.name = name.to_string();
        }
        made.song_count.get_or_insert(0);
        Ok(made)
    }

    pub fn rename_playlist(&self, playlist_id: &str, name: &str, description: &str) -> Result<()> {
        let reply = self.send(Method::POST, "/api/playlist/set_metadata", Some(&json!({ "playlist_id": playlist_id, "name": name, "description": description })))?;
        match sj::try_read_string(&reply, "moderation_error_message").filter(|m| !m.trim().is_empty()) {
            Some(why) => Err(Error::api(tf("Suno didn't accept that name: {0}", &[&why]))),
            None => Ok(()),
        }
    }

    pub fn trash_playlist(&self, playlist_id: &str) -> Result<()> {
        self.send(Method::POST, "/api/playlist/trash/", Some(&json!({ "playlist_id": playlist_id }))).map(drop)
    }

    pub fn add_to_playlist(&self, playlist_id: &str, clip_ids: &[&str]) -> Result<()> {
        for chunk in clip_ids.chunks(100) {
            let reply = self.send(Method::POST, &format!("/api/playlist/v2/{}/tracks/add", escape(playlist_id)), Some(&json!({ "clip_ids": chunk })))?;
            if playlist::change_failed(&reply) {
                return Err(Error::api(t("Suno didn't add it to the playlist.")));
            }
        }
        Ok(())
    }

    pub fn remove_from_playlist(&self, playlist_id: &str, clip_ids: &[&str]) -> Result<()> {
        for chunk in clip_ids.chunks(100) {
            let reply = self.send(Method::POST, &format!("/api/playlist/v2/{}/tracks/remove", escape(playlist_id)), Some(&json!({ "clip_ids": chunk })))?;
            if playlist::change_failed(&reply) {
                return Err(Error::api(t("Suno didn't remove it from the playlist.")));
            }
        }
        Ok(())
    }

    /// Moves a song to `index` (0 first) in the playlist.
    pub fn move_in_playlist(&self, playlist_id: &str, clip_id: &str, index: usize) -> Result<()> {
        let body = json!({ "positions": [{ "clip_id": clip_id, "index": index }] });
        self.send(Method::POST, &format!("/api/playlist/v2/{}/tracks/reorder-by-index", escape(playlist_id)), Some(&body)).map(drop)
    }

    // ── Song files ──────────────────────────────────────────────────────

    /// The song's file, for playing it, by the same route as Tin Pan 1's free downloads (which work):
    /// an unencrypted progressive file as it is; otherwise the song's own encrypted file address (from
    /// its media_urls, then the rights reply, then the CDN's usual one) decrypted with the content-rights
    /// key. Uses none of the account's downloads.
    pub fn song_audio(&self, clip: &SunoClip) -> Result<Vec<u8>> {
        let (media_url, encrypted) = drm::pick_media_url(clip, true);
        if !encrypted && !drm::is_forbidden_decoy(Some(&media_url)) {
            match self.fetch_file(&media_url) {
                Ok(data) => return Ok(data),
                Err(e) if e.status() == Some(403) => log::warn!("The unencrypted file of {} was refused; using the encrypted one", clip.id),
                Err(e) => return Err(e),
            }
        }
        let (token, device_id) = self.token(false)?;
        let rights = self.fetch_rights(&clip.id, &token, &device_id)?;
        let url = Some(media_url)
            .filter(|u| encrypted && !drm::is_forbidden_decoy(Some(u)))
            .or_else(|| rights.media_url.clone().filter(|u| !drm::is_forbidden_decoy(Some(u))))
            .unwrap_or_else(|| drm::default_media_url(&clip.id));
        let (key, iv) = drm::unwrap_rights(&rights, &clip.id, Some(&token))?;
        let mut data = self.fetch_file(&url)?;
        drm::apply_aes_ctr(&mut data, &key, &iv)?;
        if drm::detect_container(&data[..data.len().min(32)]) == "unknown" {
            log::warn!("{} decrypted to something that isn't a song file", clip.id);
            return Err(Error::api(t("Suno's song file didn't decode.")));
        }
        Ok(data)
    }

    /// A file from Suno's file host (no sign-in needed there).
    fn fetch_file(&self, url: &str) -> Result<Vec<u8>> {
        let host = url.split('/').nth(2).unwrap_or("");
        let response = self.http.get(url).timeout(Duration::from_secs(60)).send().map_err(|e| {
            log::warn!("Song file from {host}: {e}");
            Error::api(t("Can't reach Suno. Check your internet connection."))
        })?;
        let status = response.status().as_u16();
        if !response.status().is_success() {
            log::warn!("Song file from {host} answered {status}");
            return Err(Error::api_status(tf("The download failed ({0}).", &[&status]), status));
        }
        Ok(response.bytes().map_err(|_| Error::api(t("Can't reach Suno. Check your internet connection.")))?.to_vec())
    }

    fn fetch_rights(&self, clip_id: &str, token: &str, device_id: &str) -> Result<drm::SunoRights> {
        let shapes = [
            json!({ "content_params": { "content_id": clip_id, "content_type": "clip" } }),
            json!({ "content_id": clip_id, "content_type": "clip" }),
            json!({ "content_id": clip_id }),
        ];
        let mut last_status = 0;
        for host in self.host_order() {
            for body in &shapes {
                let request = self
                    .request(Method::POST, &format!("{host}/api/mango/rights"), token, device_id, host == HOSTS[0])
                    .header("Content-Type", "application/json")
                    .body(body.to_string());
                let Ok(response) = request.send() else { continue };
                last_status = response.status().as_u16();
                if !response.status().is_success() {
                    continue;
                }
                if let Some(rights) = response.text().ok().and_then(|text| drm::parse_rights(&text, clip_id)) {
                    return Ok(rights);
                }
            }
        }
        log::warn!("No content rights for {clip_id} (last answer {last_status})");
        let status = if last_status == 0 { 403 } else { last_status };
        Err(Error::api_status(tf("The download failed ({0}).", &[&status]), status))
    }
}

/// Writes a file through path.part, renamed into place when it's whole; the part is removed on failure.
fn write_file(path: &std::path::Path, write: impl FnOnce(&mut std::fs::File) -> std::io::Result<()>) -> Result<()> {
    let part = path.with_extension(format!("{}.part", path.extension().and_then(|e| e.to_str()).unwrap_or("")));
    let saved = std::fs::File::create(&part).and_then(|mut file| write(&mut file)).and_then(|()| std::fs::rename(&part, path));
    if let Err(e) = saved {
        let _ = std::fs::remove_file(&part);
        log::warn!("Couldn't save a download: {e}");
        return Err(Error::Io(t("Couldn't save the file. Check the download folder in Settings.")));
    }
    Ok(())
}

pub const ANY: &str = "Any";
pub const ONLY: &str = "True";
pub const HIDE: &str = "False";

/// Every on/off filter Suno's web library has (its bundle's default filter object), except bpm and key:
/// Suno's field name, whether it's sent as {"presence": value}, and its default.
pub const FEED_FILTERS: [(&str, bool, &str); 13] = [
    ("trashed", false, HIDE),
    ("stem", true, HIDE),
    ("fromStudioProject", true, HIDE),
    ("liked", false, ANY),
    ("public", false, ANY),
    ("upload", false, ANY),
    ("cover", true, ANY),
    ("extend", true, ANY),
    ("remaster", true, ANY),
    ("concat", true, ANY),
    ("persona", true, ANY),
    ("playlist", true, ANY),
    ("unlocked", false, ANY),
];

/// Suno's sort choices: id, field and direction.
pub const SORTS: [(&str, &str, &str); 4] = [
    ("newest", "created_at", "desc"),
    ("oldest", "created_at", "asc"),
    ("most-liked", "upvote_count", "desc"),
    ("least-liked", "upvote_count", "asc"),
];

fn valid<'a>(value: Option<&'a str>, fallback: &'a str) -> &'a str {
    value.filter(|v| [ANY, ONLY, HIDE].contains(v)).unwrap_or(fallback)
}

/// The filter object Suno's web library sends: every filter with an explicit value, as in its bundle's
/// default filter object. Hiding stems also hides their complements, as the web app does. Suno's server
/// fails (500) on a filter object missing these or with the presence ones unwrapped.
pub fn feed_filters(library: &LibraryConfig, search: Option<&str>) -> Value {
    let mut filters = serde_json::Map::new();
    filters.insert("disliked".into(), json!(valid(Some(&library.disliked), HIDE)));
    for (field, presence, default) in FEED_FILTERS {
        let value = valid(library.show.get(field).map(String::as_str), default);
        filters.insert(field.into(), if presence { json!({ "presence": value }) } else { json!(value) });
    }
    let stems = valid(library.show.get("stem").map(String::as_str), HIDE);
    filters.insert("stemComplement".into(), json!(if stems == HIDE { HIDE } else { ANY }));
    let (_, sort_by, direction) = SORTS.iter().find(|(id, _, _)| Some(*id) == library.sort.as_deref()).copied().unwrap_or(SORTS[0]);
    filters.insert("sort".into(), json!({ "sortBy": sort_by, "sortDirection": direction }));
    if !library.workspace_id.is_empty() {
        filters.insert("workspace".into(), json!({ "presence": "True", "workspaceId": library.workspace_id }));
    }
    if let Some(search) = search.map(str::trim).filter(|s| !s.is_empty()) {
        filters.insert("searchText".into(), json!(search));
    }
    Value::Object(filters)
}

/// {"token":"base64 of {"timestamp":ms}"}, as the Suno web app sends.
pub fn browser_token(now_ms: i64) -> String {
    let payload = format!(r#"{{"timestamp":{now_ms}}}"#);
    format!(r#"{{"token":"{}"}}"#, base64::engine::general_purpose::STANDARD.encode(payload))
}

/// The host didn't take the request, so the other may: the route isn't there (404, 405: nothing was done),
/// the site host doesn't accept this sign-in (401), or a web page came back instead of JSON (a gateway).
/// Reads also move on from a server error page. Any other JSON answer is Suno's own.
pub fn not_taken_here(method: &Method, status: u16, body: &str, site_host: bool) -> bool {
    if (200..300).contains(&status) {
        return false;
    }
    if status == 404 || status == 405 || (status == 401 && site_host) {
        return true;
    }
    let trimmed = body.trim_start();
    if trimmed.starts_with('{') || trimmed.starts_with('[') {
        return false;
    }
    matches!(status, 403 | 421 | 502 | 503 | 520..=524) || (*method == Method::GET && status >= 500)
}

pub fn looks_like_stale_token(body: &str) -> bool {
    let lower = body.to_lowercase();
    ["token validation failed", "jwt expired", "jwt is expired", "invalid jwt", "not authenticated", "unauthenticated"]
        .iter()
        .any(|s| lower.contains(s))
}

/// Suno's 429 body says {"retryable": true} when the request may simply be sent again.
pub fn retryable_rate_limit(body: &str) -> bool {
    serde_json::from_str::<Value>(body).ok().and_then(|v| v.get("retryable").and_then(Value::as_bool)).unwrap_or(false)
}

fn describe_failure(status: u16, body: &str) -> String {
    let lower = body.to_lowercase();
    let has = |words: &[&str]| words.iter().any(|w| lower.contains(w));
    // A whole word, as Tin Pan 1's \b(pro|premier|plan)\b: "pro" but not "problem".
    let word = |w: &str| lower.split(|c: char| !c.is_alphanumeric()).any(|x| x == w);
    match status {
        401 => t("Suno turned the request down. If it keeps happening, sign in again from Settings."),
        403 if word("pro") || word("premier") || word("plan") || has(&["subscri"]) => t("That needs a Pro or Premier plan."),
        403 => t("Suno says this account can't do that."),
        402 => t("Not enough credits."),
        404 => t("Suno couldn't find that song, or this feature moved."),
        400 | 422 if has(&["credit"]) => t("Not enough credits."),
        400 | 422 if has(&["moderat", "inappropriate", "policy"]) => t("Suno's content rules blocked this. Try different words."),
        400 | 422 if has(&["captcha"]) => t("Suno didn't accept the captcha. Try again."),
        400 | 422 if has(&["length", "too long"]) => t("Something is too long for this model."),
        400 | 422 => t("Suno didn't accept those settings.") + &detail_from(body),
        429 => t("Suno says too many requests. Wait a minute and try again."),
        500.. => tf("Suno had a problem on its side ({0}). Try again later.", &[&status]),
        _ => tf("Suno refused the request ({0}).", &[&status]),
    }
}

/// Suno's own "detail" text, when it's short enough to read out.
fn detail_from(body: &str) -> String {
    match sj::try_read_string(body, "detail").map(|d| d.trim().to_string()).filter(|d| !d.is_empty() && d.chars().count() <= 160) {
        Some(detail) => format!(" {}", tf("Suno says: {0}", &[&detail])),
        None => String::new(),
    }
}

fn new_clip(json: &str) -> Result<SunoClip> {
    serde_json::from_str::<Value>(json)
        .ok()
        .and_then(|v| SunoClip::from_value(&v))
        .filter(|c| !c.id.is_empty())
        .ok_or_else(|| Error::api(t("Suno didn't say which song it made. Refresh the library to check.")))
}

/// A stand-in for the clip a crop or fade is making, until the watcher reads the real one.
fn action_clip(json: &str, title: &str) -> Result<SunoClip> {
    match sj::try_read_string(json, "action_clip_id").map(|id| id.trim().to_string()).filter(|id| !id.is_empty()) {
        Some(id) => Ok(SunoClip { id, title: Some(title.to_string()), status: Some("submitted".into()), ..SunoClip::default() }),
        None => Err(Error::api(t("Suno didn't say which song it made. Refresh the library to check."))),
    }
}

pub(crate) fn escape(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

fn queryless(path: &str) -> &str {
    path.split('?').next().unwrap_or(path)
}

fn truncate(s: &str, max: usize) -> &str {
    match s.char_indices().nth(max) {
        Some((i, _)) => &s[..i],
        None => s,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_other_host_is_tried_only_when_this_one_did_nothing() {
        assert!(not_taken_here(&Method::POST, 404, "{}", false));
        assert!(not_taken_here(&Method::POST, 401, "{}", true));
        assert!(!not_taken_here(&Method::POST, 401, "{}", false));
        assert!(not_taken_here(&Method::POST, 403, "<html>blocked</html>", false));
        assert!(!not_taken_here(&Method::POST, 403, r#"{"detail":"no"}"#, false));
        assert!(not_taken_here(&Method::GET, 500, "oops", false));
        assert!(!not_taken_here(&Method::POST, 500, "oops", false), "a POST may already have been done");
    }

    #[test]
    fn stale_tokens_and_retryable_rate_limits_are_recognised() {
        assert!(looks_like_stale_token(r#"{"detail":"JWT expired"}"#));
        assert!(!looks_like_stale_token(r#"{"detail":"nope"}"#));
        assert!(retryable_rate_limit(r#"{"retryable":true}"#));
        assert!(!retryable_rate_limit("<html>"));
    }

    #[test]
    fn the_filters_match_suno_web_librarys_shape() {
        let mut library = LibraryConfig::default();
        let f = feed_filters(&library, None);
        assert_eq!(f["stem"], json!({ "presence": "False" }));
        assert_eq!(f["trashed"], json!("False"));
        assert_eq!(f["stemComplement"], json!("False"));
        assert_eq!(f["sort"], json!({ "sortBy": "created_at", "sortDirection": "desc" }));
        assert_eq!(f.as_object().unwrap().len(), 16, "disliked, 13 filters, stemComplement and sort");
        library.show.insert("stem".into(), "Any".into());
        library.show.insert("liked".into(), "nonsense".into());
        library.sort = Some("most-liked".into());
        library.workspace_id = "w1".into();
        let f = feed_filters(&library, Some(" rain "));
        assert_eq!((f["stemComplement"].clone(), f["liked"].clone(), f["searchText"].clone()), (json!("Any"), json!("Any"), json!("rain")));
        assert_eq!(f["sort"]["sortBy"], json!("upvote_count"));
        assert_eq!(f["workspace"]["workspaceId"], json!("w1"));
    }

    #[test]
    fn the_browser_token_wraps_the_time() {
        assert_eq!(browser_token(1), r#"{"token":"eyJ0aW1lc3RhbXAiOjF9"}"#);
    }

    #[test]
    fn failures_are_said_plainly() {
        assert_eq!(describe_failure(402, ""), "Not enough credits.");
        assert_eq!(describe_failure(403, "Upgrade to Pro to use this"), "That needs a Pro or Premier plan.");
        assert_eq!(describe_failure(403, "A problem with this account"), "Suno says this account can't do that.");
        assert_eq!(describe_failure(422, r#"{"detail":"Lyrics are too long"}"#), "Something is too long for this model.");
        assert_eq!(describe_failure(422, r#"{"detail":"Bad tag"}"#), "Suno didn't accept those settings. Suno says: Bad tag");
        assert_eq!(describe_failure(403, "Upgrade to a Premier plan"), "That needs a Pro or Premier plan.");
        assert_eq!(describe_failure(503, ""), "Suno had a problem on its side (503). Try again later.");
    }
}
