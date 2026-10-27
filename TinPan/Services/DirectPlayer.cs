using System;
using System.IO;
using System.Linq;
using NAudio.CoreAudioApi;
using NAudio.Wave;
using NAudio.Wave.SampleProviders;
using static TinPan.Localization.Loc;

namespace TinPan.Services
{
    /// <summary>
    /// Plays a song file Tin Pan already has in memory, with Windows' own decoder (Media Foundation) and
    /// output (WASAPI). Made and driven on the UI thread; NAudio raises PlaybackStopped there too.
    /// </summary>
    internal sealed class DirectPlayer : IDisposable
    {
        private WasapiOut? _out;
        private StreamMediaFoundationReader? _reader;
        private SampleChannel? _channel;
        private bool _stopping;
        private float _volume = 1f;

        /// <summary>The song played to its end by itself.</summary>
        public event EventHandler? Ended;

        /// <summary>Playback broke off (the output went away, say).</summary>
        public event EventHandler? Failed;

        public bool IsLoaded => _out != null;
        public bool IsPaused => _out?.PlaybackState == PlaybackState.Paused;
        public TimeSpan Duration => _reader?.TotalTime ?? TimeSpan.Zero;

        public TimeSpan Position
        {
            get => _reader?.CurrentTime ?? TimeSpan.Zero;
            set
            {
                if (_reader == null) return;
                var max = Math.Max(0, _reader.TotalTime.TotalSeconds - 0.5);
                _reader.CurrentTime = TimeSpan.FromSeconds(Math.Clamp(value.TotalSeconds, 0, max));
            }
        }

        public float Volume
        {
            get => _volume;
            set
            {
                _volume = Math.Clamp(value, 0f, 1f);
                if (_channel != null) _channel.Volume = _volume;
            }
        }

        /// <summary>Starts the song on the output with this Windows name ("" for the default). Throws if Windows can't play it.</summary>
        public void Play(byte[] song, string output)
        {
            Stop();
            var reader = new StreamMediaFoundationReader(new MemoryStream(song, writable: false));
            WasapiOut? player = null;
            try
            {
                _channel = new SampleChannel(reader, forceStereo: false) { Volume = _volume };
                player = Open(output);
                player.Init(_channel);
                player.PlaybackStopped += OnStopped;
                player.Play();
                _reader = reader;
                _out = player;
            }
            catch
            {
                player?.Dispose();
                reader.Dispose();
                _channel = null;
                throw;
            }
        }

        public void Pause() => _out?.Pause();
        public void Resume() => _out?.Play();

        public void Stop()
        {
            if (_out == null) return;
            _stopping = true;
            _out.PlaybackStopped -= OnStopped;
            try { _out.Stop(); } catch (Exception ex) when (ex is InvalidOperationException or System.Runtime.InteropServices.COMException) { }
            _out.Dispose();
            _reader?.Dispose();
            _out = null;
            _reader = null;
            _channel = null;
            _stopping = false;
        }

        /// <summary>Moves a song that's playing to another output, from where it was.</summary>
        public void MoveTo(string output)
        {
            if (_out == null || _channel == null) return;
            var paused = IsPaused;
            _out.PlaybackStopped -= OnStopped;
            _out.Stop();
            _out.Dispose();
            _out = Open(output);
            _out.Init(_channel);
            _out.PlaybackStopped += OnStopped;
            if (!paused) _out.Play();
        }

        private static WasapiOut Open(string output)
        {
            if (string.IsNullOrEmpty(output)) return new WasapiOut(AudioClientShareMode.Shared, 200);
            using var devices = new MMDeviceEnumerator();
            var device = devices.EnumerateAudioEndPoints(DataFlow.Render, DeviceState.Active).FirstOrDefault(d => d.FriendlyName == output)
                ?? throw new SunoApiException(T("{0} isn't connected.", output));
            return new WasapiOut(device, AudioClientShareMode.Shared, true, 200);
        }

        private void OnStopped(object? sender, StoppedEventArgs e)
        {
            if (_stopping) return;
            var failed = e.Exception != null;
            if (failed) Log.Warn($"Direct playback stopped: {e.Exception!.Message}");
            Stop();
            (failed ? Failed : Ended)?.Invoke(this, EventArgs.Empty);
        }

        public void Dispose() => Stop();
    }
}
