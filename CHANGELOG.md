# Changelog

All notable changes to QobuzDLX.Web are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Fixed
- **Connexion : un mauvais mot de passe n'est plus rejoué sur chaque couple de repli.** Toute
  erreur de la résolution dynamique était avalée, y compris un refus d'authentification : chaque
  clic sur « Se connecter » tentait donc jusqu'à cinq logins avec les mêmes identifiants, puis
  affichait « impossible d'obtenir les identifiants de l'application » au lieu de « identifiants
  invalides ». Le couple `app_id` / `app_secret` est désormais validé **avant** le login, qui n'est
  plus tenté qu'une seule fois ; un 400/401/403 signale alors de mauvais identifiants.
- **Le repli de qualité ne bascule plus de FLAC vers MP3.** La chaîne 27 → 7 → 6 → 5 pouvait
  enregistrer un flux MP3 sous l'extension `.flac` ; TagLib l'ouvrait alors comme du FLAC et la
  piste échouait avec une erreur obscure. Une qualité FLAC ne se replie plus que sur une qualité
  FLAC inférieure.
- **Mauvaise pochette après l'échec d'une piste.** Quand une piste de playlist ou de favoris
  échouait, la pochette temporaire n'était pas supprimée et la piste suivante — d'un autre album —
  l'embarquait à son tour. Le nettoyage est désormais garanti (`finally`), et le fichier temporaire
  porte l'identifiant de l'album pour qu'un résidu ne puisse plus être réutilisé.
- **Un `settings.json` corrompu n'empêche plus le démarrage.** L'écriture n'était pas atomique et
  la lecture ne gérait aucune erreur : une coupure pendant la sauvegarde laissait un JSON tronqué,
  et le conteneur redémarrait en boucle. Les réglages sont désormais écrits dans un fichier voisin
  puis substitués à l'original (l'ancienne version reste dans `settings.json.bak`). Au démarrage,
  un fichier illisible est mis de côté sous `settings.json.corrupt-<date>` et la dernière version
  saine est restaurée, au lieu de faire planter l'application.
- **Redirection ouverte sur le déverrouillage.** `POST /auth/unlock` renvoyait vers la destination
  fournie par le formulaire, même avec un mauvais mot de passe : `/\evil.com` ou `/<tabulation>/evil.com`
  passaient le filtre et étaient lus par les navigateurs comme `//evil.com`. L'antislash et les
  caractères de contrôle sont désormais refusés (`LocalRedirect`).
- **Le palier de blocage du mot de passe d'accès ne redescendait jamais.** Après quelques séries
  d'échecs — les siens ou ceux d'un tiers —, les blocages de 5 puis 15 minutes restaient la règle
  jusqu'au prochain déverrouillage réussi. Une heure sans tentative fait désormais redescendre le
  palier d'un cran et efface les échecs isolés. Le seau reste global : tant qu'un tiers insiste, il
  continue de bloquer les autres.
- **La langue du premier lancement n'est plus figée par une requête qui n'est pas un navigateur.**
  La détection par `Accept-Language` s'exécutait sur toute requête : la sonde `/healthz` de
  l'application Android (qui n'envoie pas l'en-tête) ou un outil de supervision enregistrait donc
  le français pour tout le monde. Elle ne se déclenche plus que sur une navigation vers une page,
  et uniquement si l'en-tête est présent.
- **« Fix MD5s » ne peut plus bloquer un téléchargement.** `flac` était lancé avec ses sorties
  redirigées mais jamais lues ; sa progression, écrite sur la sortie d'erreur, pouvait remplir le
  tampon du tube et suspendre le processus — donc toute la file — indéfiniment. Les deux flux sont
  désormais lus pendant l'attente et la progression est désactivée (`-s`). Un code de sortie non nul
  apparaît dans le journal, et l'annulation termine le processus `flac` au lieu de le laisser
  tourner.
