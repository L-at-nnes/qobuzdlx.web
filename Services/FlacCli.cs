using System.Diagnostics;

namespace QobuzDLX.Web.Services;

public readonly record struct FlacResult(int ExitCode, string Output, string Error);

/// <summary>L'outil en ligne de commande <c>flac</c> (installé dans l'image Docker).</summary>
public static class FlacCli
{
    /// <summary>
    /// Lance flac et renvoie son code de sortie et ses sorties. Lève une exception si flac ne peut pas
    /// être démarré (absent) et, si <paramref name="ct"/> se déclenche, tue le processus avant de propager.
    /// </summary>
    public static async Task<FlacResult> RunAsync(IEnumerable<string> arguments, CancellationToken ct)
    {
        var startInfo = new ProcessStartInfo("flac")
        {
            CreateNoWindow = true,
            RedirectStandardOutput = true,
            RedirectStandardError = true
        };
        foreach (var argument in arguments)
            startInfo.ArgumentList.Add(argument);

        using var process = Process.Start(startInfo)
            ?? throw new InvalidOperationException("flac could not be started");

        // Both pipes are drained while waiting: once a pipe buffer is full flac blocks on its
        // write, and WaitForExitAsync would then never return.
        var stdout = process.StandardOutput.ReadToEndAsync();
        var stderr = process.StandardError.ReadToEndAsync();

        using (ct.Register(() => KillQuietly(process)))
            await process.WaitForExitAsync(ct);

        return new FlacResult(process.ExitCode, (await stdout).Trim(), (await stderr).Trim());
    }

    private static void KillQuietly(Process process)
    {
        try { process.Kill(entireProcessTree: true); }
        catch (InvalidOperationException) { /* already exited */ }
    }
}
