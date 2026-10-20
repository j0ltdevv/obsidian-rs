# obsidian-rs

Client de prise de notes **Obsidian-like** écrit en **Rust + iced**
(clean-room : aucune ligne reprise d'Obsidian ou d'un autre client).
Le comportement se déduit du format Markdown et des conventions Obsidian
(wikilinks, frontmatter, tags, callouts, math, embeds, Canvas).

## Fonctionnalités (état courant)

- **Vault** : scan récursif, tags, frontmatter YAML, wikilinks, aliases,
  backlinks/outgoing, ops atomiques (créer/déplacer/écrire) avec confinement
  des chemins.
- **Édition** : éditeur texte + prévisualisation live (titres, listes,
  tâches, tableaux, callouts, math, embeds, block refs, footnotes),
  sauvegarde atomique, indicateur « non enregistré » (●), renommage
  (préserve le dossier), suppression avec confirmation, création de
  note/dossier (confinée).
- **Navigation** : Quick Switcher (notes récentes), palette de commandes
  (Ctrl+P), onglet Explorer, plan (outline), tâches globales, recherche
  full-text avec opérateurs (`path:`, `tag:`, `line:`, `-exclu`) et
  extraits, remplacement global.
- **Graphe** : vues complète/local, filtres dossier/tag, groupes colorés
  par dossier, glisser-déposer des nœuds, clic tag → filtre.
- **Canvas** : cartes éditables (titre/contenu/groupe), auto-layout,
  import/export `.obsidian/canvas.json`, suppression de carte.
- **Extensibilité** : registre de plugins (API Rust), UI activer/désactiver,
  manifest `.obsidian/plugins/*/manifest.json`, snippets CSS (variables de
  couleurs appliquées au fond), thèmes dark/light persistés.
- **Paramètres** : `.obsidian/app.json` (thème, vim, spellcheck) et
  `.obsidian/hotkeys.json` lus/écrits de façon atomique.
- **Gabarits / notes du jour** : `{{title}}`/`{{date}}`, dossier
  `Templates` ou `templates`, note `AAAA-MM-JJ.md`.

## Compiler / tester

```sh
cargo fmt --all --check
cargo build
cargo test
cargo clippy --all-targets -- -D warnings
```

L'application démarre sur `demo-vault` (dossier fourni). Lancez
`cargo run` depuis la racine du dépôt.

## Architecture

Voir `docs/ARCHITECTURE.md` (couches `app` / `ui` / `vault` / `plugins`).
La feuille de route complète (labels SEEN/INFERRED et phases) est dans
`docs/ROADMAP-PARITE.md`.

## Raccourcis clavier

| Touche | Action |
|---|---|
| Ctrl+K | Quick Switcher |
| Ctrl+S | Sauvegarder |
| Ctrl+G | Graphe |
| Ctrl+P | Palette de commandes |
| Ctrl+F | Rechercher/remplacer dans la note |
| Ctrl+Shift+F | Recherche globale (vault) |
| Échap | Fermer l'overlay courant |

## Périmètre honnête

Le « cœur » local d'Obsidian est couvert (noter, lier, chercher, graphe,
canvas, plugins natifs, thèmes). Les éléments d'écosystème suivants ne sont
pas livrés : la mobile, sync multi-appareils, compatibilité des plugins
tiers JavaScript/CSS, export PDF, multi-fenêtres — ils nécessitent des
décisions d'architecture à part (moteur JS, protocole de sync).

## Licence

MIT (voir `Cargo.toml`).
