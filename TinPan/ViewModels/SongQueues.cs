using System.Collections.Generic;
using System.Linq;
using TinPan.Models;

namespace TinPan.ViewModels
{
    internal static class SongQueues
    {
        /// <summary>The id of the first playable song after current in the list, for fetching it ahead.</summary>
        public static string? After(IList<SunoClip> songs, SunoClip? current)
        {
            var index = current == null ? -1 : songs.IndexOf(current);
            return index < 0 ? null : songs.Skip(index + 1).FirstOrDefault(c => c.CanPlay)?.Id;
        }
    }
}
