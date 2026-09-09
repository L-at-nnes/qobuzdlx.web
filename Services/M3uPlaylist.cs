using System.Text;

namespace QobuzDLX.Web.Services;

/// <summary>Une piste d'une playlist : sa place, le fichier écrit, sa durée et le texte affiché par les lecteurs.</summary>
public record PlaylistEntry(int Index, string FilePath, int DurationSeconds, string Title);

/// <summary>Écrit une playlist au format M3U étendu (UTF-8), lisible par la plupart des lecteurs.</summary>
public static class M3uPlaylist
{
    /// <summary>
    /// Contenu du fichier, dans l'ordre de la playlist. Les chemins sont relatifs au dossier du fichier
    /// et utilisent « / » : la bibliothèque peut être déplacée ou lue depuis une autre machine sans
    /// que la playlist se casse.
    /// </summary>
    public static string Build(string folder, IEnumerable<PlaylistEntry> entries)
    {
        var text = new StringBuilder("#EXTM3U\n");
        foreach (var entry in entries.OrderBy(e => e.Index))
        {
            text.Append("#EXTINF:").Append(entry.DurationSeconds).Append(',')
                .Append(entry.Title.ReplaceLineEndings(" ")).Append('\n');
            text.Append(Path.GetRelativePath(folder, entry.FilePath).Replace('\\', '/')).Append('\n');
        }
        return text.ToString();
    }
}
