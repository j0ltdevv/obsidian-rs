# Feuille de route — Parité Obsidian (obsidian-rs)

Objectif officiel : **réécrire Obsidian en Rust (iced), clean-room**, jusqu'à
parité fonctionnelle complète. Chaque item est livré avec un critère
vérifiable (commandes, test, ou capture). Les items non atteignables dans
un laps de temps raisonnable sont explicitement étiquetés « étiré ».

_Mise à jour du 21 octobre 2026 : intégration de l'audit de parité, des
tranches en cours et des mesures de RAM (rendu logiciel)._

## État actuel (livré et vérifié)

| Domaine | Statut | Preuve |
|---|---|---|
| Core vault (scan, frontmatter, tags, wikilinks, ops, index) | ✅ | tests (9 suites, 427+ lib) |
| Moteur HTTP `obsidian-engine` (`/health /files /file /metadata /backlinks /search /graph` + `POST /sync`, 23 opérations, chemins confinés) | ✅ | 21 tests |
| Comportements Obsidian clean-room (wikilinks, frontmatter, tags) | ✅ | fixtures `tests/fixtures/comportements/` |
| Édition + prévisualisation live + sauvegarde atomique | ✅ | `cargo test` vert |
| Quick Switcher, palette de commandes, propriétés éditables | ✅ | `cargo test` vert |
| Vague 2 : aliases, attachments, block_refs, bookmarks, callouts, embeds, math, tag_tree | ✅ | tests + specs |
| Vues Plan / Recherche / Tâches câblées | ✅ | build + tests |
| P1 : rendu callouts/math/embeds + résolution par alias | ✅ | segments + tests |
| P2 : graphe local, filtres dossier/tag, groupes colorés, drag | ✅ | tests + canvas |
| P3 : cartes éditables, groupes, auto-layout, export JSON (roundtrip) | ✅ | tests canvas_data |
| P4 : opérateurs de recherche, extraits, remplacement global | ✅ | tests |
| P5 : manifest plugins, UI toggle, snippets CSS (variables couleurs) | ✅ | tests |
| P6 : app.json/hotkeys.json persistés, vue Réglages, thème/snippets appliqués (fond) | ✅ | tests roundtrip |
| P6bis : renommage (préserve dossier), suppression confirmée, création dossier confinée | ✅ | build + tests |
| P8a : commandes palette (create/daily/template), Ctrl+Shift+F, hotkeys affichés | ✅ | build + tests |
| P8b partiel : notes récentes, clic tag → filtre graphe | ✅ | build + tests |
| Thème du vault : 21 variables CSS d'Obsidian → palette iced | ✅ | 17 tests |
| 19 raccourcis clavier au format Obsidian, remappables via `hotkeys.json` | ✅ | 12 tests |
| Coloration de la source en 18 types de fragments + 15 constantes de typographie Obsidian | ✅ | 11 tests |
| Nouveau module `src/ui/markdown.rs` : source Markdown colorée (Live Preview), bascule Lecture/Édition | ✅ | `src/ui/markdown.rs` |
| Une note ouverte au démarrage (persistée en priorité, sinon la première), écriture atomique | ✅ | `cargo test` vert |
| Correctifs d'interface : icônes ribbon visibles, onglets sidebar sur une ligne, un seul état vide par panneau, fenêtre 1400×900, barre d'outils compactée, plus de chevauchement | ✅ | fenêtre 1400×900 |
| Rendu logiciel (tiny-skia) — voir section « Mesures » | ✅ | `docs/BENCHMARKS.md` |
| Docs : ARCHITECTURE.md, ROADMAP, specs, MANUEL.md | ✅ | dépôt |
| Smoke run | ⚠️ | binaire lancé 8 s sans crash ; capture brute produite, contenu visuel non vérifié |

## Audit de parité (21 octobre 2026)

### Complète
- Ouvrir / créer / renommer / supprimer une note.
- Tags.
- Recherche globale (opérateurs path/tag/file/line/exclusion, remplacement global).
- Graphe (local + profondeur, filtres, drag/zoom/pan persistés).
- Propriétés / frontmatter éditable.
- Interface multi-onglets, raccourcis, onglets de sidebar (6).

### Partielle

