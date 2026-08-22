using Microsoft.AspNetCore.Components;
using QobuzDLX.Web.Services;

namespace QobuzDLX.Web.Shared;

/// <summary>
/// Base des pages traduites : redessine le composant quand la langue change, sans recharger la
/// page. Le rechargement complet, lui, détruirait le circuit Blazor — donc le déverrouillage par
/// mot de passe (<see cref="AppGuardService"/>, scoped) — et redemanderait le mot de passe à
/// chaque bascule de langue.
/// </summary>
public abstract class LocalizedComponentBase : ComponentBase, IDisposable
{
    [Inject] protected LocalizationService Loc { get; set; } = default!;

    protected override void OnInitialized()
    {
        Loc.OnLanguageChanged += OnLanguageChanged;
    }

    private void OnLanguageChanged() => InvokeAsync(StateHasChanged);

    public virtual void Dispose()
    {
        Loc.OnLanguageChanged -= OnLanguageChanged;
    }
}
