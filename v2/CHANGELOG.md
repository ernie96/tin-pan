# Tin Pan 2 changes

## 2.6.0

- The Create window and the Cover, Extend, Remaster, Use as inspiration, Make speech and Make sounds forms
  have a Workspace choice: My Workspace, or one of your other workspaces. Tin Pan adds the new songs to the
  one you choose as soon as Suno starts them, and says so if Suno doesn't take them. The Create window keeps
  your choice, and F5 there loads your workspaces again.
- Suno's player, which Tin Pan uses when it can't fetch a song itself, no longer gets permission to use the
  microphone. Without it the page can't see the names of your audio outputs, so a song played that way goes
  to the default output even when another one is chosen in Settings, and Tin Pan says so.

### Fixed

- On some computers, making a song when Suno asked for its check gave "Error running JavaScript" messages
  and then stopped, and the check failed with "turnstile is not defined". Tin Pan now waits until the hidden
  page has really loaded before running the check, loads the check again if the page drops it, and no longer
  shows those error messages.
- Adding songs to a playlist or taking them out said it worked when Suno turned some of them down. It says
  so now.
- Strip Suno watermark on an M4A file could leave a copy that didn't play, and missed credentials stored in
  another kind of box. The copy keeps the file's layout now, and those credentials go too.
- Check Suno watermark said a file was verified when Suno's answer wasn't one Tin Pan knew. It says the
  check was inconclusive now.
- A clean-up that failed partway could delete the clean copy you already had. It's kept now.
- Songs downloaded as FLAC or OGG weren't found again for playing, cleaning or other tools.
- Sign-in took cookies from any site whose name ended in "suno.com". It takes only suno.com and its own
  sites now, and a cookie read that fails is tried again instead of signing in with part of them.
- After signing out, answers still on their way for the old account could reach the window.
- A problem report could miss your Windows user name next to some letters, such as the Turkish dotted I.

## 2.5.1

### Fixed

- Choosing a command in a song's Applications key menu (or Shift+F10), or in a list's, did nothing, though
  its shortcut key worked. It runs now.

## 2.5.0

- The Cover, Extend and Use as inspiration forms have a Voice choice too: any voice, male or female.
- With your own lyrics, the Create window's Voice choice now goes to Suno as its own male or female setting,
  as Suno's Create page sends it, instead of as "male vocals" or "female vocals" in the style. A described
  song still gets those words in its style, since Suno has no voice setting for it.

### Fixed

- Batch edit marked songs: making them public or private didn't change them, though Tin Pan said it had.
  It now uses the same way as Make public.
- Batch edit's title prefix gave a song with no title the words "Untitled song" after the prefix. It gets the
  prefix alone now.
- Premier upload could wait forever when Suno kept saying to slow down, or keep retrying the same part
  without end. It stops and says so now.
- Some Premier upload messages were only in English.
- The Linux and Mac notes still said to download Tin Pan from GitHub's build pages; it's on the Releases page.

## 2.4.0

- Create window: Takes makes 1, 2, 3 or 5 takes at once (two songs each), and Vary across takes moves the
  weirdness or the style influence from low to high across them.
- Create window and the Cover, Extend, Remaster and Use as inspiration forms: Lyrics obfuscation swaps the
  lyrics' letters for lookalike ones or adds invisible characters, for lyrics Suno's filter turns down. It's
  off unless you choose it.
- Upload a song offers Premier upload on the Premier plan: the file goes up in 5-second parts, Suno Studio
  joins them into one song, and the parts go to the trash. A part Suno refuses is sent again in smaller
  pieces, and a 1-second piece it still refuses is left out.
- Library menu, Browse community styles: the styles most used in Suno's songs for a word you type.
- Library menu, Export catalog to CSV or to JSON: the songs in the list shown, saved to a file.
- Song menu, Song tempo (BPM), from Suno's beat analysis.
- Song menu, Batch edit marked songs: a title prefix, more styles, or public or private, for all the marked
  songs at once.
- Song menu, Check Suno watermark asks Suno whether a downloaded file carries its content credentials, and
  Strip Suno watermark saves a copy of the file without them.

### Fixed

- A download whose file turned out to be an MP3, WAV, FLAC or Ogg was still saved with an .m4a name. It's
  named after what it is now.
- When moving many songs to the trash, or deleting them for good, failed partway, the songs Suno had already
  taken stayed in the list as if nothing had happened. They leave the list now, and the failure is said.

## 2.3.1

### Fixed

- Discover, Following didn't load: Suno stopped accepting how Tin Pan asked for the newest songs first.
- When the message about Tin Pan was open at start, the update check that should have run then was skipped
  until 6 hours later. It now runs once the message is closed.
- Closing Tin Pan, or installing an update, wrote "Unexpected error" to the log as Tin Pan shut down.
- A song file that came unencrypted, or as WAV, FLAC or Ogg, was taken for a broken file and the song played
  through Suno's player instead. Some songs still go to Suno's player because their file doesn't decode, and
  why isn't known yet; those now leave a line in the log that says what Suno sent.
