using System;
using System.Collections.Generic;
using System.Collections.ObjectModel;
using System.ComponentModel;
using System.IO;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using System.Windows;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using TinPan.Models;
using TinPan.Services;
using TinPan.Views;
using static TinPan.Localization.Loc;

namespace TinPan.ViewModels
{
    // Extend, cover, remaster, stems and whole song.
    public partial class LibraryPanelViewModel
    {
        // ── Making new versions ─────────────────────────────────────────────

        private ChoiceField ModelChoice(string label = "_Model") =>
            new(label, _account.Models, _account.FindModel(_config.Current.LastModel) ?? _account.DefaultModel);

        /// <summary>Suno only extends, covers, remasters or splits a song once it's finished.</summary>
        private bool Finished(SunoClip clip)
        {
            if (clip.IsComplete) return true;
            Announcer.Announce(clip.HasFailed ? T("That song failed to generate.") : T("That song isn't finished yet."));
            return false;
        }

        private bool ModelsLoaded()
        {
            if (_account.Models.Count > 0) return true;
            Announcer.Announce(T("Models haven't loaded yet. Press F5 in the Create window (Ctrl+N), then try again."));
            return false;
        }

        [RelayCommand]
        private async Task ExtendAsync(SunoClip? clip)
        {
            clip ??= SelectedClip;
            if (clip == null || !Finished(clip) || !ModelsLoaded()) return;
            var length = clip.Duration ?? 0;
            var from = new NumberField(T("Extend f_rom (seconds, or minutes:seconds)"), length > 0 ? Math.Floor(length) : null, 0,
                length > 0 ? length : 3600, allowEmpty: false, isTime: true);
            var title = new TextField(T("_Title"), clip.Title ?? "", maxLength: 100);
            var lyrics = new TextField(T("_Lyrics for the new part"), "", multiline: true);
            var style = new TextField(T("_Style"), clip.Metadata?.Tags ?? "");
            var boost = new ButtonField(T("Style _boost"), () => BoostStyleAsync(style, lyrics, null));
            var ideas = new ButtonField(T("Style i_deas..."), () => StyleIdeasAsync(style));
            var exclude = new TextField(T("_Exclude styles"), "");
            var model = ModelChoice();
            var weirdness = new NumberField(T("_Weirdness, 0 to 100 (empty lets Suno choose)"), null, 0, 100);
            var styleInfluence = new NumberField(T("Style in_fluence, 0 to 100 (empty lets Suno choose)"), null, 0, 100);
            var persona = await PersonaChoiceAsync();
            var maxMode = new CheckField(T("Ma_x mode"), false);
            var intro = T("Continues {0} from a point you choose. Leave lyrics empty to let Suno carry on. Use Get whole song afterwards to join the parts.", clip.DisplayTitle);
            // Stays open: extend, listen, change something and extend again.
            _dialogs.ShowToolForm(T("Extend {0}", clip.DisplayTitle), intro, T("Extend"),
                new FormField[] { from, title, lyrics, style, boost, ideas, exclude, model, weirdness, styleInfluence, persona, maxMode }, () =>
                {
                    var chosen = (SunoModel?)model.Selected ?? _account.DefaultModel!;
                    var request = new GenerateRequest(chosen.ExternalKey, GenerateRequest.Modes.Custom)
                    {
                        Title = title.Value,
                        Prompt = lyrics.Value,
                        Tags = style.Value,
                        NegativeTags = exclude.Value,
                        ContinueClipId = clip.Id,
                        ContinueAt = from.Value,
                        Task = "extend",
                        PersonaId = SelectedPersonaId(persona)
                    };
                    request.Metadata.ControlSliders = ControlSliders.FromPercent(weirdness.Value, styleInfluence.Value, null);
                    request.Metadata.IsMaxMode = maxMode.Value;
                    return StartJobAsync(clip, T("Extending {0}."), c => _api.GenerateAsync(request));
                });
        }

        private static Persona NoPersona => new("", T("No persona"));

        // ── Writing help inside the Extend and Cover forms ─────────────────

