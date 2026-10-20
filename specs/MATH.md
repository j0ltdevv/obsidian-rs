# Objectif

Documenter l'extraction des segments mathématiques Markdown : inline
`$...$` et display `$$...$$`.

## Comportements

1. Un segment inline est délimité par `$...$` sur une même ligne ; le contenu non vide (après trim) est retenu. — INFERRED
2. Un segment display est délimité par `$$...$$` et peut s'étendre sur plusieurs lignes. — INFERRED
3. En cas de conflit, `$$` gagnent sur `$`. — INFERRED
4. Un délimiteur non fermé (inline ou display) est ignoré, sans segment. — INFERRED
5. Un contenu vide (`$$`, `$$$$`) est ignoré. — INFERRED
6. Les blocs de code (fences ``` ou ~~~) sont ignorés. — INFERRED
7. Les segments sont retournés dans l'ordre d'apparition, avec leurs lignes de début et fin (0-based). — INFERRED

## Limites et hors périmètre

- Aucun rendu (KaTeX/MathJax) n'est effectué : le module extrait et découpe.
- Les `$` échappés (`\$`) ne sont pas traités.
- Le contenu est conservé brut, sans validation de syntaxe LaTeX.

## Label

Phase 2 : comportements déduits du format mathématique d'Obsidian, sans
observation REA — chacun est étiqueté **INFERRED**.
