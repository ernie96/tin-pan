# Tin Pan 2 changes

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