- When Suno says to wait more than 10 seconds before trying again, Tin Pan no longer asks again at once (only
  to be refused again) and says to wait a minute.
- Shift+Up and Shift+Down could mark a contest that has no song, and count it among the marked songs.
- Report a problem could still close Tin Pan with some rare mixes of letters in the log.

## 2.3.0

- Tin Pan now also checks for a new version every 6 hours while it's open, not only when it starts. It says
  nothing unless there is one, and it waits to ask until Tin Pan is the window in front and no song is starting,
  no download is running and no other Tin Pan window is open. Saying No means it isn't offered again by itself.
  Settings, Check for new versions when Tin Pan starts, turns both checks off.
- When it starts, Tin Pan looks for a message from Ernesto about Tin Pan, such as a problem with Suno. A new
  one is shown once, in a window titled "Message about Tin Pan", when Tin Pan is the window in front and
  nothing is loading. Escape or Close closes it and the keyboard goes back to where it was.
- Library menu, Make speech: Suno reads words you type in a voice, with music under them or not, in a tone
  you describe and a male or female voice if you choose one.
- Library menu, Make sounds: a sound effect, sample or loop from a description, one-shot or loop, with a tempo
  and key if you want them. Both forms stay open like Cover, and the results join your songs.
- Discover menu, Listen history: the songs you played on Suno, as a list. Your hooks and Hooks you liked open
  the songs in your Hooks the same way.
- Library menu, Playlists in the trash: Enter on one brings it back.
- Account menu, Listen for credits stats: your ratings and credits earned, your streak and the next streak
  reward, and today's leaderboard.

## 2.2.0

- You can mark several songs in a list and act on them at once. Ctrl+Space marks or unmarks the selected song,
  Shift+Up and Shift+Down mark songs as you move, and Ctrl+A marks the whole list; marked songs say "marked".
  With songs marked, Delete moves them all to the trash (asking first), or out of a playlist or Up next;
  Shift+Delete, Add to playlist, Add to queue, Play next and the MP3, WAV and Download commands work on all
  of them too. Escape clears the marks. Other commands still act on the selected song.

## 2.1.2

### Fixed

- Listen for Credits always said Suno had no songs to rate. Suno now sends its tasks in a newer form, which
  Tin Pan skipped; it reads both forms now. Questions can be one choice, several choices or typed words, some
  with an Other answer, and a question that only applies after a certain answer stays greyed out until then.
  Tasks of kinds Tin Pan still can't show, such as making songs or video, are handed back to Suno.
- A free account's plan said "Plan unknown, not active". It says "Free plan" now.

## 2.1.1

- Settings no longer gives every field its own Alt+letter. Tab and Shift+Tab move through it as before.
- Custom models other people shared with you show up in the Create window's model list, named as shared.
- When Suno's sign-in service is busy, Tin Pan says how long to wait. When Suno no longer accepts a sign-in,
  Tin Pan says so and asks you to sign in again.

### Fixed

- Report a problem could close Tin Pan when the log held certain letters, such as the Turkish İ.
- The Donate window from the update offer could lose its place behind the offer; it opens over Tin Pan now.
- Sending a rating too early in Listen for Credits now moves to the song still to be heard, not always song A.
- Make a voice with no recording chosen now says to choose one, and a voice isn't made before Suno has
  finished taking the vocals out of the singing.

## 2.1.0

- Help, Contact the author lists Ernesto's email, Mastodon, the Telegram channel for news and updates, and
  Tin Pan's GitHub page. Enter opens the one you choose.
- Help, Donate lists ways to support Tin Pan (PayPal and Ko-fi). The update offer has a Donate button too.
- The update offer's text, with what's new, can now be read line by line.
- Search Suno (Ctrl+Shift+E, in the Discover menu) finds songs from everyone on Suno, not only yours.
- Up next: Ctrl+Shift+A queues the selected song to play after the current one, and Play next in the Song menu
  puts it first. Up next is a list in the Discover tab; Delete there takes a song out. When it's empty, the list
  you were playing carries on.
- Settings, "When a list ends, keep playing songs Suno picks": Suno's own autoplay, off unless you turn it on.
- Account, Listen for credits: rate songs for Suno and earn credits, in a window that plays each pair and asks
  Suno's questions about them.
- Library, Make a voice from your singing: a private voice persona from a recording of you singing, checked
  with a short phrase you record.

## 2.0.3

- Open a song someone shared with you: Ctrl+G, or Open a Suno link in the Discover menu. Paste a suno.com link,
  or a message with one in it. A link you've copied is already filled in. The song opens as a list in the
  Discover tab, ready to play.

## 2.0.2

