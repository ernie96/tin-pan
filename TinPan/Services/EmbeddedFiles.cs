using System;
using System.IO;
using System.Reflection;
using System.Runtime.InteropServices;
using System.Security.Cryptography;

namespace TinPan.Services
{
    /// <summary>
    /// Files built into TinPan.exe so the published app is one file: NVDA's controller DLL and the
    /// license texts. A nvdaControllerClient.dll placed next to TinPan.exe is used instead of the
    /// built-in one, so it can be replaced with any other build (its LGPL license allows that).
    /// </summary>
    public static class EmbeddedFiles
    {
        public const string NvdaDll = "nvdaControllerClient.dll";

        /// <summary>Sends DllImports of the NVDA DLL to the copy next to the exe, or else the built-in one.</summary>
        public static void RegisterNativeLibraries() =>
            NativeLibrary.SetDllImportResolver(typeof(EmbeddedFiles).Assembly, (name, _, _) =>
            {
                if (!name.Equals(NvdaDll, StringComparison.OrdinalIgnoreCase) &&
                    !name.Equals(Path.GetFileNameWithoutExtension(NvdaDll), StringComparison.OrdinalIgnoreCase))
                    return IntPtr.Zero; // everything else loads the normal way

                foreach (var path in new[] { NextToExe(NvdaDll), ExtractNvda() })
                    if (path != null && NativeLibrary.TryLoad(path, out var handle)) return handle;
                return IntPtr.Zero;
            });

        /// <summary>The text of a built-in file (THIRD_PARTY_NOTICES.md, nvdaControllerClient-license.txt), or null.</summary>
        public static string? ReadText(string name)
        {
            using var stream = Open(name);
            if (stream == null) return null;
            using var reader = new StreamReader(stream);
            return reader.ReadToEnd();
        }

        private static Stream? Open(string name) => Assembly.GetExecutingAssembly().GetManifestResourceStream("TinPan." + name);

        private static string? NextToExe(string file)
        {
            var dir = Path.GetDirectoryName(Environment.ProcessPath);
            var path = dir == null ? null : Path.Combine(dir, file);
            return path != null && File.Exists(path) ? path : null;
        }

        /// <summary>
        /// Writes the built-in DLL to %LOCALAPPDATA%\Tin Pan\native, named by its hash so a different
        /// build never loads an old copy, and rewrites it if the file there doesn't match.
        /// </summary>
        private static string? ExtractNvda()
        {
            try
            {
                using var stream = Open(NvdaDll);
                if (stream == null) return null;
                using var memory = new MemoryStream();
                stream.CopyTo(memory);
                var bytes = memory.ToArray();
                var expected = SHA256.HashData(bytes);
                var dir = Path.Combine(AppFolders.Local, "native", Convert.ToHexString(expected)[..16]);
                var path = Path.Combine(dir, NvdaDll);
                if (File.Exists(path) && SHA256.HashData(File.ReadAllBytes(path)).AsSpan().SequenceEqual(expected)) return path;

                Directory.CreateDirectory(dir);
                var temp = path + "." + Guid.NewGuid().ToString("N") + ".tmp";
                File.WriteAllBytes(temp, bytes);
                File.Move(temp, path, overwrite: true);
                return path;
            }
            catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
            {
                Log.Warn($"Couldn't unpack NVDA's helper; NVDA direct speech is off: {ex.Message}");
                return null;
            }
        }
    }
}
