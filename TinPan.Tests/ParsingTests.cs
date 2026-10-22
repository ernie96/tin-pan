using System.Collections.Generic;
using System.Text;
using System.Text.Json;
using TinPan.Models;
using TinPan.Services;
using TinPan.ViewModels;
using Xunit;

namespace TinPan.Tests
{
    public class CookieTests
    {
        [Fact]
        public void Parse_keeps_values_and_skips_cookie_attributes()
        {
            var jar = SunoCookies.Parse("__client=abc.def; Path=/; HttpOnly; ajs_anonymous_id=dev-1; Secure; __session=eyJ=x");

            Assert.True(jar.HasClient);
            Assert.Equal("abc.def", jar.Client);
            Assert.Equal("dev-1", jar.AnonymousId);
            Assert.Equal("eyJ=x", jar.Get("__session"));
            Assert.Null(jar.Get("Path"));
        }

        [Fact]
        public void MergeSetCookie_replaces_the_rotated_client_cookie()
        {
            var jar = SunoCookies.Parse("__client=old");
            jar.MergeSetCookie("__client=new; Domain=auth.suno.com; Expires=Wed, 21 Oct 2027 07:28:00 GMT; HttpOnly");

            Assert.Equal("new", jar.Client);
            Assert.Equal("__client=new", jar.ToHeader());
        }

        [Fact]
        public void No_client_cookie_means_not_signed_in()
        {
            Assert.False(SunoCookies.Parse("__session=x; ajs_anonymous_id=y").HasClient);
            Assert.False(SunoCookies.Parse(null).HasClient);
        }
    }

    public class JwtTests
    {
        private static string Token(string payloadJson)
        {
            static string B64(string s) => Convert.ToBase64String(Encoding.UTF8.GetBytes(s)).TrimEnd('=').Replace('+', '-').Replace('/', '_');
            return $"{B64("{\"alg\":\"none\"}")}.{B64(payloadJson)}.sig";
        }

        [Fact]
        public void ReadExpiry_reads_exp_claim()
        {
            Assert.Equal(DateTimeOffset.FromUnixTimeSeconds(1800000000), Jwt.ReadExpiry(Token("{\"exp\":1800000000,\"sid\":\"s\"}")));
        }

