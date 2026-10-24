namespace TinPan.Models
{
    public class Notification
    {
        public string Title { get; set; } = string.Empty;
        public string Message { get; set; } = string.Empty;

        /// <summary>False for ones you haven't seen yet, which say "new" first.</summary>
        public bool IsRead { get; set; } = true;

        public override string ToString()
        {
            var text = string.IsNullOrEmpty(Title) ? Message : string.IsNullOrEmpty(Message) ? Title : $"{Title}: {Message}";
            return IsRead ? text : TinPan.Localization.Loc.T("New: {0}", text);
        }
    }
}