        /// <summary>Suno's lyric co-writing on the form's Lyrics box, as the How to change box asks.</summary>
        private async Task RewriteLyricsAsync(TextField lyrics, TextField change, TextField title, TextField style)
        {
            if (string.IsNullOrWhiteSpace(lyrics.Current))
            {
                Announcer.Announce(T("Put lyrics in the Lyrics box first."));
                return;
            }
            if (string.IsNullOrWhiteSpace(change.Current))
            {
                Announcer.Announce(T("Say how to change them in the How to change box first."));
                return;
            }
            Announcer.Announce(T("Rewriting the lyrics."));
            await RunAsync(async () =>
            {
                lyrics.Current = await _api.CowriteLyricsAsync(lyrics.Current, change.Current.Trim(), title.Current, style.Current);
                Announcer.Announce(T("New lyrics are in the Lyrics box. Back to the original words undoes it."));
            });
        }

        /// <summary>Style boost on the form's Style box, using its lyrics unless it's instrumental.</summary>
        private async Task BoostStyleAsync(TextField style, TextField lyrics, CheckField? instrumental)
        {
            if (string.IsNullOrWhiteSpace(style.Current))
            {
                Announcer.Announce(T("Type a style first, even a word or two."));
                return;
            }
            var noWords = instrumental?.Current == true;
            Announcer.Announce(T("Boosting the style."));
            await RunAsync(async () =>
            {
                style.Current = await _api.BoostStyleAsync(style.Current.Trim(), noWords ? null : lyrics.Current, noWords, null);
                Announcer.Announce(T("Style boosted: {0}", style.Current));
            });
        }

        /// <summary>Suno's style ideas; Enter puts one in the form's Style box.</summary>
        private async Task StyleIdeasAsync(TextField style)
        {
            IReadOnlyList<string> ideas = Array.Empty<string>();
            if (!await RunAsync(async () => ideas = await _api.GetStyleSuggestionsAsync())) return;
            if (ideas.Count == 0)
            {
                Announcer.Announce(T("Suno has no style ideas right now."));
                return;
            }
            _dialogs.ShowLines(T("Style ideas. Enter puts one in the Style box."), ideas, index =>
            {
                style.Current = ideas[index];
                Announcer.Announce(T("Style: {0}", style.Current));
            });
        }

        /// <summary>A Persona field filled with the account's personas; just "No persona" if they don't load.</summary>
        private async Task<ChoiceField> PersonaChoiceAsync()
        {
            var items = new List<object> { NoPersona };
            try { items.AddRange(await _api.GetPersonasAsync()); }
            catch (SunoApiException ex) { Log.Warn($"Personas didn't load for the form: {ex.Message}"); }
            return new ChoiceField(T("_Persona"), items, NoPersona);
        }

        private static string? SelectedPersonaId(ChoiceField field) =>
            field.Selected is Persona { Id.Length: > 0 } p ? p.Id : null;

        /// <summary>The song's words: its stored lyrics, or the timed lyrics for songs Suno wrote the words for.</summary>
        private async Task<string> LyricsTextAsync(SunoClip clip)
        {
            var lyrics = clip.Metadata?.Prompt?.Trim();
            if (!string.IsNullOrEmpty(lyrics)) return lyrics;
            try
            {
                var aligned = await _api.GetAlignedLyricsAsync(clip.Id);
                return aligned == null ? "" : JoinWords(aligned.Words);
            }
            catch (SunoApiException ex)
            {
                Log.Warn($"Couldn't fetch lyrics for a cover: {ex.Message}");
                return "";
            }
        }

