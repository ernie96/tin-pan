using System;
using System.IO;
using System.Security.Cryptography;
using System.Text;
using System.Text.Json;
using System.Threading.Tasks;
using TinPan.Models;
using TinPan.Services;
using Xunit;

namespace TinPan.Tests
{
    public class SunoDrmTests
    {
        [Fact]
        public void PickMediaUrl_ReturnsUnencrypted_WhenAvailable()
        {
            var clip = new SunoClip
            {
                Id = "test-clip-1",
                AudioUrl = "https://studio-api.prod.suno.com/api/forbidden",
                MediaUrls = new()
                {
                    new MediaUrl { Url = "https://cdn.example.com/enc.m4a", Encoding = JsonDocument.Parse("\"m4a-opus\"").RootElement },
                    new MediaUrl { Url = "https://cdn.example.com/plain.m4a", Encoding = null }
                }
            };

            var (url, isEncrypted) = SunoDrm.PickMediaUrl(clip, preferUnencrypted: true);

            Assert.Equal("https://cdn.example.com/plain.m4a", url);
            Assert.False(isEncrypted);
        }

        [Fact]
        public void PickMediaUrl_ReturnsEncrypted_WhenAllEncrypted()
        {
            var clip = new SunoClip
            {
                Id = "test-clip-2",
                AudioUrl = "https://studio-api.prod.suno.com/api/forbidden",
                MediaUrls = new()
                {
                    new MediaUrl { Url = "https://cdn.example.com/enc.m4a", Encoding = JsonDocument.Parse("\"m4a-opus\"").RootElement }
                }
            };

            var (url, isEncrypted) = SunoDrm.PickMediaUrl(clip, preferUnencrypted: true);

            Assert.Equal("https://cdn.example.com/enc.m4a", url);
            Assert.True(isEncrypted);
        }

        [Fact]
        public void PickMediaUrl_FallsBackToDefaultCdn_WhenAudioUrlIsDecoyAndMediaUrlsEmpty()
        {
            var clip = new SunoClip
            {
                Id = "test-clip-3",
                AudioUrl = "https://studio-api.prod.suno.com/api/forbidden",
                MediaUrls = new()
            };

            var (url, isEncrypted) = SunoDrm.PickMediaUrl(clip, preferUnencrypted: true);

            Assert.Equal("https://d2lwuy8qc234o3.cloudfront.net/1/clip/test-clip-3.m4a", url);
            Assert.True(isEncrypted);
        }

        [Fact]
        public void SunoClip_AudioUrlIsDecoy_Detected()
        {
            var clip1 = new SunoClip { AudioUrl = "https://studio-api.prod.suno.com/api/forbidden" };
            Assert.True(clip1.AudioUrlIsDecoy);

            var clip2 = new SunoClip { AudioUrl = "https://cdn1.suno.ai/track.mp3" };
            Assert.False(clip2.AudioUrlIsDecoy);
        }

        [Fact]
        public void IncrementCounter_HandlesCarriesCorrectly()
        {
            // Initial IV: all zeroes except last byte is 0xFF
            var iv = new byte[16];
            iv[15] = 0xFF;

            var result = new byte[16];
            SunoDrm.IncrementCounter(iv, 1, result);

            // 0xFF + 1 = 0x100 -> byte 15 is 0, byte 14 is 1
            Assert.Equal(0, result[15]);
            Assert.Equal(1, result[14]);

            // Now test 32-bit boundary carry: counter 00...0001 + 0xFFFFFFFF -> ...000100000000
            var iv2 = new byte[16];
            iv2[15] = 1;
            var result2 = new byte[16];
            SunoDrm.IncrementCounter(iv2, 0xFFFFFFFF, result2);

            Assert.Equal(0, result2[15]);
            Assert.Equal(0, result2[14]);
            Assert.Equal(0, result2[13]);
            Assert.Equal(0, result2[12]);
            Assert.Equal(1, result2[11]);
        }

