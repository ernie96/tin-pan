//! Library, Make a voice from your singing: a voice persona from a recording of you singing, as Suno's
//! Create, Voice page makes one. Suno checks the voice is yours with a short phrase you record and choose
//! next; Tin Pan doesn't record, so both recordings are files made with any recorder.

use std::path::PathBuf;

use tinpan_core::i18n::{t, tf};
use tinpan_core::voice::{Phrase, Singing};

use crate::form::{self, Field, Value};
use crate::net::Command;
use crate::{App, speech};

const AUDIO: (&str, &str) = ("Audio files", "*.mp3;*.wav;*.m4a;*.flac;*.ogg;*.aac");

/// What the persona will be called and say about itself, kept while Suno checks the voice.
#[derive(Debug, Clone)]
pub struct VoiceDetails {
    pub name: String,
    pub description: String,
    pub styles: String,
}

impl App {
    pub(crate) fn make_voice(&mut self) {
        if !self.signed_in {
            speech::say(&t("Not signed in."));
            return;
        }
        let mut fields = [
            Field { label: "Recording of you _singing", value: Value::Path { value: String::new(), files: Some(AUDIO), button: "_Browse...", picker_title: "Choose a recording of you singing" } },
            Field { label: "_Name", value: Value::Text { value: String::new(), multiline: false, max_len: 100 } },
            Field { label: "_Description (optional)", value: Value::Text { value: String::new(), multiline: true, max_len: 500 } },
            Field { label: "S_tyles it suits (optional)", value: Value::Text { value: String::new(), multiline: false, max_len: 300 } },
        ];
        let intro = t("Makes a voice persona that sings new songs in your voice. Choose a clear recording of you singing, without music, from about 30 seconds to a few minutes. Suno then asks you to record a short phrase to check the voice is yours. The voice stays private.");
        if !form::show(&self.frame, &t("Make a voice from your singing"), &intro, &t("Next"), &mut fields) {
            return;
        }
        let path = PathBuf::from(fields[0].text().trim());
        if !path.is_file() {
            speech::say_error(&t("Choose the recording of you singing."));
            return;
        }
        let name = fields[1].text().trim().to_string();
        if name.is_empty() {
            speech::say_error(&t("Give the voice a name."));
            return;
        }
        let Some(Some(seconds)) = crate::uploads::length_of(&path) else {
            speech::say_error(&t("Tin Pan can't read that file as audio."));
            return;
        };
        if seconds < 10.0 {
            speech::say_error(&t("That recording is too short. Sing for at least 10 seconds."));
            return;
        }
        let details = VoiceDetails { name, description: fields[2].text(), styles: fields[3].text() };
        speech::say(&t("Sending your singing to Suno. This can take a minute."));
        self.net.send(Command::VoiceSinging { path, seconds, details: Box::new(details), language: tinpan_core::i18n::current().to_string() });
    }

    /// Suno has the singing and a phrase to check it with: the phrase recording is asked for.
    pub(crate) fn voice_phrase(&mut self, singing: Singing, phrase: Phrase, details: VoiceDetails, attempt: u8) {
        let mut fields = [Field { label: "Recording of you saying the _phrase", value: Value::Path { value: String::new(), files: Some(AUDIO), button: "_Browse...", picker_title: "Choose the recording of the phrase" } }];
        let intro = tf("Suno checks the voice is yours. Record yourself saying this phrase in your own voice, in under 15 seconds, then choose the recording:\n\n{0}", &[&phrase.text]);
        if !form::show(&self.frame, &t("Check it's your voice"), &intro, &t("Check"), &mut fields) {
            return;
        }
        let path = PathBuf::from(fields[0].text().trim());
        if !path.is_file() {
            speech::say_error(&t("Choose the recording of the phrase."));
            return;
        }
        speech::say(&t("Sending the phrase to Suno to check."));
        self.net.send(Command::VoiceCheck { singing: Box::new(singing), phrase: Box::new(phrase), path, details: Box::new(details), attempt });
    }
}
