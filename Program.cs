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

var app = builder.Build();

if (!app.Environment.IsDevelopment())
{
    app.UseHsts();
}

app.UseStaticFiles();
app.UseRouting();
app.MapBlazorHub();
app.MapFallbackToPage("/_Host");

app.Run();
