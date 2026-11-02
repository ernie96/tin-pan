//! Suno's own player, in a signed-in web view kept off screen, as Tin Pan 1's fallback: when a song can't be
//! played in Tin Pan itself (a file it can't decode, Suno refusing the key, no connection to its file host),
//! the song's page on suno.com is opened, its Play button pressed, and its audio element driven from here.
//! Streaming this way uses none of the account's downloads.
//!
//! A script set on every page reports what the audio element does into `window.__tinpan`, which the window's
//! timer reads twice a second.

use std::time::{Duration, Instant};

use wxdragon::prelude::*;
use wxdragon::widgets::{WebView, WebViewUserScriptInjectionTime};

/// How long a song may take to start, per load of its page; a cold first load gets one more try.
const START_LIMIT: Duration = Duration::from_secs(30);

/// Tin Pan 1's start-up script, reporting by a variable instead of a message: on a song page it presses Suno's
/// Play button (again, on a cold page that ignores the first press), then follows the audio element.
const BOOT_SCRIPT: &str = r#"
(() => {
  if (window.__tinpan || !location.pathname.startsWith('/song/')) return;
  const s = window.__tinpan = { state: 'loading', t: 0, d: 0, paused: false };
  let hooked = null, clicks = 0, clickedAt = 0, started = false, tries = 0;
  const vol = parseFloat((location.hash.match(/cv=([0-9.]+)/) || [])[1]);
  // The audio output Tin Pan plays on, by its system name; empty is the default.
  let sink = decodeURIComponent((location.hash.match(/[#&]co=([^&]*)/) || [])[1] || '');
  // Tin Pan only plays here; the page may see the names of the audio outputs, but is never let record.
  try { if (navigator.mediaDevices) navigator.mediaDevices.getUserMedia = () => Promise.reject(new DOMException('Not allowed', 'NotAllowedError')); } catch (e) {}
  const applySink = a => {
    if (!a.setSinkId) { if (sink) s.sink = 'failed:unsupported'; return; }
    if (!sink) { a.setSinkId('').catch(() => {}); s.sink = ''; return; }
    navigator.mediaDevices.enumerateDevices().then(list => {
      const outs = list.filter(x => x.kind === 'audiooutput');
      const d = outs.find(x => x.label === sink && x.deviceId !== 'default' && x.deviceId !== 'communications') || outs.find(x => x.label === sink);
      if (!d) { s.sink = 'failed:' + (outs.some(x => x.label) ? 'notfound' : 'nolabels'); return; }
      return a.setSinkId(d.deviceId).then(() => { s.sink = 'ok'; });
    }).catch(e => { s.sink = 'failed:' + String((e && e.name) || e); });
  };
  window.__tinpanSink = label => { sink = label || ''; if (hooked) applySink(hooked); return 'ok'; };
  const hook = a => {
    if (hooked === a) return;
    hooked = a;
    if (sink) applySink(a);
    if (!isNaN(vol)) { a.volume = vol; a.addEventListener('play', () => { if (!started) a.volume = vol; }); }
    const time = () => { s.t = a.currentTime; s.d = a.duration || 0; s.paused = a.paused; };
    a.addEventListener('playing', () => { started = true; s.state = 'playing'; time(); });
    a.addEventListener('pause', time);
    a.addEventListener('timeupdate', () => { time(); if (!a.paused && a.currentTime > 0) { started = true; s.state = 'playing'; } });
    a.addEventListener('ended', () => { s.state = 'ended'; });
    a.addEventListener('error', () => { s.state = 'error'; });
    if (!a.paused && a.currentTime > 0) { started = true; s.state = 'playing'; time(); }
  };
  const timer = setInterval(() => {
    tries++;
    const a = document.getElementById('active-audio-play');
    if (a) hook(a);
    if (!started && a && a.paused && clicks < 6 && Date.now() - clickedAt > 4000) {
      const b = document.querySelector('button[aria-label="Play"]');
      if (b) { clicks++; clickedAt = Date.now(); b.click(); }
    }
    if (clicks === 0 && tries > 80) { clearInterval(timer); if (!started) s.state = 'nobutton'; }
    if (tries > 2400) clearInterval(timer);
  }, 250);
})();
"#;

/// What the page did since it was last looked at.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    Started,
    Ended,
    /// It couldn't play; true when Suno's page didn't offer the song at all.
    Failed { no_button: bool },
    /// The chosen output couldn't be used, so the song plays on the default; true when it isn't connected.
    OutputFailed { not_found: bool },
}

pub struct WebPlayer {
    frame: Frame,
    view: WebView,
    parent: Frame,
    clip_id: Option<String>,
    started: bool,
    retried: bool,
    since: Instant,
    position: f64,
    duration: f64,
    paused: bool,
    /// The output the page plays on ("" the default), and the one it last said it couldn't use.
    output: String,
    output_failed_for: Option<String>,
}

impl WebPlayer {
    /// The off-screen window, made the first time a song needs it. It never takes the focus for long.
    pub fn new(parent: &Frame) -> Self {
        let frame = Frame::builder()
            .with_parent(parent)
            .with_title("Suno player")
            .with_size(Size::new(900, 700))
            .with_position(Point::new(-20_000, -20_000))
            .with_style(FrameStyle::Caption | FrameStyle::NoTaskbar | FrameStyle::ToolWindow)
            .build();
        let view = WebView::builder(&frame).build();
        let sizer = BoxSizer::builder(Orientation::Vertical).build();
        sizer.add(&view, 1, SizerFlag::Expand, 0);
        frame.set_sizer(sizer, true);
        view.add_user_script(BOOT_SCRIPT, WebViewUserScriptInjectionTime::AtDocumentStart);
        // Alt+F4 lands here when Windows has put the focus here: that means "close Tin Pan", and the player
        // must stay until then (Tin Pan 1.9.1). Closing the main window takes this one with it.
        let main = *parent;
        frame.on_close(move |event| {
            event.skip(false);
            main.close(true);
        });
        crate::out_of_reach::set(&frame, true);
        crate::out_of_reach::give_focus_back(&frame, main);
        frame.show(true);
        parent.raise();
        Self { frame, view, parent: *parent, clip_id: None, started: false, retried: false, since: Instant::now(), position: 0.0, duration: 0.0, paused: false, output: String::new(), output_failed_for: None }
    }

    /// The output songs play on ("" the default). A song playing moves over at once.
    pub fn set_output(&mut self, name: &str) {
        if name == self.output {
            return;
        }
        self.output = name.to_string();
        self.output_failed_for = None;
        // Back on the default, the page no longer needs to see the outputs, so that's taken back.
        crate::signin::browser::allow_device_names(&self.view, !name.is_empty());
        if self.clip_id.is_some() {
            let quoted = js_string(name);
            let _ = self.view.run_script(&format!("window.__tinpanSink ? window.__tinpanSink({quoted}) : 'none'"));
        }
    }

    /// Signing out: the page's Suno sign-in goes with the account's, at once.
    pub fn forget_sign_in(&mut self) {
        self.stop();
        crate::signin::browser::read_cookies(&self.view, &crate::signin::COOKIE_SITES, true, |_| {});
    }

    /// Opens the song's page and lets the script start it; `poll` says when it has.
    pub fn play(&mut self, clip_id: &str, volume: f32) {
        self.clip_id = Some(clip_id.to_string());
        self.started = false;
        self.retried = false;
        self.position = 0.0;
        self.duration = 0.0;
        self.paused = false;
        self.load(volume);
    }

    fn load(&mut self, volume: f32) {
        let Some(id) = &self.clip_id else { return };
        self.since = Instant::now();
        let output = if self.output.is_empty() { String::new() } else { format!("&co={}", percent_encode(&self.output)) };
        self.view.load_url(&format!("https://suno.com/song/{id}#cv={volume:.2}{output}"));
        // Loading a page can take the focus; give it back.
        self.parent.raise();
    }

    pub fn stop(&mut self) {
        self.clip_id = None;
        self.started = false;
        self.view.load_url("about:blank");
    }

    /// Whether a song is playing or paused here (or still starting).
    pub fn is_active(&self) -> bool {
        self.clip_id.is_some()
    }

    fn script(&self, js: &str) -> Option<String> {
        Some(self.view.run_script(js)?.trim().trim_matches('"').to_string())
    }

    /// Runs a statement on the page's audio element, if there is one.
    fn on_audio(&self, statement: &str) {
        if self.clip_id.is_some() {
            let _ = self.view.run_script(&format!("(() => {{ const a = document.getElementById('active-audio-play'); if (a) {{ {statement} }} return 'ok'; }})()"));
        }
    }

    /// Reads the page: where the song is, and what happened.
    pub fn poll(&mut self, volume: f32) -> Option<Event> {
        self.clip_id.as_ref()?;
        let state = self.script("(() => { const s = window.__tinpan; return s ? [s.state, s.t, s.d, s.paused, s.sink || ''].join('|') : 'none'; })()").unwrap_or_default();
        let mut parts = state.split('|');
        let kind = parts.next().unwrap_or("none").to_string();
        let number = |p: Option<&str>| p.and_then(|v| v.parse::<f64>().ok()).filter(|v| v.is_finite()).unwrap_or(0.0);
        let (t, d) = (number(parts.next()), number(parts.next()));
        let paused = parts.next() == Some("true");
        // Once per output: every song after that would only repeat it.
        if let Some(why) = parts.next().and_then(|s| s.strip_prefix("failed:"))
            && !self.output.is_empty()
            && self.output_failed_for.as_deref() != Some(self.output.as_str())
        {
            log::warn!("Suno's player couldn't play on \"{}\": {why}", self.output);
            self.output_failed_for = Some(self.output.clone());
            return Some(Event::OutputFailed { not_found: why == "notfound" });
        }
        if kind == "playing" || kind == "ended" {
            self.position = t;
            if d > 0.0 {
                self.duration = d;
            }
            self.paused = paused;
        }
        match kind.as_str() {
            "playing" if !self.started => {
                self.started = true;
                Some(Event::Started)
            }
            "ended" if self.started => {
                self.clip_id = None;
                self.view.load_url("about:blank");
                Some(Event::Ended)
            }
            "nobutton" if !self.started => {
                self.clip_id = None;
                Some(Event::Failed { no_button: true })
            }
            "error" => {
                self.clip_id = None;
                Some(Event::Failed { no_button: false })
            }
            _ if !self.started && self.since.elapsed() > START_LIMIT => {
                // A cold first load can miss; one fresh load nearly always works.
                if self.retried {
                    log::warn!("Suno's player didn't start the song: {state}");
                    self.clip_id = None;
                    return Some(Event::Failed { no_button: false });
                }
                log::warn!("Suno's player didn't start the song on the first load ({state}); trying once more");
                self.retried = true;
                self.load(volume);
                None
            }
            _ => None,
        }
    }

    pub fn position(&self) -> Duration {
        Duration::from_secs_f64(self.position.max(0.0))
    }

    pub fn total(&self) -> Option<Duration> {
        (self.duration > 0.0).then(|| Duration::from_secs_f64(self.duration))
    }

    pub fn is_paused(&self) -> bool {
        self.paused
    }

    /// Pauses or resumes; whether it's playing now.
    pub fn toggle_pause(&mut self) -> bool {
        self.paused = !self.paused;
        self.on_audio(if self.paused { "a.pause();" } else { "a.play();" });
        !self.paused
    }

    pub fn seek_to(&mut self, seconds: f64) -> Duration {
        let end = if self.duration > 0.0 { self.duration - 0.5 } else { seconds };
        self.position = seconds.clamp(0.0, end.max(0.0));
        self.on_audio(&format!("a.currentTime = {};", self.position));
        self.position()
    }

    pub fn set_volume(&self, volume: f32) {
        self.on_audio(&format!("a.volume = {volume:.2};"));
    }
}

/// A JavaScript string literal holding `text`.
fn js_string(text: &str) -> String {
    let mut out = String::from('"');
    for c in text.chars() {
        match c {
            '"' | '\\' => {
                out.push('\\');
                out.push(c);
            }
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// The output's name as it goes in the page's address.
fn percent_encode(text: &str) -> String {
    text.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect()
}

impl Drop for WebPlayer {
    fn drop(&mut self) {
        self.frame.destroy();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_names_reach_the_page_unchanged() {
        assert_eq!(js_string(r#"Speakers ("Realtek")\x"#), r#""Speakers (\"Realtek\")\\x""#);
        assert_eq!(percent_encode("Speakers (Realtek(R) Audio)"), "Speakers%20%28Realtek%28R%29%20Audio%29");
    }
}
