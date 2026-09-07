using System.Diagnostics;
using System.Text.RegularExpressions;
using QobuzDLX.Web.Models;

namespace QobuzDLX.Web.Services;

public enum QueueItemStatus { Pending, Running, Completed, Failed, Cancelled }

public enum EnqueueResult { Added, Requeued, AlreadyQueued, InvalidUrl }

public class QueueItem
{
    /// <summary>Identité interne stable : sert de @key Blazor et de cible aux actions de l'UI.
    /// Ne participe jamais à l'unicité, qui repose sur <see cref="Url"/>.</summary>
    public string Id { get; init; } = Guid.NewGuid().ToString("N")[..8];

    /// <summary>Clé métier : URL canonique play.qobuz.com/{kind}/{refId}, sans requête ni fragment.
    /// C'est elle qui porte l'unicité, le grisage des boutons et la marque « déjà téléchargé ».</summary>
    public string Url { get; init; } = "";

    public string Kind { get; init; } = "";   // album | track | artist | playlist | label | user
    public string RefId { get; init; } = "";

    // Affichage. Renseignés par l'appelant quand il les a sous la main (page Recherche) ; repli sur
    // l'URL quand l'ajout vient du champ libre de la page Téléchargement.
    public string Label { get; set; } = "";
    public string? SubLabel { get; set; }
    public string? Thumbnail { get; set; }

    public QueueItemStatus Status { get; set; } = QueueItemStatus.Pending;
    public string? Error { get; set; }

    public DateTime EnqueuedAt { get; set; } = DateTime.UtcNow;
    public DateTime? StartedAt { get; set; }
    public DateTime? FinishedAt { get; set; }

    public bool IsFinished =>
        Status is QueueItemStatus.Completed or QueueItemStatus.Failed or QueueItemStatus.Cancelled;
}

public class DownloadProgress
{
    public int Percent { get; set; }
    public string Status { get; set; } = "";
    public string CurrentItem { get; set; } = "";
    public string Speed { get; set; } = "";

    /// <summary>Un élément de la file est en cours de traitement.</summary>
    public bool IsRunning { get; set; }

    /// <summary>La file a été drainée au moins une fois et est au repos.</summary>
    public bool IsComplete { get; set; }

    public QobuzAlbum? CurrentAlbum { get; set; }

    /// <summary>Identifiant de l'élément en cours, pour surligner sa ligne sans comparer de libellé.</summary>
    public string? CurrentQueueItemId { get; set; }
}

/// <summary>Demande d'ajout à la file. Les champs d'affichage sont facultatifs : la page Recherche
/// les connaît, le champ libre de la page Téléchargement non.</summary>
public record QueueRequest(string Url, string? Label = null, string? SubLabel = null, string? Thumbnail = null);

public class DownloadService
{
    private readonly QobuzApiService _api;
    private readonly SettingsService _settings;
    private readonly TemplateService _templates;
    private readonly TaggingService _tagging;
    private readonly LocalizationService _loc;
    private readonly ILogger<DownloadService> _logger;

    // ─── État partagé ───
    //
    // Un verrou unique protège la file, le journal et les deux index dérivés. Le service est
    // Singleton et le worker tourne hors du circuit Blazor : toute lecture destinée au rendu passe
    // par un instantané (QueueSnapshot / LogSnapshot), jamais par la collection vive, sinon
    // l'énumération concurrente lève « Collection was modified ».
    private readonly object _gate = new();
    private readonly List<QueueItem> _queue = new();
    private readonly List<string> _log = new();

    // Index dérivés de _queue, pour que le grisage des boutons de la page Recherche ne soit pas
    // un balayage linéaire par bouton rendu.
    private readonly HashSet<string> _activeUrls = new(StringComparer.OrdinalIgnoreCase);
    private readonly HashSet<string> _doneUrls = new(StringComparer.OrdinalIgnoreCase);

    // Trois portées d'annulation, trois gestes distincts :
    //   _appCts      → l'application s'arrête (docker stop) : on abandonne tout, immédiatement
    //   _itemCts     → « ANNULER » : l'élément courant est abandonné, le worker enchaîne
    //   _skipCurrent → « PASSER » : saute l'album courant, reste dans le même élément
    private readonly CancellationTokenSource _appCts = new();
    private readonly CancellationToken _appCt;
    private CancellationTokenSource? _itemCts;
    private volatile bool _skipCurrent;

    // Réveil du worker. Borné à 1 : enfiler cinquante éléments d'un coup ne provoque pas cinquante
    // réveils, et le worker draine de toute façon jusqu'à ne plus rien trouver.
    private readonly SemaphoreSlim _signal = new(0, 1);
    private Task? _worker;

    private string _sessionId = "";
    private long _lastProgressTick;

