# Feuille de route — Parité Obsidian (obsy)

Objectif officiel : **réécrire Obsidian en Rust (iced), clean-room**, jusqu'à
parité fonctionnelle complète. Chaque item est livré avec un critère
vérifiable (commandes, test, ou capture). Les items non atteignables dans
un laps de temps raisonnable sont explicitement étiquetés « étiré ».

_Mise à jour du 21 octobre 2026 : intégration de l'audit de parité, des
tranches en cours et des mesures de RAM (rendu logiciel)._

## État actuel (livré et vérifié)

| Domaine | Statut | Preuve |
|---|---|---|
| Core vault (scan, frontmatter, tags, wikilinks, ops, index) | livré | tests (9 suites, 427+ lib) |
| Moteur HTTP `obsy-engine` (`/health /files /file /metadata /backlinks /search /graph` + `POST /sync`, 23 opérations, chemins confinés) | livré | 21 tests |
| Comportements Obsidian clean-room (wikilinks, frontmatter, tags) | livré | fixtures `tests/fixtures/comportements/` |
| Édition + prévisualisation live + sauvegarde atomique | livré | `cargo test` vert |
| Quick Switcher, palette de commandes, propriétés éditables | livré | `cargo test` vert |
| Vague 2 : aliases, attachments, block_refs, bookmarks, callouts, embeds, math, tag_tree | livré | tests + specs |
| Vues Plan / Recherche / Tâches câblées | livré | build + tests |
| P1 : rendu callouts/math/embeds + résolution par alias | livré | segments + tests |
| P2 : graphe local, filtres dossier/tag, groupes colorés, drag | livré | tests + canvas |
| P3 : cartes éditables, groupes, auto-layout, export JSON (roundtrip) | livré | tests canvas_data |
| P4 : opérateurs de recherche, extraits, remplacement global | livré | tests |
| P5 : manifest plugins, UI toggle, snippets CSS (variables couleurs) | livré | tests |
| P6 : app.json/hotkeys.json persistés, vue Réglages, thème/snippets appliqués (fond) | livré | tests roundtrip |
| P6bis : renommage (préserve dossier), suppression confirmée, création dossier confinée | livré | build + tests |
| P8a : commandes palette (create/daily/template), Ctrl+Shift+F, hotkeys affichés | livré | build + tests |
| P8b partiel : notes récentes, clic tag → filtre graphe | livré | build + tests |
| Thème du vault : 21 variables CSS d'Obsidian → palette iced | livré | 17 tests |
| 19 raccourcis clavier au format Obsidian, remappables via `hotkeys.json` | livré | 12 tests |
| Coloration de la source en 18 types de fragments + 15 constantes de typographie Obsidian | livré | 11 tests |
| Nouveau module `src/ui/markdown.rs` : source Markdown colorée (Live Preview), bascule Lecture/Édition | livré | `src/ui/markdown.rs` |
| Une note ouverte au démarrage (persistée en priorité, sinon la première), écriture atomique | livré | `cargo test` vert |
| Correctifs d'interface : icônes ribbon visibles, onglets sidebar sur une ligne, un seul état vide par panneau, fenêtre 1400×900, barre d'outils compactée, plus de chevauchement | livré | fenêtre 1400×900 |
| Rendu logiciel (tiny-skia) — voir section « Mesures » | livré | `docs/BENCHMARKS.md` |
| Docs : ARCHITECTURE.md, ROADMAP, specs, MANUEL.md | livré | dépôt |
| Smoke run | partiel | binaire lancé 8 s sans crash ; capture brute produite, contenu visuel non vérifié |

## Audit de parité (21 octobre 2026)

### Complète
- Ouvrir / créer / renommer / supprimer une note.
- Tags.
- Wikilinks : liens du **corps** de la prévisualisation cliquables (la cible
  brute est conservée dans `Segment::target`, alias compris).
- Bases/Dataview : moteur DQL (`FROM`, `WHERE`, `SORT`, `GROUP BY`, `FLATTEN`,
  `LIMIT`) + vue requêtable (barre de saisie, erreurs, résultats triés).
