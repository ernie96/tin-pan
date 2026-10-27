# Tin Pan

A Windows app for Suno, built for NVDA and keyboard use. Sign in with Suno's own page, play, search and download your songs, and make new ones: describe a song, use your own lyrics, extend, cover, remaster, join parts into a whole song, and split it into stems.

By Ernesto Melendez. Unofficial: it uses the same private API as the Suno website, which can change without notice.

## Keys

Alt or F10 opens the menu bar, which has every command with its keys. It stays hidden otherwise, so Tab and the screen reader never run into it. The Song menu acts on the song selected in the tab you're on (Library, a playlist, or Trending). It's grouped into submenus: Download, Make from it (extend, cover, remaster, whole song, stems), Related songs, Comments and Sharing (links, public or private, remixes). Its keys (Ctrl+I, Alt+Enter, Ctrl+L, Ctrl+C, Ctrl+D, Ctrl+E) work in the Library, a playlist's songs and Trending. On Trending, Space pauses or resumes and F9 carries on down the list. The Help menu's Speech submenu sets how much Tin Pan says. Ctrl+F1 lists the same commands; Enter runs one.

Tab only stops where it matters: on the Library, the search box, the song list and the volume; on Playlists, the two lists. The page buttons still work with their Alt letters. Switching pages with Ctrl+1 to Ctrl+5 puts you on the page's list or first field.

Anywhere:

- F1: short help for where you are. Ctrl+F1: all commands; Enter runs one.
- Ctrl+R: repeat the last message. Ctrl+Shift+R: message history.
- Escape: step back one level.
- F2: credits left. Shift+F2: your plan, when it renews, downloads left, and what it includes.
- F3: what's playing and how far in.
- Shift+F3: read the lyrics aloud, line by line, as they're sung (for songs with timed lyrics). Off to start with; also in Settings.
- F5: refresh the current tab.
- Ctrl+1 to Ctrl+5: Library, Playlists, Trending, Account, Settings. Ctrl+F: search your songs.
- In any song list: Ctrl+Shift+U like, Ctrl+Shift+N dislike, Ctrl+P add to a playlist, Ctrl+T edit the title and lyrics, Ctrl+Shift+C cover, Ctrl+Shift+M remaster, Ctrl+J get the whole song, Ctrl+Shift+S stems, Ctrl+Shift+D download MP3, Ctrl+Shift+W download WAV, Ctrl+Shift+V download the video, Ctrl+Shift+H clean up the download, Ctrl+M similar songs, Ctrl+U the creator's songs, Ctrl+O the original song, Ctrl+Shift+O covers and remixes of it, Ctrl+K comments, Ctrl+Shift+K write a comment, Ctrl+Shift+L copy the Suno share link, Ctrl+Shift+P public or private, Ctrl+Shift+X allow or stop remixes. The Song menu shows each key.
- Ctrl+N: create a song, in a window that stays open (also New song on the Library page).
- F6 and Shift+F6: move between Tin Pan's open windows (the main one, Create, and any Extend, Cover or Remaster form).
- Ctrl+Shift+F12: switch between your main and second audio output.
- F8 or the Play/Pause media key: play or pause. Shift+F8: stop.
- F7 and F9: previous and next song. Shift+F7 and Shift+F9: back and forward 10 seconds. Ctrl+F7 and Ctrl+F9: 10 percent.
- Ctrl+F8: back to the start. Ctrl+Shift+F8: resume the last song where you left off, even after closing Tin Pan.
- F11 and F12: volume down and up.

In the Library list:

