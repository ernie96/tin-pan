using System;
using System.Collections.Generic;
using System.Linq;
using System.Text.Json;
using TinPan.Models;

namespace TinPan.Services
{
    // Creator pages, Explore, contests, style suggestions and lyrics drafts. Most of these replies were never
    // captured whole, so each reader takes the fields it knows and skips the rest.
    public static partial class SunoJson
    {
        /// <summary>
        /// GET /api/profiles/{handle}: handle, display_name, profile_description, num_total_clips, clips, and
        /// stats with followers_count, following_count and play_count__sum.
        /// </summary>
        public static CreatorPage ReadCreatorPage(string json, string handle)
        {
            using var doc = Parse(json);
            var p = doc.RootElement;
            if (p.ValueKind != JsonValueKind.Object) return new CreatorPage(handle, "", "", 0, 0, 0, 0, Array.Empty<SunoClip>());
            var stats = p.ValueKind == JsonValueKind.Object && p.TryGetProperty("stats", out var s) && s.ValueKind == JsonValueKind.Object ? s : default;
            var songs = p.ValueKind == JsonValueKind.Object && FirstArray(p, "clips") is { } clips ? ToClips(clips) : FindClipsIn(p);
            return new CreatorPage(
                FirstString(p, "handle") ?? handle,
                FirstString(p, "display_name") ?? "",
                FirstString(p, "profile_description", "description", "bio") ?? "",
                ReadNumber(p, "num_total_clips", "clip_count"),
                stats.ValueKind == JsonValueKind.Object ? ReadNumber(stats, "followers_count", "num_followers") : ReadNumber(p, "followers_count", "num_followers"),
                stats.ValueKind == JsonValueKind.Object ? ReadNumber(stats, "following_count", "num_following") : ReadNumber(p, "following_count", "num_following"),
                stats.ValueKind == JsonValueKind.Object ? ReadLong(stats, "play_count__sum") : 0,
                songs);
        }

        /// <summary>
        /// POST /api/unified/explore: feeds[] with feed_title (older replies: title, name) and items. An item's
        /// content_item is a song, or itself a feed (content_type "generic_feed") whose items are the songs: that
        /// one becomes a shelf of its own, so its title ("Staff Picks") isn't lost. Shortcut tiles carry no song
        /// and are skipped. A shelf with no songs of its own isn't listed.
        /// </summary>
        public static (IReadOnlyList<ExploreSection> Sections, string? NextCursor) ReadExplore(string json)
        {
            using var doc = Parse(json);
            var root = doc.RootElement;
            var sections = new List<ExploreSection>();
            if (FirstArray(root, "feeds", "sections") is { } feeds)
                foreach (var feed in feeds.EnumerateArray().Where(f => f.ValueKind == JsonValueKind.Object))
                    AddShelf(feed, sections, depth: 0);
            return (sections, root.ValueKind == JsonValueKind.Object ? FirstString(root, "next_cursor") : null);
        }

        private static void AddShelf(JsonElement feed, List<ExploreSection> sections, int depth)
        {
            var title = FirstString(feed, "feed_title", "title", "name", "label", "display_name")
                        ?? FirstString(feed, "feed_id", "id", "identifier") ?? "Explore";
            var songs = new List<SunoClip>();
            var nested = new List<JsonElement>();
            if (feed.TryGetProperty("items", out var items) && items.ValueKind == JsonValueKind.Array)
            {
                foreach (var item in items.EnumerateArray().Where(i => i.ValueKind == JsonValueKind.Object))
                {
                    var content = item.TryGetProperty("content_item", out var c) && c.ValueKind == JsonValueKind.Object ? c : item;
                    if (depth < 2 && IsFeed(item, content)) nested.Add(content);
                    else songs.AddRange(FindClipsIn(content).Where(s => songs.All(o => o.Id != s.Id)));
                }
            }
            else songs.AddRange(FindClipsIn(feed));
            if (songs.Count > 0) sections.Add(new ExploreSection(title, songs));
            foreach (var inner in nested) AddShelf(inner, sections, depth + 1);
        }

        private static bool IsFeed(JsonElement item, JsonElement content) =>
            FirstString(item, "content_type") == "generic_feed" || FirstString(content, "content_type") == "generic_feed"
            || (content.TryGetProperty("items", out var i) && i.ValueKind == JsonValueKind.Array && FirstString(content, "feed_id", "feed_title") != null);

        /// <summary>
        /// tracks/add and tracks/remove answer {successes, failures}, as lists or counts (chadvis 2026-10-26). True when
        /// something failed and nothing succeeded. Any other reply (an older empty one) counts as done.
        /// </summary>
        public static bool PlaylistChangeFailed(string json)
        {
            try
            {
                using var doc = JsonDocument.Parse(string.IsNullOrWhiteSpace(json) ? "{}" : json);
                var root = doc.RootElement;
                if (root.ValueKind != JsonValueKind.Object) return false;
                static int Count(JsonElement o, string name) =>
                    !o.TryGetProperty(name, out var v) ? 0 : v.ValueKind == JsonValueKind.Array ? v.GetArrayLength() : AsInt(v) ?? 0;
                return Count(root, "failures") > 0 && Count(root, "successes") == 0;
            }
            catch (JsonException)
            {
                return false;
            }
        }

