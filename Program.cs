using QobuzDLX.Web.Services;

var builder = WebApplication.CreateBuilder(args);

builder.Services.AddRazorPages();
builder.Services.AddServerSideBlazor();

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

var app = builder.Build();

// Instancié dès le démarrage : charge les catalogues, applique la culture et signale dans les
// logs toute clé de traduction manquante avant la première requête.
var localization = app.Services.GetRequiredService<LocalizationService>();

if (!app.Environment.IsDevelopment())
{
    app.UseHsts();
}

app.UseStaticFiles();
app.UseRouting();

// Premier lancement uniquement : tant qu'aucune langue n'a été choisie, on suit l'en-tête
// Accept-Language du navigateur. L'appel ne fait plus rien dès que le choix est persisté.
app.Use(async (context, next) =>
{
    localization.DetectFromBrowser(context.Request.Headers["Accept-Language"].ToString());
    await next();
});

app.MapBlazorHub();
app.MapFallbackToPage("/_Host");

app.Run();
