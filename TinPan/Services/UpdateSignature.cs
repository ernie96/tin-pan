using System;
using System.IO;
using System.Security.Cryptography;
using System.Text;
using Org.BouncyCastle.Crypto.Parameters;
using Org.BouncyCastle.Crypto.Signers;
using Org.BouncyCastle.Security;

namespace TinPan.Services
{
    /// <summary>
    /// Ed25519 signatures for release downloads, the same scheme as TimePlayer. The signed message
    /// binds the file's SHA-256 to its release asset name, so a signature for one file can't be
    /// passed off for another. The private key lives only with whoever publishes releases (the
    /// UPDATE_SIGNING_KEY secret); the app carries the public key and refuses any update that
    /// doesn't verify. Also compiled into build/Signer.
    /// </summary>
    public static class UpdateSignature
    {
        private const string Domain = "TinPan-update-v1";

        public static string Message(string assetName, byte[] sha256) =>
            $"{Domain}:{assetName}:{Convert.ToHexString(sha256).ToLowerInvariant()}";

        public static byte[] HashFile(string path)
        {
            using var stream = File.OpenRead(path);
            return SHA256.HashData(stream);
        }

        /// <returns>(private key, public key), both base64.</returns>
        public static (string PrivateKey, string PublicKey) GenerateKeyPair()
        {
            var privateKey = new Ed25519PrivateKeyParameters(new SecureRandom());
            return (Convert.ToBase64String(privateKey.GetEncoded()),
                    Convert.ToBase64String(privateKey.GeneratePublicKey().GetEncoded()));
        }

        /// <returns>The signature file's contents (base64).</returns>
        public static string Sign(string privateKeyBase64, string filePath, string assetName)
        {
            var key = new Ed25519PrivateKeyParameters(Convert.FromBase64String(privateKeyBase64.Trim()));
            var message = Encoding.UTF8.GetBytes(Message(assetName, HashFile(filePath)));
            var signer = new Ed25519Signer();
            signer.Init(true, key);
            signer.BlockUpdate(message, 0, message.Length);
            return Convert.ToBase64String(signer.GenerateSignature());
        }

        public static bool Verify(string publicKeyBase64, string filePath, string assetName, string signatureBase64)
        {
            try
            {
                var key = new Ed25519PublicKeyParameters(Convert.FromBase64String(publicKeyBase64.Trim()));
                var message = Encoding.UTF8.GetBytes(Message(assetName, HashFile(filePath)));
                var verifier = new Ed25519Signer();
                verifier.Init(false, key);
                verifier.BlockUpdate(message, 0, message.Length);
                return verifier.VerifySignature(Convert.FromBase64String(signatureBase64.Trim()));
            }
            catch (Exception ex) when (ex is FormatException or ArgumentException)
            {
                return false;
            }
        }
    }
}