        [Fact]
        public void DecodeKeyMaterial_HandlesHex_And_Base64_And_Base64Url()
        {
            // Hex "01020304"
            var hex = SunoDrm.DecodeKeyMaterial("01020304");
            Assert.Equal(new byte[] { 1, 2, 3, 4 }, hex);

            // Base64 of "Hello"
            var b64 = SunoDrm.DecodeKeyMaterial("SGVsbG8=");
            Assert.Equal(Encoding.UTF8.GetBytes("Hello"), b64);

            // Base64Url with - and _
            var original = new byte[] { 251, 239, 255 }; // in base64: ++// -> in base64url: --__
            var b64url = "--__";
            var decoded = SunoDrm.DecodeKeyMaterial(b64url);
            Assert.Equal(original, decoded);
        }

        [Fact]
        public void UnwrapEnvelope_And_UnwrapRights_Works()
        {
            var userKey = SHA256.HashData(Encoding.UTF8.GetBytes("test-bearer-token"));
            var contentKey = new byte[32];
            RandomNumberGenerator.Fill(contentKey);
            var contentIv = new byte[16];
            RandomNumberGenerator.Fill(contentIv);

            var aad = Encoding.UTF8.GetBytes("clip-123");

            // Wrap contentKey into AES-GCM envelope
            var nonceKey = new byte[12];
            RandomNumberGenerator.Fill(nonceKey);
            var cipherKey = new byte[contentKey.Length];
            var tagKey = new byte[16];
            using (var gcm = new AesGcm(userKey, 16))
            {
                gcm.Encrypt(nonceKey, contentKey, cipherKey, tagKey, aad);
            }
            var envKey = new byte[12 + cipherKey.Length + 16];
            nonceKey.CopyTo(envKey, 0);
            cipherKey.CopyTo(envKey, 12);
            tagKey.CopyTo(envKey, 12 + cipherKey.Length);

            // Wrap contentIv into AES-GCM envelope
            var nonceIv = new byte[12];
            RandomNumberGenerator.Fill(nonceIv);
            var cipherIv = new byte[contentIv.Length];
            var tagIv = new byte[16];
            using (var gcm = new AesGcm(userKey, 16))
            {
                gcm.Encrypt(nonceIv, contentIv, cipherIv, tagIv, aad);
            }
            var envIv = new byte[12 + cipherIv.Length + 16];
            nonceIv.CopyTo(envIv, 0);
            cipherIv.CopyTo(envIv, 12);
            tagIv.CopyTo(envIv, 12 + cipherIv.Length);

            var rights = new SunoRights
            {
                Key = Convert.ToBase64String(envKey),
                Iv = Convert.ToBase64String(envIv),
                Aad = "clip-123",
                Glt = "test-glt"
            };

            var (unwrappedKey, unwrappedIv) = SunoDrm.UnwrapRights(rights, "clip-123", "test-bearer-token");

            Assert.Equal(contentKey, unwrappedKey);
            Assert.Equal(contentIv, unwrappedIv);
        }

        [Fact]
        public void UnwrapRights_FallsBackToGlt_WhenBearerMismatch()
        {
            var gltKey = SHA256.HashData(Encoding.UTF8.GetBytes("valid-glt-token"));
            var contentKey = new byte[32];
            RandomNumberGenerator.Fill(contentKey);
            var contentIv = new byte[16];
            RandomNumberGenerator.Fill(contentIv);

            var aad = Encoding.UTF8.GetBytes("clip-456");

            // Wrap with gltKey
            var nonceKey = new byte[12];
            RandomNumberGenerator.Fill(nonceKey);
            var cipherKey = new byte[contentKey.Length];
            var tagKey = new byte[16];
            using (var gcm = new AesGcm(gltKey, 16))
            {
                gcm.Encrypt(nonceKey, contentKey, cipherKey, tagKey, aad);
            }
            var envKey = new byte[12 + cipherKey.Length + 16];
            nonceKey.CopyTo(envKey, 0);
            cipherKey.CopyTo(envKey, 12);
            tagKey.CopyTo(envKey, 12 + cipherKey.Length);

            var nonceIv = new byte[12];
            RandomNumberGenerator.Fill(nonceIv);
            var cipherIv = new byte[contentIv.Length];
            var tagIv = new byte[16];
            using (var gcm = new AesGcm(gltKey, 16))
            {
                gcm.Encrypt(nonceIv, contentIv, cipherIv, tagIv, aad);
            }
            var envIv = new byte[12 + cipherIv.Length + 16];
            nonceIv.CopyTo(envIv, 0);
            cipherIv.CopyTo(envIv, 12);
            tagIv.CopyTo(envIv, 12 + cipherIv.Length);

            var rights = new SunoRights
            {
                Key = Convert.ToBase64String(envKey),
                Iv = Convert.ToBase64String(envIv),
                Aad = "clip-456",
                Glt = "valid-glt-token"
            };

            // Pass a wrong bearer token, which fails GCM auth tag, so it should fall back to GLT and succeed!
            var (unwrappedKey, unwrappedIv) = SunoDrm.UnwrapRights(rights, "clip-456", "wrong-bearer-token");

            Assert.Equal(contentKey, unwrappedKey);
            Assert.Equal(contentIv, unwrappedIv);
        }

