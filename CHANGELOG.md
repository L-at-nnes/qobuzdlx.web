# Changelog

All notable changes to QobuzDLX.Web are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.4.0] - 2026-08-25

### Added
- **File d'attente de téléchargement.** Cliquer sur « Télécharger » n'attend plus la fin du
  téléchargement en cours : la demande est **empilée** et traitée dès que possible. On peut donc
  lancer l'intégrale d'un artiste, continuer à chercher, et ajouter trois albums au passage sans
  jamais bloquer l'interface.
  - La page **Téléchargement** affiche la file : vignette, titre, artiste, état
    (*En attente*, *En cours*, *Terminé*, *Échec*, *Annulé*) et actions.
  - Actions sur les éléments **en attente** : monter, descendre, retirer.
  - Actions sur les éléments **terminés** : relancer, retirer. Ils restent visibles jusqu'à
    « Effacer les terminés ».
  - **Pastille** sur « Téléchargement » dans le menu : nombre d'éléments restants.
  - Côté **Recherche**, un **toast** confirme l'ajout, le bouton d'un élément déjà en file est
    **grisé**, et un **✓** discret marque ce qui a déjà été téléchargé. Le bouton reste cliquable
    une fois l'élément terminé, pour retélécharger si besoin.
  - La file vit **en mémoire** : elle est perdue au redémarrage du conteneur, comme les résultats
    de recherche.
- **Trois gestes d'interruption distincts**, là où il n'y avait qu'un bouton « ANNULER » :
  - **PASSER** saute l'album en cours et poursuit le même élément (utile sur un artiste de vingt
    albums) ;
  - **ANNULER** abandonne l'élément entier et enchaîne sur le suivant ;
  - **VIDER LA FILE** retire les éléments en attente, sans toucher à celui qui tourne.
- Bouton **« Effacer le journal »**.
- **Adaptation mobile.** L'interface est utilisable sur téléphone : barre supérieure avec menu
  tiroir sous 768 px, listes de résultats densifiées, boutons tactiles élargis, prise en compte
  des encoches et barres système (`safe-area`), et plus de zoom intempestif au focus sur iOS.

### Changed
- Le **journal ne s'efface plus automatiquement** au début d'un téléchargement : avec une file
  qui enchaîne, tout effacement automatique serait arbitraire. Il est borné à 1200 lignes, séparé
  par un en-tête à chaque élément, et effaçable à la demande.
- Le journal **défile automatiquement** vers les nouvelles lignes (il ne défilait pas du tout).
- La carte album et la vitesse sont **remises à zéro** en fin de file, au lieu de rester
  affichées indéfiniment.
- Les boutons **ANNULER** et **PASSER** quittent la barre d'URL pour la ligne de l'élément en
  cours, où ils désignent sans ambiguïté ce sur quoi ils agissent.
- Le **téléchargement en lot** depuis la Recherche et la zone multi-URL alimentent la file au
  lieu de bloquer l'interface jusqu'au dernier élément.

### Fixed
- Les URL portant une **chaîne de requête** (`.../album/abc?utm_source=share`) échouaient : le
  `?utm_source=share` finissait dans l'identifiant envoyé à l'API. Elles sont désormais
  normalisées à l'ajout, ce qui les rend aussi reconnaissables comme doublons.
- **Tempête d'événements** pendant les téléchargements : la progression était signalée à chaque
  bloc de 80 Ko reçu — des dizaines de fois par seconde, sur tous les onglets ouverts. Elle est
  maintenant cadencée à 200 ms. Le calcul de vitesse, lui, reste une moyenne depuis le début du
  fichier.
- L'arrêt du conteneur (`docker stop`) interrompt immédiatement le téléchargement en cours au
  lieu d'attendre l'expiration du délai de grâce.
- Un échec sur un élément n'interrompt plus la suite : le traitement se poursuit avec l'élément
  suivant, l'élément fautif étant marqué *Échec* avec son message d'erreur.