        [RelayCommand]
        private async Task CoverAsync(SunoClip? clip)
        {
            clip ??= SelectedClip;
            if (clip == null || !Finished(clip) || !ModelsLoaded()) return;
            var original = await LyricsTextAsync(clip);
            var wasInstrumental = clip.Metadata?.MakeInstrumental == true || original.Length == 0;

            var title = new TextField(T("_Title"), string.IsNullOrWhiteSpace(clip.Title) ? "" : T("{0} (Cover)", clip.Title.Trim()), maxLength: 100);
            var lyrics = new TextField(T("_Lyrics (change them, or keep the original words)"), original, multiline: true);
            var change = new TextField(T("_How to change the lyrics (for example: make the chorus rhyme)"), "");
            var instrumental = new CheckField(T("_Instrumental (no singing)"), wasInstrumental);
            var style = new TextField(T("New _style"), clip.Metadata?.Tags ?? "");
            var rewrite = new ButtonField(T("_Rewrite the lyrics now"), () => RewriteLyricsAsync(lyrics, change, title, style));
            var restore = new ButtonField(T("Back to the ori_ginal words"), () =>
            {
                lyrics.Current = original;
                Announcer.Announce(T("The original words are back in the Lyrics box."));
                return Task.CompletedTask;
            });
            var boost = new ButtonField(T("Style _boost"), () => BoostStyleAsync(style, lyrics, instrumental));
            var ideas = new ButtonField(T("Style i_deas..."), () => StyleIdeasAsync(style));
            var exclude = new TextField(T("_Exclude styles"), "");
            var influence = new NumberField(T("_Audio influence: how closely to follow the original, 0 to 100 (empty lets Suno choose)"), null, 0, 100);
            var weirdness = new NumberField(T("_Weirdness, 0 to 100 (empty lets Suno choose)"), null, 0, 100);
            var styleInfluence = new NumberField(T("Style in_fluence, 0 to 100 (empty lets Suno choose)"), null, 0, 100);
            var model = ModelChoice();
            var persona = await PersonaChoiceAsync();
            var maxMode = new CheckField(T("Ma_x mode"), false);
            var intro = T("Makes a new version of {0} in another style, keeping its melody. The lyrics start as the original words.", clip.DisplayTitle);
            // Stays open: make a cover, listen to it, fine-tune and make another.
            _dialogs.ShowToolForm(T("Cover {0}", clip.DisplayTitle), intro, T("Make cover"),
                new FormField[] { title, lyrics, change, rewrite, restore, instrumental, style, boost, ideas, exclude, influence, weirdness, styleInfluence, model, persona, maxMode }, () =>
                {
                    if (!instrumental.Value && lyrics.Value.Length == 0)
                    {
                        Announcer.Announce(T("Add lyrics, or tick Instrumental."));
                        return Task.CompletedTask;
                    }

                    var chosen = (SunoModel?)model.Selected ?? _account.DefaultModel!;
                    var request = new GenerateRequest(chosen.ExternalKey, GenerateRequest.Modes.Cover)
                    {
                        Title = title.Value,
                        Prompt = instrumental.Value ? "" : lyrics.Value,
                        MakeInstrumental = instrumental.Value,
                        Tags = style.Value,
                        NegativeTags = exclude.Value,
                        CoverClipId = clip.Id,
                        Task = "cover",
                        PersonaId = SelectedPersonaId(persona)
                    };
                    request.Metadata.ControlSliders = ControlSliders.FromPercent(weirdness.Value, styleInfluence.Value, influence.Value);
                    request.Metadata.IsMaxMode = maxMode.Value;
                    return StartJobAsync(clip, T("Covering {0}."), c => _api.GenerateAsync(request));
                });
        }

        // Suno's own values and labels for remaster strength (variation_category) and kind (style_profile).
        private static (string Value, string Label)[] RemasterStrengths => new[] { ("subtle", T("Subtle")), ("normal", T("Normal")), ("high", T("High")) };
        private static (string Value, string Label)[] RemasterProfiles => new[] { ("natural", T("Natural")), ("boost", T("Boost")), ("clarity", T("Clarity")) };

