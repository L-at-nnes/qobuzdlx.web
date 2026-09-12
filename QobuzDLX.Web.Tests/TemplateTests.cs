using QobuzDLX.Web.Models;
using QobuzDLX.Web.Services;
using Xunit;

namespace QobuzDLX.Web.Tests;

public static class TemplateTests
{
    private static QobuzAlbum Album(int discs = 1) => new()
    {
        Id = "1", Title = "Alb", Artist = new QobuzPerformer { Name = "Art" }, TracksCount = 12, MediaCount = discs
    };

    private static QobuzTrack Track(int number = 3, int disc = 1, int? playlistTrackId = null) => new()
    {
        Id = 77, Title = "Song", TrackNumber = number, MediaNumber = disc, PlaylistTrackId = playlistTrackId
    };

    private static string Norm(string path) => path.Replace('\\', '/');

    [Fact]
    public static void File_paths_previews_and_validation_follow_the_templates()
    {
        var fx = new Fixtures();
        var t = fx.Templates;
        const string dir = "/dl/Art/Alb";

        // ── file path: same results as the inline code it replaces ──
        H.Check("plain album track", Norm(t.TrackFilePath(dir, "album", Album(), Track(), null)) == "/dl/Art/Alb/03 - Song.flac");
        H.Check("multi-disc album gets a disc folder", Norm(t.TrackFilePath(dir, "album", Album(2), Track(3, 2), null)) == "/dl/Art/Alb/CD 2/03 - Song.flac");
        H.Check("a single-track download gets no disc folder", Norm(t.TrackFilePath(dir, "track", Album(2), Track(3, 2), null)) == "/dl/Art/Alb/03 - Song.flac");
        H.Check("a playlist entry gets no disc folder", Norm(t.TrackFilePath(dir, "playlist", Album(2), Track(3, 2, playlistTrackId: 5), new QobuzPlaylist { Name = "P" })) == "/dl/Art/Alb/03 - Song.flac");
        fx.Settings.Update(s => s.CdTemplate = "");
        H.Check("an empty disc template disables disc folders", Norm(t.TrackFilePath(dir, "album", Album(2), Track(3, 2), null)) == "/dl/Art/Alb/03 - Song.flac");
        fx.Settings.Update(s => { s.CdTemplate = "Disc %DiscNumber%"; s.AudioFormat = ".mp3"; });
        H.Check("custom disc template and extension", Norm(t.TrackFilePath(dir, "album", Album(2), Track(3, 2), null)) == "/dl/Art/Alb/Disc 2/03 - Song.mp3");
        fx.Settings.Update(s => { s.CdTemplate = "CD %DiscNumber%"; s.AudioFormat = ".flac"; });

        // ── the file cannot leave the download folder, whatever the template says ──
        foreach (var hostile in new[] { "../../etc/%TrackTitle%", "/etc/%TrackTitle%", "..\\..\\x", "a/../../b/%TrackTitle%" })
        {
            fx.Settings.Update(s => s.TrackTemplate = hostile);
            var path = Norm(t.TrackFilePath(dir, "album", Album(), Track(), null));
            var relative = Path.GetRelativePath("/dl/Art/Alb", path).Replace('\\', '/');
            H.Check($"template '{hostile}' stays inside the album folder ({relative})", !relative.StartsWith("..") && !Path.IsPathRooted(relative) && path.EndsWith(".flac"));
        }
        fx.Settings.Update(s => s.TrackTemplate = "");
        H.Check("a blank template still gives a usable name", Norm(t.TrackFilePath(dir, "album", Album(), Track(), null)) == "/dl/Art/Alb/77.flac");
        fx.Settings.Update(s => s.TrackTemplate = "%TrackNumber%/%TrackTitle%");
        H.Check("a slash in the template still makes a sub-folder", Norm(t.TrackFilePath(dir, "album", Album(), Track(), null)) == "/dl/Art/Alb/03/Song.flac");
        fx.Settings.Update(s => s.TrackTemplate = "%TrackNumber% - %TrackTitle%");

        // ── preview ──
        fx.Settings.Update(s => s.DownloadLocation = "/downloads");
        var preview = t.Preview();
        H.Check("album preview (disc 2, version in the title)", preview.Album == "/downloads/Nina Alvarez/Blue Hour/CD 2/05 - Midnight Garden (Live).flac");
        H.Check("compilation preview uses the V/A track template", preview.Compilation == "/downloads/Various Artists/Summer Mix/07 - Nina Alvarez - Midnight Garden (Live).flac");
        H.Check("playlist preview uses the playlist folder", preview.Playlist == "/downloads/Morning Playlist/05 - Midnight Garden (Live).flac");
        fx.Settings.Update(s => { s.TrackTemplate = "%TrackTitle%"; s.AlbumTemplate = "%Year% - %AlbumTitle% [%FormatWithQuality%]"; });
        H.Check("the preview follows the settings", t.Preview().Album == "/downloads/Nina Alvarez/2013 - Blue Hour [FLAC (24bit-96kHz)]/CD 2/Midnight Garden (Live).flac");
        fx.Settings.Update(s => { s.TrackTemplate = "%TrackNumber% - %TrackTitle%"; s.AlbumTemplate = "%AlbumTitle%"; });

        // ── validation ──
        H.Check("default templates raise no issue", t.Validate().Count == 0);

        List<TemplateIssue> With(Action<AppSettings> change)
        {
            var f = new Fixtures();
            f.Settings.Update(change);
            return f.Templates.Validate().ToList();
        }

        var empty = With(s => s.TrackTemplate = "  ");
        H.Check("empty track template", empty.Count == 1 && empty[0] is { Field: "TrackTemplate", Code: "empty" });
        H.Check("empty folder templates are allowed", With(s => { s.ArtistTemplate = ""; s.AlbumTemplate = ""; s.PlaylistTemplate = ""; s.CdTemplate = ""; }).Count == 0);

        H.Check("rooted template (slash)", With(s => s.AlbumTemplate = "/music/%AlbumTitle%").Any(i => i is { Field: "AlbumTemplate", Code: "rooted" }));
        H.Check("rooted template (drive letter)", With(s => s.ArtistTemplate = "C:/music").Any(i => i.Code == "rooted"));
        H.Check("parent component", With(s => s.TrackTemplate = "../%TrackTitle%").Any(i => i is { Field: "TrackTemplate", Code: "parent" }));

        var typo = With(s => s.TrackTemplate = "%TrackNumbre% - %TrackTitle%");
        H.Check("misspelt tag is reported with its name", typo.Any(i => i is { Code: "unknownTag" } && i.Detail == "%tracknumbre%"));
        var misplaced = With(s => s.AlbumTemplate = "%TrackNumber% %AlbumTitle%");
        H.Check("a track tag in a folder template is reported", misplaced.Any(i => i is { Field: "AlbumTemplate", Code: "unknownTag" } && i.Detail == "%tracknumber%"));
        H.Check("playlist tags are fine in a playlist template", With(s => s.PlaylistTemplate = "%PlaylistTitle% (%PlaylistID%)").Count == 0);
        H.Check("disc number is fine in the disc template", With(s => s.CdTemplate = "Disc %DiscNumber%").Count == 0);
        H.Check("other tags are not in the disc template", With(s => s.CdTemplate = "%AlbumTitle% %DiscNumber%").Any(i => i is { Field: "CdTemplate", Code: "unknownTag" }));

        var chars = With(s => s.TrackTemplate = "%TrackNumber%: %TrackTitle%?");
        H.Check("characters that will be rewritten are listed", chars.Any(i => i is { Code: "illegalChars" } && i.Detail!.Contains(':') && i.Detail.Contains('?')));
        H.Check("every parental-advisory tag of the README resolves", With(s => s.TrackTemplate = "%TrackTitle% %TrackPAifExEnclosed[]% %AlbumPAShort%").Count == 0);
        H.Check("several issues can accumulate", With(s => s.TrackTemplate = "/x/%Oops%?").Count >= 3);
    }
}
