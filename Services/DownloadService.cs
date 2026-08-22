using System.Diagnostics;
using System.Text.RegularExpressions;
using QobuzDLX.Web.Models;

namespace QobuzDLX.Web.Services;

public class DownloadProgress
{
    public int Percent { get; set; }
    public string Status { get; set; } = "";
    public string CurrentItem { get; set; } = "";
    public string Speed { get; set; } = "";
    public bool IsRunning { get; set; }
    public bool IsComplete { get; set; }
    public List<string> Log { get; set; } = new();
    public QobuzAlbum? CurrentAlbum { get; set; }
}

public class DownloadService
{
    private readonly QobuzApiService _api;
    private readonly SettingsService _settings;
    private readonly TemplateService _templates;
    private readonly TaggingService _tagging;
    private readonly LocalizationService _loc;
    private readonly ILogger<DownloadService> _logger;

    private CancellationTokenSource? _cts;
    private bool _skipCurrent;
    private string _sessionId = "";
    public DownloadProgress Progress { get; } = new();

    public event Action? OnProgressChanged;

    private static readonly Regex QobuzStoreLinkRegex = new(
        @"https://(?:.*?).qobuz.com/(?<region>.*?)/(?<type>.*?)/(?<name>.*?)/(?<id>.*?)$", RegexOptions.Compiled);
    private static readonly Regex QobuzLinkIdGrabRegex = new(
        @"https://(?:.*?).qobuz.com/(?<type>.*?)/(?<id>.*?)$", RegexOptions.Compiled);
    private static readonly Regex QobuzUrlRegex = new(
        @"^https?://(?!.*https?://)(?:[\w\-]+\.)?qobuz\.com/[^\s]+$", RegexOptions.Compiled | RegexOptions.IgnoreCase);

    public DownloadService(QobuzApiService api, SettingsService settings, TemplateService templates, TaggingService tagging, LocalizationService loc, ILogger<DownloadService> logger)
    {
        _api = api;
        _settings = settings;
        _templates = templates;
        _tagging = tagging;
        _loc = loc;
        _logger = logger;

        MigrateTempFolder();
    }

    /// <summary>
    /// Migrate qbdlx-temp from /downloads to /config for existing installations.
    /// </summary>
    private void MigrateTempFolder()
    {
        try
        {
            var oldTempDir = Path.Combine(_settings.Settings.DownloadLocation, "qbdlx-temp");
            var configPath = Environment.GetEnvironmentVariable("CONFIG_PATH") ?? "/config";
            var newTempDir = Path.Combine(configPath, "qbdlx-temp");

            if (Directory.Exists(oldTempDir) && oldTempDir != newTempDir)
            {
                if (Directory.Exists(newTempDir))
                    Directory.Delete(newTempDir, true);

                Directory.Move(oldTempDir, newTempDir);
                _logger.LogInformation("Migrated temp folder from {Old} to {New}", oldTempDir, newTempDir);
            }
        }
        catch (Exception ex)
        {
            _logger.LogWarning(ex, "Failed to migrate temp folder, it will be recreated in /config");
        }
    }

    public bool IsRunning => Progress.IsRunning;

    public void Abort()
    {
        _cts?.Cancel();
    }

    public void SkipCurrent()
    {
        _skipCurrent = true;
    }

    private void Log(string msg)
    {
        Progress.Log.Add(msg);
        OnProgressChanged?.Invoke();
    }

    private void UpdateProgress(int percent, string status, string currentItem = "")
    {
        Progress.Percent = percent;
        Progress.Status = status;
        if (!string.IsNullOrEmpty(currentItem)) Progress.CurrentItem = currentItem;
        OnProgressChanged?.Invoke();
    }

    // ─── Main entry: process a Qobuz URL ───

