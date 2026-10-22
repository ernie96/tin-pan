using System;

namespace TinPan.Services
{
    /// <summary>
    /// A failure worth telling the user about. Message is already a short, plain sentence.
    /// </summary>
    public class SunoApiException : Exception
    {
        public int? StatusCode { get; }

        public SunoApiException(string message, int? statusCode = null, Exception? inner = null)
            : base(message, inner)
        {
            StatusCode = statusCode;
        }
    }

    /// <summary>The saved sign-in no longer works; the user has to sign in again.</summary>
    public sealed class SunoSignInRequiredException : SunoApiException
    {
        public SunoSignInRequiredException(string message = "Your Suno sign-in has expired. Sign in again from Settings.")
            : base(message, 401)
        {
        }
    }
}