        [Theory]
        [InlineData(null)]
        [InlineData("")]
        [InlineData("not-a-jwt")]
        [InlineData("a.!!!.c")]
        public void ReadExpiry_is_null_for_garbage(string? token) => Assert.Null(Jwt.ReadExpiry(token));
    }

    public class SunoJsonTests
    {
        private const string Clip = "{\"id\":\"c1\",\"title\":\"Song\",\"audio_url\":\"https://cdn1.suno.ai/c1.mp3\",\"status\":\"complete\",\"metadata\":{\"duration\":125.4,\"tags\":\"pop\"}}";

        [Fact]
        public void ReadFeedPage_reads_wrapped_clips_and_total()
        {
            var page = SunoJson.ReadFeedPage($"{{\"clips\":[{Clip}],\"num_total\":41}}");

            Assert.Equal(41, page.NumTotal);
            var clip = Assert.Single(page.Clips);
            Assert.Equal("c1", clip.Id);
            Assert.Equal(125.4, clip.Duration);
            Assert.Equal("pop", clip.Metadata?.Tags);
        }

        [Fact]
        public void ReadFeedPage_reads_v3_cursor()
        {
            var page = SunoJson.ReadFeedPage($"{{\"clips\":[{Clip}],\"next_cursor\":\"abc\",\"has_more\":true}}");
            Assert.Equal("abc", page.NextCursor);
            Assert.True(page.HasMore);
        }

        [Fact]
        public void FindClipsById_reads_a_single_clip_or_a_list()
        {
            Assert.Single(SunoJson.FindClipsById(Clip));
            Assert.Single(SunoJson.FindClipsById($"{{\"clips\":[{Clip}]}}"));
            Assert.Empty(SunoJson.FindClipsById("{\"ok\":true}"));
        }

        [Fact]
        public void TryReadString_ignores_html_error_pages() => Assert.Null(SunoJson.TryReadString("<html>502</html>", "detail"));

        [Fact]
        public void ReadFeedPage_reads_bare_array() =>
            Assert.Single(SunoJson.ReadFeedPage($"[{Clip}]").Clips);

        [Fact]
        public void FindClips_finds_nested_clips_once()
        {
            var clips = SunoJson.FindClips($"{{\"sections\":[{{\"items\":[{Clip},{{\"clip\":{Clip}}}]}}]}}");
            Assert.Single(clips);
        }

        [Fact]
        public void Bad_json_becomes_a_speakable_error() =>
            Assert.Throws<SunoApiException>(() => SunoJson.ReadFeedPage("<html>"));

        [Fact]
        public void ReadNotifications_accepts_several_shapes()
        {
            var items = SunoJson.ReadNotifications("{\"notifications\":[{\"title\":\"Hi\",\"body\":\"New feature\"},{\"other\":1}]}");
            var n = Assert.Single(items);
            Assert.Equal("Hi: New feature", n.ToString());
        }

        [Fact]
        public void ReadBool_and_ReadString()
        {
            Assert.True(SunoJson.ReadBool("{\"required\":true}", "required"));
            Assert.Null(SunoJson.ReadBool("{}", "required"));
            Assert.Equal("https://x/y.wav", SunoJson.ReadString("{\"wav_file_url\":\"https://x/y.wav\"}", "wav_file_url"));
        }
    }

    public class GenerateRequestTests
    {
        [Fact]
        public void V2_web_request_has_every_field_the_server_requires()
        {
            var req = new GenerateRequest("chirp-fenix", GenerateRequest.Modes.Inspiration) { Prompt = "a sea shanty" };
            using var doc = JsonDocument.Parse(JsonSerializer.Serialize(req));
            var root = doc.RootElement;

            Assert.Equal("chirp-fenix", root.GetProperty("mv").GetString());
            Assert.Equal("a sea shanty", root.GetProperty("prompt").GetString());
            Assert.Equal("inspiration", root.GetProperty("metadata").GetProperty("create_mode").GetString());
            Assert.Equal(JsonValueKind.Null, root.GetProperty("token").ValueKind);
            Assert.Equal(JsonValueKind.Null, root.GetProperty("continue_clip_id").ValueKind);
            Assert.Equal(JsonValueKind.Null, root.GetProperty("cover_clip_id").ValueKind);
            Assert.Equal("", root.GetProperty("negative_tags").GetString());
            Assert.Equal(JsonValueKind.Array, root.GetProperty("override_fields").ValueKind);
            Assert.True(Guid.TryParse(root.GetProperty("transaction_uuid").GetString(), out _));
            Assert.False(root.GetProperty("metadata").TryGetProperty("control_sliders", out _));
        }

        [Fact]
        public void Cover_sliders_send_only_what_was_set_as_fractions()
        {
            var req = new GenerateRequest("chirp-fenix", GenerateRequest.Modes.Cover) { CoverClipId = "c1" };
            req.Metadata.ControlSliders = ControlSliders.FromPercent(null, null, 65);
            using var doc = JsonDocument.Parse(JsonSerializer.Serialize(req));
            var sliders = doc.RootElement.GetProperty("metadata").GetProperty("control_sliders");

            Assert.Equal(0.65, sliders.GetProperty("audio_weight").GetDouble(), 3);
            Assert.False(sliders.TryGetProperty("weirdness_constraint", out _));
            Assert.Equal("c1", doc.RootElement.GetProperty("cover_clip_id").GetString());
            Assert.Null(ControlSliders.FromPercent(null, null, null));
        }

        [Fact]
        public void Length_check_names_the_field_over_the_model_limit()
        {
            var req = new GenerateRequest("m", GenerateRequest.Modes.Custom) { Prompt = new string('a', 11), Title = "ok" };
            var limits = new MaxLengths { Title = 100, Prompt = 10, Tags = 100, NegativeTags = 100, Description = 5 };

            Assert.Equal("Lyrics is 11 characters. This model allows 10.", SunoPanelViewModel.CheckLengths(req, limits, inspiration: false));
            Assert.Equal("Description is 11 characters. This model allows 5.", SunoPanelViewModel.CheckLengths(req, limits, inspiration: true));
            req.Prompt = "short";
            Assert.Null(SunoPanelViewModel.CheckLengths(req, limits, inspiration: false));
        }
    }

    public class BillingTests
    {
        private static BillingInfo Parse(string json) => JsonSerializer.Deserialize<BillingInfo>(json)!;

        [Theory]
        [InlineData("{\"total_credits_left\":50,\"plan\":{\"name\":\"Free Plan\",\"plan_key\":\"free\"}}", false)]
        [InlineData("{\"total_credits_left\":50,\"plan\":{\"name\":\"Basic\"}}", false)]
        [InlineData("{\"total_credits_left\":2500,\"plan\":{\"name\":\"Pro Plan\",\"plan_key\":\"pro\"}}", true)]
        [InlineData("{\"total_credits_left\":10000,\"plan\":{\"name\":\"Premier\"}}", true)]
        [InlineData("{\"total_credits_left\":10}", null)]
        public void IsPaid_reads_the_plan(string json, bool? expected) => Assert.Equal(expected, Parse(json).IsPaid);

        [Fact]
        public void Plan_text_reads_like_a_sentence()
        {
            // The numbers from Ernesto's own account on 2026-10-21.
            var info = Parse("{\"total_credits_left\":3100,\"credits\":810,\"monthly_limit\":2500,\"monthly_usage\":210,\"is_active\":true," +
                             "\"renews_on\":\"2026-11-09T20:48:16Z\",\"plan\":{\"name\":\"Pro Plan\",\"plan_key\":\"pro\",\"usage_plan_features\":[{\"name\":\"commercial_rights\"},{\"name\":\"auk\"},{\"name\":\"max_mode\"}]}," +
                             "\"download_usage\":{\"current_period_downloads_limit\":20,\"current_period_downloads_used\":0,\"additional_download_remaining\":7}}");
            var text = BillingText.Subscription(info);

            Assert.StartsWith("Pro plan, active, renews ", text);
            Assert.DoesNotContain("Plan plan", text);
            Assert.DoesNotContain("T20:48", text);
            Assert.Contains("of this month's", text);
            Assert.Contains("plus 810 extra", text);
            Assert.Contains("27 downloads left: 20 a month, 0 used, plus 7 extra.", text);
            Assert.Equal("Your plan includes: commercial use, v4.5 models, max mode.", BillingText.Features(info));
        }

        [Fact]
        public void Downloads_left_adds_bought_ones_and_never_goes_negative()
        {
            var info = Parse("{\"total_credits_left\":1,\"plan\":{\"name\":\"Pro\"},\"download_usage\":{\"current_period_downloads_limit\":20," +
                             "\"current_period_downloads_used\":5,\"additional_download_remaining\":3}}");
            Assert.Equal(18, info.DownloadUsage!.Remaining);
            Assert.Contains("18 downloads left: 20 a month, 5 used, plus 3 extra.", BillingText.Subscription(info));

            Assert.Equal(2, new DownloadUsage { Limit = 7, Used = 9, Extra = 2 }.Remaining);
            Assert.Null(new DownloadUsage().Remaining);
        }

        [Fact]
        public void Studio_is_no_longer_claimed_to_be_uncounted()
        {
            // Suno started counting Studio downloads on 2026-09-09.
            var info = Parse("{\"total_credits_left\":1,\"plan\":{\"name\":\"Premier\"},\"accessible_features\":[{\"name\":\"studio\"}]}");
            Assert.True(info.HasStudio);
            Assert.DoesNotContain("don't count", BillingText.Subscription(info));
        }

        [Fact]
        public void Remaster_models_count_even_when_can_use_is_false()
        {
            var info = Parse("{\"total_credits_left\":1,\"remaster_model_types\":[{\"name\":\"v5\",\"external_key\":\"chirp-carp\",\"is_default_model\":true}]," +
                             "\"models\":[{\"name\":\"v5.5\",\"external_key\":\"chirp-fenix\",\"can_use\":true},{\"name\":\"old\",\"external_key\":\"x\",\"can_use\":false}]}");
            var account = new AccountState(null!) { Billing = info };

            Assert.Equal(new[] { "chirp-carp", "chirp-flounder", "chirp-bass" }, account.RemasterModels.Select(m => m.ExternalKey));
            Assert.Equal("chirp-fenix", Assert.Single(account.Models).ExternalKey);
            Assert.Equal("Download WAV (needs Pro or Premier)", account.PaidLabel("Download WAV"));

            account.Billing = Parse("{\"total_credits_left\":1,\"plan\":{\"name\":\"Pro\"}}");
            Assert.Equal("Download WAV", account.PaidLabel("Download WAV"));
        }
    }

    public class ApiHelperTests
    {
        [Fact]
        public void Browser_token_wraps_a_base64_timestamp()
        {
            var token = SunoApiService.BrowserToken(DateTimeOffset.FromUnixTimeMilliseconds(1700000000123));
            using var doc = JsonDocument.Parse(token);
            var inner = Encoding.UTF8.GetString(Convert.FromBase64String(doc.RootElement.GetProperty("token").GetString()!));
            Assert.Equal("{\"timestamp\":1700000000123}", inner);
        }

        [Theory]
        [InlineData("{\"detail\":\"Token validation failed.\"}", true)]
        [InlineData("JWT is expired", true)]
        [InlineData("{\"detail\":\"Insufficient credits\"}", false)]
        public void Stale_token_replies_are_recognised(string body, bool stale) =>
            Assert.Equal(stale, SunoApiService.LooksLikeStaleToken(body));

        [Theory]
        [InlineData("90", 90.0)]
        [InlineData("1:30", 90.0)]
        [InlineData("2:05.5", 125.5)]
        [InlineData("1:75", null)]
        [InlineData("abc", null)]
        public void Times_accept_seconds_or_minutes_and_seconds(string text, double? seconds) =>
            Assert.Equal(seconds, TinPan.Views.NumberField.Parse(text, isTime: true));
    }

    public class FileNameTests
    {
        [Theory]
        [InlineData("My Song", "1234567890ab", "mp3", "My Song (12345678).mp3")]
        [InlineData("a/b:c?", "id", "wav", "a_b_c_ (id).wav")]
        [InlineData("CON", "id", "mp3", "_CON (id).mp3")]
        [InlineData("dots...", "id", "mp3", "dots (id).mp3")]
        [InlineData("   ", "id", "mp3", "Untitled (id).mp3")]
        [InlineData(null, "id", ".mp3", "Untitled (id).mp3")]
        public void ForSong_makes_safe_unique_names(string? title, string id, string ext, string expected) =>
            Assert.Equal(expected, FileNames.ForSong(title, id, ext));
    }

    public class SpokenTextTests
    {
        [Fact]
        public void Spoken_text_is_title_length_and_state()
        {
            var clip = new SunoClip { Title = "Rain", Status = "streaming", Metadata = new ClipMetadata { Duration = 75 } };
            Assert.Equal("Rain, 1:15", clip.SpokenText);

            clip.Status = "queued";
            Assert.Equal("Rain, 1:15, still being made", clip.SpokenText);

            clip.Status = "error";
            Assert.Equal("Rain, 1:15, failed", clip.SpokenText);

            Assert.Equal("Untitled song", new SunoClip().SpokenText);
        }

        [Theory]
        [InlineData("complete", true)]
        [InlineData("streaming", true)]
        [InlineData("queued", false)]
        [InlineData("error", false)]
        public void Songs_play_once_suno_streams_them(string status, bool playable) =>
            Assert.Equal(playable, new SunoClip { Status = status, AudioUrl = null }.CanPlay);

        [Fact]
        public void Details_list_style_link_and_lyrics()
        {
            var clip = new SunoClip
            {
                Id = "abc", Title = "Rain", Status = "complete", IsPublic = true, PlayCount = 3,
                Metadata = new ClipMetadata { Duration = 75, Tags = "lofi", Prompt = "[Verse]\nIt rains" }
            };
            var text = clip.DetailsText;

            Assert.StartsWith("Rain", text);
            Assert.Contains("Style: lofi", text);
            Assert.Contains("Public", text);
            Assert.Contains("https://suno.com/song/abc", text);
            Assert.EndsWith("[Verse]\nIt rains", text.Replace("\r\n", "\n"));
        }

        [Fact]
        public void Update_keeps_the_same_object_for_screen_readers()
        {
            var clip = new SunoClip { Id = "c1", Status = "queued" };
            var changed = new List<string?>();
            clip.PropertyChanged += (_, e) => changed.Add(e.PropertyName);

            clip.UpdateFrom(new SunoClip { Id = "c1", Status = "complete", Title = "Done" });

            Assert.Equal("Done", clip.Title);
            Assert.True(clip.IsReady);
            Assert.Contains(string.Empty, changed);
        }
    }

    public class HelpTextTests
    {
        [Fact]
        public void Every_tab_has_short_help_and_full_help_lists_the_keys()
        {
            Assert.False(string.IsNullOrWhiteSpace(HelpText.Library));
            Assert.False(string.IsNullOrWhiteSpace(HelpText.Playlists));
            Assert.False(string.IsNullOrWhiteSpace(HelpText.Create));
            Assert.False(string.IsNullOrWhiteSpace(HelpText.Trending));
            Assert.False(string.IsNullOrWhiteSpace(HelpText.Account));
            Assert.False(string.IsNullOrWhiteSpace(HelpText.Settings));
            foreach (var key in new[] { "F1", "F2", "F3", "F5", "F7", "F8", "F9", "F11", "Ctrl+1 to Ctrl+6", "Enter", "Delete" })
                Assert.Contains(key, HelpText.Full);
        }

        [Fact]
        public void Not_signed_in_help_points_at_the_Settings_key() =>
            Assert.Contains($"Ctrl+{MainViewModel.SettingsTab + 1} opens Settings", HelpText.NotSignedIn);
    }
}

