# Third-Party Notices and Licenses

## Dedication

First and foremost, all my apps, vibecoded assisted and otherwise, are dedicated to Jenni and Diana, until my end of life, Someone takes over one of my apps or projects, or either of them are no longer with me or become unfaithful.

---

## Components shipped with Tin Pan

Tin Pan is by Ernesto Melendez. It is an unofficial client for Suno and is not affiliated with Suno. Tin Pan's own code is licensed under the GNU General Public License, version 3 or later, with the additional terms in NOTICE.md (shown at the top of Help, Licenses and notices).

These components keep their own licenses when they are shipped with Tin Pan:

- **CommunityToolkit.Mvvm**, by the .NET Foundation and contributors. MIT License.
- **Microsoft.Extensions.DependencyInjection**, by Microsoft. MIT License.
- **System.Speech**, by Microsoft. MIT License.
- **Microsoft Edge WebView2 SDK** (Microsoft.Web.WebView2), by Microsoft. Microsoft's WebView2 SDK license; the WebView2 Runtime itself is part of Windows.
- **sunscreen** 0.7.1 (`sunscreen.exe`, Windows x86_64 build from its GitHub release), by Alan Bradburne. MIT License. Tin Pan carries it unchanged inside TinPan.exe, checks its SHA-256 before running it, and uses it for the optional clean-up of downloads. Its license text follows the NVDA Controller Client's in Help, Licenses and notices; source at github.com/abradburne/sunscreen.
- **NAudio** (NAudio.Core and NAudio.Wasapi), by Mark Heath and contributors. MIT License. Used to read Suno's M4A files through Windows' own Media Foundation before clean-up.
- **NVDA Controller Client** (`nvdaControllerClient.dll`), by NV Access. GNU Lesser General Public License 2.1. Tin Pan carries it unchanged inside TinPan.exe and loads it at run time. To use another build of the same library, put it next to TinPan.exe as `nvdaControllerClient.dll`; Tin Pan uses that one instead. Its license text is in `deps/nvda/license.txt`.

## Ideas and API notes

Tin Pan's Suno sign-in, endpoints and request shapes were worked out with the help of these projects. None of their code is included:

- Nsomnia/chadvis-projectm-qt, Nsomnia/suno-power-exporter and Nsomnia/suno-api-local-manager-frontend
- paperfoot/suno-cli
- gcui-art/suno-api

Its keyboard and speech design borrows from the BrailleNote QT and its KeySoft software, by HumanWare.