        [RelayCommand(CanExecute = nameof(CanRemaster))]
        private void Remaster(SunoClip? clip)
        {
            clip ??= SelectedClip;
            if (clip == null || !Finished(clip)) return;
            var models = _account.RemasterModels;
            if (models.Count == 0)
            {
                Announcer.Announce(_account.Billing == null
                    ? T("Plan details haven't loaded yet. Press F5 in the Create window (Ctrl+N), then try again.")
                    : T("Suno offers no remaster models to this account."));
                return;
            }
            if (clip.Duration is > 960)
            {
                Announcer.Announce(T("Suno only remasters songs up to 16 minutes long."));
                return;
            }
            var model = new ChoiceField(T("Remaster _model"), models, models.FirstOrDefault(m => m.IsDefault) ?? models[0]);
            var strength = new ChoiceField(T("Remaster s_trength"), RemasterStrengths.Select(x => (object)x.Label).ToList(), RemasterStrengths[1].Label);
            var profile = new ChoiceField(T("Remaster _kind"), RemasterProfiles.Select(x => (object)x.Label).ToList(), RemasterProfiles[1].Label);
            var style = new TextField(T("_Style (optional)"), "");
            var freedom = new NumberField(T("_Freedom, 0 to 100 (empty keeps Suno's 0)"), null, 0, 100);
            var tone = new NumberField(T("T_one, 0 to 100 (empty keeps Suno's 50)"), null, 0, 100);
            var clarity = new NumberField(T("_Clarity, 0 to 100 (empty keeps Suno's 50)"), null, 0, 100);
            var fine = new NumberField(T("Stre_ngth, 0 to 100 (empty keeps Suno's 50)"), null, 0, 100);
            var stereo = new NumberField(T("Stereo _width, 0 to 100 (empty keeps Suno's 50)"), null, 0, 100);
            var intro = T("Makes a cleaner, newer-sounding version of the same song. Strength is how much it may change. Kind: Natural stays close to the original, Boost lifts it, Clarity sharpens it. The five numbers are Suno's fine controls.");
            _dialogs.ShowToolForm(T("Remaster {0}", clip.DisplayTitle), intro, T("Remaster"),
                new FormField[] { model, strength, profile, style, freedom, tone, clarity, fine, stereo }, () =>
                {
                    var request = new RemasterRequest(
                        clip.Id,
                        ((SunoModel)model.Selected!).ExternalKey,
                        style.Value,
                        RemasterStrengths.First(x => x.Label == (string?)strength.Selected).Value,
                        RemasterProfiles.First(x => x.Label == (string?)profile.Selected).Value,
                        freedom.Value / 100, tone.Value / 100, clarity.Value / 100, fine.Value / 100, stereo.Value / 100);
                    return StartJobAsync(clip, T("Remastering {0}."), c => _api.RemasterAsync(request));
                });
        }

        // ── Editing into a new song: speed, reverse, crop, fade ─────────────

        private static string Titled(SunoClip clip, string suffix) =>
            string.IsNullOrWhiteSpace(clip.Title) ? suffix : $"{clip.Title.Trim()} ({suffix})";

        [RelayCommand]
        private void ChangeSpeed(SunoClip? clip)
        {
            clip ??= SelectedClip;
            if (clip == null || !Finished(clip)) return;
            var percent = new NumberField(T("_Speed in percent: 100 is as it is, 50 is half speed, 200 is double"), 100, 25, 400, allowEmpty: false);
            var keepPitch = new CheckField(T("_Keep the pitch (otherwise it gets higher when faster, like a tape)"), true);
            var title = new TextField(T("_Title"), "", maxLength: 100);
            var intro = T("Makes a new song from {0} at another speed. Leave the title empty for one that says the speed.", clip.DisplayTitle);
            _dialogs.ShowToolForm(T("Change the speed of {0}", clip.DisplayTitle), intro, T("Change speed"), new FormField[] { percent, keepPitch, title }, () =>
            {
                var multiplier = Math.Round(percent.Value!.Value / 100, 4);
                if (multiplier == 1)
                {
                    Announcer.Announce(T("Choose a speed other than 100."));
                    return Task.CompletedTask;
                }
                var name = title.Value.Length > 0 ? title.Value : Titled(clip, string.Format(Culture, "{0:0.##}x", multiplier));
                return StartJobAsync(clip, T("Changing the speed of {0}."), async c => new[] { await _api.ChangeSpeedAsync(c.Id, multiplier, keepPitch.Value, name) });
            });
        }

        [RelayCommand]
        private void Reverse(SunoClip? clip)
        {
            clip ??= SelectedClip;
            if (clip == null || !Finished(clip)) return;
            var title = new TextField(T("_Title"), Titled(clip, T("Reversed")), maxLength: 100);
            _dialogs.ShowToolForm(T("Reverse {0}", clip.DisplayTitle), T("Makes a new song from {0} played backwards.", clip.DisplayTitle), T("Reverse"),
                new FormField[] { title }, () =>
                {
                    var name = title.Value.Length > 0 ? title.Value : Titled(clip, T("Reversed"));
                    return StartJobAsync(clip, T("Reversing {0}."), async c => new[] { await _api.ReverseAsync(c.Id, name) });
                });
        }

        private static (bool Remove, string Label)[] CropKinds => new[] { (false, T("Keep only this part")), (true, T("Take this part out")) };

