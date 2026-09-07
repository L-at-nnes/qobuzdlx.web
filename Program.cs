using System.Security.Claims;
using Microsoft.AspNetCore.Authentication;
using Microsoft.AspNetCore.Authentication.Cookies;
using Microsoft.AspNetCore.Components.Authorization;
using Microsoft.AspNetCore.Components.Server;
using Microsoft.AspNetCore.DataProtection;
using Microsoft.AspNetCore.StaticFiles;
using QobuzDLX.Web.Services;

var builder = WebApplication.CreateBuilder(args);

// Le trousseau Data Protection chiffre le cookie d'accès. Par défaut il atterrit dans
// ~/.aspnet/DataProtection-Keys, soit /home/qbdlx dans l'image : un répertoire qui n'est pas
// monté en volume (seuls /downloads et /config le sont). Sans cette ligne, chaque redémarrage
// du conteneur régénérerait les clés et invaliderait silencieusement tous les cookies déjà
// émis — l'écran de verrouillage reviendrait sans la moindre trace dans les logs.
var configPath = Environment.GetEnvironmentVariable("CONFIG_PATH") ?? "/config";
builder.Services.AddDataProtection()
    .PersistKeysToFileSystem(new DirectoryInfo(Path.Combine(configPath, "dataprotection-keys")))
    .SetApplicationName("QobuzDLX.Web");

builder.Services.AddRazorPages();
builder.Services.AddServerSideBlazor(options =>
{
    // Trois minutes par défaut. Sur mobile, l'application passe en arrière-plan bien plus
    // longtemps que ça ; dix minutes couvrent « je réponds à un message et je reviens » sans
    // avoir à reconstruire le circuit. La rétention ne sert toutefois que si c'est la même page
    // qui se reconnecte : quand le système tue le processus du navigateur, un nouveau circuit
    // naît de toute façon, et c'est le rechargement automatique côté client qui prend le relais.
    options.DisconnectedCircuitRetentionPeriod = TimeSpan.FromMinutes(10);
    options.DisconnectedCircuitMaxRetained = 50;
    // Le gel de l'onglet ne doit pas faire expirer une interopérabilité JS en vol.
    options.JSInteropDefaultCallTimeout = TimeSpan.FromMinutes(1);
})
.AddHubOptions(options =>
{
    // Règle SignalR : le délai d'expiration doit valoir au moins deux fois le keep-alive.
    // Attention, le client a ses propres valeurs (30 s / 15 s) : les relever ici seulement ne
    // servirait à rien, c'est lui qui déclarerait la connexion morte en premier. Le pendant
    // côté navigateur est dans _Host.cshtml.
    options.KeepAliveInterval = TimeSpan.FromSeconds(20);
    options.ClientTimeoutInterval = TimeSpan.FromSeconds(60);
    options.HandshakeTimeout = TimeSpan.FromSeconds(30);
});

// Register services
builder.Services.AddSingleton<SettingsService>();
builder.Services.AddSingleton<QobuzApiService>();
builder.Services.AddSingleton<TemplateService>();
builder.Services.AddSingleton<TaggingService>();
builder.Services.AddSingleton<DownloadService>();
builder.Services.AddSingleton<SearchStateService>();
// La langue est un réglage global (settings.json), comme le reste : singleton.
builder.Services.AddSingleton<LocalizationService>();
builder.Services.AddScoped<AppGuardService>();
// Étranglement du déverrouillage : singleton, pour que le compteur ne soit pas remis à zéro
// par l'ouverture d'un nouvel onglet.
builder.Services.AddSingleton(TimeProvider.System);
builder.Services.AddSingleton<AppAccessGate>();