    public async Task ProcessUrlAsync(string inputUrl)
    {
        if (Progress.IsRunning) return;

        Progress.IsRunning = true;
        Progress.IsComplete = false;
        Progress.Percent = 0;
        Progress.Log.Clear();
        _sessionId = Guid.NewGuid().ToString("N")[..8];
        Progress.CurrentAlbum = null;
        _skipCurrent = false;
        _cts = new CancellationTokenSource();

        try
        {
            var url = inputUrl.Trim().TrimEnd('/');
            if (url.EndsWith("/releases", StringComparison.OrdinalIgnoreCase))
                url = url[..^"/releases".Length];

            if (!QobuzUrlRegex.IsMatch(url))
            {
                Log(_loc.T("log.invalidUrl", url));
                return;
            }

            // Normalize store links
            var storeMatch = QobuzStoreLinkRegex.Match(url);
            if (storeMatch.Success)
            {
                var storeType = storeMatch.Groups["type"].Value;
                var storeId = storeMatch.Groups["id"].Value;
                if (storeType == "album") url = $"https://play.qobuz.com/album/{storeId}";
                else if (storeType == "interpreter") url = $"https://play.qobuz.com/artist/{storeId}";
            }

            var linkMatch = QobuzLinkIdGrabRegex.Match(url);
            if (!linkMatch.Success) { Log(_loc["log.couldNotParseUrl"]); return; }

            var linkType = linkMatch.Groups["type"].Value;
            var linkId = linkMatch.Groups["id"].Value;

            Log(_loc["log.checkingLink"]);

            var s = _settings.Settings;

            switch (linkType)
            {
                case "album":
                    await DownloadAlbumAsync(linkId, _cts.Token);
                    break;
                case "track":
                    await DownloadSingleTrackAsync(linkId, _cts.Token);
                    break;
                case "playlist":
                    await DownloadPlaylistAsync(linkId, _cts.Token);
                    break;
                case "artist":
                    await DownloadArtistAsync(linkId, _cts.Token);
                    break;
                case "label":
                    await DownloadLabelAsync(linkId, _cts.Token);
                    break;
                case "user":
                    if (linkId.Contains("albums"))
                        await DownloadFavoritesAlbumsAsync(_cts.Token);
                    else if (linkId.Contains("tracks"))
                        await DownloadFavoritesTracksAsync(_cts.Token);
                    else if (linkId.Contains("artists"))
                        await DownloadFavoritesArtistsAsync(_cts.Token);
                    else
                        Log(_loc["log.unsupportedUserLink"]);
                    break;
                default:
                    Log(_loc.T("log.unsupportedLinkType", linkType));
                    break;
            }

            Log(_loc["log.complete"]);
        }
        catch (OperationCanceledException)
        {
            Log(_loc["log.aborted"]);
        }
        catch (Exception ex)
        {
            Log(_loc.T("log.error", ex.Message));
            _logger.LogError(ex, "Download failed");
        }
        finally
        {
            Progress.IsRunning = false;
            Progress.IsComplete = true;
            UpdateProgress(100, _loc["download.status.complete"]);
        }
    }

    // ─── Batch download ───

    public async Task ProcessBatchAsync(IEnumerable<string> urls)
    {
        var urlList = urls.ToList();
        int total = urlList.Count, idx = 0;

        foreach (var url in urlList)
        {
            idx++;
            Log(_loc.T("log.batchHeader", idx, total));
            await ProcessUrlAsync(url);
            if (_cts?.IsCancellationRequested == true) break;
        }
    }

    // ─── Album ───