        [RelayCommand]
        private void Crop(SunoClip? clip)
        {
            clip ??= SelectedClip;
            if (clip == null || !Finished(clip)) return;
            var length = clip.Duration ?? 0;
            var max = length > 0 ? length : 3600;
            var from = new NumberField(T("_From (seconds, or minutes:seconds)"), 0, 0, max, allowEmpty: false, isTime: true);
            var to = new NumberField(T("_To (seconds, or minutes:seconds)"), length > 0 ? Math.Floor(length) : null, 0, max, allowEmpty: false, isTime: true);
            var kind = new ChoiceField(T("_What to do with that part"), CropKinds.Select(k => (object)k.Label).ToList(), CropKinds[0].Label);
            var title = new TextField(T("T_itle (empty says what was done)"), "", maxLength: 100);
            var intro = T("Makes a new song from {0} ({1}): keep only the part between From and To, or take that part out.",
                clip.DisplayTitle, SunoClip.FormatTime(TimeSpan.FromSeconds(length)));
            _dialogs.ShowToolForm(T("Crop {0}", clip.DisplayTitle), intro, T("Crop"), new FormField[] { from, to, kind, title }, () =>
            {
                if (to.Value <= from.Value)
                {
                    Announcer.Announce(T("To must be later than From."));
                    return Task.CompletedTask;
                }
                var remove = CropKinds.First(k => k.Label == (string?)kind.Selected).Remove;
                var name = title.Value.Length > 0 ? title.Value : Titled(clip, remove ? T("Section removed") : T("Cropped"));
                return StartJobAsync(clip, T("Cropping {0}."), async c => new[] { await _api.CropAsync(c.Id, from.Value!.Value, to.Value!.Value, remove, name) });
            });
        }

        [RelayCommand]
        private void Fade(SunoClip? clip)
        {
            clip ??= SelectedClip;
            if (clip == null || !Finished(clip)) return;
            var fadeIn = new NumberField(T("Fade _in over this many seconds (empty for none)"), null, 0, 60);
            var fadeOut = new NumberField(T("Fade _out over this many seconds (empty for none)"), 5, 0, 60);
            var title = new TextField(T("_Title (empty says what was done)"), "", maxLength: 100);
            var intro = T("Makes a new song from {0} that fades in at the start, out at the end, or both.", clip.DisplayTitle);
            _dialogs.ShowToolForm(T("Fade {0}", clip.DisplayTitle), intro, T("Fade"), new FormField[] { fadeIn, fadeOut, title }, () =>
            {
                double? inS = fadeIn.Value is > 0 ? fadeIn.Value : null, outS = fadeOut.Value is > 0 ? fadeOut.Value : null;
                if (inS == null && outS == null)
                {
                    Announcer.Announce(T("Give a fade in, a fade out, or both."));
                    return Task.CompletedTask;
                }
                var suffix = inS != null && outS != null ? T("Fade") : inS != null ? T("Fade in") : T("Fade out");
                var name = title.Value.Length > 0 ? title.Value : Titled(clip, suffix);
                return StartJobAsync(clip, T("Fading {0}."), async c => new[] { await _api.FadeAsync(c.Id, inS, outS, name) });
            });
        }

        [RelayCommand]
        private Task StemsAsync(SunoClip? clip) => StartJobAsync(clip ?? SelectedClip, T("Splitting into stems: {0}."),
            c => _api.GenerateStemsAsync(c.Id));

        [RelayCommand]
        private Task WholeSongAsync(SunoClip? clip) => StartJobAsync(clip ?? SelectedClip, T("Joining the parts of {0}."),
            async c => await _api.ConcatAsync(c.Id) is { } joined ? new[] { joined } : Array.Empty<SunoClip>());

        /// <param name="starting">An already translated "Doing {0}." line; {0} is the song's title.</param>
        private async Task StartJobAsync(SunoClip? clip, string starting, Func<SunoClip, Task<IReadOnlyList<SunoClip>>> job)
        {
            if (clip == null || !Finished(clip)) return;
            Announcer.Announce(string.Format(Culture, starting, clip.DisplayTitle));
            await RunAsync(async () =>
            {
                var created = await job(clip);
                Upsert(created);
                _watcher.Watch(created);
                Announcer.Announce(created.Count == 0
                    ? T("Suno accepted it. New songs will show up when you refresh.")
                    : created.Count == 1 ? T("Started. You'll hear when it's ready.") : T("Started. You'll hear when they're ready."), interrupt: false);
                _ = RefreshAccountQuietlyAsync();
            });
        }
    }
}
