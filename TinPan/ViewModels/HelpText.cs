namespace TinPan.ViewModels
{
    /// <summary>
    /// F1 says one of the short lines for the current tab; Ctrl+F1 shows the full list.
    /// Keep README.md in step with these.
    /// </summary>
    public static class HelpText
    {
        public const string Library =
            "Library. Arrow through your songs. Enter plays, Space pauses, Left and Right seek. Applications key for extend, cover, downloads and more. Ctrl+F searches. Ctrl+Shift+F8 resumes where you left off. Alt+F for filters.";

        public const string Create =
            "Create. Set the model, style and voice, then describe a song or use your own lyrics. Tab moves between fields. F5 reloads the models.";

        public const string Trending =
            "Trending. Enter or Space plays a song. Applications key for more. F5 refreshes.";

        public const string Account =
            "Account. Your plan and credits, then Suno's notifications. F2 and Shift+F2 read them from anywhere. F5 refreshes.";

        public const string Playlists =
            "Playlists. Pick a playlist and press Enter to open its songs. Enter plays, Space pauses, Delete removes a song, Ctrl+Up and Ctrl+Down move it. Applications key for more on either list.";

        public const string Settings =
            "Settings. Sign in or out, and choose where songs are saved.";

        public const string NotSignedIn = " You're not signed in. Ctrl+6 opens Settings.";

        public const string Full =
@"Anywhere
Alt or F10: the menu bar, hidden until then. Every command is in it, with its keys. Left and Right move between menus, Down opens one, Escape or Alt leaves. The Song menu acts on the song selected in the tab you're on.
Tab only stops on what you need: the search box, the list and the volume. The buttons on each page still work with their Alt letters, and everything is in the menu bar.
F1: short help for where you are.
Ctrl+F1: all commands in a list; Enter runs one. Its first item is this guide.
Ctrl+R: repeat the last message. Ctrl+Shift+R: message history, newest first.
Escape: step back one level (to the tab headers, or from a playlist's songs to the playlists).
F2: credits left.
Shift+F2: your plan, when it renews, credits and downloads left. The Account tab lists everything the plan includes.
F3: what's playing and how far in.
F5: refresh the current tab.
Ctrl+1 to Ctrl+6: Library, Playlists, Create, Trending, Account, Settings. Each puts you on that page's list or first field.
Ctrl+F: search your songs.
F8 or the Play/Pause media key: play or pause; with nothing playing it starts the selected song on the Library or Playlists tab.
Shift+F8: stop.
F7 and F9: previous and next song.
Shift+F7 and Shift+F9: back and forward 10 seconds.
Ctrl+F7 and Ctrl+F9: back and forward 10 percent of the song.
Ctrl+F8: back to the start of the song.
Ctrl+Shift+F8: resume the last song where you left off, even after closing Tin Pan.
F11 and F12: volume down and up.

Library list
Enter: play.
Space: pause or resume.
Left and Right arrows: back and forward 5 seconds.
Ctrl+I: details (style, model, plays, link and lyrics) as one text.
Alt+Enter: song information, one fact per line.
Ctrl+L: lyrics.
Ctrl+C: copy the song's link.
Ctrl+D: download in the format chosen in Settings. It uses the song's free unencrypted file when Suno has one; if the direct download fails, the app offers Suno's own download instead.
Ctrl+E: extend.
Delete: move to Suno trash (asks first).
Applications key or Shift+F10 for everything: Download WAV, Download video, Extend, Cover, Remaster, Get whole song, Stems, Edit title and lyrics, Make public or private, Add to playlist, Copy Suno share link, Allow or stop remixes, Similar songs, Show comments, Write a comment.
WAV, Cover and Remaster need a Pro or Premier plan. They're unavailable on the free plan, and say so when the plan isn't known yet.
Reaching the end of the list loads more songs.
Alt+F, Filters: workspace, disliked songs (hide, show, or only), trash, stems, newest or oldest first. Kept between runs.
Alt+L, Download all: every finished song in the list, using the free direct download only; songs already in the folder are skipped.
Songs you liked say ""liked"".

Search box
Enter searches, Escape clears the search.

Create tab
Model, style, styles to exclude, voice, instrumental, max mode, weirdness, style influence and persona (pick one of yours, or type an ID) apply to both ways of making a song.
Describe a song lets Suno write the words. Your own lyrics uses your title and lyrics. Write lyrics makes lyrics from a topic, and Use these lyrics copies them in.
New songs appear at the top of your library. You'll hear when they're ready.
If Suno asks for a captcha, the app passes it itself. If Suno wants a click, a Suno check window comes forward with the check box.

Trending tab
Loads when you first open it. Enter or Space plays a song; the Applications key has every Song command.

Account tab
Your plan, credits and what the plan includes, then Suno's notifications.

Playlists tab
Left list: your playlists. Enter opens one, Delete moves it to the trash (asks first). New, Rename and Delete buttons.
Right list: its songs. Enter plays and carries on through the playlist, Space pauses, Delete removes the song from the playlist, Ctrl+Up and Ctrl+Down move it.

Create tab extras
Style boost rewrites the Style box into a fuller description.
Change my lyrics: say what to change, then Rewrite my lyrics now. The result goes into Generated lyrics; Use these lyrics copies it in.

Extend, Cover and Remaster
Each opens a short form. Tab moves between fields, Enter on OK starts it, Escape cancels.
Extend continues a song from a time you choose; then use Get whole song to join the parts.
Cover keeps the melody in a new style; you can change the title and lyrics, which start as the original words, and set audio influence, weirdness, style influence and persona. Remaster makes a cleaner version with a newer model: pick strength (Subtle, Normal, High), kind (Natural, Boost, Clarity) and, if you like, the fine controls.

Playing
Songs play through Suno's own player, hidden inside the app.

Speech
Help menu or Settings, Tin Pan speaks: everything; what you ask for and errors (no background news); or only errors and what you ask to hear. F1, F2, F3 and Ctrl+R always speak, and every message is kept in the history.
Settings, When a list reaches the playing song: a short click (the default), say playing (also in braille), both, or nothing.

Settings
Sign in opens Suno's own sign-in page; it closes by itself once you're in. It remembers your Google account and can save your password. Sign out only signs out of Suno.
Download folder, download format, and extra files saved next to each download: lyrics, timed lyrics (LRC), song details and cover art.";
    }
}
