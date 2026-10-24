using System;
using System.Collections.Generic;
using System.Linq;
using System.Text.Json;
using TinPan.Models;
using static TinPan.Localization.Loc;

namespace TinPan.Services
{
    /// <summary>
    /// Reads Suno replies that come in more than one shape (bare arrays, {"clips": [...]},
    /// nested sections). Kept separate so it can be tested without the network.
    /// </summary>
    public static partial class SunoJson
    {
        private static readonly JsonSerializerOptions JsonOpts = new() { PropertyNameCaseInsensitive = true };

        public static FeedPage ReadFeedPage(string json)
        {
            using var doc = Parse(json);
            var root = doc.RootElement;
            if (root.ValueKind == JsonValueKind.Array) return new FeedPage(ToClips(root), null);
            if (root.ValueKind != JsonValueKind.Object) return new FeedPage(new List<SunoClip>(), null);

            int? total = root.TryGetProperty("num_total", out var t) && t.TryGetInt32(out var n) ? n : null;
            var cursor = root.TryGetProperty("next_cursor", out var c) && c.ValueKind == JsonValueKind.String ? c.GetString() : null;
            bool? hasMore = root.TryGetProperty("has_more", out var h) && h.ValueKind is JsonValueKind.True or JsonValueKind.False
                ? h.GetBoolean()
                : null;
            var clips = root.TryGetProperty("clips", out var list) && list.ValueKind == JsonValueKind.Array
                ? ToClips(list)
                : new List<SunoClip>();
            return new FeedPage(clips, cursor, hasMore, total);
        }

        /// <summary>A reply that is a clip, a list of clips, or {"clips": [...]}.</summary>
        public static IReadOnlyList<SunoClip> FindClipsById(string json)
        {
            using var doc = Parse(json);
            var root = doc.RootElement;
            if (root.ValueKind == JsonValueKind.Array) return ToClips(root);
            if (root.ValueKind != JsonValueKind.Object) return Array.Empty<SunoClip>();
            if (root.TryGetProperty("clips", out var list) && list.ValueKind == JsonValueKind.Array) return ToClips(list);
            if (root.TryGetProperty("id", out var id) && id.ValueKind == JsonValueKind.String)
            {
                var clip = ReadClip(root);
                return clip == null ? Array.Empty<SunoClip>() : new[] { clip };
            }
            return Array.Empty<SunoClip>();
        }

        /// <summary>Like ReadString, but returns null for replies that aren't JSON at all (error pages).</summary>
        public static string? TryReadString(string json, string name)
        {
            try
            {
                using var doc = JsonDocument.Parse(json);
                return doc.RootElement.ValueKind == JsonValueKind.Object
                       && doc.RootElement.TryGetProperty(name, out var v) && v.ValueKind == JsonValueKind.String
                    ? v.GetString()
                    : null;
            }
            catch (JsonException)
            {
                return null;
            }
        }

        /// <summary>Collects every object that looks like a clip (has an id and an audio_url), at any depth.</summary>
        public static IReadOnlyList<SunoClip> FindClips(string json)
        {
            using var doc = Parse(json);
            var found = new List<SunoClip>();
            var seen = new HashSet<string>();
            Walk(doc.RootElement, found, seen);
            return found;
        }

        public static Playlist ReadPlaylist(JsonElement p)
        {
            if (p.ValueKind != JsonValueKind.Object) return new Playlist();
            var playlist = new Playlist
            {
                Id = p.TryGetProperty("id", out var id) ? id.ToString() : "",
                Name = FirstString(p, "name") ?? "",
                Description = FirstString(p, "description") ?? "",
                IsPublic = p.TryGetProperty("is_public", out var pub) && pub.ValueKind == JsonValueKind.True
            };
            if (p.TryGetProperty("num_total_results", out var n) && n.TryGetInt32(out var count)) playlist.SongCount = count;
            else if (p.TryGetProperty("playlist_clips", out var clips) && clips.ValueKind == JsonValueKind.Array) playlist.SongCount = clips.GetArrayLength();
            return playlist;
        }

        /// <summary>GET /api/playlist/{id}/: {playlist_clips: [{clip: {...}}], num_total_results}.</summary>
        public static (IReadOnlyList<SunoClip> Clips, int? Total) ReadPlaylistSongs(string json)
        {
            using var doc = Parse(json);
            var root = doc.RootElement;
            var clips = new List<SunoClip>();
            if (root.ValueKind != JsonValueKind.Object) return (clips, null);
            int? total = root.TryGetProperty("num_total_results", out var t) && t.TryGetInt32(out var n) ? n : null;
            if (root.TryGetProperty("playlist_clips", out var list) && list.ValueKind == JsonValueKind.Array)
            {
                foreach (var entry in list.EnumerateArray().Where(e => e.ValueKind == JsonValueKind.Object))
                {
                    var c = entry.TryGetProperty("clip", out var inner) ? inner : entry;
                    var clip = ReadClip(c);
                    if (clip != null && !string.IsNullOrEmpty(clip.Id)) clips.Add(clip);
                }
            }
            return (clips, total);
        }

