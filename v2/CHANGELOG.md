# Tin Pan 2 changes

## Unreleased

### Fixed

- Tin Pan's hidden Suno player could show up in Alt+Tab as "Suno player". It's out of Alt+Tab now and can't take
  the focus, and so is the Suno check window until it needs you to press its check box.

## 2.0.0

Tin Pan 2 is Tin Pan rebuilt from the ground up, so it can come to macOS and Linux too. This release is for
Windows. Your settings and sign-in carry over, and everything Tin Pan 1 did is here.

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