namespace TinPan.Tests
{
    public class ExporterIdeasTests
    {
        [Fact]
        public void Free_file_is_the_media_url_without_encoding()
        {
            var clip = System.Text.Json.JsonSerializer.Deserialize<SunoClip>(
                "{\"id\":\"c\",\"audio_url\":\"https://studio-api.prod.suno.com/api/forbidden\",\"media_urls\":[" +
                "{\"url\":\"https://cdn1.suno.ai/c.m4a\",\"encoding\":\"m4a-opus\"}," +
                "{\"url\":\"https://cdn1.suno.ai/c.mp4\"}," +
                "{\"url\":\"https://cdn1.suno.ai/c.m4a?Key-Pair-Id=k\"}]}")!;
            Assert.Equal("https://cdn1.suno.ai/c.m4a?Key-Pair-Id=k", clip.ProgressiveAudioUrl);
            Assert.Equal("m4a", FileNames.ExtensionOf(clip.ProgressiveAudioUrl, "mp3"));
            Assert.Equal("mp3", FileNames.ExtensionOf("https://x/forbidden", "mp3"));
        }

        [Fact]
        public void Every_entry_encrypted_means_no_free_file()
        {
            var clip = System.Text.Json.JsonSerializer.Deserialize<SunoClip>(
                "{\"id\":\"c\",\"media_urls\":[{\"url\":\"https://cdn1.suno.ai/c.m4a\",\"encoding\":\"m4a-opus\"}]}")!;
            Assert.Null(clip.ProgressiveAudioUrl);
        }