    private async Task DownloadAlbumAsync(string albumId, CancellationToken ct)
    {
        var album = await _api.GetAlbumFullAsync(albumId);
        Progress.CurrentAlbum = album;

        var s = _settings.Settings;
        if (!album.Streamable && s.StreamableCheck)
        {
            Log(_loc["log.notStreamable"]);
            return;
        }

        var downloadPath = _templates.CreatePath(s.DownloadLocation, s.ArtistTemplate, s.AlbumTemplate, null, album, null, null);
        Directory.CreateDirectory(downloadPath);

        // Artwork
        await DownloadArtworkAsync(downloadPath, album);

        var tracks = album.Tracks?.Items ?? new List<QobuzTrack>();
        int total = tracks.Count;

        for (int i = 0; i < total; i++)
        {
            ct.ThrowIfCancellationRequested();
            if (_skipCurrent) { _skipCurrent = false; Log(_loc["log.albumSkipped"]); break; }

            var track = tracks[i];
            UpdateProgress((int)((double)i / total * 100), _loc.T("download.status.track", i + 1, total), _templates.FormatTrackTitle(track));

            try
            {
                // Get full track info
                var trackInfo = await _api.GetTrackAsync(track.Id.ToString());
                if (trackInfo == null) { Log(_loc.T("log.trackUnavailable", track.TrackNumber)); continue; }
                if (!trackInfo.Streamable && s.StreamableCheck) { Log(_loc.T("log.trackNotStreamable", track.TrackNumber)); continue; }

                await DownloadTrackToFileAsync("album", album, trackInfo, downloadPath, null, ct);
            }
            catch (OperationCanceledException) { throw; }
            catch (Exception ex) { Log(_loc.T("log.trackFailed", track.Id, ex.Message)); }
        }

        // Goodies
        if (s.DownloadGoodies && album.Goodies != null)
        {
            foreach (var goody in album.Goodies)
            {
                if (goody.FileFormatId == 52 || string.IsNullOrEmpty(goody.Url)) continue;
                await DownloadGoodyAsync(downloadPath, album, goody, ct);
            }
        }

        DeleteTempArtwork();
    }

    // ─── Single Track ───

    private async Task DownloadSingleTrackAsync(string trackId, CancellationToken ct)
    {
        var track = await _api.GetTrackAsync(trackId);
        if (track == null) { Log(_loc["log.trackNotFound"]); return; }

        QobuzAlbum? album = null;
        if (track.Album != null)
            album = await _api.GetAlbumFullAsync(track.Album.Id);

        if (album == null) { Log(_loc["log.albumInfoUnavailable"]); return; }
        Progress.CurrentAlbum = album;

        var s = _settings.Settings;
        var downloadPath = _templates.CreatePath(s.DownloadLocation, s.ArtistTemplate, s.AlbumTemplate, null, album, track, null);
        Directory.CreateDirectory(downloadPath);

        await DownloadArtworkAsync(downloadPath, album);
        await DownloadTrackToFileAsync("track", album, track, downloadPath, null, ct);
        DeleteTempArtwork();
    }

    // ─── Playlist ───

    private async Task DownloadPlaylistAsync(string playlistId, CancellationToken ct)
    {
        var playlist = await _api.GetPlaylistFullAsync(playlistId);
        var tracks = playlist.Tracks?.Items ?? new List<QobuzTrack>();
        int total = tracks.Count;
        var s = _settings.Settings;

        for (int i = 0; i < total; i++)
        {
            ct.ThrowIfCancellationRequested();
            var item = tracks[i];
            UpdateProgress((int)((double)i / total * 100), _loc.T("download.status.track", i + 1, total));

            try
            {
                if (s.UseItemPosInPlaylist) item.TrackNumber = item.Position;

                var trackInfo = await _api.GetTrackAsync(item.Id.ToString());
                if (trackInfo == null) continue;

                QobuzAlbum? album = trackInfo.Album != null ? await _api.GetAlbumFullAsync(trackInfo.Album.Id) : null;
                if (album == null) continue;
                Progress.CurrentAlbum = album;

                var downloadPath = _templates.CreatePath(s.DownloadLocation, s.ArtistTemplate, s.AlbumTemplate, s.PlaylistTemplate, album, item, playlist);
                Directory.CreateDirectory(downloadPath);

                await DownloadArtworkAsync(downloadPath, album);
                await DownloadTrackToFileAsync("playlist", album, item, downloadPath, playlist, ct);
                DeleteTempArtwork();
            }
            catch (OperationCanceledException) { throw; }
            catch (Exception ex) { Log(_loc.T("log.playlistTrackFailed", item.Id, ex.Message)); }
        }
    }

