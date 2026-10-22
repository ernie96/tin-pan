# Tin Pan

An accessible, unofficial Windows client for [Suno](https://suno.com), made for screen readers (NVDA, JAWS, Narrator) and braille. Browse and play your library and playlists, make songs, extend, cover and remaster them, and download them, all from the keyboard. Not affiliated with Suno.

Tin Pan, like all my apps, is dedicated to Jenni and Diana.

## Get it

Download `TinPan.exe` from the [latest release](https://github.com/ernie96/tin-pan/releases/latest) and run it. It is one file and needs nothing installed (it uses the WebView2 that comes with Windows 10 and 11). Press F1 anywhere for help, Alt for the menu bar, and Ctrl+F1 for every command.

The full guide is in [TinPan/README.md](TinPan/README.md), and what changed is in [TinPan/CHANGELOG.md](TinPan/CHANGELOG.md).

## Build

Needs the .NET 10 SDK.

```
dotnet build TinPan -c Release
dotnet test TinPan.Tests
dotnet publish TinPan -p:PublishProfile=OneFile
```

The last one makes the single `TinPan\bin\publish\TinPan.exe`.

## Licenses

See [TinPan/THIRD_PARTY_NOTICES.md](TinPan/THIRD_PARTY_NOTICES.md) for the dedication and the components Tin Pan ships with. `deps/nvda` holds NV Access's NVDA Controller Client (LGPL 2.1) with its license.