        [Fact]
        public void ParseRights_HandlesDataEnvelope_AndFiltersDecoy()
        {
            var json = @"{
                ""data"": {
                    ""content_key"": ""0102030405060708090a0b0c0d0e0f10"",
                    ""content_iv"": ""1112131415161718191a1b1c1d1e1f20"",
                    ""glt_token"": ""glt-abc"",
                    ""aad"": ""custom-aad"",
                    ""media_url"": ""https://studio-api.prod.suno.com/api/forbidden""
                }
            }";

            var rights = SunoDrm.ParseRights(json, "clip-999");
            Assert.NotNull(rights);
            Assert.Equal("0102030405060708090a0b0c0d0e0f10", rights.Key);
            Assert.Equal("1112131415161718191a1b1c1d1e1f20", rights.Iv);
            Assert.Equal("glt-abc", rights.Glt);
            Assert.Equal("custom-aad", rights.Aad);
            Assert.Null(rights.MediaUrl); // forbidden decoy must be nulled
        }

        [Fact]
        public async Task DecryptAesCtrAsync_RoundTrip_Matches()
        {
            var contentKey = new byte[32];
            RandomNumberGenerator.Fill(contentKey);
            var contentIv = new byte[16];
            RandomNumberGenerator.Fill(contentIv);

            // Create 150 KB test data (spans multiple 64KB chunks and odd tail)
            var plaintext = new byte[150 * 1024 + 7];
            RandomNumberGenerator.Fill(plaintext);

            // In AES-CTR, encryption and decryption are identical
            using var inStream = new MemoryStream(plaintext);
            using var cipherStream = new MemoryStream();
            await SunoDrm.DecryptAesCtrAsync(inStream, contentKey, contentIv, cipherStream);

            var ciphertext = cipherStream.ToArray();
            Assert.Equal(plaintext.Length, ciphertext.Length);
            Assert.NotEqual(plaintext, ciphertext);

            using var decryptIn = new MemoryStream(ciphertext);
            using var decryptedStream = new MemoryStream();
            await SunoDrm.DecryptAesCtrAsync(decryptIn, contentKey, contentIv, decryptedStream);

            var decrypted = decryptedStream.ToArray();
            Assert.Equal(plaintext, decrypted);
        }

        [Fact]
        public void DetectContainer_IdentifiesM4aAndMp3()
        {
            // M4A has "ftyp" at offset 4..7
            var m4aHeader = new byte[] { 0x00, 0x00, 0x00, 0x20, 0x66, 0x74, 0x79, 0x70, 0x4D, 0x34, 0x41, 0x20 };
            Assert.Equal("m4a", SunoDrm.DetectContainer(m4aHeader));

            // MP3 ID3 header
            var mp3Id3Header = new byte[] { 0x49, 0x44, 0x33, 0x04, 0x00, 0x00 };
            Assert.Equal("mp3", SunoDrm.DetectContainer(mp3Id3Header));

            // MP3 frame sync
            var mp3SyncHeader = new byte[] { 0xFF, 0xFB, 0x90, 0x64 };
            Assert.Equal("mp3", SunoDrm.DetectContainer(mp3SyncHeader));

            // Unknown
            var unknown = new byte[] { 0x01, 0x02, 0x03, 0x04 };
            Assert.Equal("unknown", SunoDrm.DetectContainer(unknown));
        }
    }
}
