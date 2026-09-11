# Changelog

All notable changes to QobuzDLX.Web are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed
- **`docker-compose.yml` durci.** `no-new-privileges` et `cap_drop: ALL` : l'application tourne avec un
  utilisateur sans privilège et écoute sur un port élevé, elle n'a besoin d'aucune capacité. Une ligne
  `build: .` en commentaire permet de construire l'image depuis le dépôt au lieu de tirer l'image
  publiée, et `COOKIE_SECURE` y est rappelé à côté de `ACCESS_PASSWORD`.

### Fixed
- **Un MP3 sans code-barres recevait une étiquette `BARCODE` vide.** TagLib écarte seul les cadres
  standard sans valeur, mais pas les cadres personnalisés : quand Qobuz ne fournit pas d'UPC, le
  cadre était créé sans texte. Il est désormais omis. Les autres étiquettes (genre, compositeur,
  label, ISRC…) n'étaient déjà pas écrites vides.

## [1.6.0] - 2026-10-12

### Added
- **Mot de passe d'accès par variable d'environnement.** `ACCESS_PASSWORD` (8 caractères minimum),
  à placer dans `docker-compose.yml`, protège l'interface dès le premier démarrage : utile quand le
  conteneur est joignable depuis le réseau local ou depuis internet. Tant qu'elle est définie, la
  variable fait foi et les Réglages ne proposent plus de modifier ou de retirer le mot de passe. Une
  valeur trop courte arrête le démarrage plutôt que de laisser l'interface ouverte.
- **Fichier `LICENSE` (MIT)**, qui cite l'auteur du projet d'origine et les contributions de ce dépôt.
- **Recherche : « Voir plus », filtre « Hi-Res uniquement » et playlists.** Les résultats ne
  s'arrêtent plus aux 50 premiers ; une case masque ce qui ne dépasse pas 16 bits / 44,1 kHz ; un
  quatrième bouton cherche parmi les playlists publiques de Qobuz et les met en file d'un clic.
- **Bilan de chaque élément de la file et « Relancer les échecs ».** Un album, une playlist ou une
  discographie n'affiche plus « Terminé » quand des pistes ont échoué : chaque ligne indique les
  pistes téléchargées, ignorées et en échec, avec la raison de chaque échec. Le nouveau bouton ne
  retraite que les pistes en échec ; ↻ relance toujours l'élément entier.
- **Vérification des fichiers FLAC** (`flac -t`, activée par défaut). Un fichier abîmé est
  retéléchargé avec une nouvelle URL, puis dans une qualité inférieure. Sans `flac`, la vérification
  est ignorée.
- **Extraits de 30 secondes détectés.** Sans abonnement actif ou pour une piste restreinte, Qobuz ne
  fournit qu'un extrait : il était rangé parmi les vraies pistes, il est désormais écarté avec un
  message explicite.
- **Session expirée pendant un téléchargement.** L'élément en cours s'arrête avec un message clair,
  les suivants échouent aussitôt sans appeler Qobuz et toutes les pages ouvertes reviennent à
  l'écran de connexion. Après reconnexion, ↻ relance les éléments concernés.
