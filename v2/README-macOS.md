# Tin Pan on macOS

Tin Pan for macOS is an early test version. It's built on GitHub for Macs with Apple silicon (M1 or newer),
running macOS 13 or later. Nobody has tried it with VoiceOver yet, so expect rough edges, and please report
what you find: Cmd+Shift+B reports a problem and Cmd+Shift+F suggests a feature (both in the Help menu too).
Reports go privately to Tin Pan's author and say they come from a Mac, which macOS version, and that speech
goes through VoiceOver; a problem report can include Tin Pan's log, with your user name taken out.

## Getting it

There's no macOS download on the releases page yet. Each test build is kept for 30 days on GitHub:

1. Open the repository's Actions page and choose the newest run of "Build 2.x for macOS" that passed.
2. Under Artifacts, download "TinPan-macOS". You need to be signed in to GitHub for this.
3. Unzip it, open TinPan.dmg, and drag Tin Pan to your Applications folder.

Tin Pan isn't signed with an Apple developer certificate yet, so macOS refuses to open it the first time:

1. Try to open Tin Pan once, and close the message macOS shows.
2. Go to System Settings, Privacy & Security, and choose "Open Anyway" next to the line about Tin Pan.
3. Open Tin Pan again and confirm.

## Before you start: keyboard navigation

Turn on keyboard navigation so Tab reaches every button and list: System Settings, Keyboard, then turn on
"Keyboard navigation". Without it, Tab skips Tin Pan's buttons and lists.

Most of Tin Pan's keys are function keys. Either press Fn with them (Fn+F8 to play or pause), or turn on
System Settings, Keyboard, Keyboard Shortcuts, Function Keys, "Use F1, F2, etc. keys as standard function keys".

## Keys: what's different from Windows

Tin Pan's help and menus are written with Windows keys. On a Mac:

- **Ctrl becomes Cmd.** Wherever help says Ctrl, press Cmd: Cmd+1 for the Library, Cmd+N for Create,
  Cmd+F1 for all commands, Cmd+Shift+U to like a song. The menus show the Mac keys.
- **Alt becomes Option.** Option+Return for song information, Option+F for library filters, Option+L for
  Download all.
- **Delete is Cmd+Delete.** A Mac's Delete key is Backspace. Cmd+Delete moves a song to the trash, removes it
  from a playlist, or closes a list in the Lists box, as Delete does on Windows. Fn+Delete (forward delete)
  works too.
- **Quit is Cmd+Q**, not Alt+F4.
- **The Applications key** (the menu for a song) doesn't exist on a Mac. Try VoiceOver's VO+Shift+M, or
  Fn+Shift+F10. Every item in it is also in the menu bar.
- **The menu bar** is at the top of the screen, as in every Mac app: VO+M, or Ctrl+F2, gets there.

Some keys clash with macOS's own:

- **F11** is "Show Desktop" in macOS. To use F11 for volume down, turn off that shortcut in System Settings,
  Keyboard, Keyboard Shortcuts, Mission Control.
- **Cmd+M** (similar songs) may minimize the window instead, since it's every Mac app's Minimize. Similar songs
  is also in the Song menu, under Related songs.
- **F7, F8 and F9 without Fn** are the Mac's own media keys. They control whatever app macOS thinks is
  playing, which may not be Tin Pan.

## Speech

While Tin Pan is the app in front, it speaks through VoiceOver announcements. When you switch away, it still
speaks through VoiceOver, for example when a song you're making is ready. "Tin Pan speaks", in Settings,
decides how much.

## Signing in

Settings, Sign in, opens Suno's sign-in page in a Tin Pan window, which closes by itself once you're in.
Your sign-in is saved in the macOS Keychain. The first time, macOS asks whether Tin Pan may use it; choose
Always Allow. Because Tin Pan isn't signed yet, macOS may ask again after each new version.

Signing in with Google may be refused ("this browser or app may not be secure"). If it is, please report it.
Signing in with an email code, or another account type, should work.

## What's not there yet on macOS

- **Updates:** Tin Pan can't update itself on macOS yet. Help, Check for updates says when there's a new
  version, and the new build comes from GitHub as above.
- **WMA uploads:** WMA files can't be uploaded on a Mac. MP3, M4A, WAV, FLAC, OGG and AAC can.
- **Macs with Intel processors** aren't built yet.

Clean-up (Settings, Clean-up) is included and works as on Windows.

## Building it yourself

You need Xcode's command line tools (`xcode-select --install`), Homebrew, and Rust from rustup.rs. Then:

```sh
brew install cmake ninja
cd v2
cargo build --release -p tinpan
cargo xtask macos
```

The first build compiles wxWidgets and takes a while. The app ends up in `target/release/Tin Pan.app`, and
the disk image in `target/release/TinPan.dmg`. Clean-up only works if the matching sunscreen file is put in
`target/release` before `cargo xtask macos`, as `.github/workflows/build-macos.yml` does.
