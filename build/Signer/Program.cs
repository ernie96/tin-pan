using TinPan.Services;

// Usage:
//   keygen <private-key-file>       creates a key pair; prints the public key for UpdateService.PublicKey
//   sign <file> [<file> ...]        writes <file>.sig; key from UPDATE_SIGNING_KEY or UPDATE_SIGNING_KEY_FILE
//   verify <public-key> <file>      checks <file>.sig
switch (args.FirstOrDefault())
{
    case "keygen" when args.Length == 2:
        if (File.Exists(args[1]))
        {
            Console.Error.WriteLine($"{args[1]} already exists; refusing to overwrite a signing key.");
            return 1;
        }
        var (privateKey, publicKey) = UpdateSignature.GenerateKeyPair();
        Directory.CreateDirectory(Path.GetDirectoryName(Path.GetFullPath(args[1]))!);
        File.WriteAllText(args[1], privateKey);
        Console.WriteLine(publicKey);
        return 0;

    case "sign" when args.Length >= 2:
        var key = Environment.GetEnvironmentVariable("UPDATE_SIGNING_KEY");
        if (string.IsNullOrWhiteSpace(key) && Environment.GetEnvironmentVariable("UPDATE_SIGNING_KEY_FILE") is { } keyFile && File.Exists(keyFile))
            key = File.ReadAllText(keyFile);
        if (string.IsNullOrWhiteSpace(key))
        {
            Console.Error.WriteLine("No signing key: set UPDATE_SIGNING_KEY or UPDATE_SIGNING_KEY_FILE.");
            return 1;
        }
        foreach (var file in args.Skip(1))
        {
            File.WriteAllText(file + ".sig", UpdateSignature.Sign(key, file, Path.GetFileName(file)));
            Console.WriteLine($"Signed {Path.GetFileName(file)}");
        }
        return 0;

    case "verify" when args.Length == 3:
        var ok = UpdateSignature.Verify(args[1], args[2], Path.GetFileName(args[2]), File.ReadAllText(args[2] + ".sig"));
        Console.WriteLine(ok ? "Signature OK" : "Signature INVALID");
        return ok ? 0 : 2;

    default:
        Console.Error.WriteLine("Usage: keygen <key-file> | sign <file>... | verify <public-key> <file>");
        return 1;
}
