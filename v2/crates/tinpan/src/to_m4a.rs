//! Turning a song file Windows can play but Suno doesn't take (WMA, say) into AAC in M4A for uploading, with
//! Windows' own Media Foundation, as Tin Pan 1 does: a source reader decodes it to PCM and a sink writer encodes
//! that as AAC at 192 kbit/s. Media Foundation's AAC encoder takes 44.1 or 48 kHz, in mono or stereo.

use std::path::Path;

#[cfg(windows)]
pub fn convert(input: &Path, output: &Path) -> Result<(), String> {
    use windows::Win32::Media::MediaFoundation::*;
    use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx};
    use windows::core::HSTRING;

    // SAFETY: Media Foundation calls on this thread, which joins the multithreaded COM apartment first; every
    // interface is released when dropped, and MFShutdown balances MFStartup.
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        MFStartup(MF_VERSION, MFSTARTUP_FULL).map_err(|e| format!("Media Foundation didn't start: {e}"))?;
        let result = (|| -> windows::core::Result<()> {
            let stream = MF_SOURCE_READER_FIRST_AUDIO_STREAM.0 as u32;
            let reader = MFCreateSourceReaderFromURL(&HSTRING::from(input.as_os_str()), None)?;
            let pcm = MFCreateMediaType()?;
            pcm.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Audio)?;
            pcm.SetGUID(&MF_MT_SUBTYPE, &MFAudioFormat_PCM)?;
            reader.SetCurrentMediaType(stream, None, &pcm)?;
            let decoded = reader.GetCurrentMediaType(stream)?;
            let channels = decoded.GetUINT32(&MF_MT_AUDIO_NUM_CHANNELS)?;
            let rate = decoded.GetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND)?;

            let aac = MFCreateMediaType()?;
            aac.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Audio)?;
            aac.SetGUID(&MF_MT_SUBTYPE, &MFAudioFormat_AAC)?;
            aac.SetUINT32(&MF_MT_AUDIO_BITS_PER_SAMPLE, 16)?;
            aac.SetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND, rate)?;
            aac.SetUINT32(&MF_MT_AUDIO_NUM_CHANNELS, channels)?;
            aac.SetUINT32(&MF_MT_AUDIO_AVG_BYTES_PER_SECOND, 24_000)?;
            let writer = MFCreateSinkWriterFromURL(&HSTRING::from(output.as_os_str()), None, None)?;
            let index = writer.AddStream(&aac)?;
            writer.SetInputMediaType(index, &decoded, None)?;
            writer.BeginWriting()?;
            loop {
                let (mut flags, mut sample) = (0u32, None);
                reader.ReadSample(stream, 0, None, Some(&mut flags), None, Some(&mut sample))?;
                if let Some(sample) = sample {
                    writer.WriteSample(index, &sample)?;
                }
                if flags & MF_SOURCE_READERF_ENDOFSTREAM.0 as u32 != 0 {
                    break;
                }
            }
            writer.Finalize()
        })();
        let _ = MFShutdown();
        result.map_err(|e| e.message().to_string())
    }
}

#[cfg(not(windows))]
pub fn convert(_input: &Path, _output: &Path) -> Result<(), String> {
    Err("converting to M4A needs Windows' Media Foundation".into())
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    /// A second of a tone as a 44.1 kHz WAV goes through Media Foundation and comes out as an M4A that Tin Pan's
    /// own decoder reads back, about a second long.
    #[test]
    fn a_wav_becomes_an_m4a_tin_pan_can_play() {
        let dir = std::env::temp_dir().join(format!("tinpan-m4a-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let (wav, m4a) = (dir.join("tone.wav"), dir.join("tone.m4a"));
        let spec = hound::WavSpec { channels: 2, sample_rate: 44_100, bits_per_sample: 16, sample_format: hound::SampleFormat::Int };
        let mut writer = hound::WavWriter::create(&wav, spec).unwrap();
        for i in 0..44_100 {
            let v = ((i as f32 * 440.0 * 2.0 * std::f32::consts::PI / 44_100.0).sin() * 8000.0) as i16;
            writer.write_sample(v).unwrap();
            writer.write_sample(v).unwrap();
        }
        writer.finalize().unwrap();
        convert(&wav, &m4a).unwrap();
        let source = crate::song_source::SongSource::new(std::fs::read(&m4a).unwrap()).unwrap();
        let seconds = rodio::Source::total_duration(&source).map_or(0.0, |d| d.as_secs_f64());
        assert!((0.9..1.2).contains(&seconds), "{seconds} seconds");
        let _ = std::fs::remove_dir_all(dir);
    }
}
