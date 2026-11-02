//! Plays a song file Tin Pan already has in memory, on the output chosen in Settings or the system's
//! default. Outputs go by the names the system gives them ("Speakers (Realtek(R) Audio)"), the same names
//! Tin Pan 1 saves, so both keep the same choice.

use std::time::Duration;

use rodio::Source;
use rodio::cpal::traits::{DeviceTrait, HostTrait};

use crate::song_source::SongSource;
use crate::web_player::{Event, WebPlayer};

/// Plays in Tin Pan itself, or through Suno's own player (web_player.rs) when that can't; every control below
/// works on whichever is playing.
pub struct Player {
    output: Option<rodio::MixerDeviceSink>,
    player: Option<rodio::Player>,
    /// The playing song's file, kept so it can carry on on another output.
    data: Option<Vec<u8>>,
    total: Option<Duration>,
    volume: f32,
    /// Suno's player, made the first time a song needs it, and whether the song is playing there.
    web: Option<WebPlayer>,
    on_web: bool,
    /// The output's name ("" the default), which Suno's player is told too.
    device: String,
}

/// The outputs the system has switched on, by name, sorted, each once.
pub fn outputs() -> Vec<String> {
    let host = rodio::cpal::default_host();
    let mut names: Vec<String> = match host.output_devices() {
        Ok(devices) => devices.filter_map(|d| d.description().ok().map(|n| n.name().trim().to_string())).filter(|n| !n.is_empty()).collect(),
        Err(e) => {
            log::warn!("Couldn't list the audio outputs: {e}");
            Vec::new()
        }
    };
    names.sort_by_key(|n| n.to_lowercase());
    names.dedup();
    names
}

/// Opens the named output ("" for the default). When it can't be used (unplugged, say), the default is
/// opened instead, and the second value says so.
fn open(name: &str) -> (Option<rodio::MixerDeviceSink>, bool) {
    let quiet = |mut sink: rodio::MixerDeviceSink| {
        // Quitting drops it on purpose; that isn't a failure worth logging.
        sink.log_on_drop(false);
        sink
    };
    if !name.is_empty() {
        let found = rodio::cpal::default_host()
            .output_devices()
            .ok()
            .and_then(|mut devices| devices.find(|d| d.description().is_ok_and(|n| n.name().trim() == name)));
        match found.map(|d| rodio::DeviceSinkBuilder::from_device(d).and_then(rodio::DeviceSinkBuilder::open_stream)) {
            Some(Ok(sink)) => return (Some(quiet(sink)), false),
            Some(Err(e)) => log::warn!("Output {name} couldn't be opened: {e}"),
            None => log::warn!("Output {name} isn't connected"),
        }
    }
    let sink = rodio::DeviceSinkBuilder::open_default_sink().map(quiet).map_err(|e| log::error!("No audio output: {e}")).ok();
    (sink, !name.is_empty())
}

impl Player {
    /// Opens `device` ("" for the default); the second value is true when it couldn't be and the default is used.
    pub fn new(volume: f32, device: &str) -> (Self, bool) {
        let (output, fell_back) = open(device);
        (Self { output, player: None, data: None, total: None, volume: volume.clamp(0.0, 1.0), web: None, on_web: false, device: device.to_string() }, fell_back)
    }

    /// Moves to another output. A playing song carries on there from where it was, paused if it was.
    /// True when that output couldn't be used and the default plays instead.
    pub fn set_device(&mut self, device: &str) -> bool {
        self.device = device.to_string();
        if let Some(web) = &mut self.web {
            web.set_output(device);
        }
        // A song on Suno's player moves there through its page.
        if self.on_web {
            let (output, fell_back) = open(device);
            self.output = output;
            return fell_back;
        }
        let at = self.position();
        let paused = self.is_paused();
        let data = if self.is_loaded() { self.data.clone() } else { None };
        self.stop();
        self.output = None;
        let (output, fell_back) = open(device);
        self.output = output;
        if let Some(data) = data
            && self.play(data).is_ok()
            && let Some(player) = &self.player
        {
            if let Err(e) = player.try_seek(at) {
                log::warn!("Couldn't carry on where the song was: {e}");
            }
            if paused {
                player.pause();
            }
        }
        fell_back
    }

