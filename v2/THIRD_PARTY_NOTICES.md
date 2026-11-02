# Third-Party Notices and Licenses for Tin Pan 2

## Dedication

First and foremost, all my apps, vibecoded assisted and otherwise, are dedicated to Jenni and Diana, until my end of life, Someone takes over one of my apps or projects, or either of them are no longer with me or become unfaithful.

---

## Components built into Tin Pan 2

Tin Pan is by Ernesto Melendez. It is an unofficial client for Suno and is not affiliated with Suno. Tin Pan's own code is licensed under the GNU General Public License, version 3 or later, with the additional terms in NOTICE.md (shown at the top of Help, Licenses and notices).

These components keep their own licenses inside TinPan.exe:

- **wxWidgets**, by Julian Smart, Vadim Zeitlin and the wxWidgets team. wxWindows Library Licence 3.1, which allows it to be built into Tin Pan.
- **wxDragon**, Rust bindings for wxWidgets, by the wxDragon contributors and trypsynth. MIT or Apache License 2.0.
- **quiet-list**, **live-region** and **prismer**, by trypsynth. MIT License. The list Tin Pan's songs are shown in, the hidden text that carries Tin Pan's messages to the screen reader, and speech through the screen reader when Tin Pan is in the background.
- **rodio** and **cpal**, by the RustAudio contributors. MIT or Apache License 2.0, and Apache License 2.0. Playing songs and choosing the audio output.
- **Symphonia**, by Philip Deljanov and contributors. Mozilla Public License 2.0; its source is at github.com/pdeljanov/Symphonia. Reading Suno's M4A, AAC and MP3 files.
- **libopus**, by Xiph.Org and contributors, through symphonia-adapter-libopus and opusic-sys. BSD 3-Clause License, and MIT or Apache License 2.0. Decoding Opus.
- **hound**, by Ruud van Asseldonk. Apache License 2.0. Writing the WAV a song is decoded to before clean-up.
- **reqwest**, **rustls** and **ring**, by their contributors. MIT or Apache License 2.0, and ISC-style licenses. Talking to Suno and GitHub over HTTPS.
- **ed25519-dalek**, by the dalek-cryptography contributors. BSD 3-Clause License. Checking the signature of updates.
- **webview2-com** and **windows**, by Bill Avery and Microsoft. MIT License, and MIT or Apache License 2.0. Suno's sign-in page and its captcha, and Windows' own functions.
- **Microsoft Edge WebView2**, by Microsoft. Its WebView2 SDK license; the WebView2 Runtime itself is part of Windows.
- **sunscreen** 0.7.1 (`sunscreen.exe`, Windows x86_64 build from its GitHub release), by Alan Bradburne. MIT License. Tin Pan carries it unchanged inside TinPan.exe, checks its SHA-256 before running it, and uses it for the optional clean-up of downloads. Its license text follows these notices in Help, Licenses and notices; source at github.com/abradburne/sunscreen.
- Other Rust crates, among them serde, chrono, sha2, aes and log, under the MIT, Apache 2.0, BSD and similar permissive licenses. v2/Cargo.lock lists every one.

## Ideas and API notes

Tin Pan's Suno sign-in, endpoints and request shapes were worked out with the help of these projects. None of their code is included:

- Nsomnia/chadvis-projectm-qt, Nsomnia/suno-power-exporter and Nsomnia/suno-api-local-manager-frontend
- paperfoot/suno-cli
- gcui-art/suno-api

Tin Pan 2's window and lists follow trypsynth/fedra, and its updates keep a few touches of trypsynth/ship-shape. Its keyboard and speech design borrows from the BrailleNote QT and its KeySoft software, by HumanWare.