| Fonctionnalité | Ce qui manque | Preuve |
|---|---|---|
| Édition + rendu Markdown | maths = texte mono coloré ; embeds = placeholders ; tableaux en mono sans grille ; listes sans indentation de niveau ; callouts non repliables ; code sans coloration syntaxique ; barré en gris | `src/ui/markdown.rs` |
| Wikilinks | ceux du **corps** du preview ne sont pas cliquables, seuls ceux du bloc métadonnées le sont | `src/ui/mod.rs:1772` (`SegmentStyle::Wikilink`) |
| Pièces jointes | inventaire + renommage + suppression, mais pas d'affichage des images, pas de glisser-déposer, pas de lecture audio/vidéo | `src/vault/attachments.rs` |
| Canvas | cartes éditables, import/export JSON, mais planche générée automatiquement, pas de création manuelle de nœuds/arêtes | `src/ui/canvas_view.rs`, `src/vault/canvas_data.rs` |
| Thème | dark/light + 21 variables, mais pas de thèmes CSS personnalisés complets | `src/vault/themes.rs` |
| Snippets CSS | 5 variables extraites, pas de CSS complet | `src/vault/css_snippets.rs` |
| Bases/Dataview | tableau fixe de 4 colonnes, pas de langage de requête | `src/vault/dataview.rs` |
| Historique | snapshots + restauration, pas de navigation temporelle | `src/vault/history.rs` |
| Corbeille | les notes vont dans `.trash`, mais **aucune restauration ni vidage depuis l'interface** | `src/vault/trash.rs` |

### Absente
- Export PDF.
- Publication (site statique).

### Hors périmètre (AGENTS.md)
- Plugins tiers, synchronisation multi-appareils.

## En cours (tranches en développement)

| Tranche | Périmètre exact | Fichier | Capacité apportée |
|---|---|---|
| Embeds / images | Résolution et rendu des embeds (note/image/pdf) : extraction, classification par type, résolution vers l'index, dimensions d'images | `src/vault/embeds.rs` | Affichage réel des images, notes transcluses et PDF dans le preview |
| Publication HTML | Génération d'un site statique à partir du vault | module `publish` en cours | Export de la note courante / du vault en HTML |
| Corbeille avec restauration | Lister la corbeille, restaurer et vider depuis l'interface, avec confinement des chemins | `src/vault/trash.rs` | Restauration et vidage fonctionnels depuis l'UI (fonctions `restore_trash`/`empty_trash` présentes) |

Ces tranches sont **en cours de développement**, pas terminées.

## Top 8 des écarts les plus visibles

Classés par impact décroissant pour un utilisateur.

1. **Wikilinks du corps non cliquables** — ce qui manque : rendre cliquables les liens du corps du preview (comme ceux du bloc métadonnées). Taille : moyen. Fichier : `src/ui/mod.rs` (branche `SegmentStyle::Wikilink`, ligne 1772). Proposition : brancher la navigation `[[note]]` sur le clic d'un segment Wikilink du corps.
2. **Maths et embeds non rendus** — pas d'image affichée, pas de PDF, pas de note transcluse. Taille : gros. Fichier : `src/ui/markdown.rs`, `src/vault/embeds.rs`. Proposition : raccorder le module `embeds.rs` au rendu (afficher les images, insérer les notes transcluses, placeholders PDF).
3. **Pas d'export PDF** — Taille : moyen. Fichier : absent. Proposition : tranche d'export PDF de la note courante.
4. **Dataview figé** — tableau fixe de 4 colonnes, pas de langage de requête. Taille : moyen. Fichier : `src/vault/dataview.rs`. Proposition : mini-langage de requête Dataview (type `TABLE`, `WHERE`).
5. **Rendu pauvre des tableaux et des listes imbriquées** — tableaux en mono sans grille, listes sans indentation de niveau. Taille : moyen. Fichier : `src/ui/markdown.rs`. Proposition : rendu HTML/CSS des tableaux + indentation multi-niveau des listes.
6. **Canvas automatique** — pas de création libre de nœuds/arêtes. Taille : moyen. Fichier : `src/ui/canvas_view.rs`. Proposition : ajouter la création manuelle de nœuds et d'arêtes.
7. **Corbeille sans restauration ni vidage** — Taille : petit. Fichier : `src/vault/trash.rs`. Proposition : brancher l'UI sur `restore_trash`/`empty_trash`.
8. **Toggles vim / orthographe persistés mais sans effet fonctionnel** — Taille : petit. Fichier : réglages. Proposition : implémenter l'effet vim/orthographe.

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