- **Séparateur décimal dans les noms de fichiers.** Depuis la 1.3.0, la culture du processus suit
  la langue de l'interface : en français, `%SampleRate%` et `%FormatWithQuality%` écrivaient
  `44,1kHz` au lieu de `44.1kHz`. Les gabarits sont désormais formatés en culture invariante,
  indépendamment de la langue choisie.

## [1.3.0] - 2026-08-23

### Added
- **Interface multilingue français / anglais.** Une bascule `FR | EN` est disponible dans la
  barre latérale, sur l'écran de connexion et sur l'écran de mot de passe. Le changement est
  immédiat, sans rechargement de page : les résultats de recherche, un téléchargement en cours
  et le déverrouillage par mot de passe sont conservés.
  - La langue est un réglage **global**, enregistré dans `settings.json` comme le reste.
  - Au tout premier lancement, la langue du navigateur est détectée via l'en-tête
    `Accept-Language` : anglais si le navigateur est anglophone, **français dans tous les
    autres cas**. Le choix explicite de l'utilisateur prime ensuite.
  - Les installations existantes démarrent en français.
- **Tri des résultats de recherche** : pertinence, titre, artiste, date de sortie, qualité,
  nombre de pistes (albums) ou d'albums (artistes), en ordre croissant ou décroissant.

### Changed
- La page d'accueil (`/`) ouvre désormais sur la **Recherche** plutôt que sur le téléchargeur.
- « Téléchargeur » est renommé **« Téléchargement »** et accessible sur `/telechargement` ;
  l'ancienne adresse `/downloader` reste valide.
- Le **sélecteur de qualité** quitte la page Téléchargement pour rejoindre les **Paramètres**,
  aux côtés des autres réglages. La page Téléchargement n'en affiche plus qu'un rappel, avec un
  lien direct vers les paramètres.
- Le **journal de téléchargement**, jusqu'ici toujours en anglais alors que l'interface était en
  français, suit maintenant la langue choisie. Les lignes déjà affichées conservent leur langue
  d'origine après une bascule ; le journal n'est pas réécrit rétroactivement.
- **Image Docker** : ICU est installé (`icu-libs`, `icu-data-full`) et le
  *globalization-invariant mode* désactivé. Les nombres sont formatés selon la langue
  (`1,23 Mo/s` en français) et le tri alphabétique tient compte des accents. L'image gagne
  environ 30 Mo.

### Removed
- Le réglage `Language`, vestige inutilisé du portage depuis l'application desktop, remplacé par
  `UiLanguage`. L'ancienne clé est ignorée au chargement et disparaît à la première sauvegarde.

## [1.2.52] - 2026-07-17

### Fixed
- **Nettoyage des caractères interdits dans les noms de fichiers/dossiers.**
  Les caractères non autorisés sous Windows/NAS (`* : ? | " < > \ /`) présents dans les
  titres d'albums ou de pistes étaient auparavant remplacés par des homoglyphes Unicode
  obscurs (`:` → `∶`, `?` → `ʔ`, `|` → `ǀ`, `"` → `″`, `<`/`>` → `˂`/`˃`, `\` → `⧹`,
  `*` → `∗`), produisant des noms illisibles. Ils sont désormais remplacés par des
  substituts ASCII lisibles :
  - `:` → ` -` (ex. `Song: Intro` → `Song - Intro`)
  - `/` et `\` → `-` (ex. `AC/DC` → `AC-DC`)
  - `* ? | " < >` → supprimés (ex. `Who? What!` → `Who What!`)
- Un `/` provenant d'un **titre** (ex. `AC/DC`, `24/7`) créait un sous-dossier
  involontaire. Il est maintenant converti en `-`. Le `/` saisi dans les **modèles**
  de nommage reste bien un séparateur de dossier.
- Normalisation des espaces multiples et suppression des séparateurs superflus en début/fin
  de nom, y compris pour les fichiers bonus (goodies).

### Changed
- La version de l'application est désormais affichée dans la barre latérale.