- **Fichier de playlist `.m3u8`** écrit dans le dossier de chaque playlist (durées, chemins relatifs,
  ordre d'origine). Désactivable dans les Réglages.
- **Journal : niveau de détail et export.** « Échecs seulement », « Normal » ou « Détaillé »
  (qualité obtenue, taille et débit de chaque piste, essais ratés). « Exporter » enregistre le
  journal dans un fichier daté, « Copier » le place dans le presse-papiers (voie de repli pour
  l'application Android). Ce niveau remplace deux cases des Réglages qui n'avaient jamais eu d'effet.
- **Réglages : test de configuration.** Un bouton vérifie les dossiers de téléchargement et de
  configuration, la présence de `flac`, la session Qobuz et les clés de l'application, et nomme
  chaque problème.
- **Réglages : aperçu en direct des modèles de nommage.** Le chemin obtenu pour une piste d'album,
  une compilation et une playlist s'affiche pendant la saisie, avec un avertissement pour un modèle
  vide, une balise inconnue ou mal placée, un chemin absolu, `..` ou des caractères qui seront
  remplacés.
- **Réglages : tags `(Explicit)` / `(Clean)` modifiables** (texte, position, exemple) et **pochette
  en taille originale**, avec repli sur 600 px quand le label n'en fournit pas.

### Changed
- L'écran de verrouillage n'a plus de sélecteur de langue : la langue est un réglage global, qu'un
  visiteur non authentifié pouvait changer pour tout le monde. Il suit la langue du navigateur.
- Les téléchargements réutilisent leurs connexions HTTP au lieu d'en ouvrir une par fichier.
- Les erreurs passagères de l'API Qobuz (429, 502, 503, 504) sont rejouées jusqu'à trois fois, et le
  délai d'attente de l'API passe de 100 s à 30 s.
- Une playlist ou une liste de favoris ne recharge plus le même album à chaque piste.
- Chaque qualité compte désormais cinq essais au lieu de trois : un fichier que le CDN de Qobuz n'a
  pas encore servi est coupé lors des premières requêtes, puis servi normalement.
- La version affichée et celle de `/healthz` viennent d'une seule source (le `csproj`).

### Fixed
- **Téléchargement d'une playlist.** Qobuz renvoie des identifiants de piste de playlist supérieurs
  à 2³¹ que l'application lisait comme des entiers 32 bits : aucune playlist ne se téléchargeait. Deux
  pistes de même nom dans une playlist n'en faisaient plus qu'une ; la seconde reçoit maintenant son
  identifiant en suffixe. Une piste de compilation « Various Artists » créait un dossier au nom du
  fichier, et le `.m3u8` se remplissait de chemins en `../`.
- **Qualité annoncée.** Les variables de modèle (`%BitDepth%`, `%FormatWithQuality%`…) et le journal
  décrivaient la meilleure qualité de la sortie ou celle demandée, pas celle réellement reçue. Le
  repli de qualité ne bascule plus de FLAC vers MP3, qui produisait un faux `.flac`.
- **Listes de plus de 500 éléments.** Qobuz plafonne chaque réponse : les favoris et les labels
  n'étaient téléchargés que pour leurs 500 premiers éléments (près de 2 000 albums pour
  Columbia/Legacy). Des favoris vides se terminaient sans un mot.
- **Fichiers et noms.** Un modèle de piste pouvait faire sortir le fichier du dossier de l'album ;
  les titres très longs faisaient échouer les pistes (limite de 255 octets par composant) ; `Vol..2`
  devenait `Vol2` et `-OSC-` devenait `OSC` ; l'extension des fichiers bonus contenait la requête de
  l'URL ; le contrôle « reste dans le dossier de téléchargement » acceptait `/downloads-evil`.
- **Numérotation et variables.** Le numéro d'une piste de playlist était complété selon la taille de
  l'album d'origine ; `%DiscNumber%` restait vide dans le modèle de piste ; `%AlbumPA%` reflétait la
  piste ; l'aperçu des modèles ignorait la position dans la playlist.
- **Étiquettes.** « Fusionner les noms d'artistes » n'agissait pas sur l'artiste de l'album ; le
  commentaire contenait le HTML de la description Qobuz ; le tag « Qobuz URL » ne retirait que la
  langue `/fr-fr/` ; une pochette temporaire d'une piste en échec se retrouvait dans la piste
  suivante.
- **Fichiers temporaires.** Une annulation ou une erreur laissait des fichiers partiels dans
  `/config/qbdlx-temp` ; ils sont supprimés, et les restes d'un arrêt brutal sont purgés au démarrage.
  « Fix MD5s » pouvait bloquer toute la file en remplissant le tampon de `flac`.
- **Connexion.** La touche Entrée ne validait pas la page de connexion ; après connexion,
  `/telechargement` ou `/settings` atterrissait sur la recherche ; un mauvais mot de passe était
  rejoué sur chaque couple de repli ; une session expirée n'était pas détectée au démarrage ; les
  Paramètres s'affichaient sans menu hors connexion.
- **Verrouillage.** Le compte à rebours de l'écran de verrouillage restait figé ; le palier de
  blocage ne redescendait jamais ; la langue du premier lancement était figée par la sonde
  `/healthz`.
- **Réglages.** Un `settings.json` corrompu empêchait le démarrage : il est désormais écrit de façon
  atomique et restauré depuis `settings.json.bak`.
- **Interface.** La recherche échouait sans message ; la page Téléchargement pouvait planter à la fin
  d'un élément ; la barre de progression d'un artiste ou d'un label repartait de zéro à chaque
  album ; « Passer » téléchargeait quand même les fichiers bonus ; Chrome signalait une balise
  obsolète dans la console.
- Textes restés hors du catalogue de traduction, et pochette embarquée via l'API TagLib courante.

### Security
- Avertissement tant qu'aucun mot de passe d'accès n'est défini : bandeau dans l'interface, message
  au démarrage et section « Security » en tête du README.
- Option `COOKIE_SECURE=true` pour poser le drapeau `Secure` sur le cookie d'accès derrière un
  reverse proxy HTTPS.
- En-têtes de sécurité sur toutes les réponses (`X-Frame-Options`, `frame-ancestors`,
  `X-Content-Type-Options`, `Referrer-Policy`).
- Empreinte du mot de passe renforcée et versionnée : PBKDF2 à 600 000 itérations, anciens formats
  acceptés puis migrés au premier déverrouillage. Un nouveau mot de passe doit faire au moins 8
  caractères et être saisi deux fois.
- Redirection ouverte corrigée sur le déverrouillage (`/\evil.com`, tabulation).
- Les erreurs de l'API Qobuz n'exposent plus le corps brut de la réponse, et les réglages morts
  (dont un champ `Password` en clair) sont supprimés.
- Android : un certificat HTTPS refusé est expliqué au lieu d'afficher « Serveur injoignable ».

## [1.5.1] - 2026-09-28

### Fixed
- **Connexion : les couples `app_id` / `app_secret` de repli étaient inatteignables.** Les trois
  couples codés en dur ne répondent plus (HTTP 401) et un couple vérifié est ajouté en tête de
  liste — mais surtout, cette liste n'était en pratique jamais atteinte. La validation d'un secret
  acceptait toute réponse autre qu'un HTTP 400, or un `app_id` refusé en amont renvoie 401 **avant**
  que la signature soit vérifiée : la résolution dynamique depuis `bundle.js` « réussissait » donc
  avec un secret inutilisable. Seul un HTTP 200 vaut désormais succès, et le secret extrait par
  expression régulière directe est validé au même titre que les autres au lieu d'être accepté
  d'office. Symptôme typique : la connexion aboutissait, mais tous les téléchargements échouaient.
- **Séparateur décimal dans les noms de fichiers.** Depuis la 1.3.0, la culture du processus suit
  la langue de l'interface : en français, `%SampleRate%` et `%FormatWithQuality%` écrivaient
  `44,1kHz` au lieu de `44.1kHz`. Les gabarits sont désormais formatés en culture invariante,
  indépendamment de la langue choisie.

## [1.5.0] - 2026-09-07

### Added
- **Application Android** (`android/`). Une coquille WebView qui affiche l'interface web de votre
  instance : aucune interface n'est dupliquée, l'application suit donc chaque évolution du site.
  - **Écran de configuration** : saisir `192.168.1.42`, `qobuz.mondomaine.fr` ou une URL complète.
    L'adresse est essayée en HTTPS et en HTTP, avec et sans le port 8080 — les quatre en
    parallèle — et n'est enregistrée qu'une fois `/healthz` confirmé. En cas d'échec, le message
    liste les adresses tentées : un port publié autre que 8080 ne peut pas être deviné et doit
    être saisi.
  - **Changer de serveur** par appui long sur l'icône du lanceur, sans avoir à effacer les
    données de l'application.
  - **Avertissement en clair** : une adresse `http://` vers un hôte qui n'est ni sur un réseau
    privé ni dans un tunnel (Tailscale, WireGuard) déclenche une confirmation explicite, le mot
    de passe d'accès circulant alors en clair.
  - Rotation et bascule clair/sombre **ne rechargent plus la page** ; retour arrière, insets et
    clavier gérés nativement ; écran d'erreur avec « Réessayer » et « Changer de serveur ».
  - Compilation depuis Android Studio, voir [android/README.md](android/README.md).
- **Déverrouillage persistant.** Le mot de passe d'accès n'est plus redemandé à chaque
  rechargement : il est échangé contre un cookie de 30 jours à expiration glissante, qui survit à
  la fermeture du navigateur **et au redémarrage du conteneur**.
  - Changer ou retirer le mot de passe dans les Paramètres **déverrouille tous les appareils**.
- **Reconnexion automatique.** Quand le circuit tombe (mise en veille, coupure Wi-Fi, redémarrage
  du serveur), un bandeau aux couleurs de l'application remplace l'overlay gris illisible de
  Blazor, et la page se recharge d'elle-même si les tentatives échouent. Aucun état n'est perdu :
  recherche, tris et file de téléchargement sont restitués tels quels.
- **Icône et manifeste web** : l'application peut être ajoutée à l'écran d'accueil et s'ouvre
  alors sans barre d'adresse.
- Sonde **`/healthz`** exposant le nom et la version de l'application.

### Fixed
- L'écran de verrouillage **répond immédiatement**. Le formulaire attendait l'établissement du
  circuit SignalR : sur un réseau lent, le bouton restait inerte plusieurs secondes alors que la
  page était visible. C'est désormais un formulaire HTML classique, qui fonctionne dès le
  prérendu — et que le gestionnaire de mots de passe du téléphone sait remplir.
- La **limite de tentatives** de déverrouillage était décorative : son compteur vivait dans le
  circuit, donc ouvrir un nouvel onglet le remettait à zéro. Il est maintenant global à
  l'application, avec un délai qui double à chaque série d'échecs (30 s, 1 min, 5 min, 15 min).
- Un changement de langue n'est plus interrompu à mi-chemin lorsqu'un onglet fermé se trouve
  parmi les abonnés.

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