        [Fact]
        public void Feed_filters_follow_the_web_client()
        {
            var filter = new LibraryFilter
            {
                WorkspaceId = "ws1",
                Disliked = "Any",
                Show = new Dictionary<string, string> { ["trashed"] = "Any", ["stem"] = "Any", ["upload"] = "True", ["cover"] = "False", ["liked"] = "nonsense" },
                Sort = "most-liked"
            };
            var json = System.Text.Json.JsonSerializer.Serialize(SunoApiService.BuildFeedFilters(new LibraryQuery(Search: "rain", Filter: filter)));
            Assert.Contains("\"trashed\":\"Any\"", json);
            Assert.Contains("\"disliked\":\"Any\"", json);
            Assert.Contains("\"upload\":\"True\"", json);
            Assert.Contains("\"cover\":{\"presence\":\"False\"}", json);
            Assert.Contains("\"liked\":\"Any\"", json); // an unknown value falls back to the default
            Assert.Contains("\"stemComplement\":\"Any\"", json);
            Assert.Contains("\"workspace\":{\"presence\":\"True\",\"workspaceId\":\"ws1\"}", json);
            Assert.Contains("\"sort\":{\"sortBy\":\"upvote_count\",\"sortDirection\":\"desc\"}", json);
            Assert.Contains("\"searchText\":\"rain\"", json);

            // Defaults match Suno's own library: trash, stems and Studio projects hidden, everything else shown.
            var defaults = System.Text.Json.JsonSerializer.Serialize(SunoApiService.BuildFeedFilters(new LibraryQuery()));
            Assert.Contains("\"disliked\":\"False\"", defaults);
            Assert.Contains("\"trashed\":\"False\"", defaults);
            Assert.Contains("\"stem\":{\"presence\":\"False\"}", defaults);
            Assert.Contains("\"stemComplement\":\"False\"", defaults);
            Assert.Contains("\"fromStudioProject\":{\"presence\":\"False\"}", defaults);
            foreach (var field in new[] { "liked", "public", "upload", "unlocked" })
                Assert.Contains($"\"{field}\":\"Any\"", defaults);
            foreach (var field in new[] { "cover", "extend", "remaster", "concat", "persona", "playlist" })
                Assert.Contains($"\"{field}\":{{\"presence\":\"Any\"}}", defaults);
            Assert.Contains("\"sort\":{\"sortBy\":\"created_at\",\"sortDirection\":\"desc\"}", defaults);
            Assert.DoesNotContain("workspace", defaults);
        }

