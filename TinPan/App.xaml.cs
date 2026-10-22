using System;
using System.Net;
using System.Net.Http;
using System.Threading.Tasks;
using System.Windows;
using System.Windows.Threading;
using Microsoft.Extensions.DependencyInjection;
using TinPan.Services;
using TinPan.ViewModels;
using static TinPan.Localization.Loc;

namespace TinPan
{
    public partial class App : Application
    {
        private ServiceProvider? _services;

        protected override async void OnStartup(StartupEventArgs e)
        {
            base.OnStartup(e);
            DispatcherUnhandledException += OnUnhandledUiException;
            TaskScheduler.UnobservedTaskException += (_, args) =>
            {
                Log.Error("Unobserved task error", args.Exception);
                args.SetObserved();
            };
            Log.Info("Starting");
            EmbeddedFiles.RegisterNativeLibraries();

            _services = ConfigureServices();
            // The language is fixed for the session: windows read their text once, when they open.
            Localization.Loc.Use(_services.GetRequiredService<IConfigService>().Current.Language);
            var audio = _services.GetRequiredService<IAudioService>();
            audio.CurrentChanged += (_, _) => Views.PlayingCue.Instance.ClipId = audio.CurrentClipId;
            var main = _services.GetRequiredService<MainViewModel>();
            MainWindow = new MainWindow { DataContext = main };
            MainWindow.Show();
            await main.InitializeAsync();
        }

        protected override void OnExit(ExitEventArgs e)
        {
            _services?.GetService<IConfigService>()?.Save();
            _services?.Dispose();
            base.OnExit(e);
        }

        private static ServiceProvider ConfigureServices()
        {
            var services = new ServiceCollection();

            services.AddSingleton(_ =>
            {
                // We send Cookie headers ourselves, so the handler must not keep its own jar.
                var handler = new HttpClientHandler { UseCookies = false, AutomaticDecompression = DecompressionMethods.All };
                var http = new HttpClient(handler) { Timeout = TimeSpan.FromSeconds(30) };
                http.DefaultRequestHeaders.UserAgent.ParseAdd(
                    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36");
                return http;
            });
            services.AddSingleton<IConfigService, ConfigService>();
            services.AddSingleton<IAuthService, AuthService>();
            services.AddSingleton<ISunoApiService, SunoApiService>();
            services.AddSingleton<IAudioService, SunoWebPlayer>();
            services.AddSingleton<ICaptchaSolver, CaptchaSolver>();
            services.AddSingleton<IAnnouncer, ScreenReaderService>();
            services.AddSingleton<IDialogService, DialogService>();
            services.AddSingleton<SongWatcher>();
            services.AddSingleton<IUpdateService, UpdateService>();
            services.AddSingleton<AccountState>();

            services.AddSingleton<MainViewModel>();
            services.AddSingleton<SunoPanelViewModel>();
            services.AddSingleton<LibraryPanelViewModel>();
            services.AddSingleton<SettingsPanelViewModel>();
            services.AddSingleton<PlaylistsViewModel>();

            return services.BuildServiceProvider();
        }

        private void OnUnhandledUiException(object sender, DispatcherUnhandledExceptionEventArgs e)
        {
            Log.Error("Unhandled error", e.Exception);
            var message = e.Exception is SunoApiException api ? api.Message : T("Something went wrong. Details are in the log.");
            _services?.GetService<IAnnouncer>()?.Announce(message, always: true);
            e.Handled = true;
        }
    }
}
