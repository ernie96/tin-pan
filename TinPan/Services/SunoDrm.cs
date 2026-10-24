using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Numerics;
using System.Security.Cryptography;
using System.Text;
using System.Text.Json;
using System.Threading;
using System.Threading.Tasks;
using TinPan.Models;
using static TinPan.Localization.Loc;

namespace TinPan.Services
{
    public class SunoRights
    {
        public string Key { get; set; } = string.Empty;
        public string Iv { get; set; } = string.Empty;
        public string Glt { get; set; } = string.Empty;
        public string Aad { get; set; } = string.Empty;
        public string? MediaUrl { get; set; }
    }

    /// <summary>
    /// Handles unmetered audio extraction for Suno clips via direct progressive streaming
    /// or Suno's Mango DRM pipeline (POST /api/mango/rights + AES-GCM unwrap + AES-CTR decryption).
    /// Prevents falling into the audio_url /api/forbidden decoy trap and prevents consuming
    /// monthly download quotas for song files.
    /// Reference implementation: Nsomnia/suno-power-exporter.
    /// </summary>
    public static class SunoDrm
    {
        public const string DefaultCdnTemplate = "https://d2lwuy8qc234o3.cloudfront.net/1/clip/{0}.m4a";

        public static string DefaultMediaUrl(string clipId) =>
            string.Format(DefaultCdnTemplate, Uri.EscapeDataString(clipId));

        public static bool IsForbiddenDecoy(string? url)
        {
            if (string.IsNullOrWhiteSpace(url)) return true;
            return url.Contains("forbidden", StringComparison.OrdinalIgnoreCase);
        }

        /// <summary>
        /// Selects the best media URL for a clip.
        /// Unencrypted progressive URLs win when preferUnencrypted is true.
        /// Otherwise returns the encrypted stream URL from media_urls or CloudFront fallback.
        /// </summary>
        public static (string Url, bool IsEncrypted) PickMediaUrl(SunoClip? clip, bool preferUnencrypted = true)
        {
            if (clip == null) return (string.Empty, false);

            var mediaList = clip.MediaUrls;
            if (mediaList is { Count: > 0 })
            {
                if (preferUnencrypted)
                {
                    var unencrypted = mediaList.FirstOrDefault(m => !m.IsEncrypted && !m.IsVideo && !IsForbiddenDecoy(m.Url));
                    if (unencrypted?.Url != null) return (unencrypted.Url, false);
                }

                var encrypted = mediaList.FirstOrDefault(m => !m.IsVideo && !IsForbiddenDecoy(m.Url));
                if (encrypted?.Url != null) return (encrypted.Url, encrypted.IsEncrypted);
            }

            if (!IsForbiddenDecoy(clip.AudioUrl))
            {
                return (clip.AudioUrl!, false);
            }

            return (DefaultMediaUrl(clip.Id), true);
        }

        /// <summary>
        /// Decodes key material from hexadecimal, base64 or base64url strings.
        /// </summary>
        public static byte[] DecodeKeyMaterial(string? str)
        {
            if (string.IsNullOrWhiteSpace(str)) return Array.Empty<byte>();
            var clean = str.Trim();

            // 1. Hex
            if (clean.Length % 2 == 0 && !clean.Contains('=') && IsAllHex(clean))
            {
                try { return Convert.FromHexString(clean); } catch { }
            }

            // 2. Base64 / Base64Url
            try
            {
                var b64 = clean.Replace('-', '+').Replace('_', '/');
                var rem = b64.Length % 4;
                if (rem > 0) b64 = b64.PadRight(b64.Length + (4 - rem), '=');
                return Convert.FromBase64String(b64);
            }
            catch
            {
                return Array.Empty<byte>();
            }
        }

        private static bool IsAllHex(string s)
        {
            foreach (var c in s)
            {
                if (!((c >= '0' && c <= '9') || (c >= 'a' && c <= 'f') || (c >= 'A' && c <= 'F')))
                    return false;
            }
            return true;
        }

        /// <summary>
        /// 128-bit big-endian counter addition for AES-CTR.
        /// Increments the 16-byte initial counter by delta blocks (blocks, not bytes).
        /// </summary>
        public static void IncrementCounter(ReadOnlySpan<byte> initialIv, ulong blockIndex, Span<byte> output)
        {
            if (initialIv.Length != 16) throw new ArgumentException("Counter must be 16 bytes.", nameof(initialIv));
            if (output.Length < 16) throw new ArgumentException("Output buffer must be at least 16 bytes.", nameof(output));

            initialIv.CopyTo(output);
            if (blockIndex == 0) return;

            ulong remaining = blockIndex;
            for (int i = 15; i >= 0 && remaining > 0; i--)
            {
                ulong add = remaining & 0xFF;
                remaining >>= 8;
                ulong sum = (ulong)output[i] + add;
                output[i] = (byte)sum;
                if (sum > 0xFF)
                {
                    remaining++;
                }
            }
        }