        [Fact]
        public void Old_library_switches_become_the_matching_filters_once()
        {
            var old = new LibraryConfig { IncludeTrashed = true, HideStems = false, NewestFirst = false };
            ConfigService.MigrateLibraryFilters(old);
            Assert.Equal("Any", old.Show["trashed"]);
            Assert.Equal("Any", old.Show["stem"]);
            Assert.Equal("oldest", old.Sort);

            old.Show["trashed"] = "True";
            ConfigService.MigrateLibraryFilters(old); // already moved: nothing changes
            Assert.Equal("True", old.Show["trashed"]);
        }

        [Fact]
        public void Lrc_has_one_timed_line_per_sung_line()
        {
            var clip = new SunoClip { Title = "Rain", Metadata = new ClipMetadata { Duration = 61 } };
            var words = new List<AlignedWord>
            {
                new() { Word = "Hello ", Start = 1.5 }, new() { Word = "rain\n", Start = 2.0 },
                new() { Word = "Goodbye ", Start = 65.25 }, new() { Word = "sun", Start = 66 }
            };
            var lrc = LrcWriter.Write(clip, words);
            Assert.Contains("[ti:Rain]", lrc);
            Assert.Contains("[length:01:01.00]", lrc);
            Assert.Contains("[00:01.50]Hello rain\n", lrc);
            Assert.Contains("[01:05.25]Goodbye sun\n", lrc);
        }

