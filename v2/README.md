# Tin Pan 2 (in progress)

Tin Pan 2 is Tin Pan rebuilt in Rust on wxWidgets, so one app runs on Windows, macOS and Linux with each system's own, screen-reader-friendly controls. Tin Pan 1 (the `TinPan` folder) stays the app to use until 2.0 can do what it does.

## What works so far

- Signing in, on Windows: Tin Pan menu, Sign in. It opens Suno's own sign-in page, which closes by itself once you're in. Sign out is in the same menu. On the same computer it also uses Tin Pan 1's settings and saved sign-in, and Tin Pan 1's sign-in browser, so a Google sign-in saved there is remembered. On macOS and Linux it can't sign in yet.
- The Library: the list of your songs, with more loading as you reach the end. Ctrl+F searches your songs; an empty search shows them all again.
- Playing songs in the app, with Tin Pan 1's keys:
  - Enter plays, and Space pauses.
  - F7 and F9: previous and next song. F8: play or pause. Shift+F8: stop.
  - Left and Right in the list: back and forward 5 seconds. Shift+F7 and Shift+F9: 10 seconds. Ctrl+F7 and Ctrl+F9: 10 percent. Ctrl+F8: back to the start.
  - F11 and F12: volume. F3: what's playing. F5: refresh. F1: the keys.

[PORTING.md](PORTING.md) lists everything Tin Pan 1 does and what 2.0 has of it so far.
- Translations: it reads Tin Pan 1's translations, so Romanian, Ukrainian and Spanish come along.

## Building on Windows

You need:

- Rust;
- Visual Studio Build Tools with "Desktop development with C++";
- LLVM, from llvm.org.

Visual Studio's own CMake and Ninja are used. Then, in PowerShell:

```powershell
cd v2
. .\env.ps1
cargo build --release
```

The first build compiles wxWidgets and takes a while. The app is `target\release\tinpan.exe`.

### Speaking from the background

Tin Pan speaks through the screen reader even when it isn't the window in front. On Windows, building that part needs the Visual Studio component "C++ ATL for latest build tools". Without that component, build without it:

```powershell
cargo build --release --no-default-features
```

## Layout

- `crates/tinpan-core`: Suno's API and sign-in, song files, settings and translations, with no window. `cargo test -p tinpan-core` tests it.
- `crates/tinpan`: the window, speech, playback and the network thread.
