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
    decode_error: Option<String>,
}

impl SongSource {
    pub fn new(data: Vec<u8>) -> Result<Self, String> {
        Self::new_with_hint(data, None)
    }

    pub fn new_with_hint(data: Vec<u8>, extension: Option<&str>) -> Result<Self, String> {
        let stream = MediaSourceStream::new(Box::new(Cursor::new(data)), Default::default());
        let mut hint = Hint::new();
        if let Some(ext) = extension {
            hint.with_extension(ext);
        } else {
            hint.with_extension("m4a");
        }
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
            decode_error: None,
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
                Err(SymphoniaError::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => return false,
                Err(e) => { self.decode_error = Some(e.to_string()); return false; }
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
                Err(SymphoniaError::DecodeError(e)) => { self.decode_error = Some(e.to_string()); log::warn!("Skipped a damaged bit of the song: {e}"); }
                Err(e) => {
                    self.decode_error = Some(e.to_string());
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
    let mut source = SongSource::new(std::fs::read(input).map_err(|e| e.to_string())?)?;
    let spec = hound::WavSpec { channels: source.channels().get(), sample_rate: source.sample_rate().get(), bits_per_sample: 24, sample_format: hound::SampleFormat::Int };
    let mut writer = hound::WavWriter::create(output, spec).map_err(|e| e.to_string())?;
    const FULL: f32 = 8_388_607.0;
    let expected = source.total.map(|duration| duration.as_secs_f64() * f64::from(spec.sample_rate));
    let mut samples = 0u64;
    for sample in source.by_ref() {
        samples += 1;
        writer.write_sample((sample.clamp(-1.0, 1.0) * FULL) as i32).map_err(|e| e.to_string())?;
    }
    if let Some(error) = source.decode_error { return Err(error); }
    let frames = samples / u64::from(spec.channels);
    // Allow one codec packet of encoder delay and padding.
    if expected.is_some_and(|n| frames as f64 + 0.12 * f64::from(spec.sample_rate) < n) {
        return Err("Audio ended before its declared duration".into());
    }
    writer.finalize().map_err(|e| e.to_string())
}

/// Slices an audio file into sequential 16-bit PCM WAV parts of approximately `slice_seconds` each.
/// If the final part would be less than 1 second long and earlier parts exist, it merges into the preceding part.
#[allow(dead_code)]
pub fn slice_audio(input: &std::path::Path, slice_seconds: f64, output_dir: &std::path::Path) -> Result<Vec<(std::path::PathBuf, f64)>, String> {
    let ext = input.extension().and_then(|e| e.to_str());
    let source = SongSource::new_with_hint(std::fs::read(input).map_err(|e| e.to_string())?, ext)?;
    let channels = source.channels().get();
    let sample_rate = source.sample_rate().get();
    let frames_per_slice = (slice_seconds * sample_rate as f64).round() as usize;
    let samples_per_slice = frames_per_slice * channels as usize;
    let min_samples_for_last = (1.0 * sample_rate as f64).round() as usize * channels as usize;

    let spec = hound::WavSpec {
        channels,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    std::fs::create_dir_all(output_dir).map_err(|e| e.to_string())?;

    let mut slices = Vec::new();
    let mut part_number = 1;
    let mut current_samples = Vec::with_capacity(samples_per_slice);

    for sample in source {
        current_samples.push(sample);
        if current_samples.len() >= samples_per_slice {
            let part_path = output_dir.join(format!("part_{part_number:03}.wav"));
            write_wav_slice(&part_path, spec, &current_samples)?;
            let dur = current_samples.len() as f64 / (channels as f64 * sample_rate as f64);
            slices.push((part_path, dur));
            part_number += 1;
            current_samples.clear();
        }
    }

    if !current_samples.is_empty() {
        if !slices.is_empty() && current_samples.len() < min_samples_for_last {
            let (last_path, _) = slices.pop().unwrap();
            let mut all_samples = Vec::new();
            if let Ok(mut reader) = hound::WavReader::open(&last_path) {
                for s in reader.samples::<i16>().flatten() {
                    all_samples.push(s as f32 / 32767.0);
                }
            }
            all_samples.extend(current_samples);
            write_wav_slice(&last_path, spec, &all_samples)?;
            let dur = all_samples.len() as f64 / (channels as f64 * sample_rate as f64);
            slices.push((last_path, dur));
        } else {
            let part_path = output_dir.join(format!("part_{part_number:03}.wav"));
            write_wav_slice(&part_path, spec, &current_samples)?;
            let dur = current_samples.len() as f64 / (channels as f64 * sample_rate as f64);
            slices.push((part_path, dur));
        }
    }

    if slices.is_empty() {
        return Err("No audio frames found in file.".into());
    }

    Ok(slices)
}

fn write_wav_slice(path: &std::path::Path, spec: hound::WavSpec, samples: &[f32]) -> Result<(), String> {
    let mut writer = hound::WavWriter::create(path, spec).map_err(|e| e.to_string())?;
    const FULL: f32 = 32767.0;
    for &sample in samples {
        let clamped = sample.clamp(-1.0, 1.0);
        writer.write_sample((clamped * FULL) as i16).map_err(|e| e.to_string())?;
    }
    writer.finalize().map_err(|e| e.to_string())
}

/// A loaded and fully decoded song, cached in memory for high-performance dynamic slicing.
pub struct DecodedSong {
    pub samples: Vec<f32>,
    pub channels: usize,
    pub sample_rate: u32,
    pub total_frames: usize,
}

impl DecodedSong {
    pub fn load(path: &std::path::Path) -> Result<Self, String> {
        let data = std::fs::read(path).map_err(|e| e.to_string())?;
        let ext = path.extension().and_then(|e| e.to_str());
        let source = SongSource::new_with_hint(data, ext)?;
        let channels = source.channels().get() as usize;
        let sample_rate = source.sample_rate().get();
        let samples: Vec<f32> = source.collect();
        let total_frames = samples.len() / channels;
        if total_frames == 0 {
            return Err("The audio file has no frames.".into());
        }
        Ok(Self { samples, channels, sample_rate, total_frames })
    }

    /// Writes a slice from `start_frame` to `start_frame + frame_count` as a 16-bit WAV file.
    /// Returns the exact duration of the written slice in seconds.
    pub fn write_slice(&self, start_frame: usize, frame_count: usize, path: &std::path::Path) -> Result<f64, String> {
        if start_frame >= self.total_frames || frame_count == 0 {
            return Err("Invalid frame range for slice.".into());
        }
        let start_sample = start_frame * self.channels;
        let end_sample = ((start_frame + frame_count) * self.channels).min(self.samples.len());
        let slice_samples = &self.samples[start_sample..end_sample];
        let actual_frames = slice_samples.len() / self.channels;
        let duration = actual_frames as f64 / self.sample_rate as f64;
        let spec = hound::WavSpec {
            channels: self.channels as u16,
            sample_rate: self.sample_rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        write_wav_slice(path, spec, slice_samples)?;
        Ok(duration)
    }
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
    fn truncated_m4a_cannot_replace_the_original_after_cleanup() {
        let Ok(path) = std::env::var("TINPAN_TEST_SONG") else { return };
        let data = std::fs::read(path).unwrap();
        let folder = std::env::temp_dir().join(format!("tinpan-truncated-test-{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let input = folder.join("partial.m4a");
        std::fs::write(&input, &data[..data.len() / 2]).unwrap();
        assert!(decode_to_wav(&input, &folder.join("partial.wav")).is_err());
        std::fs::remove_dir_all(folder).unwrap();
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

    #[test]
    fn slices_audio_into_parts_of_given_duration() {
        let temp_dir = std::env::temp_dir().join(format!("tinpan-slice-test-{}", std::process::id()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        let input_wav = temp_dir.join("test_input.wav");
        let spec = hound::WavSpec {
            channels: 2,
            sample_rate: 44100,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        // 12 seconds of stereo audio (12 * 44100 = 529200 frames)
        {
            let mut writer = hound::WavWriter::create(&input_wav, spec).unwrap();
            for i in 0..529200 {
                let sample = ((i as f32 * 0.05).sin() * 10000.0) as i16;
                writer.write_sample(sample).unwrap();
                writer.write_sample(sample).unwrap();
            }
            writer.finalize().unwrap();
        }

        let out_dir = temp_dir.join("slices");
        let slices = slice_audio(&input_wav, 5.0, &out_dir).unwrap();
        assert_eq!(slices.len(), 3, "12 seconds sliced into 5s chunks yields 3 parts");
        assert!((slices[0].1 - 5.0).abs() < 0.01, "Part 1 should be ~5.0s, was {}", slices[0].1);
        assert!((slices[1].1 - 5.0).abs() < 0.01, "Part 2 should be ~5.0s, was {}", slices[1].1);
        assert!((slices[2].1 - 2.0).abs() < 0.01, "Part 3 should be ~2.0s, was {}", slices[2].1);
        for (path, _) in &slices {
            assert!(path.exists());
            let r = hound::WavReader::open(path).unwrap();
            assert_eq!(r.spec().channels, 2);
            assert_eq!(r.spec().sample_rate, 44100);
        }

        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn decoded_song_slices_dynamically() {
        let temp_dir = std::env::temp_dir().join(format!("tinpan-decoded-test-{}", std::process::id()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        let input_wav = temp_dir.join("test_input2.wav");
        let spec = hound::WavSpec {
            channels: 2,
            sample_rate: 44100,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        // 6 seconds of stereo audio (6 * 44100 = 264600 frames)
        {
            let mut writer = hound::WavWriter::create(&input_wav, spec).unwrap();
            for i in 0..264600 {
                let sample = ((i as f32 * 0.05).sin() * 10000.0) as i16;
                writer.write_sample(sample).unwrap();
                writer.write_sample(sample).unwrap();
            }
            writer.finalize().unwrap();
        }

        let decoded = DecodedSong::load(&input_wav).unwrap();
        assert_eq!(decoded.channels, 2);
        assert_eq!(decoded.sample_rate, 44100);
        assert_eq!(decoded.total_frames, 264600);

        let slice1_path = temp_dir.join("slice1.wav");
        let dur1 = decoded.write_slice(0, 44100 * 2, &slice1_path).unwrap(); // 2 seconds
        assert!((dur1 - 2.0).abs() < 0.01);
        assert!(slice1_path.exists());

        let slice2_path = temp_dir.join("slice2.wav");
        let dur2 = decoded.write_slice(44100 * 2, 44100, &slice2_path).unwrap(); // 1 second
        assert!((dur2 - 1.0).abs() < 0.01);

        let _ = std::fs::remove_dir_all(temp_dir);
    }
}