        [Fact]
        public void Liked_songs_say_so() =>
            Assert.Equal("Rain, liked", new SunoClip { Title = "Rain", Status = "complete", IsLiked = true }.SpokenText);
    }
}

namespace TinPan.Tests
{
    public class CaptchaTests
    {
        [Fact]
        public void Token_provider_is_sent_only_with_a_token()
        {
            var req = new GenerateRequest("m", GenerateRequest.Modes.Custom);
            Assert.DoesNotContain("token_provider", System.Text.Json.JsonSerializer.Serialize(req));

            req.Token = "solved";
            req.TokenProvider = 2;
            var json = System.Text.Json.JsonSerializer.Serialize(req);
            Assert.Contains("\"token\":\"solved\"", json);
            Assert.Contains("\"token_provider\":2", json);
        }

        [Fact]
        public void Captcha_check_reply_is_read()
        {
            Assert.True(SunoJson.ReadBool("{\"required\":true,\"captcha_version\":2}", "required"));
            Assert.Equal(2, SunoJson.ReadInt("{\"required\":true,\"captcha_version\":2}", "captcha_version"));
            Assert.Null(SunoJson.ReadInt("{\"required\":false}", "captcha_version"));
        }
    }
}

namespace TinPan.Tests
{
    public class CoverRequestTests
    {
        [Fact]
        public void Cover_and_extend_send_the_task_suno_dot_com_sends()
        {
            var cover = new GenerateRequest("chirp-fenix", GenerateRequest.Modes.Cover) { CoverClipId = "c1", Task = "cover", PersonaId = "p1" };
            var json = System.Text.Json.JsonSerializer.Serialize(cover);
            Assert.Contains("\"task\":\"cover\"", json);
            Assert.Contains("\"cover_clip_id\":\"c1\"", json);
            Assert.Contains("\"persona_id\":\"p1\"", json);

            var plain = System.Text.Json.JsonSerializer.Serialize(new GenerateRequest("m", GenerateRequest.Modes.Custom));
            Assert.DoesNotContain("\"task\"", plain);
        }
    }
}

