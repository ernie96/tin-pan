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
            Announcer.Announce(T("Models haven't loaded yet. Press F5 on the Create tab, then try again."));
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
            var exclude = new TextField(T("_Exclude styles"), "");
            var model = ModelChoice();
            var weirdness = new NumberField(T("_Weirdness, 0 to 100 (empty lets Suno choose)"), null, 0, 100);
            var styleInfluence = new NumberField(T("Style in_fluence, 0 to 100 (empty lets Suno choose)"), null, 0, 100);
            var persona = await PersonaChoiceAsync();
            var maxMode = new CheckField(T("Ma_x mode"), false);
            var intro = T("Continues {0} from a point you choose. Leave lyrics empty to let Suno carry on. Use Get whole song afterwards to join the parts.", clip.DisplayTitle);
            if (!_dialogs.ShowForm(T("Extend {0}", clip.DisplayTitle), intro, T("Extend"),
                    new FormField[] { from, title, lyrics, style, exclude, model, weirdness, styleInfluence, persona, maxMode })) return;

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
            await StartJobAsync(clip, T("Extending {0}."), c => _api.GenerateAsync(request));
        }

        private static Persona NoPersona => new("", T("No persona"));

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

        [RelayCommand(CanExecute = nameof(CanUsePaidFeatures))]
        private async Task CoverAsync(SunoClip? clip)
        {
            clip ??= SelectedClip;
            if (clip == null || !Finished(clip) || !ModelsLoaded()) return;
            var original = await LyricsTextAsync(clip);
            var wasInstrumental = clip.Metadata?.MakeInstrumental == true || original.Length == 0;

            var title = new TextField(T("_Title"), string.IsNullOrWhiteSpace(clip.Title) ? "" : T("{0} (Cover)", clip.Title.Trim()), maxLength: 100);
            var lyrics = new TextField(T("_Lyrics (change them, or keep the original words)"), original, multiline: true);
            var instrumental = new CheckField(T("_Instrumental (no singing)"), wasInstrumental);
            var style = new TextField(T("New _style"), clip.Metadata?.Tags ?? "");
            var exclude = new TextField(T("_Exclude styles"), "");
            var influence = new NumberField(T("_Audio influence: how closely to follow the original, 0 to 100 (empty lets Suno choose)"), null, 0, 100);
            var weirdness = new NumberField(T("_Weirdness, 0 to 100 (empty lets Suno choose)"), null, 0, 100);
            var styleInfluence = new NumberField(T("Style in_fluence, 0 to 100 (empty lets Suno choose)"), null, 0, 100);
            var model = ModelChoice();
            var persona = await PersonaChoiceAsync();
            var maxMode = new CheckField(T("Ma_x mode"), false);
            var intro = T("Makes a new version of {0} in another style, keeping its melody. The lyrics start as the original words.", clip.DisplayTitle);
            if (!_dialogs.ShowForm(T("Cover {0}", clip.DisplayTitle), intro, T("Make cover"),
                    new FormField[] { title, lyrics, instrumental, style, exclude, influence, weirdness, styleInfluence, model, persona, maxMode })) return;

            if (!instrumental.Value && lyrics.Value.Length == 0)
            {
                Announcer.Announce(T("Add lyrics, or tick Instrumental."));
                return;
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
            await StartJobAsync(clip, T("Covering {0}."), c => _api.GenerateAsync(request));
        }

        // Suno's own values and labels for remaster strength (variation_category) and kind (style_profile).
        private static (string Value, string Label)[] RemasterStrengths => new[] { ("subtle", T("Subtle")), ("normal", T("Normal")), ("high", T("High")) };
        private static (string Value, string Label)[] RemasterProfiles => new[] { ("natural", T("Natural")), ("boost", T("Boost")), ("clarity", T("Clarity")) };

        [RelayCommand(CanExecute = nameof(CanUsePaidFeatures))]
        private async Task RemasterAsync(SunoClip? clip)
        {
            clip ??= SelectedClip;
            if (clip == null || !Finished(clip)) return;
            var models = _account.RemasterModels;
            if (models.Count == 0)
            {
                Announcer.Announce(_account.Billing == null
                    ? T("Plan details haven't loaded yet. Press F5 on the Create tab, then try again.")
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
            if (!_dialogs.ShowForm(T("Remaster {0}", clip.DisplayTitle), intro, T("Remaster"),
                    new FormField[] { model, strength, profile, style, freedom, tone, clarity, fine, stereo })) return;

            var request = new RemasterRequest(
                clip.Id,
                ((SunoModel)model.Selected!).ExternalKey,
                style.Value,
                RemasterStrengths.First(x => x.Label == (string?)strength.Selected).Value,
                RemasterProfiles.First(x => x.Label == (string?)profile.Selected).Value,
                freedom.Value / 100, tone.Value / 100, clarity.Value / 100, fine.Value / 100, stereo.Value / 100);
            await StartJobAsync(clip, T("Remastering {0}."), c => _api.RemasterAsync(request));
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