Tin Pan now has test builds for Mac (TinPan.dmg, Apple silicon) and Linux (TinPan.AppImage), with speech
through VoiceOver and Orca. They haven't been tried with a screen reader yet, so please report what you find
with Ctrl+Shift+B (Cmd+Shift+B on a Mac); reports say which system they came from. README-macOS.md and
README-Linux.md explain how to install and use them and how the keys differ on a Mac.

### Fixed

- After signing out, the next start could sign you back in. It doesn't now.
- An unrelated failure, such as a like that didn't go through, could stop the next song from playing.
- After a search or signing out, songs from the old list could still turn up in the new one.
- Signing out now also forgets the account's personas, followed songs, Not interested marks and live lyrics.
- Fetching a song, lyrics, stems and a creator's songs no longer hold up likes and paging.
- Ctrl+Up and Ctrl+Down in a playlist move the song at once, and quick presses move the right songs.
- Deleting a playlist above the one you're in no longer jumps to another playlist.

## 2.0.1

### Fixed

- Enter in the all-commands list (Ctrl+F1) and Tin Pan's other lists to choose from (audio outputs, style
  ideas, lyrics drafts, personas, custom models, credits, a creator's playlists) didn't run the chosen line; it
  did nothing or closed the list. It runs it now, as in Tin Pan 1.
- Enter and Escape in the Lists box (Library and Discover) didn't reach it, so Enter didn't open a list. They
  work now, as do Delete and the Applications key there.
- Escape in the search box clears the search when there is one; otherwise it steps back as anywhere else.
- Enter in a field of Extend, Cover, Remaster and the other forms that stay open starts it, as in Tin Pan 1.
- A contest without a song stays in the Contests list; Enter on it says it has no song.
- If Suno ends your sign-in while Tin Pan is open, Tin Pan asks you to sign in again straight away.
- An unexpected error no longer closes Tin Pan: it says "Something went wrong" and writes the details to the log.
- Without the Microsoft Edge WebView2 Runtime, signing in, Suno's captcha and Suno's player say so instead of
  failing silently.
- Tin Pan's hidden Suno player could show up in Alt+Tab as "Suno player". It's out of Alt+Tab now and can't take
  the focus, and so is the Suno check window until it needs you to press its check box.

## 2.0.0

Tin Pan 2 is Tin Pan rebuilt from the ground up, so it can come to macOS and Linux too. This release is for
Windows. Your settings and sign-in carry over. Most of what Tin Pan 1 did is here; the rest is coming in
updates.

What's different:

- The tabs are Suno's sections: Library (Ctrl+1), Discover (Ctrl+2), Account (Ctrl+4) and Settings (Ctrl+5).
  Library and Discover each have a Lists box on the left (your songs and each playlist; Trending, Explore,
  Contests and more) and the chosen list's songs on the right. Escape goes from the songs to the lists.
- Playlists are in the Library's Lists box instead of their own tab.
- Lists you open from a song (similar songs, the creator's songs, covers and remixes) open in Discover's Lists
  box instead of their own windows. Delete there closes one.
- Weirdness, style influence and the other percentages are sliders.
- Only one Tin Pan runs at a time; starting it again brings the open one forward.
- About says which screen reader speech goes through.

Fixed along the way: Stop now also calls off a song that's still starting, and Alt+F4 closes Tin Pan even when
Windows has put the focus on its hidden Suno player.

## 2.0.0-alpha.2

### Fixed

- After Suno's captcha, you're back in the window you were in (Create or a form), not always the main window.
- The update offer after startup waits while the sign-in window is open, so it can't land on top of it.

## 2.0.0-alpha.1

The first test build of Tin Pan 2, rebuilt so the same app can run on Windows, macOS and Linux. It uses the
same settings and sign-in as Tin Pan 1, and it updates itself the same way; Tin Pan 1 can update to it once a
2.0 release is out. Not everything from Tin Pan 1 is here yet.

- Tabs for Suno's sections: Library (your songs and playlists) and Discover (Trending, From people you follow,
  Explore and Contests), each with a list of lists on the left and its songs on the right. Account (Ctrl+4) and
  Settings (Ctrl+5) are tabs too.
- Songs play in Tin Pan, start at once when fetched ahead, and carry on through the list they started from.
- Make songs in a window that stays open (Ctrl+N), with Suno's captcha passed for you; you hear when they're ready.
- Make from it: Extend, Cover, Use as inspiration, Remaster, Get whole song, Stems, Change speed, Reverse, Crop,
  Fade, Make a video and Make a persona, in forms that stay open. Percentages are sliders.
- Similar songs, the creator's songs, covers and remixes, the original song, and comments.
- Playlists: make, rename, delete, add to, remove from and reorder.
- Downloads with the clean-up, filters, search, likes, sharing and the rest of the Song menu.
- Choose the audio output and a second one to switch to with Ctrl+Shift+F12.
- Choose how much Tin Pan says, in Settings.