namespace TinPan.Tests
{
    public class RemasterTests
    {
        [Fact]
        public void Remaster_body_matches_suno_web_and_leaves_out_defaults()
        {
            var body = SunoApiService.BuildRemasterBody(new RemasterRequest("c1", "chirp-halibut", " bright ", "high", "clarity",
                Freedom: 0.0, Tone: 0.5, Clarity: 0.8, FineStrength: 1.5, StereoWidth: null));
            var json = System.Text.Json.JsonSerializer.Serialize(body);

            Assert.Contains("\"clip_id\":\"c1\"", json);
            Assert.Contains("\"model_name\":\"chirp-halibut\"", json);
            Assert.Contains("\"tags\":\"bright\"", json);
            Assert.Contains("\"variation_category\":\"high\"", json);
            Assert.Contains("\"style_profile\":\"clarity\"", json);
            Assert.Contains("\"clarity\":0.8", json);
            Assert.Contains("\"strength\":1", json);          // clamped to 0..1
            Assert.DoesNotContain("freedom", json);            // equals Suno's default 0
            Assert.DoesNotContain("\"tone\"", json);           // equals Suno's default 0.5
            Assert.DoesNotContain("stereo_width", json);
        }

        [Fact]
        public void Older_remaster_models_follow_the_plans_own_list()
        {
            var info = System.Text.Json.JsonSerializer.Deserialize<BillingInfo>(
                "{\"total_credits_left\":1,\"remaster_model_types\":[{\"name\":\"v6\",\"external_key\":\"chirp-halibut\",\"is_default_model\":true}]}")!;
            var keys = new AccountState(null!) { Billing = info }.RemasterModels.Select(m => m.ExternalKey).ToList();
            Assert.Equal(new[] { "chirp-halibut", "chirp-flounder", "chirp-carp", "chirp-bass" }, keys);
        }
    }
}

