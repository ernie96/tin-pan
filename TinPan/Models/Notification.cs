namespace TinPan.Models
{
    public class Notification
    {
        public string Title { get; set; } = string.Empty;
        public string Message { get; set; } = string.Empty;

        public override string ToString() =>
            string.IsNullOrEmpty(Title) ? Message : string.IsNullOrEmpty(Message) ? Title : $"{Title}: {Message}";
    }
}
