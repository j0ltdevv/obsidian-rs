# Changelog

Format : [Keep a Changelog](https://keepachangelog.com/fr/1.1.0/) —
semver.

## [Non publié] — 2026-10-21

### Ajouté
- **Moteur Rust en service HTTP** : binaire `obsidian-engine` exposant les
  endpoints `/health`, `/files`, `/file`, `/metadata`, `/backlinks`,
  `/search`, `/graph` et `POST /sync` (23 opérations fs/path/crypto/ipc,
  confinement des chemins) ; 21 tests d'intégration.
- **Comportements Obsidian clean-room** : wikilinks, frontmatter et tags
  déduits de l'analyse REA et couverts par des tests de comportement,
  fixtures sous `tests/fixtures/comportements/`.
- **Thème du vault** : 21 variables CSS d'Obsidian parsées et traduites en
  palette iced (`palette_from_vars`, `iced_theme`, `theme_css_path`) ; 17
  tests.
- **Raccourcis clavier** : 19 raccourcis Obsidian, remap via `hotkeys.json`
  et palette de commandes ; 12 tests.
- **Fidélité de l'éditeur** : coloration de la source en 18 types de
  fragments, 15 constantes de typographie Obsidian (interligne 1,6,
  tailles de titres, rayons, retraits) ; 11 tests.
- **Interface enrichie** : module `src/ui/markdown.rs` affichant la source
  Markdown avec sa syntaxe colorée (style Live Preview) dans le panneau
  central, bascule Lecture/Édition, ouverture automatique d'une note au
  démarrage (note persistée en priorité, sinon la première du vault), état
  persisté dans `<vault>/.obsidian/obsidian-rs-state.json` (écriture
  atomique).
- **Front HTML/CSS (interface primaire)** : `web/` (index.html, style.css,
  app.js — vanilla, sans dépendance ni build step) reproduisant l'apparence
  d'Obsidian à partir de mesures relevées sur l'application réelle
  (`demonstrations/obsidian-rea/UI-SPEC.md`) : ruban 44 px, docks 300 px,
  bandeau d'onglets 40 px, barre d'état 27 px, corps 16/24, fonds
  #1C1C1C/#282828, survol rgba(255,255,255,0.067). Arbre du coffre,
  ouverture, source colorée, aperçu rendu, liens et wikilinks cliquables,
  rétroliens, plan, recherche, sauvegarde, création et suppression — vérifiés
  par CDP (13 contrôles) dans les deux transports (REST même origine, repli
  `POST /sync`). RAM mesurée : **322,8 Mo PSS**.
- **Coquille de bureau Tauri 2** (`src-tauri/`) : fenêtre webview qui
  embarque le moteur Rust, attend `/health`, publie l'adresse du moteur dans
  la page et arrête le moteur à la sortie. Mesuré : 208 Mo PSS.
- **Mode « compat »** : leur interface réelle d'Obsidian (renderer Electron)
  pilotée par notre moteur Rust, via le shim (chemins relatifs, `id` de
  vault, module `url`, `os.version`, surface `electron.remote`, chaîne
  d'acceptation **lue à l'exécution** depuis l'installation de
  l'utilisateur — jamais recopiée) ; 41 tests de shim.
- **Modules vault** : résolution d'embeds (type + dimensions d'image lues
  dans l'en-tête PNG/JPEG, sans dépendance), corbeille restaurable
  (restauration refusée si la destination est occupée, vidage sous
  confirmation explicite), publication HTML du vault, export PDF de note
  (PDF 1.4 sans dépendance, pagination, polices base 14) et langage de
  requête Dataview (`FROM`/`WHERE`/`SORT`/`LIMIT`).
- **Spécification de fidélité** (`demonstrations/obsidian-rea/UI-SPEC.md`) :
  980 variables CSS relevées via CDP sur Obsidian 1.14.4, géométrie,
  typographie, couleurs d'état, et écarts chiffrés avec nos constantes.
- **Diagnostic du boot de leur renderer**
  (`demonstrations/obsidian-rea/RENDERER-BOOT.md`) : leur interface démarre
  (fenêtre rendue, explorateur peuplé, 0 exception) ; cause du « quit
  ~2 s » identifiée (porte de boot du vault) ; coût mesuré 298 Mo PSS.
- **ADR-001** (`docs/ADR-001-STACK-UI.md`) : double interface actée
  (front HTML/CSS primaire, mode compat, natif en filet) et comparaison
  chiffrée des quatre stacks (`docs/UI-STACKS.md`).

### Mesures
- Notre UI native (iced + tiny-skia, release) : **22,0 Mo** RSS ; **70,7 Mo**
  sur un vault de 5 000 notes.
- Front webview (Tauri + WebKitGTK + CodeMirror 6) : 387 Mo PSS avec un
  éditeur ouvert, 286 Mo pour une page vide.
- Leur interface (Electron 45) : 298 Mo PSS / 606 Mo RSS, interface chargée.

### Corrigé
- Icônes du ribbon visibles (teinte `currentColor` des SVG Lucide).
- Onglets de sidebar sur une seule ligne (scroll horizontal).
- États vides harmonisés : un seul « Aucune note ouverte » par panneau.
- Fenêtre par défaut 1400×900.
- **Rendu logiciel** : bascule d'iced vers `tiny-skia`
  (`default-features = false`) ; feature exclusive avec wgpu. RAM mesurée
  39,6 Mo (RSS stable, build debug, box instabox) contre 75,6 Mo avec
  wgpu (voir `docs/BENCHMARKS.md`).

## [0.2.0] — 2026-10-21

### Ajouté
- **UI façon Obsidian** : ribbon d'icônes Lucide (SVG embarqués), barre
  d'état, sidebar gauche à onglets (Fichiers/Recherche/Tags/Backlinks/
  Calendrier), sidebar droite (Backlinks + Plan cliquable).
- **Multi-onglets** de notes (tampons non sauvegardés préservés à la
  bascule ; les onglets suivent renommage/suppression/refresh).
- **Panneaux redimensionnables** (`pane_grid`), disposition persistée dans
  `.obsidian/pane_layout.json`.
- **Thème iced** construit depuis les tokens du vault (dark/light) ;
  panneaux/boutons/onglets dérivés de `extended_palette`.
- **Police Inter embarquée** (OFL) + feature `svg`.
- Recherche in-note précédent/suivant (compteur + curseur déplacé).
- Sélection multiple de notes (checkbox) + suppression groupée vers la
  corbeille.
- Snapshot global du vault (commande palette, copie horodatée).
- Tests de régression (intégrité onglets, ratios pane, occurrence_line_col).
- `docs/MANUEL.md` (manuel utilisateur).

### Corrigé
- `weekday_of` : décalage Zeller correct (lundi=0).
- Caches `file_mtimes` conservées au boot (filtre date fonctionnel dès le
  lancement).
- Verrou d'écriture appliqué au renommage et à la suppression.
- SaveNote vide le tampon caché ; CloseTab préserve les éditions non
  sauvegardées ; purge des sélections/onglets périmés au Refresh.
- Snapshot global écrit atomiquement (tmp → rename).

## [0.1.0] — précédent

### Ajouté
- P8c : import de `.obsidian/canvas.json` + suppression de carte (canvas).
- P8e : notes de bas de page `[^id]` parsées et stylées dans le live
  preview (références et définitions).
- P11 : indicateur « non enregistré » (●) sur la note ouverte.
- P8a/P8b : notes récentes dans le Quick Switcher, Ctrl+Shift+F (recherche
  globale), clic sur un tag → filtre le graphe.
- P6bis : renommage (préserve le dossier), suppression avec confirmation,
  création de dossier (confinée), boutons sidebar/éditeur.
- P6 : persistance `app.json` (thème/vim/spellcheck) et `hotkeys.json`,
  vue Réglages, thème et snippets appliqués au fond de l'app.
- P5 : manifest de plugins (`.obsidian/plugins/*/manifest.json`), UI
  activer/désactiver, snippets CSS (variables de couleurs).
- P4 : opérateurs de recherche (path/tag/file/line/exclusions) branchés à
  l'UI, extraits surlignés, remplacement global dans le vault.
- P3 : cartes de canvas éditables (titre/contenu/groupe), auto-layout,
  export/import JSON (roundtrip).
- P2 : graphe local + filtres dossier/tag, groupes colorés par dossier,
  glisser-déposer des nœuds.
- P1 : rendu callouts/math/embeds/block refs dans le live preview,
  résolution de liens par alias.
- Vague 2 : aliases, attachments, block_refs, bookmarks, callouts, embeds,
  math, tag_tree + vues Plan/recherche/tâches.
- Docs : `docs/ARCHITECTURE.md`, `docs/ROADMAP-PARITE.md`, README.

### Corrigé
- Fichier `src/app/mod.rs` : arm `Message::MarkDirty` manquant
  (non-exhaustif) — ajouté.
- `src/vault/mod.rs` : décalarations `search`/`scan` restaurées.
- Chemins : confinement des nouvelles notes/dossiers, renommage conservant
  le dossier d'origine.
- `SegmentStyle` : variantes callouts/math/embeds/footnotes couvertes par
  `segment_widget`.
