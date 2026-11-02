//! A song file in memory, decoded for rodio. Suno's songs are Opus inside M4A, which rodio's own decoder
//! can't play (Symphonia has no Opus decoder), so the file is read with Symphonia and the audio decoded
//! with libopus; AAC and MP3 files decode with Symphonia's own codecs.

use std::io::Cursor;
use std::num::NonZero;
use std::sync::Arc;
use std::time::Duration;

use rodio::source::SeekError;
use rodio::{ChannelCount, SampleRate, Source};
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{CODEC_TYPE_NULL, CodecRegistry, Decoder, DecoderOptions};
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::{FormatOptions, FormatReader, SeekMode, SeekTo};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use symphonia::core::units::Time;

pub struct SongSource {
    format: Box<dyn FormatReader>,
    decoder: Box<dyn Decoder>,
    track_id: u32,
    samples: Vec<f32>,
    next: usize,
    channels: ChannelCount,
    rate: SampleRate,
    total: Option<Duration>,
}

impl SongSource {
    pub fn new(data: Vec<u8>) -> Result<Self, String> {
        let stream = MediaSourceStream::new(Box::new(Cursor::new(data)), Default::default());
        let mut hint = Hint::new();
        hint.with_extension("m4a");
        let format = symphonia::default::get_probe()
            .format(&hint, stream, &FormatOptions { enable_gapless: true, ..Default::default() }, &MetadataOptions::default())
            .map_err(|e| format!("not a song file Tin Pan can read: {e}"))?
            .format;
        let track = format.tracks().iter().find(|t| t.codec_params.codec != CODEC_TYPE_NULL).ok_or("the file has no audio")?;
        let (mut params, track_id) = (track.codec_params.clone(), track.id);
        fill_opus_layout(&mut params);

        let mut codecs = CodecRegistry::new();
        symphonia::default::register_enabled_codecs(&mut codecs);
        codecs.register_all::<symphonia_adapter_libopus::OpusDecoder>();
        let decoder = codecs.make(&params, &DecoderOptions::default()).map_err(|e| format!("no decoder for this audio: {e}"))?;

        let total = params.time_base.zip(params.n_frames).map(|(base, frames)| {
            let time = base.calc_time(frames);
            Duration::from_secs_f64(time.seconds as f64 + time.frac)
        });
        let mut source = Self {
            format,
            decoder,
            track_id,
            samples: Vec::new(),
            next: 0,
            channels: NonZero::new(params.channels.map_or(2, |c| c.count() as u16).max(1)).unwrap_or(NonZero::<u16>::MIN),
            rate: NonZero::new(params.sample_rate.unwrap_or(48_000)).unwrap_or(NonZero::<u32>::MIN),
            total,
        };
        // The first audio says the real channels and rate (Opus always decodes at 48 kHz).
        if !source.refill() {
            return Err("the file has no audio Tin Pan can decode".into());
        }
        Ok(source)
    }

    /// Decodes the next packet of the song's track; false at the end or on an error it can't get past.
    fn refill(&mut self) -> bool {
        loop {
            let packet = match self.format.next_packet() {
                Ok(p) => p,
                Err(SymphoniaError::ResetRequired) => {
                    self.decoder.reset();
                    continue;
                }
                Err(_) => return false, // the end of the file, or a broken one
            };
            if packet.track_id() != self.track_id {
                continue;
            }
            match self.decoder.decode(&packet) {
                Ok(decoded) => {
                    let spec = *decoded.spec();
                    if decoded.frames() == 0 {
                        continue;
                    }
                    let mut buffer = SampleBuffer::<f32>::new(decoded.capacity() as u64, spec);
                    buffer.copy_interleaved_ref(decoded);
                    self.samples.clear();
                    self.samples.extend_from_slice(buffer.samples());
                    self.next = 0;
                    if let Some(c) = NonZero::new(spec.channels.count() as u16) {
                        self.channels = c;
                    }
                    if let Some(r) = NonZero::new(spec.rate) {
                        self.rate = r;
                    }
                    return true;
                }
                // A damaged packet: skip it, as players do.
                Err(SymphoniaError::DecodeError(e)) => log::warn!("Skipped a damaged bit of the song: {e}"),
                Err(e) => {
                    log::warn!("The song stopped decoding: {e}");
                    return false;
                }
            }
        }
    }
}

