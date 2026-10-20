# Feuille de route — Parité Obsidian (obsidian-rs)

Objectif officiel : **réécrire Obsidian en Rust (iced), clean-room**, jusqu'à
parité fonctionnelle complète. Chaque item est livré avec un critère
vérifiable (commandes, test, ou capture). Les items non atteignables dans
un laps de temps raisonnable sont explicitement étiquetés « étiré ».

## État actuel (livré et vérifié)

| Domaine | Statut | Preuve |
|---|---|---|
| Core vault (scan, frontmatter, tags, wikilinks, ops, index) | ✅ | tests (9 suites, 427+ lib) |
| Édition + prévisualisation live + sauvegarde atomique | ✅ | `cargo test` vert |
| Quick Switcher, palette de commandes, propriétés éditables | ✅ | `cargo test` vert |
## État actuel (livré et vérifié)

| Domaine | Statut | Preuve |
|---|---|---|
| Core vault (scan, frontmatter, tags, wikilinks, ops, index) | ✅ | tests (9 suites) |
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
| Docs : ARCHITECTURE.md, ROADMAP, specs | ✅ | dépôt |
| Smoke run | ⚠️ | binaire lancé 8 s sans crash ; capture brute produite, contenu visuel non vérifié |

## Backlog de parité

### P1 — Rendu intégré (en cours)
- [ ] Rendre les callouts dans le live preview (couleur + titre + corps replié) — critère : segments `Callout` produits par `render_markdown`, tests dédiés.
- [ ] Rendre les maths ($…$, $$…$$) dans le live preview — critère : segments `MathInline`/`MathDisplay`.
- [ ] Rendre les embeds + block refs dans le preview (placeholders cliquables) — critère : segments `Embed` (note/image/pdf).
- [ ] Brancher l'aliasindex à la résolution de liens (`[[alias]]` ouvre la note) — critère : test de résolution combinée index+alias.

### P2 — Vue graphe avancée
- [ ] Graphe local (autour de la note active) + filtres par dossier/tag — critère : UI + tests du filtre.
- [ ] Groupes par dossier et coloration — critère : rendu différentié.
- [ ] Déplacement souris des nœuds — critère : interaction réelle.

### P3 — Canvas avancé
- [ ] Cartes texte éditables — critère : édition dans la carte.
- [ ] Groupement de cartes — critère : modèle + rendu.
- [ ] Export + auto-layout — critère : export JSON cohérent.

### P4 — Recherche professionnelle
- [ ] Opérateurs de recherche (AND/OR/exclu, `path:`, `tag:`) dans l'UI — critère : requêtes typées passent.
- [ ] Surlignage des correspondances dans les résultats — critère : segments marqués.
- [ ] Remplacement global dans le vault (prévisualisation + appliquer) — critère : ops atomiques + tests.

### P5 — Extensibilité
- [ ] Manifest de plugins `.obsidian/plugins` (parse + chargement) — critère : fixture de plugin chargée.
- [ ] UI de réglages des plugins (activer/désactiver) — critère : toggle persistant.
- [ ] Thèmes CSS snippets personnalisés (fichiers `.css` appliqués) — critère : style appliqué.

### P6 — Explorer & Vault
- [ ] Créer / renommer / supprimer notes et dossiers depuis l'UI (confirmation avant suppression) — critère : ops atomiques existantes branchées.
- [ ] Paramètres `.obsidian/app.json` et `hotkeys.json` lus/écrits — critère : roundtrip.
- [ ] Templates + daily notes câblés (commandes + boutons) — critère : commandes exécutables.

### P7 — Durcissement
- [ ] Find/replace dans l'éditeur (Ctrl+F) — critère : remplacement local.
- [ ] Tests E2E d'intégration (boot + scénarios) — critère : suite dédiée.
- [ ] Documentation utilisateur complète (README + guide) — critère : rendu dans le dépôt.

### Écosystème « étiré » (hors périmètre immédiat, honnêteté)
- Sync multi-appareils, apps mobiles, compatibilité plugins tiers JS/CSS existants : non atteignables en clean-room solo à court terme ; nécessitent une décision d'architecture à part.

## Règles de vérification (inchangées)
`cargo fmt --all --check`, `cargo build`, `cargo test`, `cargo clippy --all-targets -- -D warnings`. Un item n'est done que si son critère est rempli.
