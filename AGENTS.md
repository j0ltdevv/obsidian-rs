# obsidian-rs

Client Obsidian-like en **Rust + iced**. Clean-room : aucune ligne reprise d'un
autre client (Obsidian, Logseq, Joplin…) ; le comportement se déduit des fichiers
Markdown et du format Obsidian (wikilinks, frontmatter, tags), pas d'un code source copié.

## Commandes

```sh
cargo fmt --all
cargo build
cargo test
cargo clippy --all-targets -- -D warnings
```

Un changement n'est fini que si `cargo build` **et** `cargo test` passent (et `cargo fmt`
appliqué). Colle la sortie, pas une promesse.

## Conventions

- Éditeur de texte : iced 0.13+, widgets composés, pas de `unsafe` sans justification écrite.
- Découpage : `src/main.rs` (bootstrap), `src/app/` (état + messages), `src/ui/` (vues),
  `src/vault/` (lecture/écriture Markdown, liens, index).
- Erreurs : `thiserror` dans les bibliothèques, `anyhow` au niveau app ; jamais de `unwrap()`
  sur une entrée utilisateur ou un fichier.
- Le vault de l'utilisateur est en lecture/écriture : toute écriture passe par un fichier
  temporaire + rename atomique, et jamais de suppression sans confirmation explicite.
- Tests : un test par comportement de parsing (wikilinks, frontmatter, tags) ; fixtures dans
  `tests/fixtures/`, jamais des fichiers du vrai vault.

## Hors périmètre (Phase 0 non validée)

Pas de plugin, pas de sync. **2026-10-21 : le propriétaire a validé une reverse-engineering
officielle d'Obsidian** via REA (`demonstrations/obsidian-rea/ANALYSE.md`). La règle clean-room
reste en vigueur : on *déduit* le comportement des artefacts et on ne copie aucune ligne de
code d'Obsidian (ou de tout autre client) dans ce dépôt.