        /// <summary>
        /// Unwraps an envelope under AES-GCM userKey (12 bytes nonce + ciphertext + 16 bytes tag).
        /// 16 or 32-byte raw inputs are returned as-is.
        /// </summary>
        public static byte[] UnwrapEnvelope(byte[] rawBytes, byte[] userKey, byte[] aadBytes)
        {
            if (rawBytes.Length is 16 or 32) return rawBytes;

            if (rawBytes.Length >= 28)
            {
                var nonce = rawBytes.AsSpan(0, 12);
                var ciphertext = rawBytes.AsSpan(12, rawBytes.Length - 28);
                var tag = rawBytes.AsSpan(rawBytes.Length - 16, 16);
                var plaintext = new byte[ciphertext.Length];

                using var aesGcm = new AesGcm(userKey, 16);
                aesGcm.Decrypt(nonce, ciphertext, tag, plaintext, aadBytes);
                return plaintext;
            }

            return rawBytes;
        }

        /// <summary>
        /// Parses the JSON response from POST /api/mango/rights.
        /// </summary>
        public static SunoRights? ParseRights(string json, string clipId)
        {
            JsonDocument doc;
            try { doc = JsonDocument.Parse(json); }
            catch (JsonException) { return null; } // an error page, not a rights reply
            using var _ = doc;
            var root = doc.RootElement;
            if (root.ValueKind != JsonValueKind.Object) return null;
            var data = root.TryGetProperty("data", out var d) && d.ValueKind == JsonValueKind.Object ? d : root;

            string? key = FirstString(data, "key", "content_key") ?? FirstString(root, "key", "content_key");
            string? iv = FirstString(data, "iv", "content_iv") ?? FirstString(root, "iv", "content_iv");

            if (string.IsNullOrEmpty(key) || string.IsNullOrEmpty(iv)) return null;

            string? glt = FirstString(data, "glt", "glt_token") ?? FirstString(root, "glt", "glt_token") ?? "";
            string? aad = FirstString(data, "aad") ?? FirstString(root, "aad") ?? clipId;
            string? mediaUrl = FirstString(data, "media_url", "mediaUrl", "url", "audio_url", "src")
                ?? FirstString(root, "media_url", "mediaUrl", "url", "audio_url", "src");

            if (mediaUrl != null && IsForbiddenDecoy(mediaUrl)) mediaUrl = null;

            return new SunoRights
            {
                Key = key,
                Iv = iv,
                Glt = glt,
                Aad = aad,
                MediaUrl = mediaUrl
            };
        }

        private static string? FirstString(JsonElement el, params string[] names)
        {
            foreach (var name in names)
            {
                if (el.TryGetProperty(name, out var prop) && prop.ValueKind == JsonValueKind.String)
                {
                    var val = prop.GetString();
                    if (!string.IsNullOrEmpty(val)) return val;
                }
            }
            return null;
        }

        /// <summary>
        /// Unwraps the contentKey and contentIv from a rights payload.
        /// Walks candidate user key seeds (bearer token, glt, empty string) until AES-GCM authentication succeeds.
        /// </summary>
        public static (byte[] ContentKey, byte[] ContentIv) UnwrapRights(SunoRights rights, string clipId, string? bearerToken)
        {
            var rawKey = DecodeKeyMaterial(rights.Key);
            var rawIv = DecodeKeyMaterial(rights.Iv);
            if (rawKey.Length == 0 || rawIv.Length == 0)
                throw new SunoApiException(T("Suno didn't send the key for this song's free file."));

            var aadText = string.IsNullOrEmpty(rights.Aad) ? clipId : rights.Aad;
            var aadBytes = Encoding.UTF8.GetBytes(aadText);

            var seeds = new List<string>();
            if (!string.IsNullOrEmpty(bearerToken)) seeds.Add(bearerToken);
            if (!string.IsNullOrEmpty(rights.Glt) && !seeds.Contains(rights.Glt)) seeds.Add(rights.Glt);
            if (seeds.Count == 0) seeds.Add(string.Empty);

            Exception? lastEx = null;
            foreach (var seed in seeds)
            {
                var userKey = SHA256.HashData(Encoding.UTF8.GetBytes(seed));
                try
                {
                    var contentKey = UnwrapEnvelope(rawKey, userKey, aadBytes);
                    var contentIv = UnwrapEnvelope(rawIv, userKey, aadBytes);

                    if (contentKey.Length is not (16 or 24 or 32))
                        throw new SunoApiException($"Unwrapped content key has invalid length ({contentKey.Length}).");
                    if (contentIv.Length != 16)
                        throw new SunoApiException($"Unwrapped counter IV has invalid length ({contentIv.Length}).");

                    return (contentKey, contentIv);
                }
                catch (Exception ex)
                {
                    lastEx = ex;
                }
            }

            throw new SunoApiException(T("Couldn't unlock this song's free file with your account."), null, lastEx);
        }

