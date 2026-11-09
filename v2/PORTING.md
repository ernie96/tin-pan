# Porting Tin Pan 1 to Tin Pan 2

Built from Tin Pan 1's code itself (every `[RelayCommand]`, every Suno call in `ISunoApiService`, every setting in `AppConfig`, every key in `MainWindow.xaml` and the views, every window), plus its `CHANGELOG.md` (1.1.0 to 1.12.0), `README.md` and `AGENTS.md`. Nothing here is from memory.

`[x]` is done in 2.0 and matches 1.x, `[~]` is partly done (what's missing is said), `[ ]` is not started.

## Rules carried over from Tin Pan 1

- **Help, menus and speech.** Every feature gets F1 help and a menu bar entry; actions people repeat get keys, and access keys, where used, are unique per menu, page and form in every language. Speech stays quiet, one line per action.
- **Sighted users too.** Pages are laid out like 1.x's: a heading, buttons with Alt letters, the list, the playback buttons and the volume. Nothing is left as a bare list.
- **Texts.** Every user-facing text goes through `t()`, `tf()` or `plural()`, with its Romanian, Ukrainian and Spanish in `TinPan/Localization/translations.json` in the same change.
  - Prefer Tin Pan 1's own texts: they are translated and their access keys are checked. Menu texts keep 1.x's "_"; commands.rs turns it into "&".
  - New texts go in a batch file (`build/translations/batch_*.py`), merged with `python -I build/translations/check.py <batch>`.
  - check.py also reads Tin Pan 2's code, command table, forms and filters, keeps 2.0's texts, and checks 2.0's menus for access-key clashes.
  - Logs, Suno API values and license texts stay in English.
- **Behaviour.** Keep working behaviour (downloads, sign-in, playback) unless asked to change it, and add no prompts or gates nobody asked for.
- **Suno's API.** Go by Tin Pan 1's code first (it works), then the reference projects listed in `TinPan/README.md`.
- **Lists.** Only the list that started a song moves on when it ends. F7, F9 and the media keys follow the list that's playing.
- **Releases.** Commit, tag, push or release only when asked.
- **Credits.** Never name other projects in READMEs or the changelog; credits go in the notices file.

## Things learned the hard way (keep them)

- **Song files.**
  - `audio_url` is a decoy (`/api/forbidden`). The real file is the progressive M4A from `media_urls`.
  - An entry without `encoding` is unencrypted. Otherwise the content-rights key (`POST /api/mango/rights`, three body shapes, both hosts) unlocks it with AES-CTR.
  - Fetch the song's own `media_urls` address first; a guessed CDN address is refused.
- **Opus.** Suno's files are Opus in M4A. Symphonia reads the M4A, libopus decodes the Opus, and the channel count comes from the OpusHead in the extra data.
- **Library request.** The body is Suno's web one: every filter given explicitly, and the "presence" filters wrapped as `{"presence": value}`. Suno answers 500 otherwise.
- **Hosts.** `suno.com/a` comes first, then `studio-api-prod.suno.com`. A request that makes or changes something only moves to the other host when this one surely did nothing.
- **Sign-in.** Clerk takes the raw `__client` cookie as Authorization, with no Origin or Referer. Read suno.com, auth.suno.com and clerk.suno.com, in that order.
- **Lenient reading.** A `null` or text count reads as 0; one odd song never stops a page.
- **quiet-list leaves Enter to the app.** Handle it in the list's key hook.

## The main window

- [x] Tabs, changed from 1.x on purpose: they are Suno's sections, and each has a L_ists box on the left (your songs, each playlist; Trending) with the chosen list's songs on the right (pages.rs, views.rs). Ctrl+1 Library, Ctrl+2 Discover, Ctrl+4 Account, Ctrl+5 Settings.
  - A song carries on through the list it started from, whichever tab is shown.
  - A list arrowed onto loads after a moment; Enter loads it now and says its count.
  - Escape in the songs goes to the lists; Escape in the lists goes to the tab headers.
- [x] Menu bar from one command table (commands.rs). It's always shown: unlike 1.x's WPF menu, a native one is never in the Tab order. As in 1.x its menus have no Alt letters (Alt or F10 opens it), so the pages' Alt letters work as in 1.x.
- [x] Menu groups: Library, Song, Discover, Playback, View (Library, Discover, Account, Settings, Next window), Account and Help.
- [x] Escape steps back one level: from songs to the lists, from the lists to the tab headers.
- [x] F6 and Shift+F6 between all of Tin Pan's open windows: the main one, Create and the forms that stay open; F1 in a form says so; closing a form goes back to the main window.
- [x] Title: "Tin Pan - your name" once signed in ("Tin Pan - signed in" when Suno gives no name).
- [x] Suno's own pages are out of reach (1.8.1): the hidden player is never in Alt+Tab or the taskbar and can't take the focus (out_of_reach.rs), and gives it back if Windows hands it over anyway; the captcha window is the same until a box needs pressing. Windows only for the Alt+Tab part; on Linux the player is a tool window.
- [x] One instance at a time (instance.rs): a second start brings the open Tin Pan forward (wxWidgets' single-instance checker, and its IPC to ask), or says it's running if asking fails. 1.x doesn't have this; Suno links aren't handled yet.
- [x] Alt+F4 always closes Tin Pan, wherever focus is (1.9.1). In the main window it's the Exit command's accelerator, so it works from any control there. If Windows puts the focus on the hidden Suno player window, Alt+F4 there closes Tin Pan, not just the player. In Create, in forms that stay open and in the sign-in window, Alt+F4 closes that window, as in 1.x. Checked in the code, not by hand.

## Library page (1.x LibraryPanelView)

- [x] Page layout, as 1.x's, with Tab going search box, lists, song list, volume.
  - "_Songs" heading;
  - buttons, Alt letters only and not Tab stops: _New song..., _Upload..., _Refresh, _Filters..., Download a_ll...;
  - "Searc_h" box with Search and Clear search buttons;
  - the L_ists box and the song list side by side;
  - Previous, Play or pause, Stop, Next, and the _Volume slider (kept in step with F11 and F12).
- [x] Song list, 100 a page, more at the end; not re-read when it changes.
- [x] The playing-song cue: a click when you arrow onto the playing song (on the song's own output), or "playing" after its name, both, or off (Settings; live.rs).
- [x] Search: Enter searches, Escape clears, Ctrl+F from anywhere.
- [x] Filters (Alt+F): workspace, disliked, 13 filters, sort; summary before the count.
- [x] Keys in the list:
  - Enter plays, Space pauses, Delete trashes or restores (in a playlist, removes from it);
  - Left and Right seek 5 seconds;
  - Ctrl+I details, Alt+Enter information, Ctrl+L lyrics, Ctrl+C link, Ctrl+D download.
- [x] Ctrl+E extend.
- [x] F9 and a song's end load further pages when the next playable song isn't loaded yet (up to 3 pages, as 1.x's FindNextAsync).
- [x] When the library runs out, "End of library." (said quietly) after a song ends, "End of list." after F9.
- [x] New songs appear at the top, update as Suno makes them, and "Ready: ..." is said once they're done (SongWatcher), also for unfinished songs the library loads.
- [x] The library loads again quietly after Suno ends a sign-in and you sign in again (every sign-in loads it, with the playlists, plan, personas and notifications).

## Playing (1.x HybridPlayer, DirectPlayer, SunoWebPlayer, OutputSwitch, LiveLyrics)

- [x] Plays in the app: unencrypted or decrypted file, Opus, AAC or MP3.
- [x] F7, F8, Shift+F8, F9, F3, F11, F12; Shift+F7 and Shift+F9 10 seconds; Ctrl+F7 and Ctrl+F9 10 percent; Ctrl+F8 back to the start.
- [x] Next song fetched ahead; the last four kept.
- [x] Volume kept between songs and runs.
- [x] Media keys: Play/Pause, Stop, Previous, Next, while Tin Pan is in front (as 1.x).
- [x] Suno's hidden player as the fallback when a song won't play in the app (the file can't be fetched or decoded), and for every song with Settings, "Play songs in Tin Pan itself" off (web_player.rs): the song's page on suno.com off screen, its Play button pressed, its audio element driven; pause, seeking, volume, resume, live lyrics and the song's end work as for songs played here. It plays on the chosen output (the page is let see the outputs' names, never record), says once when it can't, and forgets its Suno sign-in at Sign out.
- [x] Audio output and second output (Playback menu and Settings), Ctrl+Shift+F12 even mid-song, with Tin Pan 1's saved names; says when one can't be used and plays on the default (1.8.0).
- [x] Resume where you left off, Ctrl+Shift+F8, also after closing Tin Pan (saved every 5 seconds, from the start when it was near the end).
- [x] The playback keys also work in the Create window and the forms that stay open.
- [x] Read lyrics as they're sung, Shift+F3 (Settings too): each line said, section tags left out, only the newest after a jump. Uses the v3 timing when Suno has none stored.

## Song menu (1.x LibraryPanelViewModel.SongActions, .Creating, .Downloads, .More, .Discover)

- [x] Play, Details (Ctrl+I), Song information (Alt+Enter), Lyrics (Ctrl+L).
- [x] Lyrics, and the LRC next to downloads: the stored or timed words, and Suno's v3 timing when it has none stored, as 1.x.
- [x] Like (Ctrl+Shift+U), Dislike (Ctrl+Shift+N), Not interested.
- [x] Download submenu: Download (Ctrl+D), MP3 (Ctrl+Shift+D), WAV (Ctrl+Shift+W), video (Ctrl+Shift+V), Clean up download (Ctrl+Shift+H); Suno's own download as the fallback; extras; clean-up.
- [x] Download video: the song's video address, or "Suno gave no file address for this song." when it has none, as in 1.x.
- [x] Make from it: Extend... (Ctrl+E), Cover... (Ctrl+Shift+C) and Remaster... (Ctrl+Shift+M), each in a form that stays open (tool_form.rs, making.rs), with only the models that can do the task, persona checks, Style boost and Style ideas into the form's box, and for Cover the song's own words, Rewrite the lyrics now and Back to the original words.
  - Weirdness, style influence and Cover's audio influence are sliders with "Let Suno choose"; Remaster's five fine controls are sliders that start at Suno's own values, which aren't sent.
- [x] Use as inspiration..., Get whole song (Ctrl+J), Stems (Ctrl+Shift+S), Change speed..., Reverse..., Crop..., Fade in or out..., Make a video, Make a persona... (editing.rs). Speed and the fades are sliders.
- [x] The Cover and Remaster menu texts with the free ones left, and Remaster saying it needs Pro or Premier.
- [x] Related songs: Similar songs (Ctrl+M), Creator's songs (Ctrl+U), Covers and remixes (Ctrl+Shift+O), Original song (Ctrl+O), Its stems open as lists in the Discover tab; Credits and remix count (Enter opens a credited song); Creator's playlists (open, like or save; song_more.rs).
- [x] Comments: Show comments (Ctrl+K), Write a comment (Ctrl+Shift+K).
- [x] Sharing: Copy link, Copy Suno share link, Make public or private, Allow or stop remixes, Show or hide the list of remixes.
- [x] Edit title and lyrics (Ctrl+T).
- [x] Cover picture... (from a file, from a description, or Suno's own).
- [x] Add to playlist... (Ctrl+P), with a new playlist from the same form.
- [x] Move to trash or restore (Delete), Delete forever (Shift+Delete).
- [x] The song windows (similar songs, a creator's songs, Explore shelves) are lists in the Discover tab instead, with the main keys, and songs carry on through them (1.10.1's queue). Delete in the lists closes one.

## Library menu

- [x] New song... (Ctrl+N, the Create window).
- [x] Refresh (F5), Search (Ctrl+F), Filters..., Download all..., Exit.
- [x] Upload a song... (Alt+U).
- [x] Personas... (yours, loved, followed: songs made with it, its source song, love, rename and describe, public or private, trash), Custom models... (train from a playlist of 6 to 100 finished songs, with a base model; rename; archive; those still training), Lyrics mashup... (a form that stays open).

## Create window (1.x CreateWindow, CreateView, SunoPanelViewModel)

- [x] Description or custom lyrics, style, styles to exclude, voice, weirdness, style influence, persona (yours, or a typed ID, with its task and source song), model (your plan's, last one remembered), Max mode, title, instrumental. Stays open; Escape or Close hides it and keeps the fields; F5 reloads models and personas.
- [x] Write lyrics from a topic, Use these lyrics, Change my lyrics (cowrite), Style boost, Style ideas, Lyrics drafts (Enter: use, save into, rename, delete; save the Lyrics box as a new draft).
- [x] Weirdness and Style influence are sliders (0 to 100), each with "Let Suno choose" after it, ticked until the slider moves. Cover's audio influence will be one too.
- [x] Captcha (Turnstile or hCaptcha) in a web view off screen (captcha.rs), with the "Suno check" window coming on screen when a box must be ticked. Not tried against Suno yet.
- [x] Lengths checked against the model's limits before sending.
- [x] F6 between the main window, the Create window and the forms that stay open (View menu, Next window).

## Playlists page (1.x PlaylistsView, PlaylistsViewModel)

Now views in the Library tab (each playlist is one of its lists), not a page of its own.
- [x] Lists and songs: Enter opens a playlist, and on a song plays and carries on through the playlist; Space pauses; Delete removes a song or trashes a playlist (asks first); Ctrl+Up and Ctrl+Down move a song.
- [x] New playlist..., Rename playlist... (name and description), Delete playlist... in the Library menu; Remove from playlist, Move up and Move down first in a playlist song's Applications key menu.
- [x] Cover picture; Make an album (draft or published); the lists box's own Applications key menu (Open, New playlist, Rename, Cover picture, Make an album, Delete, About this creator, Refresh), as 1.x's Playlists tab menu.

## Trending page and Discover menu

- [x] Trending, as the Discover tab's first list: loads the first time the tab is shown, F5 refreshes, play, Space, carries on down the list, says who made each song.
- [x] Discover tab's other lists: From people you follow, Explore (all its songs, then each shelf as its own list), Contests (each contest's song, said as the contest).
- [x] Discover menu: Explore, From people I follow, Find a creator (handle or link), Contests.
- [~] A creator's own information: About this creator in a creator list's Applications key menu (1.x had the lines but didn't show them). Following a creator isn't doable yet: chadvis (main 27fb383 and dev/bleeding-edge 0b14d9b, 2026-11-01) still lists follows as uncaptured, and its only route is a lead it doesn't trust.

## Account page and menu

- [x] F2 credits, Shift+F2 plan, downloads, free allowances, style personalization; loaded at sign-in. Account menu, Mark notifications as read.
- [x] Account tab (Ctrl+4): the plan in full and what it includes; notifications with "New" (loaded at sign-in); Refresh account, Refresh notifications, Mark all as read.
- [x] Sign in (Windows) and Sign out.
- [~] Sign-in on Linux: the sign-in page is WebKitGTK's there, and its cookie manager is read (and cleared at Sign out) the way WebView2's is on Windows (signin.rs). Tin Pan 2 builds on Linux (Ubuntu 24.04 in WSL: GTK 3.24, WebKitGTK 2.52), passes clippy and its tests there, but hasn't been run there yet. Clean-up says it's Windows only for now (the sunscreen built in is its Windows build). macOS: not started; it can only be built on GitHub's Mac runners. Suno's official OAuth can't replace the cookie sign-in yet (chadvis T0356b).
- [x] The welcome at first start offers to sign in, and a Suno-ended sign-in asks to sign in again (1.3.0), in 1.x's words.

## Settings tab (1.x SettingsPanelView)

- [x] A tab (Ctrl+5), saved as each setting changes (settings_page.rs). The old Settings window is in the graveyard.
- [x] Suno account: status, Sign in, Sign out.
- [x] Language (takes effect at the next start).
- [x] Speech: how much Tin Pan says (Everything; what I ask for and errors; only errors). Errors are always said.
- [x] Speech: the playing-song cue; Live lyrics.
- [x] Audio output: output, quick switch output. (Play songs in Tin Pan itself has no place here: 2.0 always does.)
- [x] Updates: check at startup.
- [x] Downloads: folder (Browse), format, extra files.
- [x] Clean-up: all of sunscreen's steps, with sliders for its values (the widening amount in tenths), the reference song, and Use the recommended clean-up steps.

## Help menu

- [x] F1, help for here: per tab, and in the Create window, as 1.x's HelpText; says when you're not signed in. As in 1.x it starts with the other open windows (F6 moves between them) and ends with "Ctrl+F1 for all commands."; in a form that stays open, F1 says its name, what OK does there, and the F8 and Escape keys.
- [x] Ctrl+F1, all commands: Enter runs one; its first item is the full key guide (help.rs), with 1.x's sections where 2.0 works the same and new ones where it doesn't.
- [x] Repeat last message (Ctrl+R), Message history (Ctrl+Shift+R).
- [x] Speech submenu: Speak everything, only what I ask for, only errors (kept in step with Settings).
- [x] Report a problem (Ctrl+Shift+B), Suggest a feature (Ctrl+Shift+F), through 1.x's report relay, with the log (user name taken out; report.rs).
- [x] Check for updates, and a quiet check 10 seconds after startup; updates itself with signed downloads (1.2.0), Tin Pan 1's way (update.rs, updates.rs), so 1.x installs 2.x: TinPan.exe with the same Ed25519 .sig, the same helper, Tin Pan 1's texts. From ship-shape: the relaunched app comes to the front, and a watchdog ends Tin Pan if closing stalls. Checked: a file signed by build/Signer verifies in 2.0 (ignored test, run with --ignored).
  - .github/workflows/release-v2.yml publishes v2.* tags as TinPan.exe; release.yml now takes only v1.* tags. Versions with a "-" are pre-releases, which neither version offers.
- [~] Updates on Linux, when Tin Pan runs as an AppImage (TinPan.AppImage with its .sig, waiting in the user's cache folder, swapped in by ship-shape's kind of shell helper, then started again). No release builds an AppImage yet. macOS opens the release page; waiting for the main window before the startup check isn't done. (Two Tin Pans can't run at once now, so there's no other one to wait for.)
- [x] Licenses and notices (NOTICE.md, v2/THIRD_PARTY_NOTICES.md, sunscreen's license, the GPL); About Tin Pan (version, attribution and dedication, account, plan, where settings and the log are). About also says what speech goes through (the screen reader prism found, or Windows speech).

## Speech (1.x ScreenReaderService)

- [x] Live region in front; the screen reader directly from the background.
- [x] Speech levels: everything, what you ask for, or errors only, in Settings and the Help menu.

## Uploading (1.x UploadService)

- [x] Upload a song: MP3, M4A, WAV, FLAC, OGG and AAC as they are; WMA and other files Windows plays converted to AAC M4A with Media Foundation first (to_m4a.rs, as 1.x), then checked against the plan's lengths (upload.rs, uploads.rs).
  - The form asks for the title and lyrics, and to confirm your rights and Suno's terms.
  - It checks the plan's length limits, says when Suno is checking the upload, and puts the new song at the top.

## Checked against 1.x (2026-11-02)

Every 1.x command, key binding, setting and spoken text was compared with 2.0 (a script lists 1.x texts 2.0 never uses). Brought over from that: Suno's error wordings (captcha, too long for the model, Suno's own short reason after "didn't accept those settings", and the plan check matching whole words, so "problem" isn't "pro"); "From the start." on Restart; "End of playlist."; "Download folder set."; "Clean-up matches the tone of …" and its clearing line; and 1.x's "Not signed in. You can sign in any time from Settings…".

Bugs found and fixed in the same pass:
- Stop while a song was still on its way (fetching, or starting in Suno's player) didn't call it off, so it could start later with nothing able to stop it.
- When a song ended and the next library page failed to load, it tried again and said the error every moment.
- When a song ended after its list was reloaded or closed without it, Tin Pan kept checking for the next one for ever.
- Sign out left a song waiting in Suno's player and a waiting page load behind.

Then every entry in Tin Pan 1's CHANGELOG (1.1.0 to 1.12.0) was checked against 2.0's code. Two were missing and are in now:
- After Suno's captcha, focus goes back to the window you were in (Create or a form), not always the main window (1.8.0).
- The quiet update check after startup waits while the sign-in window is open, so its offer can't land on top of it (1.3.0).

Left as they are: the hidden Suno player isn't warmed up after sign-in (2.0 plays songs itself first, so it's rarely needed); coming back to Tin Pan with Alt+Tab returns to the window you left, which Windows does by itself for wxWidgets windows, not checked by hand.


Then (2026-11-02) the whole of Tin Pan 1 was read again, file by file, for behaviour rather than texts. Found and fixed:
- Enter didn't reach plain lists on Windows (wxWidgets' dialog handling takes Enter, Escape too, before a ListBox's key-down): Ctrl+F1's list and every pick list (dialogs.rs), and the Lists box (pages.rs) now use the key hook.
- Escape in the search box only clears a search when there is one (1.x), via the key hook too.
- Forms that stay open have their main button as the default, so Enter in a field starts it (1.x FormWindow IsDefault).
- Contests without a song are listed; Enter or a Song command says "{0} has no song to play." (discover.rs NO_SONG).
- Suno ending the sign-in mid-session asks to sign in again (1.x SignInExpired), through forget_account and sign_in_ended.
- An unexpected error (a panic) is caught in with_app, logged and said, as 1.x's unhandled-exception handler did.
- No WebView2 runtime: sign-in, captcha and Suno's player say 1.x's words instead of falling back to Internet Explorer.

Left different on purpose: forms don't wait while five starts are pending (1.x's MaxAtOnce); a sixth gets Suno's own refusal.
Not checked by hand: all of the above; none of it has been tried with NVDA yet.

## Suno calls (ISunoApiService) still to port

Notifications (v3 then v2), personas, generating, the captcha check, remaster, concat, stems, speed, reverse, crop, fade, writing and cowriting lyrics, style boost and ideas, lyrics drafts, aligned lyrics v3, credits, existing stems, show remixes, pictures, video, albums, custom models, mashup, uploads.