// Cookie d'accès à l'application. Il remplace un état qui vivait dans le circuit Blazor et que
// le moindre rechargement perdait — sur mobile, où le système gèle puis tue les WebView, cela
// revenait à ressaisir le mot de passe à chaque retour dans l'application.
builder.Services.AddAuthentication(CookieAuthenticationDefaults.AuthenticationScheme)
    .AddCookie(options =>
    {
        options.Cookie.Name = "qbdlx_access";
        options.Cookie.HttpOnly = true;
        options.Cookie.IsEssential = true;
        options.Cookie.Path = "/";
        // Lax et non None : le cookie n'a besoin de voyager que sur les navigations de premier
        // niveau, et None imposerait le drapeau Secure — impossible à garantir en HTTP sur une
        // adresse privée (VPN, réseau local).
        options.Cookie.SameSite = SameSiteMode.Lax;
        // Derrière un reverse proxy qui termine TLS, Kestrel ne voit que du HTTP : SameAsRequest ne
        // pose alors pas le drapeau Secure. Le déduire de X-Forwarded-Proto imposerait de faire
        // confiance à quiconque peut joindre Kestrel (l'adresse du proxy est dynamique en Docker).
        // C'est donc à l'opérateur, qui sait que l'accès se fait en HTTPS, de le déclarer :
        // COOKIE_SECURE=true. Le cookie n'est alors plus accepté par les navigateurs sur du HTTP
        // simple, d'où une option et non un défaut.
        options.Cookie.SecurePolicy = builder.Configuration.GetValue<bool>("COOKIE_SECURE")
            ? CookieSecurePolicy.Always
            : CookieSecurePolicy.SameAsRequest;

        options.ExpireTimeSpan = TimeSpan.FromDays(30);
        options.SlidingExpiration = true;

        // Aucun endpoint n'est [Authorize] : ces redirections vers une page de connexion qui
        // n'existe pas ne doivent jamais se déclencher.
        options.Events.OnRedirectToLogin = ctx =>
        {
            ctx.Response.StatusCode = StatusCodes.Status401Unauthorized;
            return Task.CompletedTask;
        };
        options.Events.OnRedirectToAccessDenied = ctx =>
        {
            ctx.Response.StatusCode = StatusCodes.Status403Forbidden;
            return Task.CompletedTask;
        };

        // Le cookie fige l'empreinte du mot de passe pour lequel il a été émis. Le changer ou le
        // retirer dans les Paramètres déconnecte donc tous les appareils déjà déverrouillés.
        options.Events.OnValidatePrincipal = ctx =>
        {
            var settings = ctx.HttpContext.RequestServices.GetRequiredService<SettingsService>();
            var expected = AppAccessGate.StampFor(settings.Settings.AppAccessPasswordHash);

            if (ctx.Principal?.FindFirst(AppAccessGate.StampClaim)?.Value == expected)
                return Task.CompletedTask;

            ctx.RejectPrincipal();
            return ctx.HttpContext.SignOutAsync(CookieAuthenticationDefaults.AuthenticationScheme);
        };
    });

// AddServerSideBlazor() l'enregistre déjà via TryAdd ; l'écrire explicitement documente la
// dépendance de AppGuardService et met la résolution à l'abri d'un changement d'implémentation.
builder.Services.AddScoped<AuthenticationStateProvider, ServerAuthenticationStateProvider>();

var app = builder.Build();

// Instancié dès le démarrage : charge les catalogues, applique la culture et signale dans les
// logs toute clé de traduction manquante avant la première requête.
var localization = app.Services.GetRequiredService<LocalizationService>();

if (string.IsNullOrEmpty(app.Services.GetRequiredService<SettingsService>().Settings.AppAccessPasswordHash))
    app.Logger.LogWarning("No access password is set: anyone who can reach this server can use the linked Qobuz account. Set one in Settings.");

if (!app.Environment.IsDevelopment())
{
    app.UseHsts();
}

// Posés sur toutes les réponses, fichiers statiques compris. frame-ancestors (et X-Frame-Options
// pour les navigateurs qui l'ignorent) interdit d'encadrer l'interface dans la page d'un tiers
// pour piéger un clic ; l'application elle-même n'a besoin d'aucun cadre. Pas de CSP complète :
// Blazor Server s'appuie sur des scripts et styles en ligne, et les vignettes viennent de
// serveurs d'images Qobuz qu'on ne maîtrise pas.
app.Use(async (context, next) =>
{
    var headers = context.Response.Headers;
    headers.XContentTypeOptions = "nosniff";
    headers.XFrameOptions = "DENY";
    headers.ContentSecurityPolicy = "frame-ancestors 'none'";
    headers["Referrer-Policy"] = "same-origin";
    await next();
});