    // ─── Artist ───

    private async Task DownloadArtistAsync(string artistId, CancellationToken ct)
    {
        var artist = await _api.GetArtistFullAsync(artistId);
        var albums = artist.Albums?.Items ?? new List<QobuzAlbum>();
        int total = albums.Count;

        for (int i = 0; i < total; i++)
        {
            ct.ThrowIfCancellationRequested();
            if (_skipCurrent) { _skipCurrent = false; Log(_loc["log.albumSkipped"]); continue; }

            UpdateProgress((int)((double)i / total * 100), _loc.T("download.status.album", i + 1, total), albums[i].Title);
            try { await DownloadAlbumAsync(albums[i].Id, ct); }
            catch (OperationCanceledException) { throw; }
            catch (Exception ex) { Log(_loc.T("log.albumFailed", albums[i].Id, ex.Message)); }
        }
    }

    // ─── Label ───

    private async Task DownloadLabelAsync(string labelId, CancellationToken ct)
    {
        var label = await _api.GetLabelAsync(labelId);
        if (label?.Albums?.Items == null) { Log(_loc["log.labelNotFound"]); return; }

        var albums = label.Albums.Items;
        int total = albums.Count;

        for (int i = 0; i < total; i++)
        {
            ct.ThrowIfCancellationRequested();
            if (_skipCurrent) { _skipCurrent = false; continue; }

            UpdateProgress((int)((double)i / total * 100), _loc.T("download.status.album", i + 1, total), albums[i].Title);
            try { await DownloadAlbumAsync(albums[i].Id, ct); }
            catch (OperationCanceledException) { throw; }
            catch (Exception ex) { Log(_loc.T("log.albumFailed", albums[i].Id, ex.Message)); }
        }
    }

    // ─── Favorites ───

    private async Task DownloadFavoritesAlbumsAsync(CancellationToken ct)
    {
        var s = _settings.Settings;
        var favs = await _api.GetFavoritesAsync(s.UserId, "albums");
        if (favs?.Albums?.Items == null) return;

        var albums = favs.Albums.Items;
        int total = albums.Count;
        for (int i = 0; i < total; i++)
        {
            ct.ThrowIfCancellationRequested();
            UpdateProgress((int)((double)i / total * 100), _loc.T("download.status.favAlbum", i + 1, total));
            try { await DownloadAlbumAsync(albums[i].Id, ct); }
            catch (OperationCanceledException) { throw; }
            catch (Exception ex) { Log(_loc.T("log.favAlbumFailed", albums[i].Id, ex.Message)); }
        }
    }

    private async Task DownloadFavoritesTracksAsync(CancellationToken ct)
    {
        var s = _settings.Settings;
        var favs = await _api.GetFavoritesAsync(s.UserId, "tracks");
        if (favs?.Tracks?.Items == null) return;

        var tracks = favs.Tracks.Items;
        int total = tracks.Count;
        for (int i = 0; i < total; i++)
        {
            ct.ThrowIfCancellationRequested();
            UpdateProgress((int)((double)i / total * 100), _loc.T("download.status.favTrack", i + 1, total));
            try { await DownloadSingleTrackAsync(tracks[i].Id.ToString(), ct); }
            catch (OperationCanceledException) { throw; }
            catch (Exception ex) { Log(_loc.T("log.favTrackFailed", tracks[i].Id, ex.Message)); }
        }
    }

    private async Task DownloadFavoritesArtistsAsync(CancellationToken ct)
    {
        var s = _settings.Settings;
        var favs = await _api.GetFavoritesAsync(s.UserId, "artists");
        if (favs?.Artists?.Items == null) return;

        foreach (var artist in favs.Artists.Items)
        {
            ct.ThrowIfCancellationRequested();
            try { await DownloadArtistAsync(artist.Id.ToString(), ct); }
            catch (OperationCanceledException) { throw; }
            catch (Exception ex) { Log(_loc.T("log.favArtistFailed", artist.Id, ex.Message)); }
        }
    }

