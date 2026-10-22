# Tin Pan: notes for coding agents

- The app is `TinPan/` (C# WPF, .NET 10); tests are in `TinPan.Tests/`. User guide `TinPan/README.md`, history `TinPan/CHANGELOG.md`.
- It is screen-reader first: every feature needs F1 help, a menu bar entry (built from `MainViewModel.Commands`), unique access keys per page/menu, and quiet, one-line speech.
- Build in place after every change: `dotnet build TinPan -c Release` (exe at `TinPan/bin/Release/net10.0-windows/TinPan.exe`). Release build: `dotnet publish TinPan -p:PublishProfile=OneFile` (one exe in `TinPan/bin/publish/`).
- Every user-facing text goes through `T("...")`, `Plural(n, "{0} x", "{0} xs")` or `{l:T '...'}` in XAML (`TinPan/Localization`). Add its Romanian, Ukrainian and Spanish to `TinPan/Localization/translations.json` in the same change, then run `python -I build/translations/check.py` (missing texts, placeholders, plural counts, Alt-letter clashes per page/menu/form in every language). Logs, Suno API values and the license texts stay English.
- Keep working behaviour (downloads, sign-in, playback) unless asked to change it; add no unrequested prompts or gates.
- Suno API notes and references: the "API references" section of `TinPan/README.md`. Reference projects: Nsomnia/chadvis-projectm-qt (docs/suno_api), Nsomnia/suno-power-exporter, Nsomnia/suno-api-local-manager-frontend, paperfoot/suno-cli, gcui-art/suno-api.
- Never delete files with rm; add a date-time to the name and move them into `.backup_graveyard/` (git-ignored by convention, not committed).
- Releases: bump `<Version>` in `TinPan/TinPan.csproj`, add a matching `## x.y.z` changelog section, push tag `vx.y.z`; `.github/workflows/release.yml` builds, signs (Ed25519, `UPDATE_SIGNING_KEY` secret) and publishes TinPan.exe + .sig, which the in-app updater (`Services/UpdateService.cs`) installs. The private key file is `%APPDATA%\TinPan-release\update-signing.key`; never commit it, never replace the key pair (installed copies would stop trusting updates).
- Commit, tag, push or release only when asked. No AI attribution in commits, PRs or releases.
