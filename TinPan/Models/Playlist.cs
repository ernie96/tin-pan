using CommunityToolkit.Mvvm.ComponentModel;
using static TinPan.Localization.Loc;

namespace TinPan.Models
{
    public class Playlist : ObservableObject
    {
        private string _name = "";
        private int? _songCount;

        public string Id { get; set; } = "";
        public string Description { get; set; } = "";
        public bool IsPublic { get; set; }

        public string Name
        {
            get => _name;
            set { if (SetProperty(ref _name, value)) OnPropertyChanged(nameof(SpokenText)); }
        }

        public int? SongCount
        {
            get => _songCount;
            set { if (SetProperty(ref _songCount, value)) OnPropertyChanged(nameof(SpokenText)); }
        }

        public string DisplayName => string.IsNullOrWhiteSpace(Name) ? T("Untitled playlist") : Name.Trim();

        public string SpokenText => SongCount is { } n ? $"{DisplayName}, {Plural(n, "{0} song", "{0} songs")}" : DisplayName;

        public override string ToString() => SpokenText;
    }

    public class SongComment
    {
        public string Author { get; set; } = "";
        public string Text { get; set; } = "";
        public string? When { get; set; }
        public int Likes { get; set; }
        public int Replies { get; set; }

        public override string ToString()
        {
            var head = string.IsNullOrEmpty(Author) ? T("Someone") : Author;
            var extra = Likes > 0 ? $" ({Plural(Likes, "{0} like", "{0} likes")})" : "";
            return $"{head}: {Text}{extra}";
        }
    }
}
