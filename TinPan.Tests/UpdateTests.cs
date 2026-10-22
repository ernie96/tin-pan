using System;
using System.Collections.Generic;
using System.IO;
using TinPan.Services;
using TinPan.ViewModels;
using Xunit;

namespace TinPan.Tests
{
    public class UpdateTests
    {
        [Theory]
        [InlineData("v1.2.0", "1.2.0")]
        [InlineData("1.2", "1.2.0")]
        [InlineData("V2.0.1-beta", "2.0.1")]
        [InlineData(" v3.4.5 ", "3.4.5")]
        public void Release_tags_read_as_versions(string tag, string expected) =>
            Assert.Equal(Version.Parse(expected), UpdateService.ParseVersion(tag));

        [Theory]
        [InlineData("")]
        [InlineData("latest")]
        [InlineData(null)]
        public void Tags_without_a_version_are_refused(string? tag) => Assert.Null(UpdateService.ParseVersion(tag));

        [Fact]
        public void An_update_needs_both_the_exe_and_its_signature()
        {
            var release = new GitHubRelease
            {
                TagName = "v9.0.0", Body = "notes", HtmlUrl = "https://github.com/ernie96/tin-pan/releases/tag/v9.0.0",
                Assets = new List<GitHubAsset>
                {
                    new() { Name = "TinPan.exe", DownloadUrl = "https://example.com/TinPan.exe" },
                    new() { Name = "SHA256SUMS.txt", DownloadUrl = "https://example.com/SHA256SUMS.txt" }
                }
            };
            Assert.Null(UpdateService.PickAssets(release, new Version(9, 0, 0)));

            release.Assets.Add(new GitHubAsset { Name = "TinPan.exe.sig", DownloadUrl = "https://example.com/TinPan.exe.sig" });
            var info = UpdateService.PickAssets(release, new Version(9, 0, 0));
            Assert.NotNull(info);
            Assert.Equal("https://example.com/TinPan.exe", info!.DownloadUrl);
            Assert.Equal("https://example.com/TinPan.exe.sig", info.SignatureUrl);
        }

        [Fact]
        public void A_signed_file_verifies_and_any_change_or_other_name_does_not()
        {
            var (privateKey, publicKey) = UpdateSignature.GenerateKeyPair();
            var file = Path.Combine(Path.GetTempPath(), $"tinpan-sign-test-{Guid.NewGuid():N}.exe");
            try
            {
                File.WriteAllBytes(file, new byte[] { 1, 2, 3, 4, 5 });
                var signature = UpdateSignature.Sign(privateKey, file, "TinPan.exe");

                Assert.True(UpdateSignature.Verify(publicKey, file, "TinPan.exe", signature));
                Assert.False(UpdateSignature.Verify(publicKey, file, "Other.exe", signature));
                Assert.False(UpdateSignature.Verify(UpdateSignature.GenerateKeyPair().PublicKey, file, "TinPan.exe", signature));
                Assert.False(UpdateSignature.Verify(publicKey, file, "TinPan.exe", "not base64!"));

                File.WriteAllBytes(file, new byte[] { 1, 2, 3, 4, 6 });
                Assert.False(UpdateSignature.Verify(publicKey, file, "TinPan.exe", signature));
            }
            finally
            {
                File.Delete(file);
            }
        }

        [Fact]
        public void The_built_in_public_key_is_a_valid_Ed25519_key() =>
            Assert.Equal(32, Convert.FromBase64String(UpdateService.PublicKey).Length);

        [Theory]
        [InlineData(@"C:\Apps\TinPan.exe", @"'C:\Apps\TinPan.exe'")]
        [InlineData(@"C:\Users\O'Brien\TinPan.exe", @"'C:\Users\O''Brien\TinPan.exe'")]
        [InlineData("C:\\Users\\O\u2019Brien\\TinPan.exe", "'C:\\Users\\O\u2019\u2019Brien\\TinPan.exe'")]
        public void Paths_are_quoted_safely_for_PowerShell(string path, string expected) =>
            Assert.Equal(expected, UpdateService.Quote(path));

        [Fact]
        public void The_install_script_waits_swaps_with_a_fallback_and_relaunches()
        {
            var script = UpdateService.InstallScript(42, @"C:\Temp\TinPanUpdate\TinPan.exe", @"D:\My Apps\TinPan.exe");
            Assert.StartsWith("Wait-Process -Id 42", script);
            Assert.Contains(@"Move-Item -LiteralPath 'D:\My Apps\TinPan.exe' -Destination 'D:\My Apps\TinPan.exe.old'", script);
            Assert.Contains(@"Copy-Item -LiteralPath 'C:\Temp\TinPanUpdate\TinPan.exe' -Destination 'D:\My Apps\TinPan.exe'", script);
            // A failed copy puts the old exe back before trying again.
            Assert.Contains(@"catch { Move-Item -LiteralPath 'D:\My Apps\TinPan.exe.old' -Destination 'D:\My Apps\TinPan.exe'", script);
            Assert.Contains("install-failed.txt", script);
            Assert.EndsWith(@"Start-Process -FilePath 'D:\My Apps\TinPan.exe'", script);
        }

        [Fact]
        public void The_update_offer_reads_only_the_changes()
        {
            var notes = "Download **TinPan.exe** below and run it. It is one file.\r\n\r\n### New\r\n\r\n- Filters.";
            Assert.Equal("### New\n\n- Filters.", UpdateService.ChangesOnly(notes));
            Assert.Equal("### Fixed\n- Things.", UpdateService.ChangesOnly("### Fixed\n- Things."));
            Assert.Equal("", UpdateService.ChangesOnly(null));
        }

        [Fact]
        public void Release_notes_are_read_as_plain_lines()
        {
            var text = MainViewModel.PlainText("### New\n\n* **Bold** item with [a link](https://x.y)\n- `code` item\n\n\n\nEnd");
            Assert.Equal("New\n\n- Bold item with a link\n- code item\n\nEnd", text);
        }
    }
}
