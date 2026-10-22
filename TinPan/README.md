# Tin Pan

A Windows app for Suno, built for NVDA and keyboard use. Sign in with Suno's own page, play, search and download your songs, and make new ones: describe a song, use your own lyrics, extend, cover, remaster, join parts into a whole song, and split it into stems.

By Ernesto Melendez. Unofficial: it uses the same private API as the Suno website, which can change without notice.

## Keys

Alt or F10 opens the menu bar, which has every command with its keys. It stays hidden otherwise, so Tab and the screen reader never run into it. The Song menu acts on the song selected in the tab you're on (Library, a playlist, or Trending). Ctrl+F1 lists the same commands; Enter runs one.

Tab only stops where it matters: on the Library, the search box, the song list and the volume; on Playlists, the two lists. The page buttons still work with their Alt letters. Switching pages with Ctrl+1 to Ctrl+6 puts you on the page's list or first field.

Anywhere:

- F1: short help for where you are. Ctrl+F1: all commands; Enter runs one.
- Ctrl+R: repeat the last message. Ctrl+Shift+R: message history.
- Escape: step back one level.
- F2: credits left. Shift+F2: your plan, when it renews, downloads left, and what it includes.
- F3: what's playing and how far in.
- F5: refresh the current tab.
- Ctrl+1 to Ctrl+6: Library, Playlists, Create, Trending, Account, Settings. Ctrl+F: search your songs.
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
- Filters (Alt+F): workspace, disliked songs (hide, show, or only), trash, stems, newest or oldest first. Suno applies them itself, so paging and search follow them, and they're kept between runs.
- Download all (Alt+L): every finished song in the list, through the free direct download only. Songs already in the folder are skipped, and the summary names any that need Suno's own download.

## Playlists

Ctrl+2. The left list is your playlists: Enter opens one, Delete moves it to the trash (it asks first), and there are New, Rename and Delete buttons. The right list is its songs: Enter plays and carries on through the playlist, Space pauses, Delete removes the song from the playlist, Ctrl+Up and Ctrl+Down move it. In the Library, the Applications key menu has Add to playlist. On a playlist, the Applications key opens a menu for the playlist or the song (with every Song command); Trending has one too.

## Playing and downloads

Since September 2026 Suno encrypts song audio, and only Suno's own player can play it. The app plays songs with that player, signed in as you, in a browser page it keeps out of sight. Focus never moves there, so NVDA stays in the app. Playing is unlimited and uses none of your downloads.

Downloads use each song's own file: the unencrypted one Suno lists in `media_urls` when there is one (it costs no download, and is often an M4A), otherwise `audio_url` as before. WAV uses Suno's WAV conversion. Settings can also save lyrics, timed lyrics (LRC), song details and cover art next to each download. If that fails for a song, the app asks whether to use Suno's own download instead. Suno counts those against your plan's downloads unless you have downloaded that song before. Since 2026-09-09 that includes Suno Studio downloads. Shift+F2 reads how many you have left.

WAV downloads, covers and remasters need a Pro or Premier plan. On the free plan they're shown as unavailable. Until the plan has loaded they say "(needs Pro or Premier)".

Extend, Cover, Remaster and Edit open a short form: Tab between fields, Enter on OK, Escape to cancel. If something is wrong, the form says what and puts you back on that field.

## Creating

Style boost rewrites the Style box into a fuller description. Change my lyrics takes your Lyrics and an instruction ("make the chorus rhyme") and puts Suno's rewrite in Generated lyrics.

The Create tab has one set of settings used both ways: model, style, styles to exclude, voice, instrumental, weirdness, style influence and persona ID. Leave weirdness or style influence empty to let Suno choose. Then either describe the song, or give a title and your own lyrics. Lengths are checked against the chosen model's limits before anything is sent, so no credits are spent on a request Suno would refuse.

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

## Updates

Tin Pan looks for a new version on GitHub a few seconds after it starts, and says nothing unless there is one (Settings can turn this off). Help, Check for updates looks now. When there's a new version, Tin Pan reads what's new and asks. Yes downloads the new `TinPan.exe`, checks that it's signed by Tin Pan's publisher (Ed25519, the same scheme as TimePlayer), closes, swaps it in and starts again. If the swap fails, the old exe is put back and the next start says so. Saying No to a version offered at startup means it isn't offered again by itself.

Only the one-file `TinPan.exe` from a release updates itself; a development build offers to open the release page instead.

### Making a release

1. Set `<Version>` in `TinPan/TinPan.csproj` and add a matching `## x.y.z` section to `TinPan/CHANGELOG.md`.
2. Commit, then push a tag `vx.y.z`. The Release workflow tests, builds the one-file exe, signs it with the `UPDATE_SIGNING_KEY` secret, checks the signature against the public key in `UpdateService.cs`, and publishes `TinPan.exe`, `TinPan.exe.sig` and `SHA256SUMS.txt`.

The signing key's private half is in `%APPDATA%\TinPan-release\update-signing.key` on the publisher's computer and in the repository secret; it is never in the repository. `build/Signer` makes keys (`keygen`), signs (`sign`) and checks (`verify`).

## Speech

Settings (or the Help menu) sets how much Tin Pan says by itself: everything; what you ask for and errors, without background news; or only errors and what you ask to hear. F1, F2, F3 and Ctrl+R always speak, and every message is kept in the history (Ctrl+Shift+R), so nothing is lost when it isn't spoken.

When the arrow keys reach the song that's playing or paused, Tin Pan gives a short click. Settings can make it say "playing" after the song's name instead (which braille shows too), do both, or do nothing.

## License and dedication

Tin Pan, like all my apps, is dedicated to Jenni and Diana. The full dedication is at the top of [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md), which also lists the components Tin Pan ships with and their licenses. A license for Tin Pan's own code hasn't been chosen yet.

## API references

- Suno's own web code (suno.com/create): covers and extends send `task` ("cover" or "extend") with the song's id; remaster is `POST /api/generate/upsample`; Trending is `POST /api/unified/feed` with `feed_id: "trending"`.
- [Nsomnia/suno-power-exporter](https://github.com/Nsomnia/suno-power-exporter): the free unencrypted file in `media_urls` (audio_url is a decoy), feed v3 filters (workspace, disliked, trash, stems, sort), workspaces, personas, and Studio downloads being counted since 2026-09-09.
- [paperfoot/suno-cli](https://github.com/paperfoot/suno-cli): most recent, endpoints checked against the live API in 2026. Source of the `/api/generate/v2-web/` request, cover, remaster, feed v3 and search, trash, title/lyrics editing, visibility, plan fields and the Browser-Token header.
- [gcui-art/suno-api](https://github.com/gcui-art/suno-api) and [Nsomnia/suno-api-local-manager-frontend](https://github.com/Nsomnia/suno-api-local-manager-frontend): Clerk sign-in (the `__client` cookie, `/v1/client` for the session, `/v1/client/sessions/{id}/tokens` for a JWT that lasts about a minute), stems, aligned lyrics and WAV conversion.
