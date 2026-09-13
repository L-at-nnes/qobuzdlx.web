using Newtonsoft.Json;
using Newtonsoft.Json.Converters;

namespace QobuzDLX.Web.Services;

/// <summary>Un élément de la file tel qu'il est écrit dans <c>queue.json</c>.</summary>
public sealed class PersistedQueueItem
{
    public string Id { get; set; } = "";
    public string Url { get; set; } = "";
    public string Kind { get; set; } = "";
    public string RefId { get; set; } = "";
    public string Label { get; set; } = "";
    public string? SubLabel { get; set; }
    public string? Thumbnail { get; set; }
    [JsonConverter(typeof(StringEnumConverter))]
    public QueueItemStatus Status { get; set; }
    public string? Error { get; set; }
    public DateTime EnqueuedAt { get; set; }
    public DateTime? StartedAt { get; set; }
    public DateTime? FinishedAt { get; set; }
    public int Downloaded { get; set; }
    public int Skipped { get; set; }
    public List<FailedTrack> Failures { get; set; } = new();
    public List<PlaylistEntry> PlaylistEntries { get; set; } = new();
    public List<int>? OnlyTrackIds { get; set; }

    public static PersistedQueueItem From(QueueItem item) => new()
    {
        Id = item.Id, Url = item.Url, Kind = item.Kind, RefId = item.RefId,
        Label = item.Label, SubLabel = item.SubLabel, Thumbnail = item.Thumbnail,
        Status = item.Status, Error = item.Error,
        EnqueuedAt = item.EnqueuedAt, StartedAt = item.StartedAt, FinishedAt = item.FinishedAt,
        Downloaded = item.Downloaded, Skipped = item.Skipped,
        Failures = item.Failures.ToList(), PlaylistEntries = item.PlaylistEntries.ToList(),
        OnlyTrackIds = item.OnlyTrackIds?.ToList()
    };

    public QueueItem ToQueueItem() => new()
    {
        Id = Id, Url = Url, Kind = Kind, RefId = RefId,
        Label = Label, SubLabel = SubLabel, Thumbnail = Thumbnail,
        Status = Status, Error = Error,
        EnqueuedAt = EnqueuedAt, StartedAt = StartedAt, FinishedAt = FinishedAt,
        Downloaded = Downloaded, Skipped = Skipped,
        Failures = Failures, PlaylistEntries = PlaylistEntries,
        OnlyTrackIds = OnlyTrackIds?.ToHashSet()
    };
}

/// <summary>
/// La file de téléchargement dans <c>/config/queue.json</c>, pour qu'un redémarrage du conteneur (mise à
/// jour, coupure de courant) ne la fasse pas oublier, ni ce qui a déjà été téléchargé (les ✓ de la
/// recherche).
/// </summary>
public sealed class QueueStore
{
    /// <summary>Les éléments terminés les plus anciens sont oubliés au-delà : le fichier reste petit.</summary>
    public const int MaxFinishedItems = 200;

    private readonly string _path;
    private readonly ILogger _logger;

    public QueueStore(string path, ILogger logger)
    {
        _path = path;
        _logger = logger;
    }

    /// <summary>
    /// Les éléments enregistrés. Un élément qui tournait à l'arrêt du processus est de nouveau en attente :
    /// il est repris depuis le début, les pistes déjà écrites étant simplement reconnues comme présentes. Un
    /// fichier illisible est mis de côté (<c>queue.json.corrupt-…</c>) et la file repart vide.
    /// </summary>
    public IReadOnlyList<QueueItem> Load()
    {
        if (!File.Exists(_path)) return Array.Empty<QueueItem>();

        try
        {
            var stored = JsonConvert.DeserializeObject<List<PersistedQueueItem>>(File.ReadAllText(_path)) ?? new();
            return stored.Select(Restore).ToList();
        }
        catch (Exception ex) when (ex is JsonException or IOException or UnauthorizedAccessException)
        {
            var aside = $"{_path}.corrupt-{DateTime.UtcNow:yyyyMMddHHmmss}";
            _logger.LogWarning(ex, "The saved download queue could not be read; it was set aside as {File}", aside);
            try { File.Move(_path, aside); } catch (Exception move) when (move is IOException or UnauthorizedAccessException) { }
            return Array.Empty<QueueItem>();
        }
    }

    private static QueueItem Restore(PersistedQueueItem stored)
    {
        var item = stored.ToQueueItem();
        if (item.Status != QueueItemStatus.Running) return item;

        item.Status = QueueItemStatus.Pending;
        item.StartedAt = null;
        // A whole item restarts from scratch: what it had counted so far would be counted twice. A "retry the
        // failures" pass keeps its earlier outcome, which is what its own counters build on.
        if (item.OnlyTrackIds == null) item.ResetOutcome();
        return item;
    }

    /// <summary>Écrit la file : fichier voisin puis substitution, comme les réglages, pour qu'une coupure en
    /// cours d'écriture ne laisse jamais un fichier tronqué.</summary>
    public void Save(IReadOnlyList<PersistedQueueItem> items)
    {
        var finished = items.Where(i => i.Status is QueueItemStatus.Completed or QueueItemStatus.Failed or QueueItemStatus.Cancelled).ToList();
        var dropped = finished.Count > MaxFinishedItems ? finished.Take(finished.Count - MaxFinishedItems).ToHashSet() : null;
        var kept = dropped == null ? items : items.Where(i => !dropped.Contains(i)).ToList();

        Directory.CreateDirectory(Path.GetDirectoryName(_path)!);
        var temp = _path + ".tmp";
        File.WriteAllText(temp, JsonConvert.SerializeObject(kept, Formatting.Indented));
        File.Move(temp, _path, overwrite: true);
    }
}