/// Symphonia's M4A reader leaves an Opus track's channels and rate out, which the decoder needs. They're in
/// the Opus identification header it keeps as extra data: "OpusHead", the version, then the channel count.
/// Opus always decodes at 48 kHz.
fn fill_opus_layout(params: &mut symphonia::core::codecs::CodecParameters) {
    use symphonia::core::audio::Channels;
    use symphonia::core::codecs::CODEC_TYPE_OPUS;
    if params.codec != CODEC_TYPE_OPUS {
        return;
    }
    if params.channels.is_none() && params.channel_layout.is_none() {
        let count = params.extra_data.as_deref().filter(|e| e.starts_with(b"OpusHead")).and_then(|e| e.get(9)).copied().unwrap_or(2);
        params.channels = Some(if count == 1 { Channels::FRONT_LEFT } else { Channels::FRONT_LEFT | Channels::FRONT_RIGHT });
    }
    if params.sample_rate.is_none() {
        params.sample_rate = Some(48_000);
    }
}

impl Iterator for SongSource {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        if self.next >= self.samples.len() && !self.refill() {
            return None;
        }
        let sample = self.samples[self.next];
        self.next += 1;
        Some(sample)
    }
}

impl Source for SongSource {
    fn current_span_len(&self) -> Option<usize> {
        // The rest of this packet: channels and rate may only change at its end.
        Some(self.samples.len().saturating_sub(self.next)).filter(|n| *n > 0)
    }

    fn channels(&self) -> ChannelCount {
        self.channels
    }

    fn sample_rate(&self) -> SampleRate {
        self.rate
    }

    fn total_duration(&self) -> Option<Duration> {
        self.total
    }

    fn try_seek(&mut self, position: Duration) -> Result<(), SeekError> {
        let time = Time::new(position.as_secs(), f64::from(position.subsec_nanos()) / 1e9);
        self.format
            .seek(SeekMode::Accurate, SeekTo::Time { time, track_id: Some(self.track_id) })
            .map_err(|e| SeekError::Other(Arc::new(std::io::Error::other(e.to_string()))))?;
        self.decoder.reset();
        self.samples.clear();
        self.next = 0;
        Ok(())
    }
}

/// Decodes a song file to a 24-bit WAV, for sunscreen, which reads only MP3 and WAV.
pub fn decode_to_wav(input: &std::path::Path, output: &std::path::Path) -> Result<(), String> {
    let source = SongSource::new(std::fs::read(input).map_err(|e| e.to_string())?)?;
    let spec = hound::WavSpec { channels: source.channels().get(), sample_rate: source.sample_rate().get(), bits_per_sample: 24, sample_format: hound::SampleFormat::Int };
    let mut writer = hound::WavWriter::create(output, spec).map_err(|e| e.to_string())?;
    const FULL: f32 = 8_388_607.0;
    for sample in source {
        writer.write_sample((sample.clamp(-1.0, 1.0) * FULL) as i32).map_err(|e| e.to_string())?;
    }
    writer.finalize().map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A real Suno song saved by Tin Pan 1, when there is one on this computer (TINPAN_TEST_SONG).
    #[test]
    fn decodes_a_suno_opus_song_when_one_is_given() {
        let Ok(path) = std::env::var("TINPAN_TEST_SONG") else { return };
        let mut source = SongSource::new(std::fs::read(path).unwrap()).unwrap();
        assert_eq!(source.sample_rate().get(), 48_000);
        let first: Vec<f32> = source.by_ref().take(48_000 * 2).collect();
        assert_eq!(first.len(), 96_000, "a second of stereo audio");
        assert!(first.iter().any(|s| s.abs() > 0.001), "not silence");
        assert!(source.total_duration().is_some_and(|d| d > Duration::from_secs(10)));
        source.try_seek(Duration::from_secs(5)).unwrap();
        assert!(source.next().is_some());
    }

    #[test]
    fn something_that_isnt_a_song_is_refused() {
        assert!(SongSource::new(b"not a song".to_vec()).is_err());
    }

    /// A whole clean-up of a real Suno song, on a copy in a temporary folder, with the built-in sunscreen.
    #[test]
    fn a_suno_song_cleans_up_end_to_end_when_one_is_given() {
        let Ok(song) = std::env::var("TINPAN_TEST_SONG") else { return };
        let work = std::env::temp_dir().join(format!("tinpan-clean-test-{}", std::process::id()));
        std::fs::create_dir_all(&work).unwrap();
        let copy = work.join("Song (test).m4a");
        std::fs::copy(song, &copy).unwrap();
        let options = tinpan_core::config::CleanupConfig::default();
        let cleaned = tinpan_core::cleanup::clean(&copy, &options, &work.join("native"), decode_to_wav).unwrap();
        assert_eq!(cleaned, work.join("Song (test) (clean).wav"));
        let reader = hound::WavReader::open(&cleaned).unwrap();
        assert_eq!(reader.spec().bits_per_sample, 24);
        assert!(reader.duration() > 48_000 * 10, "at least 10 seconds");
        assert!(copy.exists(), "the download is kept, as the default says");
        let _ = std::fs::remove_dir_all(work);
    }
}
