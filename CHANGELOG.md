# Changelog

Format : [Keep a Changelog](https://keepachangelog.com/fr/1.1.0/) —
semver.

## [Non publié] — en cours

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