    // ─── Core: download a single track to file ───

    private async Task DownloadTrackToFileAsync(string downloadType, QobuzAlbum album, QobuzTrack track, string downloadPath, QobuzPlaylist? playlist, CancellationToken ct)
    {
        var s = _settings.Settings;

        // Build file path
        var trackTemplate = _templates.ApplyTemplate(s.TrackTemplate, album, track, playlist);
        var filePath = Path.Combine(downloadPath, trackTemplate.TrimEnd() + s.AudioFormat);

        // Handle multi-disc
        if (track.PlaylistTrackId == null && downloadType != "track" && album.MediaCount > 1 && !string.IsNullOrEmpty(s.CdTemplate))
        {
            var cdDir = Regex.Replace(s.CdTemplate, "%DiscNumber%", track.MediaNumber.ToString(), RegexOptions.IgnoreCase);
            filePath = Path.Combine(downloadPath, cdDir, trackTemplate.TrimEnd() + s.AudioFormat);
        }

        // Skip existing
        if (s.DuplicateFileMode == "skip" && System.IO.File.Exists(filePath))
        {
            Log(_loc.T("log.fileExists", track.TrackNumber));
            return;
        }

        // Prepare temp path (reused across retry attempts)
        var configPath = Environment.GetEnvironmentVariable("CONFIG_PATH") ?? "/config";
        var tempDir = Path.Combine(configPath, "qbdlx-temp");
        Directory.CreateDirectory(tempDir);
        var tempFile = Path.Combine(tempDir, $"qbdlx_downloading-{track.Id}{s.AudioFormat}");

        var titleFormatted = _templates.FormatTrackTitle(track);
        Log(_loc.T("log.downloadingTrack", track.TrackNumber.ToString().PadLeft(2, '0'), titleFormatted));

        // Try requested quality, then fall back to lower qualities if download keeps failing.
        // For each quality we make a few silent attempts (with a fresh signed URL each time)
        // before giving up and stepping down — most transient CDN cuts are fixed by a quick retry.
        // Qobuz CDN sometimes throttles / cuts very-high-res files (24bit/192kHz) for certain accounts.
        const int attemptsPerQuality = 3;
        var qualityChain = BuildQualityFallbackChain(s.QualityFormat);
        Exception? lastError = null;
        bool downloaded = false;

        foreach (var (qFormat, qLabel) in qualityChain)
        {
            if (qFormat != s.QualityFormat)
            {
                Log(_loc.T("log.qualityUnavailable", qLabel));
            }

            Exception? qualityError = null;
            bool qualityUnavailable = false;

            for (int attempt = 1; attempt <= attemptsPerQuality; attempt++)
            {
                try
                {
                    var stream = await _api.GetStreamUrlAsync(track.Id.ToString(), qFormat);
                    if (stream == null || string.IsNullOrEmpty(stream.Url))
                    {
                        qualityUnavailable = true;
                        break;
                    }

                    await DownloadStreamToFileAsync(stream.Url, tempFile, ct);
                    downloaded = true;
                    lastError = null;
                    qualityError = null;
                    break;
                }
                catch (OperationCanceledException) { throw; }
                catch (Exception ex)
                {
                    qualityError = ex;
                    try { if (System.IO.File.Exists(tempFile)) System.IO.File.Delete(tempFile); } catch { }

                    if (attempt < attemptsPerQuality)
                    {
                        // Silent retry — short backoff, no log noise
                        await Task.Delay(TimeSpan.FromSeconds(attempt), ct);
                    }
                }
            }

            if (downloaded) break;

            if (qualityUnavailable)
            {
                Log(_loc.T("log.trackNotInQuality", track.TrackNumber, qLabel));
            }
            else if (qualityError != null)
            {
                lastError = qualityError;
                Log(_loc.T("log.qualityFailed", qLabel, attemptsPerQuality, qualityError.Message));
            }
        }

        if (!downloaded)
        {
            throw lastError ?? new IOException("Téléchargement impossible (toutes qualités échouées)");
        }

        // Fix MD5 for FLAC
        if (s.FixMD5s && s.AudioFormat.Contains("flac"))
        {
            try
            {
                var proc = Process.Start(new ProcessStartInfo("flac", $"-f8 \"{tempFile}\"")
                {
                    CreateNoWindow = true, RedirectStandardOutput = true, RedirectStandardError = true
                });
                if (proc != null) await proc.WaitForExitAsync(ct);
            }
            catch (Exception ex) { _logger.LogWarning("flac MD5 fix failed: {Error}", ex.Message); }
        }

        // Tag file
        var embeddedArtPath = Path.Combine(Path.GetTempPath(), $"qbdlx-{_sessionId}-{s.EmbeddedArtSize}.jpg");
        _tagging.WriteToFile(tempFile, System.IO.File.Exists(embeddedArtPath) ? embeddedArtPath : null, album, track);

        // Move to final location
        var dir = Path.GetDirectoryName(filePath);
        if (!string.IsNullOrEmpty(dir)) Directory.CreateDirectory(dir);

        if (s.DuplicateFileMode == "rename" && System.IO.File.Exists(filePath))
        {
            var name = Path.GetFileNameWithoutExtension(filePath);
            var ext = Path.GetExtension(filePath);
            var dirPath = Path.GetDirectoryName(filePath) ?? downloadPath;
            int counter = 1;
            while (System.IO.File.Exists(filePath))
            {
                filePath = Path.Combine(dirPath, $"{name} ({counter}){ext}");
                counter++;
            }
        }

        System.IO.File.Move(tempFile, filePath, s.DuplicateFileMode == "overwrite");
        Log(_loc["log.done"]);
    }

