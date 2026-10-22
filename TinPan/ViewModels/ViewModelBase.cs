using System;
using System.Threading.Tasks;
using CommunityToolkit.Mvvm.ComponentModel;
using TinPan.Services;

namespace TinPan.ViewModels
{
    public abstract class ViewModelBase : ObservableObject
    {
        protected ViewModelBase(IAnnouncer announcer) => Announcer = announcer;

        protected IAnnouncer Announcer { get; }

        /// <summary>
        /// Runs a Suno call and turns failures into one spoken sentence.
        /// background=true for loads the user didn't just ask for: they don't interrupt speech.
        /// </summary>
        protected async Task<bool> RunAsync(Func<Task> work, bool background = false)
        {
            try
            {
                await work();
                return true;
            }
            catch (SunoApiException ex)
            {
                Announcer.Announce(ex.Message, interrupt: !background, always: true);
                return false;
            }
        }
    }
}
