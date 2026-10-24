using System.Reflection;
using TinPan.Models;
using TinPan.Services;
using Xunit;

namespace TinPan.Tests
{
    public class OutputSwitchTests
    {
        [Fact]
        public void It_starts_on_the_main_output()
        {
            var (outputs, audio, _) = Make(main: "Headphones", second: "Speakers");
            Assert.Equal("Headphones", audio.OutputDevice);
            Assert.False(outputs.OnSecond);
        }

        [Fact]
        public void Shift_F12_flips_to_the_second_output_and_back()
        {
            var (outputs, audio, _) = Make(main: "", second: "Speakers");
            Assert.True(outputs.Toggle());
            Assert.Equal("Speakers", audio.OutputDevice);
            Assert.True(outputs.Toggle());
            Assert.Equal("", audio.OutputDevice);
        }

        [Fact]
        public void With_no_second_output_Shift_F12_does_nothing()
        {
            var (outputs, audio, _) = Make(main: "Headphones", second: "");
            Assert.False(outputs.Toggle());
            Assert.Equal("Headphones", audio.OutputDevice);
        }

        [Fact]
        public void The_windows_default_can_be_the_second_output()
        {
            var (outputs, audio, config) = Make(main: "Headphones", second: "");
            outputs.SetSecond("");
            Assert.Equal(OutputSwitch.WindowsDefault, config.Current.Audio.SecondOutputDevice);
            Assert.Equal("", outputs.Second);
            Assert.True(outputs.Toggle());
            Assert.Equal("", audio.OutputDevice);
        }

        [Fact]
        public void A_new_main_output_waits_while_the_second_one_plays()
        {
            var (outputs, audio, _) = Make(main: "Headphones", second: "Speakers");
            outputs.Toggle();
            outputs.SetMain("Monitor");
            Assert.Equal("Speakers", audio.OutputDevice);
            outputs.Toggle();
            Assert.Equal("Monitor", audio.OutputDevice);
        }

        [Fact]
        public void Removing_the_second_output_while_on_it_goes_back_to_the_main_one()
        {
            var (outputs, audio, _) = Make(main: "Headphones", second: "Speakers");
            outputs.Toggle();
            outputs.SetSecond(null);
            Assert.False(outputs.OnSecond);
            Assert.Equal("Headphones", audio.OutputDevice);
        }

        private static (OutputSwitch, IAudioService, IConfigService) Make(string main, string second)
        {
            var config = FakeConfig.Create();
            config.Current.Audio.OutputDevice = main;
            config.Current.Audio.SecondOutputDevice = second;
            var audio = FakeAudio.Create();
            return (new OutputSwitch(audio, config), audio, config);
        }

        /// <summary>A player that only remembers its output.</summary>
        public class FakeAudio : DispatchProxy
        {
            private string _output = "unset";

            public static IAudioService Create() => DispatchProxy.Create<IAudioService, FakeAudio>();

            protected override object? Invoke(MethodInfo? method, object?[]? args)
            {
                switch (method!.Name)
                {
                    case "get_" + nameof(IAudioService.OutputDevice): return _output;
                    case "set_" + nameof(IAudioService.OutputDevice): _output = (string)args![0]!; return null;
                    default: throw new InvalidOperationException("Unexpected call: " + method.Name);
                }
            }
        }

        public class FakeConfig : DispatchProxy
        {
            private readonly AppConfig _config = new();

            public static IConfigService Create() => DispatchProxy.Create<IConfigService, FakeConfig>();

            protected override object? Invoke(MethodInfo? method, object?[]? args) => method!.Name switch
            {
                "get_" + nameof(IConfigService.Current) => _config,
                nameof(IConfigService.SaveSoon) or nameof(IConfigService.Save) => null,
                _ => throw new InvalidOperationException("Unexpected call: " + method.Name)
            };
        }
    }
}
