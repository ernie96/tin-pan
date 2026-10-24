using System.Threading.Tasks;
using CommunityToolkit.Mvvm.Input;
using TinPan.Services;
using TinPan.Views;
using static TinPan.Localization.Loc;

namespace TinPan.ViewModels
{
    // Help menu: Report a problem (Ctrl+Shift+B) and Suggest a feature (Ctrl+Shift+F), as in TimePlayer.
    public partial class MainViewModel
    {
        [RelayCommand]
        private Task ReportProblemAsync() => SendReportAsync(Report.Problem);

        [RelayCommand]
        private Task SuggestFeatureAsync() => SendReportAsync(Report.Feature);

        private async Task SendReportAsync(string kind)
        {
            var problem = kind == Report.Problem;
            var description = new TextField(problem ? T("_What happened, and what did you expect?") : T("_What would you like Tin Pan to do?"),
                multiline: true, maxLength: ReportService.MaxDescriptionLength);
            var contact = new TextField(T("_Email or other way to reach you (optional)"), maxLength: ReportService.MaxContactLength);
            var includeLog = new CheckField(T("Include Tin Pan's _log (it has no passwords, and your Windows user name is taken out)"), true);
            var fields = problem ? new FormField[] { description, contact, includeLog } : new FormField[] { description, contact };
            var intro = problem
                ? T("Goes privately to Tin Pan's author. The log helps find the cause.")
                : T("Goes privately to Tin Pan's author.");
            if (!_dialogs.ShowForm(problem ? T("Report a problem") : T("Suggest a feature"), intro, T("Send"), fields)) return;
            if (string.IsNullOrWhiteSpace(description.Value))
            {
                Announcer.Announce(T("Nothing sent: the description was empty."), always: true);
                return;
            }

            Announcer.Announce(T("Sending."));
            var report = new Report
            {
                Kind = kind,
                Description = description.Value,
                Contact = contact.Value,
                Log = problem && includeLog.Value ? _reports.ReadRecentLog() : null
            };
            var number = 0;
            if (await RunAsync(async () => number = await _reports.SendAsync(report)))
                Announcer.Announce(problem ? T("Sent as report {0}. Thank you.", number) : T("Sent as suggestion {0}. Thank you.", number), always: true);
        }
    }
}
