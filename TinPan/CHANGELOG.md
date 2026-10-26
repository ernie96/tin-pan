# Tin Pan changelog

## 1.10.0 (2026-10-26)

### New

- Like and dislike songs: Song menu, or Ctrl+Shift+U and Ctrl+Shift+N in any song list. Press again to take it back. Songs you disliked say so in the list.
- The free plan's allowances: the Song menu shows how many free covers and remasters are left ("Cover... (3 free left)"), and Shift+F2 reads them out. Remaster works on the free plan while free remasters are left.
- The library loads 100 songs at a time instead of 50, so big libraries come in with half the waiting.

### Fixed

- Explore put the songs of a shelf inside another shelf ("Staff Picks", say) under the outer shelf's name. Each shelf now keeps its own name.
- Adding a song to a playlist, or removing one, said it worked even when Suno turned it down. It says so now. Deleting a playlist sends the request the way Suno expects.

## 1.9.1 (2026-10-25)

### Fixed

- Alt+F4 sometimes didn't close Tin Pan. If Windows had put focus on one of Tin Pan's hidden Suno windows, Alt+F4 closed that one instead, and Tin Pan stayed open without its player. Alt+F4 now closes Tin Pan wherever it lands, and Tin Pan always ends when its main window closes. Library menu, Exit does the same.
- After going back to the Windows default output, a page left over from an earlier choice could report that the output couldn't be used.

## 1.9.0 (2026-10-25)

### New