// .webmanifest est absent de la table MIME par défaut : sans ce mapping le fichier partirait en
// application/octet-stream et Chrome ignorerait le manifeste, donc l'installation sur l'écran
// d'accueil, sans rien signaler.
var contentTypes = new FileExtensionContentTypeProvider();
contentTypes.Mappings[".webmanifest"] = "application/manifest+json";
app.UseStaticFiles(new StaticFileOptions { ContentTypeProvider = contentTypes });

app.UseRouting();

// Après UseRouting et avant tout endpoint : c'est ce qui donne un Context.User au handshake
// du hub Blazor, donc à la CircuitFactory qui amorce l'état d'authentification du circuit.
app.UseAuthentication();

// Premier lancement uniquement : tant qu'aucune langue n'a été choisie, on suit l'en-tête
// Accept-Language du navigateur. L'appel ne fait plus rien dès que le choix est persisté.
// Seules les navigations vers une page comptent : /healthz, les POST et les appels de l'application
// Android ne disent rien de la langue de l'utilisateur.
app.Use(async (context, next) =>
{
    if (context.Request.Headers.Accept.ToString().Contains("text/html", StringComparison.OrdinalIgnoreCase))
        localization.DetectFromBrowser(context.Request.Headers.AcceptLanguage.ToString());

    await next();
});

// Sonde utilisée par l'application Android pour valider qu'une URL saisie pointe bien sur une
// instance QobuzDLX avant de l'enregistrer. Volontairement non authentifiée et sans donnée
// sensible : le nom de l'application et sa version suffisent à lever l'ambiguïté.
app.MapGet("/healthz", () => Results.Json(new
{
    app = "QobuzDLX.Web",
    version = AppInfo.Version
}));

// Déverrouillage par formulaire HTML classique, et non par un gestionnaire Blazor : poser un
// cookie exige une vraie réponse HTTP, et l'écran de verrouillage devient utilisable dès le
// prérendu, sans attendre l'établissement du circuit.
app.MapPost("/auth/unlock", async (HttpContext ctx, SettingsService settings, AppAccessGate gate) =>
{
    var form = await ctx.Request.ReadFormAsync();
    var password = form["password"].ToString();
    var returnUrl = LocalRedirect.Sanitize(form["returnUrl"].ToString());

    var hash = settings.Settings.AppAccessPasswordHash;
    if (string.IsNullOrEmpty(hash))
        return Results.Redirect(returnUrl); // protection désactivée entre-temps

    if (gate.IsLockedOut)
        return Results.Redirect($"{returnUrl}?lock=throttled");

    if (!AppGuardService.VerifyPassword(password, hash))
    {
        gate.RegisterFailure();
        return Results.Redirect($"{returnUrl}?lock={(gate.IsLockedOut ? "throttled" : "bad")}");
    }

    gate.RegisterSuccess();

    var identity = new ClaimsIdentity(
        new[]
        {
            new Claim(ClaimTypes.Name, "qbdlx"),
            new Claim(AppAccessGate.StampClaim, AppAccessGate.StampFor(hash)),
        },
        CookieAuthenticationDefaults.AuthenticationScheme);

    // IsPersistent : sans lui le cookie meurt avec la session du navigateur, c'est-à-dire dès
    // qu'Android tue le processus du WebView — exactement ce qu'on cherche à éviter.
    await ctx.SignInAsync(
        CookieAuthenticationDefaults.AuthenticationScheme,
        new ClaimsPrincipal(identity),
        new AuthenticationProperties { IsPersistent = true });

    return Results.Redirect(returnUrl);
});

app.MapPost("/auth/lock", async (HttpContext ctx) =>
{
    await ctx.SignOutAsync(CookieAuthenticationDefaults.AuthenticationScheme);
    return Results.Redirect("/");
});

app.MapBlazorHub();
app.MapFallbackToPage("/_Host");

app.Run();
