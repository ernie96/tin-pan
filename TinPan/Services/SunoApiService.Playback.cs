using System;
using System.Net.Http;
using System.Threading;
using System.Threading.Tasks;
using static TinPan.Localization.Loc;

namespace TinPan.Services
{
    // The song file for playing it in Tin Pan itself, the way Nsomnia/chadvis-projectm-qt plays its library
    // (src/suno/MangoLicense.cpp, MangoCrypto.cpp): Suno's content-rights call gives the key for the song's
    // progressive file, which decrypts to an ordinary M4A. It's the same route as the free downloads, so it
    // uses none of the account's downloads, and it skips loading Suno's whole song page.
    public partial class SunoApiService
    {
        public async Task<byte[]> GetSongAudioAsync(string clipId, CancellationToken ct = default)
        {
            var token = await _auth.GetTokenAsync(forceRefresh: false, ct);
            var rights = await FetchRightsAsync(clipId, token, ct);
            var url = !string.IsNullOrEmpty(rights.MediaUrl) && !SunoDrm.IsForbiddenDecoy(rights.MediaUrl)
                ? rights.MediaUrl
                : SunoDrm.DefaultMediaUrl(clipId);
            var (key, iv) = SunoDrm.UnwrapRights(rights, clipId, token);
            try
            {
                var body = await _http.GetByteArrayAsync(url, ct);
                var plain = SunoDrm.DecryptAesCtrBuffer(body, key, iv);
                if (SunoDrm.DetectContainer(plain.AsSpan(0, Math.Min(32, plain.Length))) == "unknown")
                    throw new SunoApiException(T("Suno's song file didn't decode."));
                return plain;
            }
            catch (HttpRequestException ex)
            {
                throw new SunoApiException(T("Can't reach Suno. Check your internet connection."), (int?)ex.StatusCode, ex);
            }
        }
    }
}
