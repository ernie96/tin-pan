using System;
using System.Threading;
using System.Threading.Tasks;

namespace TinPan.Services
{
    public interface IAuthService
    {
        bool IsAuthenticated { get; }

        /// <summary>Name of the signed-in account, when Suno shares one.</summary>
        string? UserName { get; }

        /// <summary>Value for Suno's Device-Id header.</summary>
        string DeviceId { get; }

        /// <summary>Cookie header for studio API calls.</summary>
        string CookieHeader { get; }

        event EventHandler AuthStateChanged;

        /// <summary>Tries the sign-in saved last time. Returns false if the user needs to sign in.</summary>
        Task<bool> RestoreAsync();

        /// <summary>Opens the Suno sign-in window. Returns true once the app is signed in.</summary>
        Task<bool> SignInAsync();

        void SignOut();

        /// <summary>A fresh API token. Throws SunoSignInRequiredException if the sign-in is gone.</summary>
        Task<string> GetTokenAsync(bool forceRefresh = false, CancellationToken ct = default);
    }
}
