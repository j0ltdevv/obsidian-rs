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
