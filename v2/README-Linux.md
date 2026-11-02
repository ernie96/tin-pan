# Tin Pan on Linux

Tin Pan for Linux is an early test version, built on GitHub as an AppImage for 64-bit PCs (x86_64). Nobody
has tried it with Orca yet, so expect rough edges, and please report what you find (Help menu, Report a
problem).

## What your system needs

Tin Pan uses your desktop's own GTK 3 and web engine, which most desktops that run Orca already have. On
Ubuntu or Debian these are:

```sh
sudo apt install libgtk-3-0 libwebkit2gtk-4.1-0 libglibmm-2.68-1 libfuse2t64
```

On older Ubuntu (22.04), the last one is `libfuse2`. Other distributions have the same libraries under their
own names: GTK 3, WebKitGTK 4.1, glibmm 2.68 and FUSE 2 (for AppImages).

You also need a keyring for the saved sign-in: GNOME Keyring, or KWallet with its Secret Service turned on.
Without one, Tin Pan works, but you sign in each time you start it.

## Getting it

There's no Linux download on the releases page yet. Each test build is kept for 30 days on GitHub:

1. Open the repository's Actions page and choose the newest run of "Build 2.x for Linux" that passed.
2. Under Artifacts, download "TinPan-Linux". You need to be signed in to GitHub for this.
3. Unzip it, then make TinPan.AppImage runnable and start it:

```sh
chmod +x TinPan.AppImage
./TinPan.AppImage
```

## Keys

The keys are the same as on Windows: Ctrl+1 for the Library, F8 to play or pause, the Applications key or
Shift+F10 for a song's menu, F10 for the menu bar (Alt on its own doesn't open it on Linux), Ctrl+F1 for all commands, and so on.

A few desktops take some keys for themselves. If one doesn't reach Tin Pan, its command is also in the menu
bar, or you can free the key in your desktop's keyboard settings. Keys to watch for:

- Alt+F4 is your desktop's own "close window", which closes Tin Pan, as it should.
- F11 is "full screen" in some desktops' window managers.
- Ctrl+Alt with arrow keys, or with function keys, switch workspaces or consoles. Tin Pan doesn't use them.

## Speech

While Tin Pan is the active window, it speaks through Orca, as announcements Orca reads in its own voice.
When you switch away, it still speaks:
- with a recent Orca, through Orca itself;
- otherwise straight through speech-dispatcher, which may use a different voice from Orca's.

"Tin Pan speaks", in Settings, decides how much.

## Signing in

Settings, Sign in, opens Suno's sign-in page in a Tin Pan window, which closes by itself once you're in. The
sign-in is saved in your keyring.

Signing in with Google may be refused ("this browser or app may not be secure"). If it is, please report it.
Signing in with an email code, or another account type, should work.

## What's not there yet on Linux

- **Updates:** Tin Pan updates itself when it runs as an AppImage, but the releases don't include the
  AppImage yet. Until they do, new builds come from GitHub as above.
- **WMA uploads:** WMA files can't be uploaded on Linux. MP3, M4A, WAV, FLAC, OGG and AAC can.
- **ARM computers** (a Raspberry Pi, say) aren't built yet.

Clean-up (Settings, Clean-up) is included in the AppImage and works as on Windows.

## Building it yourself

On Ubuntu 24.04:

```sh
sudo apt install build-essential cmake ninja-build pkg-config clang libclang-dev libgtk-3-dev \
  libwebkit2gtk-4.1-dev libasound2-dev libsecret-1-dev libdbus-1-dev libglibmm-2.68-dev
cd v2
cargo build --release -p tinpan
```

Install Rust from rustup.rs first. The first build compiles wxWidgets and takes a while. The program is
`target/release/tinpan`. For an AppImage, see `packaging/linux/make-appimage.sh`, which
`.github/workflows/build-linux.yml` runs.