    // ─── HTTP stream download helpers ───

    /// <summary>
    /// Returns the chain of (format_id, label) to try for downloading a track.
    /// Starts with the user's chosen quality, then falls back to lower qualities
    /// if Qobuz refuses or throttles the high-res stream.
    /// </summary>
    private static IEnumerable<(string format, string label)> BuildQualityFallbackChain(string requested)
    {
        // Quality order from highest to lowest:
        //  27 = FLAC 24bit 192kHz, 7 = FLAC 24bit 96kHz, 6 = FLAC 16bit 44.1kHz, 5 = MP3 320
        var allQualities = new (string format, string label)[]
        {
            ("27", "FLAC 24bit/192kHz"),
            ("7",  "FLAC 24bit/96kHz"),
            ("6",  "FLAC 16bit/44.1kHz"),
            ("5",  "MP3 320"),
        };

        // Find requested quality, then yield it followed by lower ones
        bool foundRequested = false;
        foreach (var q in allQualities)
        {
            if (q.format == requested) foundRequested = true;
            if (foundRequested) yield return q;
        }
        // If the requested quality wasn't in our list (e.g. unknown id), just try it raw
        if (!foundRequested)
        {
            yield return (requested, $"format {requested}");
            foreach (var q in allQualities) yield return q;
        }
    }

