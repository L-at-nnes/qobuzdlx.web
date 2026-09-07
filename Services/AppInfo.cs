namespace QobuzDLX.Web.Services;

public static class AppInfo
{
    /// <summary>
    /// Version du projet (<c>&lt;Version&gt;</c> du csproj), sans le numéro de révision. Source unique
    /// de la barre latérale et de la sonde <c>/healthz</c> consommée par l'application Android.
    /// </summary>
    public static string Version { get; } =
        typeof(AppInfo).Assembly.GetName().Version is { } v
            ? $"{v.Major}.{v.Minor}.{v.Build}"
            : "0.0.0";
}