- **Plus de fichiers partiels abandonnés dans `/config/qbdlx-temp`.** Une annulation, une erreur de
  tag ou un déplacement raté laissait le fichier en cours de téléchargement sur le volume de
  configuration. Il est maintenant supprimé quoi qu'il arrive, et les restes d'un arrêt brutal
  (fichiers `qbdlx_downloading-*` et pochettes temporaires) sont purgés au démarrage.
- **La recherche n'échoue plus en silence.** Une erreur de l'API (session expirée, réseau) vidait
  les résultats sans un mot, et une recherche sans résultat n'affichait rien non plus. Un message
  distingue désormais l'échec de l'absence de résultat. Lancer une recherche avec la touche Entrée
  pendant qu'une autre est en cours n'en démarre plus une seconde en parallèle.
- **Une session Qobuz expirée est détectée.** Le jeton enregistré n'était jamais revalidé : après
  un redémarrage, l'interface s'affichait comme connectée et chaque recherche échouait. Le jeton est
  désormais vérifié au démarrage (un Qobuz injoignable ne déconnecte pas), et une réponse 401
  pendant une recherche ramène à l'écran de connexion avec un message explicite. Un jeton refusé à
  la connexion ne laisse plus l'interface se comporter comme si l'on était connecté.
- **`%AlbumPA%` et ses variantes reflétaient l'avertissement parental de la piste.** Dès qu'une
  piste était fournie (nom de fichier), les balises `%Album…%` étaient résolues avec le drapeau de
  la piste avant que celui de l'album ne soit consulté. Chaque portée utilise maintenant son propre
  drapeau.
- **Les titres très longs ne font plus échouer les pistes.** Les noms de fichiers et de dossiers
  n'étaient jamais raccourcis, alors que les systèmes de fichiers limitent chaque composant à 255
  **octets** (ext4) — et un titre japonais ou cyrillique en pèse deux à trois par caractère. Chaque
  composant est désormais tronqué à 240 octets UTF-8 (avec « … »), sans jamais couper un caractère
  en deux. Les fichiers bonus sont concernés de la même façon.
- **Les en-têtes d'identification Qobuz ne sont plus modifiés sur un client HTTP partagé.**
  `X-App-Id` était ajouté et retiré dans `DefaultRequestHeaders` à chaque connexion ou
  déconnexion, une opération qui n'est pas thread-safe si une requête est en cours — un
  téléchargement, par exemple. Les en-têtes sont désormais posés requête par requête.
- **Extension des fichiers bonus.** Elle était lue dans l'URL entière : une URL signée donnait
  `.pdf?Expires=…`, un nom invalide sous Windows et sur les partages SMB. Seul le chemin est
  désormais pris en compte.
- **La barre de progression d'un artiste, d'un label ou des favoris ne repart plus de zéro à chaque
  album.** Chaque album écrasait le pourcentage global avec son propre avancement ; il se loge
  désormais dans la part de la barre qui lui revient.
- **« Passer » ne télécharge plus les fichiers bonus de l'album sauté.** Le bouton interrompait les
  pistes mais l'album continuait avec ses livrets.
- **Numérotation des pistes d'une playlist.** Avec l'option « position dans la playlist », le
  numéro était complété selon la taille de l'album d'origine : `7` et `35` voisinaient avec `120`
  et le tri alphabétique des fichiers n'était plus celui de la playlist. Le complément suit
  désormais la taille de la playlist.
- **`%DiscNumber%` fonctionne dans le modèle de piste.** La balise était listée parmi les
  variables disponibles mais n'était résolue que par le modèle du sous-dossier de disque ; partout
  ailleurs elle donnait une chaîne vide.
- **Garde du dossier de téléchargement.** Le contrôle « le chemin reste dans le dossier de
  destination » comparait des préfixes de chaîne : `/downloads-evil` passait pour être dans
  `/downloads`. Il compare désormais des composants de chemin.
- **Un `..` à l'intérieur d'un nom n'est plus supprimé.** « Vol..2 » devenait « Vol2 ». Seuls les
  composants `.` et `..` d'un chemin sont écartés ; les points et espaces en bout de chaque
  composant sont retirés, comme l'exigent Windows et les partages SMB.