- Enter plays, Space pauses or resumes, Left and Right go back and forward 5 seconds.
- Ctrl+I details, Alt+Enter song information one fact per line, Ctrl+L lyrics, Ctrl+C copy link, Ctrl+D download, Ctrl+E extend.
- Delete moves the song to your Suno trash (it asks first).
- The Applications key (or Shift+F10) has everything else: Download WAV, Download video, Cover, Remaster, Get whole song, Stems, Edit title and lyrics, Make public or private.
- When you reach the end of the list, the next page loads by itself.
- Filters (Alt+F): workspace, disliked songs, and every other filter Suno's own library has: trash, stems, Suno Studio songs, liked, public or private, uploaded, covers, extended, remasters, whole songs, personas, songs in a playlist and songs downloaded before. Each shows those songs, shows only them, or hides them; the defaults are Suno's (trash, stems and Studio songs hidden). Sort newest, oldest, most liked or least liked first. Suno applies them itself, so paging and search follow them, and they're kept between runs.
- Download all (Alt+L): every finished song in the list, through the free direct download only. Songs already in the folder are skipped, and the summary names any that need Suno's own download.

## Audio output

Playback menu, **Audio output** chooses where songs play: headphones, speakers, a USB box, or the Windows default. **Second audio output** is one more, for example external speakers to show someone a song; **Ctrl+Shift+F12** switches to it and back, even in the middle of a song. Both are in Settings too. Tin Pan always starts on the main output. If an output is unplugged, Tin Pan says so and the song plays on the Windows default.

Suno's own player plays the songs, so Tin Pan moves it to the output you choose. For that, Suno's player page is allowed to see the names of your audio devices, which Windows and Edge count as microphone permission; Tin Pan turns recording off on that page, so it can never listen. This is only switched on once you choose an output other than the Windows default.

## Playlists

Ctrl+2. The left list is your playlists: Enter opens one, Delete moves it to the trash (it asks first), and there are New, Rename and Delete buttons. The right list is its songs: Enter plays and carries on through the playlist, Space pauses, Delete removes the song from the playlist, Ctrl+Up and Ctrl+Down move it. In the Library, the Applications key menu has Add to playlist. On a playlist, the Applications key opens a menu for the playlist or the song (with every Song command); Trending has one too.

## Playing and downloads