    /// <summary>
    /// Downloads a signed Qobuz stream URL to a local temp file in one shot.
    /// Throws on any failure (caller handles fallback to a lower quality).
    /// </summary>
    private async Task DownloadStreamToFileAsync(string url, string tempFile, CancellationToken ct)
    {
        // Start clean — never resume; signed URLs are essentially single-use here.
        if (System.IO.File.Exists(tempFile))
        {
            try { System.IO.File.Delete(tempFile); } catch { }
        }

        using var httpClient = new HttpClient { Timeout = TimeSpan.FromMinutes(30) };
        using var response = await httpClient.GetAsync(url, HttpCompletionOption.ResponseHeadersRead, ct);
        response.EnsureSuccessStatusCode();

        long totalBytes = response.Content.Headers.ContentLength ?? -1;
        long bytesRead = 0;
        var buffer = new byte[81920];
        var sw = Stopwatch.StartNew();

        await using (var fs = new FileStream(tempFile, FileMode.Create, FileAccess.Write, FileShare.Read, 81920, true))
        await using (var contentStream = await response.Content.ReadAsStreamAsync(ct))
        {
            int read;
            while ((read = await contentStream.ReadAsync(buffer, ct)) > 0)
            {
                ct.ThrowIfCancellationRequested();
                await fs.WriteAsync(buffer.AsMemory(0, read), ct);
                bytesRead += read;

                if (totalBytes > 0 && sw.ElapsedMilliseconds > 250)
                {
                    var pct = (int)(bytesRead * 100 / totalBytes);
                    var speed = bytesRead / (sw.Elapsed.TotalSeconds + 0.001);
                    Progress.Speed = speed >= 1024 * 1024
                        ? $"{speed / (1024 * 1024):F2} MB/s"
                        : $"{speed / 1024:F2} KB/s";
                    Progress.Status = _loc.T("download.status.downloading", pct, Progress.Speed);
                    OnProgressChanged?.Invoke();
                }
            }
        }

        if (totalBytes > 0 && bytesRead < totalBytes)
        {
            throw new IOException($"Téléchargement incomplet : {bytesRead}/{totalBytes} octets");
        }
    }

    // ─── Artwork ───

    private async Task DownloadArtworkAsync(string downloadPath, QobuzAlbum album)
    {
        var s = _settings.Settings;
        using var httpClient = new HttpClient { Timeout = TimeSpan.FromMinutes(2) };

        try
        {
            // Cover.jpg to disk
            if (!s.DontSaveArtworkToDisk)
            {
                var coverPath = Path.Combine(downloadPath, "Cover.jpg");
                if (!System.IO.File.Exists(coverPath))
                {
                    var url = album.Image.Large.Replace("_600", $"_{s.SavedArtSize}");
                    var data = await httpClient.GetByteArrayAsync(url);
                    Directory.CreateDirectory(downloadPath);
                    await System.IO.File.WriteAllBytesAsync(coverPath, data);
                }
            }

            // Embedded artwork to temp
            var embeddedPath = Path.Combine(Path.GetTempPath(), $"qbdlx-{_sessionId}-{s.EmbeddedArtSize}.jpg");
            if (!System.IO.File.Exists(embeddedPath))
            {
                var url = album.Image.Large.Replace("_600", $"_{s.EmbeddedArtSize}");
                var data = await httpClient.GetByteArrayAsync(url);
                await System.IO.File.WriteAllBytesAsync(embeddedPath, data);
            }
        }
        catch (Exception ex) { _logger.LogWarning("Artwork download failed: {Error}", ex.Message); }
    }

    private async Task DownloadGoodyAsync(string downloadPath, QobuzAlbum album, QobuzGoody goody, CancellationToken ct)
    {
        var fileName = TemplateService.MakeSafeFilename(goody.Description ?? album.Title);
        var ext = Path.GetExtension(goody.Url ?? ".pdf");
        var filePath = Path.Combine(downloadPath, $"{fileName} ({goody.Id}){ext}");

        if (System.IO.File.Exists(filePath)) { Log(_loc["log.goodyExists"]); return; }

        try
        {
            Log(_loc["log.goodyFound"]);
            using var http = new HttpClient { Timeout = TimeSpan.FromMinutes(5) };
            var data = await http.GetByteArrayAsync(goody.Url, ct);
            await System.IO.File.WriteAllBytesAsync(filePath, data, ct);
            Log(_loc["log.done"]);
        }
        catch (Exception ex) { Log(_loc.T("log.goodyFailed", ex.Message)); }
    }

    private void DeleteTempArtwork()
    {
        try
        {
            var s = _settings.Settings;
            var path = Path.Combine(Path.GetTempPath(), $"qbdlx-{_sessionId}-{s.EmbeddedArtSize}.jpg");
            if (System.IO.File.Exists(path)) System.IO.File.Delete(path);
        }
        catch { }
    }
}
