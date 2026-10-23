# Architecture — obsy

Client Obsidian-like en Rust + iced (clean-room). Ce document décrit le
découpage actuel du dépôt.

## Couches

```
src/
  main.rs        bootstrap iced (fenêtre + subscription)
  app/mod.rs     état Elm (App, Message, update, boot, subscription) + I/O disque
  ui/mod.rs      vues iced (rendu pur, aucune I/O) + overlays
  ui/graph.rs    vue graphe (canvas, filtres, drag)
  ui/canvas_view.rs  vue canvas (sélection, édition)
  ui/{search,outline,tasks,themes,plugins,history,dataview}_view.rs
  vault/         cœur métier pur, testable sans fenêtre
    scan.rs        découverte récursive des notes
    note.rs        types Note / NoteMetadata / Wikilink
    frontmatter.rs parser YAML
    tags.rs        extraction #tag
    wikilinks.rs  parser [[cible|alias]]
    aliases.rs    aliases frontmatter + index de résolution
    index.rs       index backlinks/outgoing/recherche floue
    search.rs      index inversé full-text naïf
    search_ops.rs  opérateurs path:/tag:/file:/line:/exclusions
    editor.rs      édition texte (utilisé par app)
    live_preview.rs segments Markdown → styles
    outline.rs     plan (titres)
    tasks.rs       agrégation de tâches
    callouts.rs    callouts > [!type]
    embeds.rs      ![[…]] et classification
    math.rs        $…$ / $$…$$
    block_refs.rs  ^id et snippets
    canvas_data.rs modèle Canvas (cartes, arêtes, groupes, JSON, auto-layout)
    bookmarks.rs   store JSON .obsidian/bookmarks.json
    attachments.rs inventaire pièces jointes + import atomique
    tag_tree.rs    hiérarchie de tags
    history.rs     snapshots locaux
    templates.rs   expansion {{variable}}
    daily_notes.rs notes journalières AAAA-MM-JJ
    themes.rs      palettes dark/light
    css_snippets.rs variables de couleurs depuis .obsidian/snippets
    settings.rs    app.json + hotkeys.json (atomique)
    plugins_manifest.rs manifest de plugins
    ops.rs         création/déplacement/écriture atomique, confinement
  plugins/        registre (PluginRegistry) + API (Plugin/PluginContext)
specs/            comportements (labels INFERRED)
tests/            fixtures + tests d'intégration
evidence/         artéfacts REA
```

## Règles

- La vue ne fait aucune I/O ; tout accès disque passe par `app::update`.
- Écriture atomique (temporaire + rename) partout.
- Jamais de `unwrap()` sur entrée utilisateur/fichier ; jamais de `#[allow]`.
- Qualité : `cargo fmt --all --check`, `cargo build`, `cargo test`,
  `cargo clippy --all-targets -- -D warnings` — les 4 doivent passer.