- Export PDF (`vault/pdf.rs`, encodage WinAnsi étendu aux caractères
  typographiques) et publication statique (`vault/publish.rs`, liens internes
  `.html` + section Backlinks par page).
- Recherche globale (opérateurs path/tag/file/line/exclusion, remplacement global).
- Graphe (local + profondeur, filtres, drag/zoom/pan persistés).
- Propriétés / frontmatter éditable.
- Interface multi-onglets, raccourcis, onglets de sidebar (6).

### Partielle (état réel au 24 octobre 2026)

| Fonctionnalité | Ce qui manque | Preuve |
|---|---|---|
| Édition + rendu Markdown | maths = texte mono coloré ; blocs de code sans coloration syntaxique par langue ; embeds de notes transcluses en placeholder | `src/ui/markdown.rs`, `src/vault/live_preview.rs` |
| Pièces jointes | images affichées dans le preview (`EmbedImage`), mais pas de glisser-déposer ni de lecture audio/vidéo | `src/ui/embed_view.rs` |
| Snippets CSS | variables extraites et fusionnées dans le thème appliqué à l'UI ; CSS arbitraire non interprété | `src/ui/theme_vars.rs` |
| Historique | snapshots + restauration branchés (`RestoreSnapshot`) ; pas de curseur de navigation avant/arrière dans le temps | `src/vault/history.rs` |

Livré depuis l'audit du 21 octobre (à sortir de cette liste) :
tableaux à grille, listes indentées, callouts repliables, barré réel,
création manuelle de cartes et d'arêtes canvas, projection `SELECT`
Dataview, thème du vault appliqué à l'UI, corbeille (restauration +
vidage confirmé), publication HTML.

### Absente (au 24 octobre, après la passe de fin)
- Chiffrement optionnel du vault.
- i18n fr/en.
- Toggles vim / orthographe fonctionnels.

Livré dans la passe du 24 octobre : maths LaTeX converties en Unicode
(inline et display), coloration syntaxique des blocs de code par langue
(syntect, Rust pur), glisser-déposer de fichiers (import en pièce
jointe + insertion d'embed), transclusion des notes `![[cible]]`
(profondeur 1), Tab / Shift+Tab pour le focus entre widgets, curseur
temporel précédent/suivant dans l'historique. Les raccourcis globaux
et le keymap étaient déjà branchés.

### Absente
- Navigation clavier complète (le keymap existe, le focus iced n'est pas câblé).
- Chiffrement optionnel du vault.
- i18n fr/en.

### Hors périmètre (AGENTS.md)
- Plugins tiers, synchronisation multi-appareils.

## En cours

Aucune tranche ouverte au 24 octobre 2026 : embeds/images, publication
HTML et corbeille avec restauration sont livrées et vérifiées.

## Top 8 des écarts les plus visibles

_Liste du 21 octobre : les huit points ont été traités ou remplacés au
24 octobre._ 1 (wikilinks corps), 2 (embeds/images), 3 (export PDF),
4 (Dataview `SELECT` + DQL), 5 (tableaux/listes), 6 (canvas manuel),
7 (corbeille) : livrés. 8 (toggles vim/orthographe sans effet) :
toujours sans effet fonctionnel, resté en liste pour une tranche
dédiée (vim en mode édition requiert un éditeur modal, hors périmètre
iced `text_editor`).

## Mesures (RSS, processus unique)

Mesures du 21 octobre 2026 (rendu logiciel tiny-skia), détail dans `docs/BENCHMARKS.md` :

| Contexte | RSS processus unique |
|---|---|
| Release (`cargo build --release`) | **22,0 Mo** |
| Debug (`CARGO_PROFILE_DEV_DEBUG=0`) | **39,6 Mo** |
| Debug, vault de 5 000 notes | **70,7 Mo** |

Il s'agit de la **RSS d'un processus unique**. Le chiffre d'Obsidian (180-250 Mo) est un **rapport tiers, non mesuré ici** (voir `docs/BENCHMARKS.md`).

## Règles de vérification (inchangées)

`cargo fmt --all --check`, `cargo build`, `cargo test`, `cargo clippy --all-targets -- -D warnings`. Un item n'est done que si son critère est rempli.