    private const int LogMaxLines = 1200;
    private const int LogTrimLines = 400;

    public DownloadProgress Progress { get; } = new();

    /// <summary>Haute fréquence (pourcentage, vitesse) : étranglé à 200 ms, sauf appel forcé.</summary>
    public event Action? OnProgressChanged;

    /// <summary>Basse fréquence (ajout, fin, réordonnancement) : jamais étranglé. C'est celui-là
    /// qu'écoutent la pastille du menu et le grisage des boutons de recherche — les abonner à
    /// OnProgressChanged redessinerait tous les circuits ouverts à chaque bloc reçu.</summary>
    public event Action? OnQueueChanged;

    private static readonly Regex QobuzStoreLinkRegex = new(
        @"https://(?:.*?).qobuz.com/(?<region>.*?)/(?<type>.*?)/(?<name>.*?)/(?<id>.*?)$", RegexOptions.Compiled);
    private static readonly Regex QobuzLinkIdGrabRegex = new(
        @"https://(?:.*?).qobuz.com/(?<type>.*?)/(?<id>.*?)$", RegexOptions.Compiled);
    private static readonly Regex QobuzUrlRegex = new(
        @"^https?://(?!.*https?://)(?:[\w\-]+\.)?qobuz\.com/[^\s]+$", RegexOptions.Compiled | RegexOptions.IgnoreCase);

    public DownloadService(QobuzApiService api, SettingsService settings, TemplateService templates, TaggingService tagging, LocalizationService loc, IHostApplicationLifetime lifetime, ILogger<DownloadService> logger)
    {
        _api = api;
        _settings = settings;
        _templates = templates;
        _tagging = tagging;
        _loc = loc;
        _logger = logger;

        _appCt = _appCts.Token;
        // Sans cela, `docker stop` attendrait le délai de grâce complet pendant qu'un téléchargement
        // continue dans le vide.
        lifetime.ApplicationStopping.Register(() => { try { _appCts.Cancel(); } catch { } });

        MigrateTempFolder();
        PurgeStaleTempFiles();
    }

    private static string TempDirectory =>
        Path.Combine(Environment.GetEnvironmentVariable("CONFIG_PATH") ?? "/config", "qbdlx-temp");

