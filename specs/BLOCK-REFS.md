# Objectif

Documenter l'extraction des références de blocs (`^id`) et la résolution
d'un bloc vers son extrait de texte (utilisés par les embeds de bloc
`![[note#^id]]`).

## Comportements

1. Une ancre de bloc est un marqueur `^id` en fin de ligne ; `id` est une suite de caractères alphanumériques, `-` ou `_`. — INFERRED
2. L'extraction retourne les ancres dans l'ordre des lignes, avec leur numéro de ligne (0-based). — INFERRED
3. Les ancres situées dans un bloc de code (fences ``` ou ~~~, fermées ou non) sont ignorées. — INFERRED
4. La résolution d'un bloc par id retrouve la ligne-ancre et retire le marqueur `^id`. — INFERRED
5. Le bloc s'étend de la ligne-ancre aux lignes suivantes non vides ; une ligne vide le termine. — INFERRED
6. Un id inconnu retourne `None`. — INFERRED
7. `parse_block_embed("note#^id")` retourne `("note", "id")` ; tout autre format retourne `None`. — INFERRED

## Limites et hors périmètre

- La portée d'un bloc est volontairement simple (jusqu'à la ligne vide), sans connaissance des listes/citations profondes.
- La résolution de `note#^id` vers un fichier réel relève de l'index du vault, pas de ce module.
- L'ancrage dans des listes (multi-lignes) n'est pas différencié du texte continu.

## Label

Phase 2 : comportements déduits du format de blocs d'Obsidian, sans
observation REA — chacun est étiqueté **INFERRED**.