Since September 2026 Suno encrypts song audio. Tin Pan asks Suno for the song's key, the same way the free downloads do, and plays the file itself with Windows' own audio. That starts a song far sooner than loading Suno's song page, and the next song in the list is fetched while the current one plays, so it starts at once. If a song won't play that way (Suno refuses the key, or Windows can't decode the file), Suno's own player plays it instead, signed in as you, in a browser page the app keeps out of sight; Settings, "Play songs in Tin Pan itself", turns the faster way off. Focus never moves to that page, so NVDA stays in the app. Playing is unlimited and uses none of your downloads. Songs played in Tin Pan itself may not add to Suno's play counts.

## More in the Song and Library menus

- Not interested: Suno suggests fewer songs like it (press again to undo, in the same run).
- Cover picture: a picture from your computer (JPG, PNG or WebP), one Suno draws from your description, or Suno's own again.
- Delete forever (Shift+Delete): removes a song that's in the trash for good, after asking. Delete on a song in the trash (Filters, Songs in the trash) restores it.
- Make from it, Make a video: Suno makes the song's video, and you hear when Download video can save it. Make a persona: a persona from the song, for new songs, covers and extends.
- Related songs: Its stems (the stems already made from it), Credits and remix count (the songs it credits, and how many remixes and covers it has), Creator's playlists (play, like or save them).
- Sharing: show or hide the list of remixes on the song's page.
- Library menu, Personas: yours, the ones you love, and the ones you follow. For each: its songs, its source song, love it, and for your own, rename and describe, public or private, trash or restore.
- Library menu, Custom models: train one from a playlist of 6 to 100 of your finished songs (your plan has to include custom models), rename or archive the ones you have.
- Library menu, Lyrics mashup: Suno writes one set of lyrics from two.
- Playlists tab, Applications key: Cover picture, and Make an album from it (an album or EP, kept as a draft or published).
- Shift+F2 also says whether Suno shapes style ideas and boosts to your taste.
- Timed lyrics: when Suno has none stored for a song that has lyrics, Tin Pan asks Suno to time them.

Downloads use each song's own file: the unencrypted one Suno lists in `media_urls` when there is one (it costs no download, and is often an M4A), otherwise `audio_url` as before. WAV uses Suno's WAV conversion. Settings can also save lyrics, timed lyrics (LRC), song details and cover art next to each download. If that fails for a song, the app asks whether to use Suno's own download instead. Suno counts those against your plan's downloads unless you have downloaded that song before. Since 2026-09-09 that includes Suno Studio downloads. Shift+F2 reads how many you have left.

WAV downloads need a Pro or Premier plan. Free plans get a few covers and remasters: the Song menu shows how many are left, and Shift+F2 reads them out. On the free plan they're shown as unavailable. Until the plan has loaded they say "(needs Pro or Premier)".

Extend, Cover, Remaster and Edit open a short form: Tab between fields, Enter on OK, Escape to cancel. If something is wrong, the form says what and puts you back on that field.

The Song menu's Make from it submenu can also change a song into a new one, leaving the original alone:

- Change speed: a percentage (100 is as it is), keeping the pitch or letting it rise and fall like a tape.
- Reverse: the song played backwards.
- Crop: keep only the part between two times, or take that part out.
- Fade in or out: over the seconds you choose, at the start, the end or both.

With a persona, Tin Pan checks it first: a persona in the trash or hidden, or one without its source song, is refused with a clear message instead of making a song that ignores it. A voice persona on a model too old for voice personas is used as an ordinary persona when it has a source song.

## Creating

Ctrl+N (or New song on the Library page) opens the Create window. It stays open beside the main window until you close it with Escape or Close, so you can make a song, go back to the Library with F6 to listen, change the style and make another. F8, F9 and the other playback keys work in it too. Extend, Cover and Remaster open the same kind of form: it stays open after each one, so you can make a variation, listen, fine-tune and make another. You can start up to five at once, as Suno allows; the button only waits while five are still being started. The Cover form also has **Rewrite the lyrics now** (it changes the Lyrics box the way the How to change box says), **Back to the original words**, **Style boost** and **Style ideas**; Extend has Style boost and Style ideas too.

Style boost rewrites the Style box into a fuller description. Change my lyrics takes your Lyrics and an instruction ("make the chorus rhyme") and puts Suno's rewrite in Generated lyrics.

The Create window has one set of settings used both ways: model, style, styles to exclude, voice, instrumental, weirdness, style influence and persona ID. Leave weirdness or style influence empty to let Suno choose. Then either describe the song, or give a title and your own lyrics. Lengths are checked against the chosen model's limits before anything is sent, so no credits are spent on a request Suno would refuse.

If Suno asks for a captcha, the app passes it itself with Suno's own check on suno.com/create, in your signed-in browser and out of sight. Usually no puzzle appears. If Suno does want a click, a "Suno check" window comes forward with the check box; when it's done the app goes on and makes the song.

## Signing in

Settings, Sign in. Suno's sign-in page opens inside the app and closes by itself once you're in. The app keeps Suno's sign-in cookie, encrypted for your Windows account, and renews the short-lived API token itself, so you stay signed in between runs. Sign out forgets it and signs the sign-in browser out of Suno. Your Google account stays remembered there, and the browser can save your password.

## Where things are

- Settings: `%APPDATA%\Tin Pan\config.json`
- Log (no tokens or cookies in it): `%LOCALAPPDATA%\Tin Pan\logs\tinpan.log`
- Sign-in browser: `%LOCALAPPDATA%\Tin Pan\WebView2`

The first run after the rename copies the settings from the old `Suno Desktop` folder and moves the sign-in browser from `Suno Companion`, so you stay signed in.
- Downloads: the folder in Settings, or `Music\Suno`. Files are named `Title (first 8 characters of the song id).mp3`, so Suno's two takes of a song don't overwrite each other.

## Building

```
dotnet build TinPan -c Release
dotnet test TinPan.Tests
```

The app is then `TinPan\bin\Release\net10.0-windows\TinPan.exe`.

### The one-file version (for everyone else)

```
dotnet publish TinPan -p:PublishProfile=OneFile
```

That makes a single `TinPan\bin\publish\TinPan.exe` (about 60 MB) with .NET, WPF, WebView2's loader, NVDA's helper and the license texts inside. Nothing needs installing: WebView2 comes with Windows 10 and 11. On first run Windows unpacks the native parts to a temporary folder and Tin Pan unpacks NVDA's helper to `%LOCALAPPDATA%\Tin Pan\native`, so the second start is quicker. Help, Licenses and notices shows the licenses. A `nvdaControllerClient.dll` placed next to the exe is used instead of the built-in one.

## Discover

The Discover menu finds songs beyond your own:

- **Explore**: Suno's shelves of songs. Enter opens a shelf.
- **From people I follow**: new songs from the people you follow on Suno.
- **Find a creator**: type a handle (the name after suno.com/@) or paste a profile link to list their songs, newest first.
- **Contests**: Suno's contests, with when each ends and how many entries it has. Enter plays a contest's song.

The Song menu's Related songs submenu (also in the Applications key menu) has **Similar songs**, **Creator's songs**, **Covers and remixes of it** and **Original song**, which finds what a cover, extension or whole song was made from. Songs open in their own list: Enter plays one, Escape closes it, and each says who made it.

In the Create window, **Style ideas** lists styles Suno suggests for you, and **Lyrics drafts** lists your drafts from Suno's lyrics editor; Enter puts one in the Style or Lyrics box. On the Account tab, notifications you haven't seen say "New", and **Mark all as read** marks them read on Suno too.

The following feed's ranking settings and the shape of a lyrics draft aren't known for certain, so if either stops working, Tin Pan says so and logs Suno's reply.

## Reporting a problem or suggesting a feature

Help, **Report a problem** (Ctrl+Shift+B) and **Suggest a feature** (Ctrl+Shift+F) send a message privately to Tin Pan's author; no account is needed. A problem report can include Tin Pan's log. The log has no passwords, and your Windows user name is taken out before sending. Reports go through a small relay (in `report-relay/`) that files them in a private GitHub repository.

## Uploading songs

Library menu, Upload a song (or the Upload button on the Library page, Alt+U) sends an audio file from your computer to your Suno account. There it becomes a song like any other: play it, extend it, cover it, remaster it. Choose the file, give it a title (leave it empty to use the one in the file), add its lyrics if you like, and confirm you own it or have the rights to upload it and accept Suno's upload terms; Tin Pan never ticks that for you. MP3, M4A, WAV, FLAC, OGG and AAC are sent as they are; WMA and other files Windows can play are converted to M4A first with Windows' own encoder. The cover picture Suno makes for the upload is kept. Suno checks every upload, which takes a minute or two, and may refuse music it finds is copyrighted. The shortest and longest upload come from your plan, and Tin Pan checks them before sending.

## Clean-up (sunscreen)

Suno songs carry a faint haze, like a radio slightly off-station, and a steady decoder buzz. [sunscreen](https://github.com/abradburne/sunscreen) by Alan Bradburne measures and removes them, and Tin Pan has it built in (version 0.7.1, MIT license, checked by its SHA-256 before every run). It runs on your computer; nothing is uploaded.

- **Download (Ctrl+D)** asks every time: clean this song up too?
- **Download MP3, Download WAV and Download all** don't ask. They clean up when Settings, "Clean up after Download MP3, Download WAV and Download all", is on (it's off to start with).
- **Clean up download**, in the Song menu and the Applications key menu, cleans up a song you've already downloaded.

The result is a 24-bit WAV next to the download, named like it with "(clean)" added. It's often 10 to 20 times bigger than the download. sunscreen reads MP3 and WAV only, so Suno's M4A files are converted to WAV first (Windows does that itself). A 5-minute song takes about 10 seconds.

Settings, Clean-up (sunscreen) has each of sunscreen's steps with its value: remove the decoder buzz, only remove haze you can hear, steady the flutter (milliseconds), widen the highs (amount), rebuild the top octave (dB), match the tone of commercial masters or of a song you choose, and master the loudness (LUFS). "Use the recommended clean-up steps" puts sunscreen's own recommendation back. "Keep the downloaded file next to the cleaned-up WAV" is on to start with.

## Languages

English, Romanian (Română), Ukrainian (Українська) and Spanish (Español). Settings, Language chooses one, or "Automatic" follows Windows; it changes the next time Tin Pan starts. Text Tin Pan hasn't got a translation for stays in English. Messages that come straight from Suno stay in Suno's words.

The translations are in `TinPan/Localization/translations.json`: each English text with its Romanian, Ukrainian and Spanish. Counts have each language's plural forms, separated by `|` (Romanian and Ukrainian three, Spanish two). After changing any text, run `python -I build/translations/check.py`; it lists texts without translations, placeholders that don't match, and Alt letters used twice on a page, menu or form in any language. The tests check the same.

## Signing in

The first time (or after Sign out), Tin Pan welcomes you and offers to sign in. If Suno ends your sign-in later, for example after a password change, Tin Pan asks you to sign in again. A network problem never does; your saved sign-in stays and is tried again.

## Updates

Tin Pan looks for a new version on GitHub a few seconds after it starts, and says nothing unless there is one (Settings can turn this off). Help, Check for updates looks now. When there's a new version, Tin Pan reads what's new and asks. Yes downloads the new `TinPan.exe`, checks that it's signed by Tin Pan's publisher (Ed25519), closes, swaps it in and starts again. If the swap fails, the old exe is put back and the next start says so. Saying No to a version offered at startup means it isn't offered again by itself.

Only the one-file `TinPan.exe` from a release updates itself; a development build offers to open the release page instead.

### Making a release

1. Set `<Version>` in `TinPan/TinPan.csproj` and add a matching `## x.y.z` section to `TinPan/CHANGELOG.md`.
2. Commit, then push a tag `vx.y.z`. The Release workflow tests, builds the one-file exe, signs it with the `UPDATE_SIGNING_KEY` secret, checks the signature against the public key in `UpdateService.cs`, and publishes `TinPan.exe`, `TinPan.exe.sig` and `SHA256SUMS.txt`.

The signing key's private half is in `%APPDATA%\TinPan-release\update-signing.key` on the publisher's computer and in the repository secret; it is never in the repository. `build/Signer` makes keys (`keygen`), signs (`sign`) and checks (`verify`).

## Speech

Settings (or the Help menu) sets how much Tin Pan says by itself: everything; what you ask for and errors, without background news; or only errors and what you ask to hear. F1, F2, F3 and Ctrl+R always speak, and every message is kept in the history (Ctrl+Shift+R), so nothing is lost when it isn't spoken.

When the arrow keys reach the song that's playing or paused, Tin Pan gives a short click. Settings can make it say "playing" after the song's name instead (which braille shows too), do both, or do nothing.

## License and dedication

Tin Pan, like all my apps, is dedicated to Jenni and Diana. The full dedication is at the top of [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md), which also lists the components Tin Pan ships with and their licenses.

Copyright (C) 2026 Ernesto Melendez. Tin Pan is free software under the [GNU General Public License, version 3 or later](../LICENSE), with additional terms under section 7 in [NOTICE.md](../NOTICE.md). Anyone who shares Tin Pan or a version of it must keep "Tin Pan was created by Ernesto Melendez" and the dedication in its About or credits screen, next to the copyright and license notices, and mark a changed version as changed. Help, About shows them; Help, Licenses and notices shows the full license and terms.

## API references

- Suno's own web code (suno.com/create): covers and extends send `task` ("cover" or "extend") with the song's id; remaster is `POST /api/generate/upsample`; Trending is `POST /api/unified/feed` with `feed_id: "trending"`.

The projects Tin Pan's Suno support learned from are credited in [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
