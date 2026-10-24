using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Text.Json;

namespace TinPan.Services
{
    /// <summary>
    /// Cookie jar for suno.com and its Clerk sign-in domain, kept as name/value pairs.
    /// The long-lived "__client" cookie is what lets us mint fresh API tokens.
    /// </summary>
    public sealed class SunoCookies
    {
        private static readonly HashSet<string> Attributes = new(StringComparer.OrdinalIgnoreCase)
        {
            "expires", "max-age", "domain", "path", "secure", "httponly", "samesite", "partitioned"
        };

        private readonly Dictionary<string, string> _values = new(StringComparer.Ordinal);

        public string? Client => Get("__client");

        /// <summary>Suno's analytics id doubles as the Device-Id header in the official clients.</summary>
        public string? AnonymousId => Get("ajs_anonymous_id");

        public bool HasClient => !string.IsNullOrEmpty(Client);

        public IReadOnlyCollection<string> Names => _values.Keys;

        public string? Get(string name) => _values.TryGetValue(name, out var v) ? v : null;

        public void Set(string name, string value)
        {
            name = name.Trim();
            if (name.Length == 0 || Attributes.Contains(name)) return;
            _values[name] = value.Trim();
        }

        /// <summary>Parses "a=1; b=2" or Set-Cookie style text, skipping cookie attributes.</summary>
        public static SunoCookies Parse(string? header)
        {
            var jar = new SunoCookies();
            jar.Merge(header);
            return jar;
        }

        public void Merge(string? header)
        {
            if (string.IsNullOrWhiteSpace(header)) return;
            foreach (var part in header.Split(';'))
            {
                var eq = part.IndexOf('=');
                if (eq <= 0) continue;
                Set(part[..eq], part[(eq + 1)..]);
            }
        }

        /// <summary>Merges the name=value pair at the start of a Set-Cookie header.</summary>
        public void MergeSetCookie(string setCookie)
        {
            var first = setCookie.Split(';', 2)[0];
            var eq = first.IndexOf('=');
            if (eq > 0) Set(first[..eq], first[(eq + 1)..]);
        }

        public string ToHeader() => string.Join("; ", _values.Select(kv => $"{kv.Key}={kv.Value}"));
    }

    public static class Jwt
    {
        /// <summary>Reads the "exp" claim without validating the signature. Null if unreadable.</summary>
        public static DateTimeOffset? ReadExpiry(string? token)
        {
            if (string.IsNullOrEmpty(token)) return null;
            var parts = token.Split('.');
            if (parts.Length < 2) return null;
            try
            {
                var payload = parts[1].Replace('-', '+').Replace('_', '/');
                payload = payload.PadRight(payload.Length + (4 - payload.Length % 4) % 4, '=');
                using var doc = JsonDocument.Parse(Encoding.UTF8.GetString(Convert.FromBase64String(payload)));
                return doc.RootElement.TryGetProperty("exp", out var exp) && exp.ValueKind == JsonValueKind.Number && exp.TryGetInt64(out var seconds)
                    ? DateTimeOffset.FromUnixTimeSeconds(seconds)
                    : null;
            }
            catch (FormatException) { return null; }
            catch (JsonException) { return null; }
            catch (ArgumentOutOfRangeException) { return null; }
            catch (InvalidOperationException) { return null; }
        }
    }
}
