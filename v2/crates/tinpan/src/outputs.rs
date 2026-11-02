//! Your main audio output and a second one to flip to (Ctrl+Shift+F12), say external speakers to show someone
//! a song, as in Tin Pan 1. Both are saved with Tin Pan 1's names and values; Tin Pan always starts on the main
//! one, and when an output can't be used it says so once and plays on the default.

use tinpan_core::i18n::{t, tf};

use crate::{App, dialogs, player, speech};

/// Saved for a second output that is the system default, since "" means none (Tin Pan 1's value).
const DEFAULT_AS_SECOND: &str = "*";

/// The system's own output, as Settings shows it.
pub fn default_name() -> String {
    if cfg!(windows) { t("Windows default output") } else { t("Default output") }
}

/// The chosen output couldn't be opened, so the system's own plays instead.
pub fn fell_back(name: &str) -> String {
    if cfg!(windows) {
        tf("{0} can't be used right now, so songs play on the Windows default output.", &[&name])
    } else {
        tf("{0} can't be used right now, so songs play on the default output.", &[&name])
    }
}

/// The output as said and shown: its name, or "Windows default output".
pub fn spoken(name: &str) -> String {
    if name.is_empty() { default_name() } else { name.to_string() }
}

/// The choices for an output, and which is chosen: optionally None, then the default, then every connected
/// output, then the chosen one when it isn't connected now. None stands for no output at all.
pub fn choices(include_none: bool, current: Option<&str>) -> (Vec<Option<String>>, Vec<String>, usize) {
    let connected = player::outputs();
    let mut names: Vec<Option<String>> = Vec::new();
    if include_none {
        names.push(None);
    }
    names.push(Some(String::new()));
    names.extend(connected.iter().cloned().map(Some));
    if let Some(chosen) = current.filter(|c| !c.is_empty())
        && !connected.iter().any(|c| c == chosen)
    {
        names.push(Some(chosen.to_string()));
    }
    let at = names.iter().position(|n| n.as_deref() == current).unwrap_or(0);
    let lines = names
        .iter()
        .map(|n| {
            let mut line = n.as_deref().map_or_else(|| t("None"), spoken);
            if n.as_deref().is_some_and(|n| !n.is_empty() && !connected.iter().any(|c| c == n)) {
                line = format!("{line}, {}", t("not connected"));
            }
            line
        })
        .collect();
    (names, lines, at)
}

impl App {
    /// The main output's name; "" is the default.
    pub(crate) fn main_output(&self) -> String {
        self.config.current.audio.output_device.clone()
    }

    /// The second output's name ("" the default), or None when none is chosen.
    pub(crate) fn second_output(&self) -> Option<String> {
        match self.config.current.audio.second_output_device.as_str() {
            "" => None,
            DEFAULT_AS_SECOND => Some(String::new()),
            name => Some(name.to_string()),
        }
    }

    /// Plays on `name`; says once when it can't be used and the default plays instead.
    fn use_output(&mut self, name: &str) {
        if self.player.set_device(name) {
            speech::say_error(&fell_back(name));
        }
    }

    pub(crate) fn set_main_output(&mut self, name: String) {
        self.config.current.audio.output_device = name.clone();
        self.save_settings();
        if !self.on_second_output {
            self.use_output(&name);
        }
    }

    pub(crate) fn set_second_output(&mut self, name: Option<String>) {
        self.config.current.audio.second_output_device = match name.as_deref() {
            None => String::new(),
            Some("") => DEFAULT_AS_SECOND.to_string(),
            Some(n) => n.to_string(),
        };
        self.save_settings();
        if self.on_second_output {
            if name.is_none() {
                self.on_second_output = false;
            }
            let target = name.unwrap_or_else(|| self.main_output());
            self.use_output(&target);
        }
    }

    /// Playback menu, Audio output.
    pub(crate) fn choose_output(&mut self) {
        let current = self.main_output();
        let (names, lines, _) = choices(false, Some(&current));
        let Some(i) = dialogs::pick(&self.frame, &t("Audio output. Enter plays songs there."), &lines) else { return };
        let name = names[i].clone().unwrap_or_default();
        self.set_main_output(name.clone());
        self.settings_show_outputs();
        speech::say(&if self.on_second_output { tf("Main output: {0}. Ctrl+Shift+F12 goes back to it.", &[&spoken(&name)]) } else { tf("Playing on {0}.", &[&spoken(&name)]) });
    }

    /// Playback menu, Second audio output: the one Ctrl+Shift+F12 flips to and back; or none.
    pub(crate) fn choose_second_output(&mut self) {
        let current = self.second_output();
        let (names, lines, _) = choices(true, current.as_deref());
        let Some(i) = dialogs::pick(&self.frame, &t("Second audio output, for Ctrl+Shift+F12. Enter chooses it."), &lines) else { return };
        let name = names[i].clone();
        self.set_second_output(name.clone());
        self.settings_show_outputs();
        speech::say(&match name {
            None => t("No second output. Ctrl+Shift+F12 does nothing."),
            Some(n) => tf("Second output: {0}. Ctrl+Shift+F12 switches to it and back.", &[&spoken(&n)]),
        });
    }

    /// Ctrl+Shift+F12: flips between the main and second output, even while a song plays.
    pub(crate) fn toggle_output(&mut self) {
        let Some(second) = self.second_output() else {
            speech::say(&t("Choose a second output first: Playback menu, Second audio output."));
            return;
        };
        self.on_second_output = !self.on_second_output;
        let target = if self.on_second_output { second } else { self.main_output() };
        self.use_output(&target);
        speech::say(&tf("Playing on {0}.", &[&spoken(&target)]));
    }
}
