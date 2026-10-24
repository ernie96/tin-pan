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

## License

Copyright (C) 2026 Ernesto Melendez. Tin Pan is free software under the [GNU General Public License, version 3 or later](LICENSE), with additional terms under section 7 in [NOTICE.md](NOTICE.md): copies and changed versions must keep the author's credit and the dedication visible in their About or credits screen, and changed versions must be marked as changed. Those terms don't limit anything else the GPL lets you do.

See [TinPan/THIRD_PARTY_NOTICES.md](TinPan/THIRD_PARTY_NOTICES.md) for the dedication and the components Tin Pan ships with. `deps/nvda` holds NV Access's NVDA Controller Client (LGPL 2.1) with its license.