namespace TinPan.Tests
{
    public class PlaylistAndCommentTests
    {
        [Fact]
        public void Playlist_songs_come_out_of_playlist_clips()
        {
            var (clips, total) = SunoJson.ReadPlaylistSongs(
                "{\"num_total_results\":2,\"playlist_clips\":[{\"clip\":{\"id\":\"a\",\"title\":\"One\",\"status\":\"complete\"}},{\"clip\":{\"id\":\"b\",\"title\":\"Two\"}},{\"clip\":null}]}");
            Assert.Equal(2, total);
            Assert.Equal(new[] { "a", "b" }, clips.Select(c => c.Id));
        }

        [Fact]
        public void Playlist_reads_name_and_count()
        {
            using var doc = System.Text.Json.JsonDocument.Parse("{\"id\":\"p1\",\"name\":\"Road trip\",\"playlist_clips\":[{},{},{}]}");
            var p = SunoJson.ReadPlaylist(doc.RootElement);
            Assert.Equal("Road trip, 3 songs", p.SpokenText);
            Assert.Equal("Untitled playlist", new Playlist().SpokenText);
        }

        [Fact]
        public void Comments_read_author_text_likes_and_cursor()
        {
            var (comments, next) = SunoJson.ReadComments(
                "{\"results\":[{\"content\":\"Love it\",\"user\":{\"display_name\":\"Ana\"},\"num_likes\":2},{\"content\":\"\"},{\"text\":\"Nice\",\"handle\":\"bo\"}],\"next_cursor\":\"c2\"}");
            Assert.Equal("c2", next);
            Assert.Equal(2, comments.Count);
            Assert.Equal("Ana: Love it (2 likes)", comments[0].ToString());
            Assert.Equal("bo: Nice", comments[1].ToString());
        }
    }
}

namespace TinPan.Tests
{
    public class KeySoftIdeasTests
    {
        [Fact]
        public void Every_message_goes_into_the_history_even_when_not_spoken()
        {
            // ErrorsOnly with always:false means nothing is spoken, so no speech engine is touched here.
            var announcer = new ScreenReaderService { Level = SpeechLevel.ErrorsOnly };
            announcer.Announce("Ready: Rain.", interrupt: false);
            announcer.Announce("Playing: Rain.");

            Assert.Equal(2, announcer.History.Count);
            Assert.EndsWith("Playing: Rain.", announcer.History[0]);   // newest first
            Assert.EndsWith("Ready: Rain.", announcer.History[1]);
        }

        [Fact]
        public void History_keeps_the_most_recent_hundred()
        {
            var announcer = new ScreenReaderService { Level = SpeechLevel.ErrorsOnly };
            for (var i = 0; i < 130; i++) announcer.Announce($"Message {i}");
            Assert.Equal(100, announcer.History.Count);
            Assert.EndsWith("Message 129", announcer.History[0]);
        }

        [Fact]
        public void Song_information_is_one_fact_per_line_without_the_lyrics()
        {
            var clip = new SunoClip { Id = "abc", Title = "Rain", Status = "complete", Metadata = new ClipMetadata { Duration = 75, Tags = "lofi", Prompt = "la la" } };
            var lines = clip.InfoLines;

            Assert.Equal("Rain", lines[0]);
            Assert.Contains("Length: 1:15", lines);
            Assert.Contains("Style: lofi", lines);
            Assert.DoesNotContain(lines, l => l.Contains("la la"));
            Assert.Contains("la la", clip.DetailsText);
        }

        [Fact]
        public void Command_labels_are_spoken_without_access_key_marks()
        {
            var entry = new CommandEntry("Song", "Similar so_ngs...", "", new CommunityToolkit.Mvvm.Input.RelayCommand(() => { }));
            Assert.Equal("Similar songs", entry.SpokenLabel);
        }

        [Fact]
        public void Speech_level_labels_cover_every_level()
        {
            foreach (var level in Enum.GetValues<SpeechLevel>())
                Assert.False(string.IsNullOrWhiteSpace(SettingsPanelViewModel.SpeechLevelLabel(level)));
        }

        [Fact]
        public void The_dedication_names_Jenni_and_Diana() =>
            Assert.Equal("Tin Pan, like all my apps, is dedicated to Jenni and Diana.", MainViewModel.Dedication);
    }
}