        /// <summary>
        /// Sniffs container format from the initial bytes of decrypted plaintext.
        /// Returns "m4a", "mp3" or "unknown".
        /// </summary>
        public static string DetectContainer(ReadOnlySpan<byte> header)
        {
            // ISO-BMFF: byte 4..7 is "ftyp" (0x66, 0x74, 0x79, 0x70)
            if (header.Length >= 8 &&
                header[4] == 0x66 && header[5] == 0x74 && header[6] == 0x79 && header[7] == 0x70)
                return "m4a";

            // MP3: ID3 tag at 0..2
            if (header.Length >= 3 &&
                header[0] == 0x49 && header[1] == 0x44 && header[2] == 0x33)
                return "mp3";

            // MP3: frame sync (0xFF 0xEx or 0xFx)
            if (header.Length >= 2 &&
                header[0] == 0xFF && (header[1] & 0xE0) == 0xE0)
                return "mp3";

            return "unknown";
        }

        /// <summary>
        /// Decrypts a ciphertext stream into a plaintext stream using AES-CTR with chunked processing
        /// and hardware-accelerated block transformation.
        /// </summary>
        public static async Task DecryptAesCtrAsync(Stream ciphertextStream, byte[] contentKey, byte[] contentIv, Stream plaintextStream, CancellationToken ct = default)
        {
            if (contentIv.Length != 16)
                throw new ArgumentException("Content IV must be exactly 16 bytes for AES-CTR.", nameof(contentIv));
            if (contentKey.Length is not (16 or 24 or 32))
                throw new ArgumentException("Content key must be 16, 24, or 32 bytes.", nameof(contentKey));

            using var aes = Aes.Create();
            aes.Mode = CipherMode.ECB;
            aes.Padding = PaddingMode.None;
            aes.Key = contentKey;
            using var encryptor = aes.CreateEncryptor();

            const int chunkSize = 65536; // 64 KiB = 4096 AES blocks
            var inBuffer = new byte[chunkSize];
            var counterBuffer = new byte[chunkSize];
            var keystreamBuffer = new byte[chunkSize];
            var outBuffer = new byte[chunkSize];

            ulong blockIndex = 0;
            int leftover = 0;

            while (true)
            {
                int read = await ciphertextStream.ReadAsync(inBuffer.AsMemory(leftover, chunkSize - leftover), ct);
                if (read == 0)
                {
                    if (leftover > 0)
                    {
                        var singleCounter = new byte[16];
                        IncrementCounter(contentIv, blockIndex, singleCounter);
                        encryptor.TransformBlock(singleCounter, 0, 16, keystreamBuffer, 0);

                        for (int i = 0; i < leftover; i++)
                        {
                            outBuffer[i] = (byte)(inBuffer[i] ^ keystreamBuffer[i]);
                        }
                        await plaintextStream.WriteAsync(outBuffer.AsMemory(0, leftover), ct);
                    }
                    break;
                }

                int totalAvailable = leftover + read;
                int fullBlocks = (totalAvailable / 16) * 16;

                if (fullBlocks > 0)
                {
                    int blockCount = fullBlocks / 16;
                    for (int b = 0; b < blockCount; b++)
                    {
                        IncrementCounter(contentIv, blockIndex + (ulong)b, counterBuffer.AsSpan(b * 16, 16));
                    }

                    encryptor.TransformBlock(counterBuffer, 0, fullBlocks, keystreamBuffer, 0);
                    XorBuffers(inBuffer.AsSpan(0, fullBlocks), keystreamBuffer.AsSpan(0, fullBlocks), outBuffer.AsSpan(0, fullBlocks));
                    await plaintextStream.WriteAsync(outBuffer.AsMemory(0, fullBlocks), ct);

                    blockIndex += (ulong)blockCount;
                }

                leftover = totalAvailable - fullBlocks;
                if (leftover > 0)
                {
                    Array.Copy(inBuffer, fullBlocks, inBuffer, 0, leftover);
                }
            }
        }

        /// <summary>
        /// In-memory helper to decrypt an entire byte buffer using AES-CTR.
        /// </summary>
        public static byte[] DecryptAesCtrBuffer(byte[] ciphertext, byte[] contentKey, byte[] contentIv)
        {
            using var inStream = new MemoryStream(ciphertext);
            using var outStream = new MemoryStream(ciphertext.Length);
            DecryptAesCtrAsync(inStream, contentKey, contentIv, outStream).GetAwaiter().GetResult();
            return outStream.ToArray();
        }

        private static void XorBuffers(ReadOnlySpan<byte> a, ReadOnlySpan<byte> b, Span<byte> destination)
        {
            int i = 0;
            int vectorSize = Vector<byte>.Count;
            while (i <= a.Length - vectorSize)
            {
                var va = new Vector<byte>(a.Slice(i));
                var vb = new Vector<byte>(b.Slice(i));
                (va ^ vb).CopyTo(destination.Slice(i));
                i += vectorSize;
            }
            while (i < a.Length)
            {
                destination[i] = (byte)(a[i] ^ b[i]);
                i++;
            }
        }
    }
}
