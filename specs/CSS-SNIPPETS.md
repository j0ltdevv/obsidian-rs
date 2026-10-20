# Objectif

Documenter le chargement des snippets CSS (`.obsidian/snippets/*.css`)
comme **sous-ensemble volontaire** : extraction des variables de couleurs
(`--nom: #rrggbb;` ou `rgb(r,g,b)`) uniquement, pour personnaliser la
palette d'une app native iced.

## Comportements

1. Seuls les fichiers `.css` de `.obsidian/snippets/` sont lus ; dossier absent → liste vide. — INFERRED
2. Un fichier illisible est ignoré silencieusement. — INFERRED
3. La sélection retourne les fichiers dans l'ordre alphabétique du chemin. — INFERRED
4. Une déclaration `--nom: valeur;` est extraite lorsque `valeur` est une couleur `#rgb`/`#rrggbb` ou `rgb(r,g,b)`. — INFERRED
5. Les autres déclarations (dimensions, chaînes, noms de couleurs) et la syntaxe CSS hors variables sont ignorées. — INFERRED
6. Une valeur de couleur invalide n'empêche pas les autres déclarations du fichier. — INFERRED
7. Cinq clés connues mappent vers les composantes d'un thème (`--interactive-accent` → accent, `--background-primary` → bg, `--text-normal` → fg, `--background-secondary` → surface, `--background-modifier-border` → border). — INFERRED

## Limites et hors périmètre

- AUCUN rendu CSS général (selecteurs, cascade, media queries, ombres, polices) n'est appliqué : une app native iced ne dispose pas d'un moteur CSS.
- Le mappage des clés vers la palette de l'application est partiel ; les variables inconnues sont simplement listées.
- L'API se limite aux `--background-primary` etc. observées dans les thèmes Obsidian (INFERRED).

## Label

Phase 5 : comportements déduits du format de variables CSS d'Obsidian,
sans observation REA — chacun est étiqueté **INFERRED**.
