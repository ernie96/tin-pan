using System.Collections.Generic;
using System.Text.Json.Serialization;

namespace TinPan.Models
{
    /// <summary>GET /api/gen/{id}/aligned_lyrics/v2/ — each sung word with its time in seconds.</summary>
    public class AlignedLyrics
    {
        [JsonPropertyName("aligned_words")]
        public List<AlignedWord> Words { get; set; } = new();
    }

    public class AlignedWord
    {
        [JsonPropertyName("word")]
        public string Word { get; set; } = string.Empty;

        [JsonPropertyName("start_s")]
        public double Start { get; set; }

        [JsonPropertyName("end_s")]
        public double End { get; set; }
    }
}
