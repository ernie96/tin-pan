# Tin Pan 2

Tin Pan 2 is Tin Pan rebuilt in Rust on wxWidgets, so one app runs on Windows, macOS and Linux with each
system's own, screen-reader-friendly controls. On Windows it's the released Tin Pan, from 2.0.0 on, and Tin
Pan 1 (the `TinPan` folder) updates to it by itself. It uses Tin Pan 1's settings, saved sign-in and
translations (English, Romanian, Ukrainian and Spanish).

- macOS (Apple silicon, with VoiceOver): an early test build. See [README-macOS.md](README-macOS.md), which
  also covers how the keys differ on a Mac.
- Linux (x86_64 AppImage, with Orca): an early test build. See [README-Linux.md](README-Linux.md).

Press F1 anywhere in Tin Pan for help where you are, and Ctrl+F1 (Cmd+F1 on a Mac) for every command; its
first item is the full key guide. [CHANGELOG.md](CHANGELOG.md) says what changed in each version, and
[PORTING.md](PORTING.md) lists what Tin Pan 1 does and where 2.0 has it.

## Building on Windows

You need Rust, Visual Studio Build Tools with "Desktop development with C++", and LLVM from llvm.org.
Visual Studio's own CMake and Ninja are used. Then, in PowerShell:

```powershell
cd v2
. .\env.ps1
cargo build --release -p tinpan
```

The first build compiles wxWidgets and takes a while. The app is `target\release\tinpan.exe`. Speaking from
the background needs the Visual Studio component "C++ ATL for latest build tools"; without it, add
`--no-default-features`. Building for macOS and Linux is in their own READMEs.

## Layout

- `crates/tinpan-core`: Suno's API and sign-in, song files, settings, updates and translations, with no window.
- `crates/tinpan`: the windows, speech, playback and the network thread.
- `xtask`: packaging (`cargo xtask macos` makes the macOS app and disk image).
- `packaging/linux`: the AppImage's desktop file, icon and build script.
