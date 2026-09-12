using System.Net;
using System.Reflection;
using Microsoft.AspNetCore.Builder;
using Microsoft.AspNetCore.Hosting;
using Microsoft.AspNetCore.Http;
using Microsoft.AspNetCore.Routing;
using Microsoft.Extensions.Hosting;
using Microsoft.Extensions.Logging;
using QobuzDLX.Web.Models;
using QobuzDLX.Web.Services;
using Xunit;

namespace QobuzDLX.Web.Tests;

public static class QueueSummaryTests
{
    [Fact]
    public static void Each_item_reports_downloaded_skipped_and_failed_tracks()
    {
        var cdn = FakeBackend.StartCdn();
        try
        {
            var (fx, qobuz, downloads) = FakeBackend.NewLibrary();
            const string url = "https://play.qobuz.com/album/alb";

            // track 3 is already on disk
            var albumModel = new QobuzAlbum { Id = "alb", Title = "Alb", TracksCount = 3, Artist = new QobuzPerformer { Name = "Art" } };
            var folder = fx.Templates.CreatePath(downloads, "%ArtistName%", "%AlbumTitle%", null, albumModel, null, null);
            Directory.CreateDirectory(folder);
            var existing = Path.Combine(folder, fx.Templates.ApplyTemplate("%TrackNumber% - %TrackTitle%", albumModel, new QobuzTrack { Id = 3, Title = "Three", TrackNumber = 3 }, null) + ".flac");
            File.WriteAllText(existing, "already here");

            fx.Download.Enqueue(url);
            var item = FakeBackend.WaitFinished(fx, url);
            H.Check("the item completes", item.Status == QueueItemStatus.Completed);
            H.Check("one track downloaded", item.Downloaded == 1);
            H.Check("one track skipped (already on disk)", item.Skipped == 1);
            H.Check("one failure, identified by id and title", item.Failures.Count == 1 && item.Failures[0].TrackId == 2 && item.Failures[0].Title == "Two");
            H.Check("the failure carries Qobuz's reason", item.Failures[0].Error.Contains("Track restricted"));
            H.Check("the log gets a summary line", fx.Download.LogSnapshot().Any(l => l.Contains(fx.Loc.T("log.itemSummary", 1, 1, 1))));
            H.Check("the existing file was left alone", File.ReadAllText(existing) == "already here");

            // retry only the failure
            qobuz.Track2Works = true;
            lock (qobuz.Requests) qobuz.Requests.Clear();
            fx.Download.RetryFailures(item.Id);
            item = FakeBackend.WaitFinished(fx, url);
            H.Check("the retry completes with no failure left", item.Status == QueueItemStatus.Completed && item.Failures.Count == 0);
            H.Check("earlier results are kept and the retried track counted", item.Downloaded == 2 && item.Skipped == 1);
            string[] seen; lock (qobuz.Requests) seen = qobuz.Requests.ToArray();
            H.Check("the retry never asked for the other tracks", !seen.Any(r => r.EndsWith("?1") && r.Contains("track")) && !seen.Any(r => r.EndsWith("?3") && r.Contains("track")));
            H.Check("the retry did ask for track 2", seen.Any(r => r.Contains("/track/getFileUrl") && r.EndsWith("?2")));
            H.Check("retried track is on disk", Directory.GetFiles(folder, "2 - Two*.flac").Length == 1);

            // a full retry resets the summary
            fx.Download.Retry(item.Id);
            item = FakeBackend.WaitFinished(fx, url);
            H.Check("a full retry recounts from zero (3 files now exist)", item.Downloaded == 0 && item.Skipped == 3 && item.Failures.Count == 0);

            // retry-failures on an item without failures does nothing
            fx.Download.RetryFailures(item.Id);
            H.Check("nothing to retry: item stays finished", fx.Download.QueueSnapshot().First(i => i.Id == item.Id).IsFinished);
        }
        finally
        {
            cdn.StopAsync().GetAwaiter().GetResult();
        }
    }
}