- Keys for every Song command, in every song list (the Library, a playlist's songs, Trending and song windows): Ctrl+P adds to a playlist, Ctrl+T edits the title and lyrics, Ctrl+Shift+C covers, Ctrl+Shift+M remasters, Ctrl+J gets the whole song, Ctrl+Shift+S makes stems, Ctrl+Shift+D, W and V download the MP3, WAV and video, Ctrl+Shift+H cleans up the download, Ctrl+M finds similar songs, Ctrl+U the creator's songs, Ctrl+O the original, Ctrl+Shift+O covers and remixes, Ctrl+K shows comments, Ctrl+Shift+K writes one, Ctrl+Shift+L copies the share link, Ctrl+Shift+P makes it public or private and Ctrl+Shift+X allows or stops remixes. The Song menu shows each one, and a key for something that isn't available right now (WAV on the free plan, say) says so.

### Fixed

- Cover was marked "needs Pro or Premier" and blocked on the free plan, but Suno lets every plan cover songs now. It's open to everyone; if Suno ever refuses an account, Tin Pan says why. Thanks to the person who reported it.
- The update check left Windows handles open, and choosing an audio output asked Windows for the device list once per line.

## 1.8.2 (2026-10-24)

### Changed

- Tin Pan now talks to Suno at the new address Suno's own site uses (suno.com), and falls back to the old one by itself when the new one doesn't take a request. That keeps everything working if Suno switches the old address off. A request that makes or changes something is only sent again when it's certain the first one wasn't done, so nothing is made or charged twice.
- Notifications come from Suno's newer list, read as full sentences; the older list is used if the newer one isn't there.

## 1.8.1 (2026-10-24)

### Changed

- F1 starts with the way between windows: in the main window, F6 to the open Create or Cover window (or Ctrl+N to open Create); in those windows, F6 back to the main window.

### Fixed

- Coming back to Tin Pan (Alt+Tab, or after Windows+D) put you in the main window even if you'd left from the Create window or a Cover, Extend or Remaster form. You're back where you were now.
- Minimising Tin Pan (Alt+Space, Minimise) could leave you on a "Suno check" window showing Suno's web site. Suno's own pages (the player and the captcha check) are now kept out of reach: they're never in Alt+Tab, can't take focus, and the check window only exists while a captcha is running. If Windows lands on one anyway, Tin Pan takes focus back.

## 1.8.0 (2026-10-24)

### New

- Choose the audio output songs play on (Playback menu, Audio output, or Settings), and a second one to flip to with Ctrl+Shift+F12, for example external speakers to show someone a song. It switches even mid-song. Tin Pan always starts on the main output, and says so if one is unplugged.
- Extend, Cover and Remaster forms stay open until you close them (Escape or Close): make a variation, listen to it, change something and make another. F6 moves between the form and the main window, and the playback keys work in the form. You can start up to five at once, as Suno allows.
- The Cover form has Rewrite the lyrics now (Suno changes the words the way you ask), Back to the original words, Style boost and Style ideas. Extend has Style boost and Style ideas.

### Changed

- Create is now a window instead of a tab: Ctrl+N, or New song on the Library page. It stays open beside the Library, with the same fields and keys as before. The tabs are now Ctrl+1 to Ctrl+5: Library, Playlists, Trending, Account, Settings.
- Trending's quick help says Space pauses, as it now does.

### Fixed

- A song Suno described with an empty or odd count (a play count of null, say) was left out of the list. It's kept now, with the count read as 0. Empty totals no longer cut playlists short after the first page.
- When Suno's sign-in service said you were no longer signed in, Tin Pan took it for an unreadable reply and never asked you to sign in again. It asks now. An account without a name no longer stops sign-in.
- Saving settings in the background while they were changing (the Library filters, say) could close Tin Pan.
- After playing a song from a playlist, looking at another playlist lost your place: Next and the end of the song said "End of playlist". They carry on through the playlist the song came from.
- If the free file failed and you took Suno's own MP3 download, it could be saved with the free file's .m4a name.
- Several songs started at once that each needed Suno's captcha fought over the one check page; they take turns. After the check, focus goes back to the window you were in.
- Some refusals were reported as "That needs a Pro or Premier plan" when Suno's message only mentioned "process" or "problem".
- Next in the Library stopped at songs still being made near the end of a page instead of loading further.
- Moving a song to the trash from a playlist or Trending changed the Library's selection; showing a song's lyrics after a failed fetch also said it had none.
- Uploads and update downloads that stall now end with a message instead of waiting forever; an upload that fails to convert says why. Two download error messages weren't translated.
- The audio-device permission given to Suno's player page is taken back when you return to the Windows default output.

## 1.7.0 (2026-10-23)

### New

- Discover menu: Explore (Suno's shelves of songs), From people I follow, Find a creator (by handle or link) and Contests.
- Song menu: Creator's songs, Covers and remixes of it, and Original song (what a cover, extension or whole song was made from). Lists of other people's songs say who made each one, and song information says who made it.
- Create tab: Style ideas from Suno, and your Lyrics drafts from Suno's lyrics editor; Enter puts one in the Style or Lyrics box.
- Account tab: notifications you haven't seen say "New", and Mark all as read (also in the Account menu) marks them read on Suno.
- Help menu: Report a problem (Ctrl+Shift+B) and Suggest a feature (Ctrl+Shift+F), sent privately to Tin Pan's author. A problem report can include the log, with your Windows user name taken out.
- Read lyrics aloud as they're sung: Shift+F3 (Playback menu, or Settings) turns it on or off. When a song has timed lyrics, Tin Pan says each line as it starts, also in braille, and after a jump only the line being sung. Off to start with.

### Changed

- Tidier menus: the Song menu groups its commands into Download, Make from it, Related songs, Comments and Sharing, and the Help menu's speech levels are in a Speech submenu. Lines separate each menu's sections.
- Trending works like the other lists: Space pauses or resumes, F9 and the end of a song carry on down the list, and each song says who made it.
- Tin Pan is now free software under the GNU General Public License, version 3 or later. Its extra terms ask anyone sharing Tin Pan, or a changed version of it, to keep the author's credit and the dedication in its About screen and to mark changed versions as changed. Help, About shows the copyright and license; Help, Licenses and notices has the full license and terms.

### Fixed

- The Song menu's keys (Ctrl+I, Alt+Enter, Ctrl+L, Ctrl+C, Ctrl+D, Ctrl+E) only worked in the Library. They now work in a playlist's songs and in Trending too.
- On the Playlists tab, Make public or private and Allow or stop remixes showed the Library song's state instead of the playlist song's. In a playlist, Move to trash no longer shows Delete as its key, since Delete removes the song from the playlist there.
- After a Trending song, F9 played the next Library song instead of the next Trending one, and Play or pause (F8) on the Trending tab could start a Library song.
- Enter on a Trending song could be ignored while another was still starting.
- The "playing" said by lists for the current song wasn't translated.

## 1.6.1 (2026-10-22)

### Fixed

- Pressing Enter on another song while one was playing sometimes did nothing, so the old song kept going until you pressed Space. Tin Pan sometimes missed that a song had started, waited up to a minute for it, and ignored Enter in the meantime. It now notices the song playing straight away, and Enter always switches songs.
- When Suno says "too many requests" (for example, after opening Tin Pan a few times in a row), Tin Pan waits a moment and tries again, up to twice, before telling you.
- When a song doesn't start, the log now says which song and what Suno's player page showed, to help track down the cause.
- Running Tin Pan's tests no longer writes test lines into your Tin Pan log.

## 1.6.0 (2026-10-22)

### New

- Upload a song (Library menu, or Alt+U on the Library page): sends an audio file from this computer to your Suno account, where it becomes a song you can play, extend, cover and remaster. MP3, WAV, M4A, FLAC, AAC and WMA work; anything but M4A is converted to M4A first. You confirm your rights and Suno's upload terms each time. Tin Pan checks your plan's length limits first, says when Suno is checking the upload, and puts the new song at the top of the library.

## 1.5.1 (2026-10-22)

### Fixed

- Free song downloads now work for encrypted songs without using metered quota. Direct download transparently fetches rights and decrypts Suno's Mango DRM streams (AES-CTR) locally so you are no longer prompted that "Suno only has an encrypted copy of this song, so there's no free download".
- Download all includes encrypted songs directly instead of leaving them out.

## 1.5.0 (2026-10-22)

### New

- Library filters (Alt+F) now have every filter Suno's own library has: liked, public or private, uploaded, covers, extended, remasters, whole songs, personas, songs in a playlist, songs downloaded before and Suno Studio songs, as well as trash and stems. Each one shows those songs, shows only them (for example only your uploads), or hides them. Sort adds most liked and least liked first. Your earlier trash, stems and order choices carry over.

### Fixed

- The update offer reads only what changed, not the download instructions at the top of the release page. The dedication is no longer in release notes.

## 1.4.0 (2026-10-22)

### New

- Clean-up with sunscreen (github.com/abradburne/sunscreen), built in. Download (Ctrl+D) asks every time whether to clean the song up; Download MP3, Download WAV and Download all follow Settings. It removes Suno's haze and decoder buzz and saves a 24-bit WAV next to the download with "(clean)" in its name. Suno's M4A files are converted to WAV first. Clean up download, in the Song menu, does a song you already have.
- Settings, Clean-up (sunscreen): every sunscreen step with its value (buzz, hearing model, flutter, widening, top octave, tone matching with an optional reference song, loudness), whether Download MP3, Download WAV and Download all clean up, and whether to keep the original. Use the recommended clean-up steps puts sunscreen's own recommendation back.
- Download MP3, in the Song menu, alongside Download WAV.

### Fixed

- Downloading a song Suno only keeps encrypted no longer fails with "The download failed (403)". Suno's audio address for those songs is a placeholder that always refuses; Tin Pan now says there's no free download and offers Suno's own one straight away.
- Download all names the songs that only have an encrypted copy and leaves them out up front, instead of failing on each.
- Download attempts are now written to the log, so a failed one can be traced.

## 1.3.0 (2026-10-22)

### New

- Tin Pan speaks Romanian, Ukrainian and Spanish as well as English. Settings, Language picks one, or follows Windows (the default); it changes the next time Tin Pan starts. Each language is named in its own words and in yours, so every voice can read the list. Menus, pages and forms have their own Alt letters in each language.
- First time, or after signing out: a welcome that offers to sign in. If Suno ends your sign-in later (not a network problem), Tin Pan asks you to sign in again right away. Saying No points to Settings.

### Fixed

- Extend, Cover, Remaster, Stems and Get whole song now say when a song isn't finished, instead of sending it to Suno to be refused.
- Times of an hour or more in the Extend form read correctly (60:00, not 0:00).
- The update offer at startup waits until Tin Pan's own window is in front, so it can't land on top of the sign-in window.
- The Library's Applications key menu is now the same as the Song menu, so the two can't differ.

## 1.2.0 (2026-10-22)

### New

- Tin Pan updates itself. A few seconds after starting it quietly looks for a new version (Settings can turn that off), and Help, Check for updates looks now. It reads what's new, and on Yes downloads the new TinPan.exe, checks its signature, closes, swaps it in and opens again. If the swap fails the old copy is put back and the next start says so. Updating starts working from this version on; 1.1.0 needs a manual download once.

## 1.1.0 (2026-10-22)

Suno Companion is now called **Tin Pan**, after Tin Pan Alley. The program is `TinPan.exe`, in `TinPan\bin\Release\net10.0-windows`. Your settings and sign-in carry over by themselves.

You'll need to sign in once more after updating, because the old saved sign-in can't be renewed.

### New

- Tin Pan can be published as one TinPan.exe that needs nothing installed (`dotnet publish TinPan -p:PublishProfile=OneFile`). NVDA's helper and the license texts are built in; Help, Licenses and notices shows them.

- The menu bar is hidden until you press Alt or F10, so Tab and the screen reader don't walk through it. Escape or Alt leaves it.
- The Song menu, and Ctrl+F1, act on the song selected in the tab you're on: a playlist's song or a trending song, not only the Library's.
- Playlists and Trending have an Applications key menu.
- Tab stops only on the search box, lists and volume; page buttons still work with their Alt letters.
- The Suno tab is split into its own tabs. Ctrl+1 to Ctrl+6 are now Library, Playlists, Create, Trending, Account and Settings (Playlists moved from Ctrl+4 to Ctrl+2, Settings from Ctrl+3 to Ctrl+6). Each lands on the page's list or first field, Trending loads the first time you open it, and Tin Pan starts on your songs.
- Song lists click when you arrow onto the song that's playing. Settings can make them say "playing" instead (shown in braille too), do both, or stay quiet.
- A menu bar (Alt or F10) with every command and its keys. Ctrl+F1 lists the same commands, and Enter runs one.
- Ctrl+R repeats the last message; Ctrl+Shift+R shows the message history.
- How much Tin Pan says: everything, what you ask for and errors, or only errors and what you ask to hear (Settings or the Help menu).
- Ctrl+Shift+F8 resumes the last song where you left off, even after closing Tin Pan.
- Ctrl+F7 and Ctrl+F9 skip 10 percent; Ctrl+F8 goes back to the start.
- Alt+Enter on a song lists its information one fact per line.
- Escape steps back one level: from a playlist's songs to the playlists, otherwise to the tab headers.
- About Tin Pan, in the Help menu: version, speech output, account, where settings and the log are, and the dedication.
- Tin Pan is dedicated to Jenni and Diana; see THIRD_PARTY_NOTICES.md.

- Cover: a new version of a song in another style, with optional audio influence.
- Remaster, using the remaster models your plan offers.
- Extend from any point (type seconds or minutes:seconds), with new lyrics and style.
- Search your library: Ctrl+F from anywhere, Enter to search, Escape to clear.
- Song details (Ctrl+I) and lyrics (Ctrl+L) in a window you can read line by line.
- Edit a song's title and lyrics, make it public or private, copy its link, download its video.
- Shift+F2 reads your plan, when it renews, downloads left, and what it includes.
- Create has styles to exclude, voice (male or female), weirdness, style influence and persona ID.
- Seeking: Left and Right in the list, Shift+F7 and Shift+F9 anywhere.
- Library filters (Alt+F): workspace, disliked songs, trash, stems, newest or oldest first. Kept between runs.
- Download all (Alt+L) for every finished song in the list, using only the free direct download.
- Optional lyrics, timed lyrics (LRC), song details and cover art saved next to each download.
- Pick one of your personas on the Create tab.
- Max mode on Create, Extend and Cover.
- Cover asks for title, lyrics (starting as the original words), instrumental, style and styles to exclude. Before, it sent no words, so covers came out instrumental.
- Covers really follow the chosen song. The request now says "task: cover", like suno.com does; without it Suno made an unrelated song. Extend sends "task: extend" the same way.
- Cover and Extend have weirdness, style influence and persona. Cover titles start as "Song (Cover)", as on suno.com.
- Remaster uses Suno's own remaster request (/api/generate/upsample) with all its options: model (your plan's, plus older v5.5, v5 and v4.5+), strength (Subtle, Normal, High), kind (Natural, Boost, Clarity), style, and the five fine controls: freedom, tone, clarity, strength, stereo width.
- Shift+F2 reads plan, renewal date, credits (this month's and extra) and downloads left in plain sentences. The Account tab lists what the plan includes, in plain words.
- Trending works again (Suno's feed needed a feed id).
- The hidden player starts right after sign-in, so the first song and the first captcha start sooner.
- Liked songs say "liked" in the list.

- Playlists tab (Ctrl+2): your playlists and their songs. Play through a playlist, add songs from the library (Add to playlist, with New playlist), remove them, move them with Ctrl+Up and Ctrl+Down, rename or delete a playlist.
- Song menu: Copy Suno share link, Allow or stop remixes, Similar songs (Enter plays one), Show comments, Write a comment.
- Create tab: Style boost rewrites your style into a fuller one; Change my lyrics rewrites your lyrics the way you ask.

### Fixed

- One song Suno describes oddly no longer stops the whole library page, playlist or trending list from loading.
- Timeouts and unreadable replies from Suno's sign-in now say what went wrong instead of "Something went wrong".
- A timed-out request that changes something (like making songs) now says to refresh first, so you don't pay twice.
- A download folder that can't be used, or a full disk, now says so; Download all stops at once instead of failing song by song, and Suno's own download isn't offered when it would fail the same way.
- Previous song skips songs still being made, as Next does.
- Moving the playing song to the trash stops it even after the library was refreshed.
- Trending says when you need to sign in, and when a song isn't ready.
- Safety: the hidden Suno player and captcha windows can't open pop-ups or leave suno.com, the player only listens to suno.com, NVDA's helper DLL is only loaded from Tin Pan's own folder, and text from Suno can't fake lines in the log.
- F8 and the Play/Pause media key start the selected song of the tab you're on (Library or Playlists) when nothing is playing.
- After you open a playlist, search, delete a song or remove one from a playlist, focus lands on a song instead of the bare list, so NVDA reads it and the arrow keys keep working.
- Forms (Extend, Cover, Remaster and the rest) have their explanation read with the first field.
- A playlist's spoken name updates its song count after you open it.
- Space plays a song in Trending and in Similar songs, like Enter.
- The captcha check no longer pulls focus back to Tin Pan when its window never appeared.

- Only the list that started a song moves on when it ends. Before, the Library and a playlist could both start their next song.
- F7, F9 and the media keys follow whichever list is playing (Library or a playlist).
- Songs start at your volume instead of full volume for the first second.

- Making songs works when Suno asks for a captcha. The app runs Suno's own check (Turnstile or hCaptcha) in your signed-in browser, out of sight. If a check box is needed, a "Suno check" window comes forward for it.
- Songs play again. Suno now encrypts song audio, so the app plays them with Suno's own player, hidden inside the app. Playing is unlimited and uses no downloads.
- Downloads use the song's free unencrypted file when Suno lists one. audio_url turned out to be a decoy that never held the music. If a direct download fails, the app offers Suno's own download instead, which Suno counts unless you've downloaded that song before.
- The sign-in window remembers your Google account and can save passwords. Sign out only signs out of Suno.
- You stay signed in. The app saves Suno's long-lived sign-in cookie and renews the one-minute API token itself.
- Making songs uses Suno's current endpoint. The old one now refuses every request.
- Library, credits and your name in the title bar load straight after sign-in and at start-up, once each.
- Arrowing to the last song loads the next page.
- Stop, Previous and choosing another song no longer skip ahead a track.
- Being offline or a Suno error no longer crashes the app. You hear one short sentence instead.
- The lyrics topic box no longer overwrites your style tags.
- Downloads are real MP3 or WAV files, and two songs with the same title no longer overwrite each other.
- Volume is kept between songs and between runs. The Trending tab's songs play.
- Credit totals are right. Placeholder numbers no longer show when something fails.

### Changed

- WAV, Cover and Remaster are unavailable on the free plan, and say "(needs Pro or Premier)" until the plan is known.
- Lengths are checked against the model's limits before sending, so no credits go on a request Suno would refuse.
- Song lists read as "Title, 2:05", then "still being made", "failed", "whole song" or "stem" when that applies. Per-row buttons are replaced by keys and the Applications key menu.
- Moving a song to trash asks first.
- New songs appear in the library, and you hear one line when they're ready.
- Models come from your account, so new Suno models show up without an app update.
- F1 gives help for the current tab, and Ctrl+F1 lists every key.
- Speech goes through NVDA when it's running, through JAWS or Narrator otherwise, and through Windows speech only when no screen reader is on.
- The sign-in cookie is stored encrypted for your Windows account.
- Sign out button. Browse button for the download folder.

### Removed

- Upsample, now replaced by Remaster, and a handful of unused endpoints nobody called.
- The "keep an MP3 copy of each song I play" setting. With Suno's download limits it could have used up your downloads without asking.