    /// <summary>
    /// Migrate qbdlx-temp from /downloads to /config for existing installations.
    /// </summary>
    private void MigrateTempFolder()
    {
        try
        {
            var oldTempDir = Path.Combine(_settings.Settings.DownloadLocation, "qbdlx-temp");
            var newTempDir = TempDirectory;

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

    /// <summary>
    /// A previous run killed mid-download (docker kill, power loss) leaves partial audio files and
    /// cover art behind. No download is running yet at construction time, so everything carrying our
    /// prefixes is stale.
    /// </summary>
    private void PurgeStaleTempFiles()
    {
        try
        {
            if (Directory.Exists(TempDirectory))
            {
                foreach (var file in Directory.EnumerateFiles(TempDirectory, "qbdlx_downloading-*"))
                    DeleteQuietly(file);
            }

            foreach (var file in Directory.EnumerateFiles(Path.GetTempPath(), "qbdlx-*.jpg"))
                DeleteQuietly(file);
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
        {
            _logger.LogWarning(ex, "Failed to purge stale temporary files");
        }
    }

    // ─── Lecture pour l'interface ───

    /// <summary>Un élément de la file est en cours de traitement. Ne grise plus aucun bouton
    /// d'ajout : c'est tout l'intérêt de la file.</summary>
    public bool IsRunning => Progress.IsRunning;

    /// <summary>Copie sous verrou. Les champs des QueueItem retournés sont mutés par le worker sous
    /// ce même verrou ; la barrière mémoire prise ici garantit donc qu'on les lit à jour, et les
    /// relire hors verrou après l'instantané est sûr. Ne pas « optimiser » en retournant _queue.</summary>
    public IReadOnlyList<QueueItem> QueueSnapshot()
    {
        lock (_gate) return _queue.ToList();
    }

    public IReadOnlyList<string> LogSnapshot()
    {
        lock (_gate) return _log.ToList();
    }

    /// <summary>Pour détecter l'arrivée de nouvelles lignes sans recopier tout le journal.</summary>
    public int LogCount
    {
        get { lock (_gate) return _log.Count; }
    }

    /// <summary>En attente + en cours : ce qu'affiche la pastille du menu.</summary>
    public int RemainingCount
    {
        get { lock (_gate) return _queue.Count(i => !i.IsFinished); }
    }

    /// <summary>Déjà en file (en attente ou en cours) : le bouton correspondant est grisé.</summary>
    public bool IsQueued(string url)
    {
        if (!TryParseQobuzUrl(url, out var canonical, out _, out _)) return false;
        lock (_gate) return _activeUrls.Contains(canonical);
    }

    /// <summary>Terminé avec succès : le bouton reste cliquable, avec une marque ✓.</summary>
    public bool IsDone(string url)
    {
        if (!TryParseQobuzUrl(url, out var canonical, out _, out _)) return false;
        lock (_gate) return _doneUrls.Contains(canonical);
    }

    // ─── Analyse d'URL ───

    /// <summary>
    /// Valide une URL Qobuz et en extrait la forme canonique play.qobuz.com/{kind}/{refId}.
    /// Pure et statique : appelée à l'ajout, jamais au traitement, pour que deux écritures de la
    /// même adresse (avec ou sans ?utm_source=…) produisent la même clé et donc un seul élément.
    /// </summary>
    public static bool TryParseQobuzUrl(string? input, out string canonicalUrl, out string kind, out string refId)
    {
        canonicalUrl = ""; kind = ""; refId = "";
        if (string.IsNullOrWhiteSpace(input)) return false;

        var url = input.Trim();

        // Couper requête et fragment AVANT toute chose : QobuzUrlRegex se termine par [^\s]+$ et
        // les laisserait passer jusque dans l'identifiant, créant un doublon invisible.
        var cut = url.IndexOfAny(new[] { '?', '#' });
        if (cut >= 0) url = url[..cut];

        url = url.TrimEnd('/');
        if (url.EndsWith("/releases", StringComparison.OrdinalIgnoreCase))
            url = url[..^"/releases".Length];

        if (!QobuzUrlRegex.IsMatch(url)) return false;

        // Lien boutique (qobuz.com/fr-fr/album/nom/id) → lien lecteur
        var storeMatch = QobuzStoreLinkRegex.Match(url);
        if (storeMatch.Success)
        {
            var storeType = storeMatch.Groups["type"].Value;
            var storeId = storeMatch.Groups["id"].Value;
            if (storeType == "album") url = $"https://play.qobuz.com/album/{storeId}";
            else if (storeType == "interpreter") url = $"https://play.qobuz.com/artist/{storeId}";
        }

        var linkMatch = QobuzLinkIdGrabRegex.Match(url);
        if (!linkMatch.Success) return false;

        kind = linkMatch.Groups["type"].Value;
        refId = linkMatch.Groups["id"].Value;   // pour « user », contient des « / » (library/favorites/albums)
        if (string.IsNullOrEmpty(kind) || string.IsNullOrEmpty(refId)) return false;

        canonicalUrl = $"https://play.qobuz.com/{kind}/{refId}";
        return true;
    }

    // ─── Ajout à la file ───

    public EnqueueResult Enqueue(string url, string? label = null, string? subLabel = null, string? thumbnail = null)
    {
        if (!TryParseQobuzUrl(url, out var canonical, out var kind, out var refId))
        {
            Log(_loc.T("log.invalidUrl", url));
            return EnqueueResult.InvalidUrl;
        }

        EnqueueResult result;
        lock (_gate)
        {
            var existing = _queue.FirstOrDefault(i =>
                string.Equals(i.Url, canonical, StringComparison.OrdinalIgnoreCase));

            if (existing is { IsFinished: false })
            {
                result = EnqueueResult.AlreadyQueued;
            }
            else if (existing != null)
            {
                // Déjà traité : le remettre en attente en fin de file plutôt que d'en créer un
                // doublon, sinon l'unicité par URL ne tiendrait plus.
                existing.Status = QueueItemStatus.Pending;
                existing.Error = null;
                existing.StartedAt = null;
                existing.FinishedAt = null;
                existing.EnqueuedAt = DateTime.UtcNow;
                if (!string.IsNullOrWhiteSpace(label)) existing.Label = label!;
                if (!string.IsNullOrWhiteSpace(subLabel)) existing.SubLabel = subLabel;
                if (!string.IsNullOrWhiteSpace(thumbnail)) existing.Thumbnail = thumbnail;
                _queue.Remove(existing);
                _queue.Add(existing);
                ReindexLocked();
                result = EnqueueResult.Requeued;
            }
            else
            {
                _queue.Add(new QueueItem
                {
                    Url = canonical,
                    Kind = kind,
                    RefId = refId,
                    Label = string.IsNullOrWhiteSpace(label) ? canonical : label!,
                    SubLabel = subLabel,
                    Thumbnail = thumbnail
                });
                ReindexLocked();
                result = EnqueueResult.Added;
            }

            // Démarrage paresseux : aucun thread ne tourne tant que rien n'a été demandé.
            // Task.Run se contente de mettre en file d'attente du pool, donc pas de réentrance ici.
            if (result != EnqueueResult.AlreadyQueued)
                _worker ??= Task.Run(WorkerLoopAsync);
        }

        // Hors verrou : un abonné Blazor poste sur le dispatcher de son circuit, lever l'événement
        // sous _gate inviterait l'interblocage.
        if (result != EnqueueResult.AlreadyQueued)
        {
            Signal();
            RaiseQueueChanged();
        }
        return result;
    }

    public (int added, int requeued, int duplicates, int invalid) EnqueueMany(IEnumerable<QueueRequest> requests)
    {
        int added = 0, requeued = 0, duplicates = 0, invalid = 0;
        foreach (var r in requests)
        {
            switch (Enqueue(r.Url, r.Label, r.SubLabel, r.Thumbnail))
            {
                case EnqueueResult.Added: added++; break;
                case EnqueueResult.Requeued: requeued++; break;
                case EnqueueResult.AlreadyQueued: duplicates++; break;
                default: invalid++; break;
            }
        }
        return (added, requeued, duplicates, invalid);
    }

    // ─── Gestes ───

    /// <summary>Saute l'album courant à l'intérieur de l'élément en cours (un artiste en compte
    /// souvent des dizaines). L'élément, lui, poursuit.</summary>
    public void SkipCurrent() => _skipCurrent = true;

    /// <summary>Abandonne l'élément en cours dans son entier ; le worker enchaîne sur le suivant.</summary>
    public void CancelCurrent()
    {
        try { _itemCts?.Cancel(); } catch (ObjectDisposedException) { }
    }

    /// <summary>Purge les éléments en attente. Celui qui tourne va au bout — c'est la différence
    /// avec « ANNULER », et elle est volontaire.</summary>
    public void ClearPending()
    {
        bool changed;
        lock (_gate)
        {
            changed = _queue.RemoveAll(i => i.Status == QueueItemStatus.Pending) > 0;
            if (changed) ReindexLocked();
        }
        if (changed) RaiseQueueChanged();
    }

    public void ClearFinished()
    {
        bool changed;
        lock (_gate)
        {
            changed = _queue.RemoveAll(i => i.IsFinished) > 0;
            if (changed) ReindexLocked();
        }
        if (changed) RaiseQueueChanged();
    }

    public void Remove(string itemId)
    {
        bool changed;
        lock (_gate)
        {
            // L'élément en cours ne se retire pas : il faut d'abord l'annuler.
            changed = _queue.RemoveAll(i => i.Id == itemId && i.Status != QueueItemStatus.Running) > 0;
            if (changed) ReindexLocked();
        }
        if (changed) RaiseQueueChanged();
    }

    public void MoveUp(string itemId) => Move(itemId, -1);
    public void MoveDown(string itemId) => Move(itemId, +1);

    private void Move(string itemId, int direction)
    {
        bool changed = false;
        lock (_gate)
        {
            var idx = _queue.FindIndex(i => i.Id == itemId);
            if (idx >= 0 && _queue[idx].Status == QueueItemStatus.Pending)
            {
                // On n'échange qu'avec un autre élément en attente : les terminés et celui qui
                // tourne gardent leur place, sinon l'ordre affiché sauterait sans raison visible.
                var target = idx + direction;
                while (target >= 0 && target < _queue.Count && _queue[target].Status != QueueItemStatus.Pending)
                    target += direction;

                if (target >= 0 && target < _queue.Count)
                {
                    (_queue[idx], _queue[target]) = (_queue[target], _queue[idx]);
                    changed = true;   // l'ordre change, pas les statuts : pas de réindexation
                }
            }
        }
        if (changed) RaiseQueueChanged();
    }

    public void Retry(string itemId)
    {
        bool changed = false;
        lock (_gate)
        {
            var item = _queue.FirstOrDefault(i => i.Id == itemId);
            if (item is { IsFinished: true })
            {
                item.Status = QueueItemStatus.Pending;
                item.Error = null;
                item.StartedAt = null;
                item.FinishedAt = null;
                item.EnqueuedAt = DateTime.UtcNow;
                _queue.Remove(item);
                _queue.Add(item);
                ReindexLocked();
                _worker ??= Task.Run(WorkerLoopAsync);
                changed = true;
            }
        }
        if (changed) { Signal(); RaiseQueueChanged(); }
    }

    public void ClearLog()
    {
        lock (_gate) _log.Clear();
        RaiseProgress(force: true);
    }

    // ─── Mécanique interne ───

    /// <summary>À appeler sous _gate après toute mutation de statut ou de composition de la file.
    /// Reconstruire les index plutôt que d'y appliquer des deltas : la file compte quelques
    /// dizaines d'éléments au plus, et un index dérivé ne peut alors pas diverger de sa source.</summary>
    private void ReindexLocked()
    {
        _activeUrls.Clear();
        _doneUrls.Clear();
        foreach (var it in _queue)
        {
            if (!it.IsFinished) _activeUrls.Add(it.Url);
            else if (it.Status == QueueItemStatus.Completed) _doneUrls.Add(it.Url);
        }
    }

    private void Signal()
    {
        // Borné à 1 : le sémaphore est un drapeau « il y a du travail », pas un compteur.
        try { _signal.Release(); } catch (SemaphoreFullException) { }
    }

    /// <summary>Mute le statut sous verrou puis notifie hors verrou. Toutes les transitions passent
    /// par ici, c'est ce qui garantit que les index restent cohérents.</summary>
    private void SetStatus(QueueItem item, QueueItemStatus status, string? error = null)
    {
        lock (_gate)
        {
            item.Status = status;
            item.Error = error;
            if (item.IsFinished) item.FinishedAt = DateTime.UtcNow;
            ReindexLocked();
        }
        RaiseQueueChanged();
    }

    private void RaiseProgress(bool force = false)
    {
        // Sans cet étranglement, la boucle d'octets de DownloadStreamToFileAsync redessinerait tous
        // les circuits ouverts à chaque bloc de 80 Ko, soit des dizaines de fois par seconde.
        if (!force && Environment.TickCount64 - _lastProgressTick < 200) return;
        _lastProgressTick = Environment.TickCount64;
        RaiseSafely(OnProgressChanged, nameof(OnProgressChanged));
    }

    private void RaiseQueueChanged() => RaiseSafely(OnQueueChanged, nameof(OnQueueChanged));

    /// <summary>
    /// Invoque chaque abonné séparément, en absorbant ses exceptions. Deux raisons, toutes deux
    /// vécues : un abonné Blazor dont le circuit vient de se fermer lève ObjectDisposedException,
    /// et une exception remontant depuis le worker le tuerait — ou, pire, ferait passer l'élément
    /// en cours pour un échec. Parcourir la liste d'invocation plutôt que d'appeler le délégué
    /// multicast évite qu'un onglet mort prive les autres de leur notification.
    /// </summary>
    private void RaiseSafely(Action? handler, string name)
    {
        if (handler == null) return;
        foreach (var d in handler.GetInvocationList())
        {
            try { ((Action)d)(); }
            catch (Exception ex) { _logger.LogDebug(ex, "{Event}: un abonné a levé une exception", name); }
        }
    }

    private void Log(string msg)
    {
        lock (_gate)
        {
            _log.Add(msg);
            // Purge par tranches : un RemoveRange de 400 lignes toutes les 400 lignes coûte bien
            // moins qu'un décalage du tableau à chaque ajout. Aucun effacement automatique en
            // dehors de cette borne — avec une file qui enchaîne, il serait toujours arbitraire.
            if (_log.Count > LogMaxLines) _log.RemoveRange(0, LogTrimLines);
        }
        RaiseProgress(force: true);
    }

    /// <summary>
    /// Part de la barre globale occupée par ce qui est en cours de traitement. Un artiste ou un label
    /// enchaîne des albums qui rapportent chacun leur avancement de 0 à 100 % : sans portée, la barre
    /// repartirait de zéro à chaque album au lieu de refléter l'ensemble.
    /// </summary>
    private readonly record struct ProgressScope(double Start, double Span)
    {
        public static ProgressScope Full => new(0, 100);

        public ProgressScope Slice(int index, int total) => new(Start + Span * index / total, Span / total);

        public int Map(int percent) => (int)(Start + Span * percent / 100);
    }

    /// <summary>Rétablit la portée précédente à la sortie du <c>using</c>.</summary>
    private readonly struct ScopeLease(DownloadService owner, ProgressScope previous) : IDisposable
    {
        public void Dispose() => owner._scope = previous;
    }

    private ProgressScope _scope = ProgressScope.Full;

    /// <summary>Restreint la progression au <paramref name="index"/>-ième élément sur <paramref name="total"/>.</summary>
    private ScopeLease EnterScope(int index, int total)
    {
        var lease = new ScopeLease(this, _scope);
        _scope = _scope.Slice(index, total);
        return lease;
    }

    private void UpdateProgress(int percent, string status, string currentItem = "")
    {
        Progress.Percent = _scope.Map(percent);
        Progress.Status = status;
        if (!string.IsNullOrEmpty(currentItem)) Progress.CurrentItem = currentItem;
        RaiseProgress(force: true);
    }

    // ─── Worker ───

    private async Task WorkerLoopAsync()
    {
        while (!_appCt.IsCancellationRequested)
        {
            try { await _signal.WaitAsync(_appCt); }
            catch (OperationCanceledException) { return; }

            // Drainer jusqu'à ne plus rien trouver : le sémaphore signale « il y a du travail »
            // sans dire combien.
            var processedAny = false;
            while (!_appCt.IsCancellationRequested)
            {
                QueueItem? item;
                lock (_gate)
                {
                    item = _queue.FirstOrDefault(i => i.Status == QueueItemStatus.Pending);
                    if (item != null)
                    {
                        item.Status = QueueItemStatus.Running;
                        item.StartedAt = DateTime.UtcNow;
                        ReindexLocked();
                    }
                }
                if (item == null) break;

                processedAny = true;
                RaiseQueueChanged();

                try
                {
                    await RunItemAsync(item);
                }
                catch (Exception ex)
                {
                    // Filet de sécurité : le worker ne meurt jamais sur l'échec d'un élément,
                    // sans quoi toute la file resterait bloquée en attente pour toujours.
                    _logger.LogError(ex, "Queue worker: unhandled failure on {Url}", item.Url);
                    try { SetStatus(item, QueueItemStatus.Failed, ex.Message); } catch { }
                }
            }

            if (_appCt.IsCancellationRequested) return;

            // Le sémaphore étant borné à 1, un réveil peut survenir alors que la file a déjà été
            // drainée au tour précédent : sans ce garde, le message de fin serait écrit deux fois.
            if (!processedAny) continue;

            // File vidée : remettre la carte de progression au repos. Sans cela l'album et la
            // vitesse du dernier téléchargement resteraient affichés indéfiniment.
            Progress.IsRunning = false;
            Progress.IsComplete = true;
            Progress.CurrentQueueItemId = null;
            Progress.CurrentAlbum = null;
            Progress.CurrentItem = "";
            Progress.Speed = "";
            Progress.Percent = 0;
            Progress.Status = "";
            Log(_loc["log.queueDrained"]);
            RaiseProgress(force: true);
            RaiseQueueChanged();
        }
    }

    private async Task RunItemAsync(QueueItem item)
    {
        // Lié au jeton applicatif : l'arrêt du conteneur interrompt le téléchargement au lieu
        // d'attendre le délai de grâce de docker stop.
        var cts = CancellationTokenSource.CreateLinkedTokenSource(_appCt);
        _itemCts = cts;
        var ct = cts.Token;

        _sessionId = Guid.NewGuid().ToString("N")[..8];
        _skipCurrent = false;
        _scope = ProgressScope.Full;

        Progress.IsRunning = true;
        Progress.IsComplete = false;
        Progress.Percent = 0;
        Progress.Speed = "";
        Progress.CurrentAlbum = null;
        Progress.CurrentItem = item.Label;
        Progress.CurrentQueueItemId = item.Id;
        RaiseProgress(force: true);

        // Séparateur par élément : remplace l'effacement automatique du journal, qui n'a plus de
        // sens dès lors que plusieurs téléchargements s'enchaînent.
        Log(_loc.T("log.queueItemStart", item.Label));

        try
        {
            Log(_loc["log.checkingLink"]);

            switch (item.Kind)
            {
                case "album":
                    await DownloadAlbumAsync(item.RefId, ct);
                    break;
                case "track":
                    await DownloadSingleTrackAsync(item.RefId, ct);
                    break;
                case "playlist":
                    await DownloadPlaylistAsync(item.RefId, ct);
                    break;
                case "artist":
                    await DownloadArtistAsync(item.RefId, ct);
                    break;
                case "label":
                    await DownloadLabelAsync(item.RefId, ct);
                    break;
                case "user":
                    if (item.RefId.Contains("albums"))
                        await DownloadFavoritesAlbumsAsync(ct);
                    else if (item.RefId.Contains("tracks"))
                        await DownloadFavoritesTracksAsync(ct);
                    else if (item.RefId.Contains("artists"))
                        await DownloadFavoritesArtistsAsync(ct);
                    else
                        throw new NotSupportedException(_loc["log.unsupportedUserLink"]);
                    break;
                default:
                    throw new NotSupportedException(_loc.T("log.unsupportedLinkType", item.Kind));
            }

            Log(_loc["log.complete"]);
            SetStatus(item, QueueItemStatus.Completed);
        }
        catch (OperationCanceledException)
        {
            // Annulation utilisateur : on le signale et on enchaîne. Arrêt applicatif : inutile de
            // journaliser, le processus s'éteint et la boucle sortira sur _appCt.
            if (!_appCt.IsCancellationRequested) Log(_loc["log.aborted"]);
            SetStatus(item, QueueItemStatus.Cancelled);
        }
        catch (Exception ex)
        {
            Log(_loc.T("log.error", ex.Message));
            _logger.LogError(ex, "Download failed for {Url}", item.Url);
            SetStatus(item, QueueItemStatus.Failed, ex.Message);
        }
        finally
        {
            _itemCts = null;
            cts.Dispose();
            Progress.CurrentQueueItemId = null;
            Progress.CurrentAlbum = null;
            Progress.Speed = "";
            RaiseProgress(force: true);
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

        try
        {
            // Artwork
            await DownloadArtworkAsync(downloadPath, album);

            var tracks = album.Tracks?.Items ?? new List<QobuzTrack>();
            int total = tracks.Count;
            var skipped = false;

            for (int i = 0; i < total; i++)
            {
                ct.ThrowIfCancellationRequested();
                if (_skipCurrent) { _skipCurrent = false; skipped = true; Log(_loc["log.albumSkipped"]); break; }

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

            // Goodies — an album the user chose to skip is skipped entirely
            if (!skipped && s.DownloadGoodies && album.Goodies != null)
            {
                foreach (var goody in album.Goodies)
                {
                    if (goody.FileFormatId == 52 || string.IsNullOrEmpty(goody.Url)) continue;
                    await DownloadGoodyAsync(downloadPath, album, goody, ct);
                }
            }
        }
        finally
        {
            DeleteTempArtwork(album);
        }
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

        try
        {
            await DownloadArtworkAsync(downloadPath, album);
            await DownloadTrackToFileAsync("track", album, track, downloadPath, null, ct);
        }
        finally
        {
            DeleteTempArtwork(album);
        }
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

                try
                {
                    await DownloadArtworkAsync(downloadPath, album);
                    await DownloadTrackToFileAsync("playlist", album, item, downloadPath, playlist, ct);
                }
                finally
                {
                    DeleteTempArtwork(album);
                }
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
            try { using (EnterScope(i, total)) await DownloadAlbumAsync(albums[i].Id, ct); }
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
            try { using (EnterScope(i, total)) await DownloadAlbumAsync(albums[i].Id, ct); }
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
            try { using (EnterScope(i, total)) await DownloadAlbumAsync(albums[i].Id, ct); }
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

        var artists = favs.Artists.Items;
        for (int i = 0; i < artists.Count; i++)
        {
            ct.ThrowIfCancellationRequested();
            try { using (EnterScope(i, artists.Count)) await DownloadArtistAsync(artists[i].Id.ToString(), ct); }
            catch (OperationCanceledException) { throw; }
            catch (Exception ex) { Log(_loc.T("log.favArtistFailed", artists[i].Id, ex.Message)); }
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
        Directory.CreateDirectory(TempDirectory);
        var tempFile = Path.Combine(TempDirectory, $"qbdlx_downloading-{track.Id}{s.AudioFormat}");

        var titleFormatted = _templates.FormatTrackTitle(track);
        Log(_loc.T("log.downloadingTrack", track.TrackNumber.ToString().PadLeft(2, '0'), titleFormatted));

        // Whatever happens below — cancellation, tagging error, failed move — the partial file must
        // not stay behind in the config volume.
        try
        {
            await DownloadWithQualityFallbackAsync(track, tempFile, ct);

            // Fix MD5 for FLAC
            if (s.FixMD5s && s.AudioFormat.Contains("flac"))
                await FixFlacMd5Async(tempFile, ct);

            // Tag file
            var embeddedArtPath = EmbeddedArtPath(album);
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
        finally
        {
            DeleteQuietly(tempFile);
        }
    }

    /// <summary>Downloads the track into <paramref name="tempFile"/>, stepping down the quality chain
    /// when a quality keeps failing. Throws when no quality could be downloaded.</summary>
    private async Task DownloadWithQualityFallbackAsync(QobuzTrack track, string tempFile, CancellationToken ct)
    {
        var s = _settings.Settings;

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
            throw lastError ?? new IOException(_loc["error.downloadAllQualitiesFailed"]);
        }
    }

    /// <summary>
    /// Re-encodes the file with the flac CLI so that its STREAMINFO MD5 is valid. A failure is not
    /// fatal — the track stays playable — so it is reported in the log and the download goes on.
    /// </summary>
    private async Task FixFlacMd5Async(string path, CancellationToken ct)
    {
        var startInfo = new ProcessStartInfo("flac")
        {
            CreateNoWindow = true,
            RedirectStandardOutput = true,
            RedirectStandardError = true
        };
        // -s: without it flac streams its progress to stderr, a few hundred kilobytes per file.
        foreach (var argument in new[] { "-s", "-f", "-8", path })
            startInfo.ArgumentList.Add(argument);

        try
        {
            using var process = Process.Start(startInfo)
                ?? throw new InvalidOperationException("flac could not be started");

            // Both pipes are drained while waiting: once a pipe buffer is full flac blocks on its
            // write, and WaitForExitAsync would then never return.
            var stdout = process.StandardOutput.ReadToEndAsync();
            var stderr = process.StandardError.ReadToEndAsync();

            using (ct.Register(() => KillQuietly(process)))
                await process.WaitForExitAsync(ct);

            await stdout;
            if (process.ExitCode != 0)
            {
                var error = (await stderr).Trim();
                Log(_loc.T("log.md5FixFailed", error.Length > 0 ? error : $"exit code {process.ExitCode}"));
            }
        }
        catch (OperationCanceledException) { throw; }
        catch (Exception ex)
        {
            _logger.LogWarning(ex, "flac MD5 fix failed");
            Log(_loc.T("log.md5FixFailed", ex.Message));
        }
    }

    private static void DeleteQuietly(string path)
    {
        try { System.IO.File.Delete(path); }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException) { /* best effort */ }
    }

    private static void KillQuietly(Process process)
    {
        try { process.Kill(entireProcessTree: true); }
        catch (InvalidOperationException) { /* already exited */ }
    }

    // ─── HTTP stream download helpers ───

    // From highest to lowest: 27 = FLAC 24bit 192kHz, 7 = FLAC 24bit 96kHz, 6 = FLAC 16bit 44.1kHz.
    private static readonly (string format, string label)[] FlacQualities =
    {
        ("27", "FLAC 24bit/192kHz"),
        ("7",  "FLAC 24bit/96kHz"),
        ("6",  "FLAC 16bit/44.1kHz"),
    };

    private static readonly (string format, string label) Mp3Quality = ("5", "MP3 320");

    /// <summary>
    /// Returns the chain of (format_id, label) to try for downloading a track.
    /// Starts with the user's chosen quality, then falls back to lower qualities
    /// if Qobuz refuses or throttles the high-res stream.
    /// MP3 is never a fallback for FLAC: the file keeps the extension of the requested format
    /// (<see cref="AppSettings.AudioFormat"/>), so an MP3 stream saved as ".flac" cannot be tagged.
    /// </summary>
    private static IEnumerable<(string format, string label)> BuildQualityFallbackChain(string requested)
    {
        if (requested == Mp3Quality.format)
            return new[] { Mp3Quality };

        var index = Array.FindIndex(FlacQualities, q => q.format == requested);

        // Unknown id: try it raw, then the whole FLAC chain
        return index < 0
            ? new[] { (requested, $"format {requested}") }.Concat(FlacQualities)
            : FlacQualities.Skip(index);
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

                // sw n'est jamais redémarré : il sert à calculer la vitesse *moyenne* depuis le
                // début du fichier. Le cadencement de l'affichage, lui, est assuré par
                // RaiseProgress(force: false) — d'où l'horodatage séparé côté service.
                if (totalBytes > 0 && sw.ElapsedMilliseconds > 250)
                {
                    var pct = (int)(bytesRead * 100 / totalBytes);
                    var speed = bytesRead / (sw.Elapsed.TotalSeconds + 0.001);
                    Progress.Speed = speed >= 1024 * 1024
                        ? $"{speed / (1024 * 1024):F2} MB/s"
                        : $"{speed / 1024:F2} KB/s";
                    Progress.Status = _loc.T("download.status.downloading", pct, Progress.Speed);
                    RaiseProgress(force: false);
                }
            }
        }

        if (totalBytes > 0 && bytesRead < totalBytes)
        {
            throw new IOException(_loc.T("error.incompleteDownload", bytesRead, totalBytes));
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
            var embeddedPath = EmbeddedArtPath(album);
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
        // 200 bytes: room for the " (id)" suffix and the extension below the 255-byte limit.
        var fileName = TemplateService.TruncateToBytes(
            TemplateService.MakeSafeFilename(goody.Description ?? album.Title), maxBytes: 200);
        var ext = GoodyExtension(goody.Url);
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

    /// <summary>Keyed by album as well as session: a leftover from one album can then never be
    /// mistaken for the cover of the next one, whatever went wrong in between.</summary>
    private string EmbeddedArtPath(QobuzAlbum album) =>
        Path.Combine(Path.GetTempPath(), $"qbdlx-{_sessionId}-{album.Id}-{_settings.Settings.EmbeddedArtSize}.jpg");

    /// <summary>Read from the URL path: a signed URL ends with a query string that must not leak into
    /// the file name (".pdf?Expires=..." is not a valid name on Windows or SMB shares).</summary>
    private static string GoodyExtension(string? url) =>
        Uri.TryCreate(url, UriKind.Absolute, out var uri) ? Path.GetExtension(uri.AbsolutePath) : "";

    private void DeleteTempArtwork(QobuzAlbum album)
    {
        try
        {
            var path = EmbeddedArtPath(album);
            if (System.IO.File.Exists(path)) System.IO.File.Delete(path);
        }
        catch { }
    }
}