        /// <summary>GET /api/gen/{id}/comments: {results: [...], next_cursor}. Field names vary, so several are tried.</summary>
        public static (IReadOnlyList<SongComment> Comments, string? NextCursor) ReadComments(string json)
        {
            using var doc = Parse(json);
            var root = doc.RootElement;
            var items = root.ValueKind == JsonValueKind.Array ? root : FirstArray(root, "results", "comments", "items");
            var comments = new List<SongComment>();
            if (items is { } array)
            {
                foreach (var c in array.EnumerateArray().Where(e => e.ValueKind == JsonValueKind.Object))
                {
                    var text = FirstString(c, "content", "text", "body");
                    if (string.IsNullOrEmpty(text)) continue;
                    var user = c.TryGetProperty("user", out var u) && u.ValueKind == JsonValueKind.Object ? u : c;
                    comments.Add(new SongComment
                    {
                        Author = FirstString(user, "display_name", "user_display_name", "handle", "user_handle") ?? "",
                        Text = text,
                        When = FirstString(c, "created_at"),
                        Likes = ReadNumber(c, "num_likes", "like_count", "upvote_count"),
                        Replies = ReadNumber(c, "num_replies", "reply_count")
                    });
                }
            }
            var next = root.ValueKind == JsonValueKind.Object ? FirstString(root, "next_cursor", "nextCursor", "cursor") : null;
            return (comments, next);
        }

        private static int ReadNumber(JsonElement obj, params string[] names)
        {
            foreach (var name in names)
                if (obj.TryGetProperty(name, out var v) && v.TryGetInt32(out var n)) return n;
            return 0;
        }

        public static IReadOnlyList<Notification> ReadNotifications(string json)
        {
            using var doc = Parse(json);
            var root = doc.RootElement;
            var items = root.ValueKind == JsonValueKind.Array ? root
                : FirstArray(root, "notifications", "results", "items", "data");
            if (items is not { } array) return Array.Empty<Notification>();

            return array.EnumerateArray()
                .Where(e => e.ValueKind == JsonValueKind.Object)
                .Select(e => new Notification
                {
                    Title = FirstString(e, "title", "heading", "type") ?? "",
                    Message = FirstString(e, "message", "body", "content", "text") ?? "",
                    IsRead = !(e.TryGetProperty("is_read", out var read) && read.ValueKind == JsonValueKind.False)
                })
                .Where(n => n.Title.Length > 0 || n.Message.Length > 0)
                .ToList();
        }

        public static string? ReadString(string json, string name)
        {
            using var doc = Parse(json);
            return doc.RootElement.ValueKind == JsonValueKind.Object
                   && doc.RootElement.TryGetProperty(name, out var v) && v.ValueKind == JsonValueKind.String
                ? v.GetString()
                : null;
        }

        public static int? ReadInt(string json, string name)
        {
            using var doc = Parse(json);
            return doc.RootElement.ValueKind == JsonValueKind.Object
                   && doc.RootElement.TryGetProperty(name, out var v) && v.TryGetInt32(out var n)
                ? n
                : null;
        }

        public static bool? ReadBool(string json, string name)
        {
            using var doc = Parse(json);
            return doc.RootElement.ValueKind == JsonValueKind.Object
                   && doc.RootElement.TryGetProperty(name, out var v) && v.ValueKind is JsonValueKind.True or JsonValueKind.False
                ? v.GetBoolean()
                : null;
        }

        private static JsonDocument Parse(string json)
        {
            try { return JsonDocument.Parse(string.IsNullOrWhiteSpace(json) ? "{}" : json); }
            catch (JsonException ex)
            {
                Log.Error("Unreadable reply from Suno", ex);
                throw new SunoApiException(T("Suno sent a reply this app doesn't understand."), null, ex);
            }
        }

        private static List<SunoClip> ToClips(JsonElement array) =>
            array.EnumerateArray()
                .Select(ReadClip)
                .Where(c => c != null && !string.IsNullOrEmpty(c.Id))
                .Select(c => c!)
                .ToList();

        /// <summary>One clip, or null when it isn't one or has a field of an unexpected type; one odd song mustn't lose the page.</summary>
        private static SunoClip? ReadClip(JsonElement e)
        {
            if (e.ValueKind != JsonValueKind.Object) return null;
            try { return e.Deserialize<SunoClip>(JsonOpts); }
            catch (JsonException ex)
            {
                Log.Warn($"Skipped a song Suno described oddly: {ex.Message}");
                return null;
            }
        }

        private static void Walk(JsonElement e, List<SunoClip> found, HashSet<string> seen)
        {
            switch (e.ValueKind)
            {
                case JsonValueKind.Object:
                    if (e.TryGetProperty("id", out var id) && id.ValueKind == JsonValueKind.String
                        && e.TryGetProperty("audio_url", out _))
                    {
                        var clip = ReadClip(e);
                        if (clip != null && seen.Add(clip.Id)) found.Add(clip);
                        return;
                    }
                    foreach (var p in e.EnumerateObject()) Walk(p.Value, found, seen);
                    break;
                case JsonValueKind.Array:
                    foreach (var item in e.EnumerateArray()) Walk(item, found, seen);
                    break;
            }
        }

        private static JsonElement? FirstArray(JsonElement obj, params string[] names)
        {
            if (obj.ValueKind != JsonValueKind.Object) return null;
            foreach (var name in names)
                if (obj.TryGetProperty(name, out var v) && v.ValueKind == JsonValueKind.Array) return v;
            return null;
        }

        private static string? FirstString(JsonElement obj, params string[] names)
        {
            foreach (var name in names)
                if (obj.TryGetProperty(name, out var v) && v.ValueKind == JsonValueKind.String && !string.IsNullOrWhiteSpace(v.GetString()))
                    return v.GetString()!.Trim();
            return null;
        }
    }
}
