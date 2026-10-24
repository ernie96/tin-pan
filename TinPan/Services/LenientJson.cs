using System;
using System.Globalization;
using System.Text.Json;
using System.Text.Json.Serialization;

namespace TinPan.Services
{
    /// <summary>
    /// Suno's replies sometimes carry null, or a number as text, where a count or a yes/no usually is
    /// ("play_count": null). Read strictly, one such field drops the whole song from the list; these
    /// read it as 0 or false instead, and take "12" as 12.
    /// </summary>
    public static class LenientJson
    {
        public static JsonSerializerOptions Options(JsonSerializerOptions options)
        {
            options.Converters.Add(new Int32Reader());
            options.Converters.Add(new Int64Reader());
            options.Converters.Add(new DoubleReader());
            options.Converters.Add(new BooleanReader());
            return options;
        }

        private static string? Text(ref Utf8JsonReader reader) => reader.TokenType == JsonTokenType.String ? reader.GetString()?.Trim() : null;

        private sealed class Int32Reader : JsonConverter<int>
        {
            public override bool HandleNull => true;

            public override int Read(ref Utf8JsonReader reader, Type typeToConvert, JsonSerializerOptions options) => reader.TokenType switch
            {
                JsonTokenType.Number when reader.TryGetInt32(out var n) => n,
                JsonTokenType.Number when reader.TryGetDouble(out var d) => (int)Math.Clamp(Math.Round(d), int.MinValue, int.MaxValue),
                JsonTokenType.String when int.TryParse(Text(ref reader), NumberStyles.Integer, CultureInfo.InvariantCulture, out var s) => s,
                JsonTokenType.True => 1,
                _ => Skip(ref reader, 0)
            };

            public override void Write(Utf8JsonWriter writer, int value, JsonSerializerOptions options) => writer.WriteNumberValue(value);
        }

        private sealed class Int64Reader : JsonConverter<long>
        {
            public override bool HandleNull => true;

            public override long Read(ref Utf8JsonReader reader, Type typeToConvert, JsonSerializerOptions options) => reader.TokenType switch
            {
                JsonTokenType.Number when reader.TryGetInt64(out var n) => n,
                JsonTokenType.String when long.TryParse(Text(ref reader), NumberStyles.Integer, CultureInfo.InvariantCulture, out var s) => s,
                _ => Skip(ref reader, 0L)
            };

            public override void Write(Utf8JsonWriter writer, long value, JsonSerializerOptions options) => writer.WriteNumberValue(value);
        }

        private sealed class DoubleReader : JsonConverter<double>
        {
            public override bool HandleNull => true;

            public override double Read(ref Utf8JsonReader reader, Type typeToConvert, JsonSerializerOptions options) => reader.TokenType switch
            {
                JsonTokenType.Number => reader.GetDouble(),
                JsonTokenType.String when double.TryParse(Text(ref reader), NumberStyles.Float, CultureInfo.InvariantCulture, out var s) => s,
                _ => Skip(ref reader, 0.0)
            };

            public override void Write(Utf8JsonWriter writer, double value, JsonSerializerOptions options) => writer.WriteNumberValue(value);
        }

        private sealed class BooleanReader : JsonConverter<bool>
        {
            public override bool HandleNull => true;

            public override bool Read(ref Utf8JsonReader reader, Type typeToConvert, JsonSerializerOptions options) => reader.TokenType switch
            {
                JsonTokenType.True => true,
                JsonTokenType.False => false,
                JsonTokenType.Number => reader.TryGetDouble(out var d) && d != 0,
                JsonTokenType.String => Text(ref reader)?.ToLowerInvariant() is "true" or "1" or "yes",
                _ => Skip(ref reader, false)
            };

            public override void Write(Utf8JsonWriter writer, bool value, JsonSerializerOptions options) => writer.WriteBooleanValue(value);
        }

        /// <summary>null, or an object or list where a number was expected: passed over, giving the fallback.</summary>
        private static T Skip<T>(ref Utf8JsonReader reader, T fallback)
        {
            if (reader.TokenType is JsonTokenType.StartObject or JsonTokenType.StartArray) reader.Skip();
            return fallback;
        }
    }
}