    /// A soft 30 ms tick over the song, on the same output: the cue that the arrow keys reached the playing song.
    /// Made here, as in Tin Pan 1, so there's no sound file to ship.
    pub fn click(&self) {
        const RATE: u32 = 44_100;
        let Some(output) = &self.output else { return };
        let samples: Vec<f32> = (0..RATE * 30 / 1000)
            .map(|i| {
                let t = f64::from(i) / f64::from(RATE);
                ((2.0 * std::f64::consts::PI * 1800.0 * t).sin() * (-t * 180.0).exp() * 0.3) as f32
            })
            .collect();
        let (Some(channels), Some(rate)) = (std::num::NonZero::new(1), std::num::NonZero::new(RATE)) else { return };
        output.mixer().add(rodio::buffer::SamplesBuffer::new(channels, rate, samples));
    }

    /// Starts the song. An error says why it can't be played (no output, or a file it can't decode).
    pub fn play(&mut self, data: Vec<u8>) -> Result<(), String> {
        self.stop();
        let output = self.output.as_ref().ok_or("no audio output")?;
        let source = SongSource::new(data.clone())?;
        self.total = source.total_duration();
        let player = rodio::Player::connect_new(output.mixer());
        player.set_volume(self.volume);
        player.append(source);
        self.player = Some(player);
        self.data = Some(data);
        Ok(())
    }

    /// Plays the song with Suno's own player; `poll_web` says when it has started, ended or failed.
    pub fn play_web(&mut self, parent: &wxdragon::prelude::Frame, clip_id: &str) {
        self.stop();
        let (volume, device) = (self.volume, self.device.clone());
        let web = self.web.get_or_insert_with(|| WebPlayer::new(parent));
        web.set_output(&device);
        web.play(clip_id, volume);
        self.on_web = true;
    }

    /// Signing out: Suno's player forgets the sign-in too.
    pub fn forget_sign_in(&mut self) {
        self.stop();
        if let Some(web) = &mut self.web {
            web.forget_sign_in();
        }
    }

    /// What Suno's player did since it was last asked, while a song plays there.
    pub fn poll_web(&mut self) -> Option<Event> {
        if !self.on_web {
            return None;
        }
        let event = self.web.as_mut()?.poll(self.volume);
        if matches!(event, Some(Event::Failed { .. })) {
            self.on_web = false;
        }
        event
    }

    pub fn stop(&mut self) {
        if self.on_web {
            self.on_web = false;
            if let Some(web) = &mut self.web {
                web.stop();
            }
        }
        self.data = None;
        if let Some(player) = self.player.take() {
            player.stop();
        }
        self.total = None;
    }

    pub fn is_loaded(&self) -> bool {
        if self.on_web {
            return self.web.as_ref().is_some_and(WebPlayer::is_active);
        }
        self.player.as_ref().is_some_and(|p| !p.empty())
    }

    pub fn is_paused(&self) -> bool {
        if self.on_web {
            return self.web.as_ref().is_some_and(WebPlayer::is_paused);
        }
        self.player.as_ref().is_some_and(rodio::Player::is_paused)
    }

    /// Pauses or resumes; returns whether it's playing now.
    pub fn toggle_pause(&mut self) -> bool {
        if self.on_web {
            return self.web.as_mut().is_some_and(WebPlayer::toggle_pause);
        }
        let Some(player) = &self.player else { return false };
        if player.is_paused() {
            player.play();
            true
        } else {
            player.pause();
            false
        }
    }

    pub fn position(&self) -> Duration {
        if self.on_web {
            return self.web.as_ref().map_or(Duration::ZERO, WebPlayer::position);
        }
        self.player.as_ref().map_or(Duration::ZERO, rodio::Player::get_pos)
    }

    pub fn total(&self) -> Option<Duration> {
        if self.on_web {
            return self.web.as_ref().and_then(WebPlayer::total);
        }
        self.total
    }

    /// Moves by `seconds` (back when negative), kept inside the song.
    pub fn seek_by(&mut self, seconds: f64) -> Duration {
        if self.on_web {
            let at = self.position().as_secs_f64() + seconds;
            return self.web.as_mut().map_or(Duration::ZERO, |w| w.seek_to(at));
        }
        let Some(player) = &self.player else { return Duration::ZERO };
        let now = player.get_pos().as_secs_f64();
        let end = self.total.map_or(f64::MAX, |t| (t.as_secs_f64() - 0.5).max(0.0));
        let target = Duration::from_secs_f64((now + seconds).clamp(0.0, end));
        if let Err(e) = player.try_seek(target) {
            log::warn!("Couldn't seek: {e}");
        }
        player.get_pos()
    }

    pub fn volume(&self) -> f32 {
        self.volume
    }

    pub fn set_volume(&mut self, volume: f32) {
        self.volume = volume.clamp(0.0, 1.0);
        if let Some(player) = &self.player {
            player.set_volume(self.volume);
        }
        if let Some(web) = self.web.as_ref().filter(|_| self.on_web) {
            web.set_volume(self.volume);
        }
    }
}