        /// <summary>GET /api/contests/: {contests: [{title, description, end_time or ends_at, submission_count, base_clip_id}]}.</summary>
        public static IReadOnlyList<Contest> ReadContests(string json)
        {
            using var doc = Parse(json);
            var root = doc.RootElement;
            var list = root.ValueKind == JsonValueKind.Array ? root : FirstArray(root, "contests", "results", "items");
            if (list is not { } array) return Array.Empty<Contest>();
            return array.EnumerateArray()
                .Where(c => c.ValueKind == JsonValueKind.Object)
                .Select(c => new Contest(
                    FirstString(c, "title", "name", "display_name") ?? "",
                    FirstString(c, "description", "subtitle") ?? "",
                    ReadDate(c, "end_time", "ends_at", "end_date"),
                    ReadNumber(c, "submission_count", "num_submissions", "entry_count"),
                    FirstString(c, "base_clip_id", "clip_id")))
                .Where(c => c.Title.Length > 0)
                .ToList();
        }

        /// <summary>GET /api/prompts/suggestions: style ideas, as strings or objects with the text in a field.</summary>
        public static IReadOnlyList<string> ReadStyleSuggestions(string json)
        {
            using var doc = Parse(json);
            var found = new List<string>();
            void Add(JsonElement array)
            {
                foreach (var e in array.EnumerateArray())
                {
                    var text = e.ValueKind == JsonValueKind.String ? e.GetString()?.Trim()
                        : e.ValueKind == JsonValueKind.Object ? FirstString(e, "prompt", "text", "tags", "tag", "name", "label", "value") : null;
                    if (!string.IsNullOrWhiteSpace(text) && !found.Contains(text, StringComparer.OrdinalIgnoreCase)) found.Add(text);
                }
            }
            var root = doc.RootElement;
            if (root.ValueKind == JsonValueKind.Array) Add(root);
            else
                foreach (var name in new[] { "prompts", "suggestions", "tags", "styles" })
                    if (FirstArray(root, name) is { } a) Add(a);
            return found;
        }

        /// <summary>GET /api/lyrics-projects: {projects: [{id, title, updated_at}], next_cursor}.</summary>
        public static (IReadOnlyList<LyricsDraft> Drafts, string? NextCursor) ReadLyricsDrafts(string json)
        {
            using var doc = Parse(json);
            var root = doc.RootElement;
            var drafts = new List<LyricsDraft>();
            if (FirstArray(root, "projects", "results", "items") is { } list)
                foreach (var p in list.EnumerateArray().Where(p => p.ValueKind == JsonValueKind.Object))
                    if (FirstString(p, "id", "project_id") is { } id)
                        drafts.Add(new LyricsDraft(id, FirstString(p, "title", "name") ?? "", ReadDate(p, "updated_at", "created_at")));
            return (drafts, root.ValueKind == JsonValueKind.Object ? FirstString(root, "next_cursor") : null);
        }

        /// <summary>
        /// The words of one lyrics draft (GET /api/lyrics-projects/{id}). Its shape wasn't captured, so this takes
        /// a field named lyrics, content, text or body, at any depth, and failing that the longest such text.
        /// </summary>
        public static string? ReadLyricsDraftText(string json)
        {
            using var doc = Parse(json);
            string? best = null;
            void Look(JsonElement e, int depth)
            {
                if (depth > 6) return;
                if (e.ValueKind == JsonValueKind.Object)
                {
                    foreach (var p in e.EnumerateObject())
                    {
                        if (p.Value.ValueKind == JsonValueKind.String && p.Name is "lyrics" or "content" or "text" or "body" or "prompt")
                        {
                            var v = p.Value.GetString();
                            if (!string.IsNullOrWhiteSpace(v) && v.Length > (best?.Length ?? 0)) best = v;
                        }
                        else Look(p.Value, depth + 1);
                    }
                }
                else if (e.ValueKind == JsonValueKind.Array)
                    foreach (var item in e.EnumerateArray()) Look(item, depth + 1);
            }
            Look(doc.RootElement, 0);
            return best?.Trim();
        }

        /// <summary>The id of the song another came from, from GET /api/clips/parent (shape not captured).</summary>
        public static string? ReadParentId(string json, string childId)
        {
            var clips = FindClips(json).Where(c => c.Id != childId).ToList();
            if (clips.Count > 0) return clips[0].Id;
            using var doc = Parse(json);
            var root = doc.RootElement;
            if (root.ValueKind != JsonValueKind.Object) return null;
            var id = FirstString(root, "parent_id", "parent_clip_id", "clip_id", "id");
            if (id == null && root.TryGetProperty("parent", out var parent) && parent.ValueKind == JsonValueKind.Object)
                id = FirstString(parent, "id", "clip_id");
            return id == childId ? null : id;
        }

        private static List<SunoClip> FindClipsIn(JsonElement e)
        {
            var found = new List<SunoClip>();
            Walk(e, found, new HashSet<string>());
            return found;
        }

        private static long ReadLong(JsonElement obj, params string[] names)
        {
            foreach (var name in names)
                if (obj.TryGetProperty(name, out var v) && v.ValueKind == JsonValueKind.Number && v.TryGetInt64(out var n)) return n;
            return 0;
        }

        private static DateTimeOffset? ReadDate(JsonElement obj, params string[] names)
        {
            foreach (var name in names)
                if (obj.TryGetProperty(name, out var v) && v.ValueKind == JsonValueKind.String && DateTimeOffset.TryParse(v.GetString(),
                        System.Globalization.CultureInfo.InvariantCulture, System.Globalization.DateTimeStyles.AssumeUniversal, out var d))
                    return d;
            return null;
        }
    }
}