- **Le tag « Qobuz URL » ne retire plus que `/fr-fr/`.** Les liens d'une autre langue (`/us-en/`,
  `/de-de/`…) gardaient leur segment de langue dans le tag. Il est retiré quelle que soit la
  langue, et la logique n'est plus dupliquée entre FLAC et MP3.
- **Textes encore en dur, hors du catalogue de traduction.** Deux messages d'erreur de
  téléchargement étaient toujours en français, et la page « introuvable » toujours en anglais,
  quelle que soit la langue choisie.
- **Les tirets en bout de nom ne sont plus supprimés.** Le nettoyage des noms de fichiers retirait
  tout tiret initial ou final : « -OSC- » devenait « OSC ». Seuls les tirets séparateurs, collés à
  un espace (le « - » laissé par `Titre:` ou par un champ vide), sont écartés.
- **Pochette embarquée via l'API TagLib courante.** `AttachedPictureFrame` est obsolète dans
  TagLibSharp 2.3 et produisait deux avertissements à la compilation ; `AttachmentFrame` le
  remplace sans changer le résultat (vérifié en FLAC et en MP3).

### Changed
- **L'écran de verrouillage n'a plus de sélecteur de langue.** La langue est un réglage global :
  tout visiteur non authentifié pouvait la changer pour tous les appareils connectés et provoquer
  des réécritures de `settings.json`. L'écran suit la langue du navigateur détectée au premier
  lancement ; le sélecteur reste disponible une fois connecté.
- La version affichée dans la barre latérale et renvoyée par `/healthz` provient d'un seul endroit
  (`AppInfo.Version`, lue dans le csproj). Elle était calculée deux fois, avec un repli « 1.5.1 »
  écrit en dur qui serait devenu faux à la première montée de version.

### Security
- **Avertissement tant qu'aucun mot de passe d'accès n'est défini.** Par défaut, toute personne
  qui joint le port utilise le compte Qobuz de l'instance, lit les réglages et lance des
  téléchargements. Un bandeau reste affiché dans l'interface jusqu'à la définition d'un mot de
  passe, un avertissement est écrit dans les journaux au démarrage, et le README ouvre désormais
  par une section « Security ».
- **Option `COOKIE_SECURE=true` pour le cookie d'accès.** Derrière un reverse proxy HTTPS, le
  serveur ne voit que du HTTP et ne posait donc jamais le drapeau `Secure`, de sorte que le cookie
  pouvait aussi partir sur une requête en clair vers le même hôte. Le déduire de `X-Forwarded-Proto`
  aurait obligé à faire confiance à n'importe quel client ; l'opérateur le déclare donc lui-même.
  Désactivé par défaut : un cookie `Secure` n'est pas accepté en HTTP simple.
- **En-têtes de sécurité sur toutes les réponses.** `X-Frame-Options: DENY` et
  `Content-Security-Policy: frame-ancestors 'none'` empêchent d'encadrer l'interface dans la page
  d'un tiers (détournement de clic) ; `X-Content-Type-Options: nosniff` et
  `Referrer-Policy: same-origin` complètent. Une CSP complète n'est pas posée : Blazor Server
  repose sur des scripts et styles en ligne, et les vignettes viennent de serveurs d'images Qobuz.
- **Empreinte du mot de passe d'accès renforcée et versionnée.** PBKDF2 passe de 100 000 à
  600 000 itérations (recommandation OWASP pour HMAC-SHA256). L'empreinte porte désormais son
  format et son nombre d'itérations (`pbkdf2-sha256$600000$sel$hash`) : relever le coût plus tard
  ne rendra pas les empreintes existantes illisibles. Les anciens formats (PBKDF2 à 100 000
  itérations, SHA-256 non salé) restent acceptés. Une empreinte dont le hash ou le sel est vide
  ou de taille inattendue est refusée au lieu de valider n'importe quel mot de passe, et la
  comparaison SHA-256 d'origine se fait désormais en temps constant.

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
