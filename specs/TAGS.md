# Objectif

Documenter l'extraction des tags (étiquettes `#…`) dans le texte d'un document.

## Comportements

1. `#tag` extrait le tag `tag` ; les tags sont retournés dans leur ordre d'apparition. — INFERRED
2. `#a/b` extrait un seul tag `a/b` (imbrication de sous-tags par `/`). — INFERRED
3. Un `#` précédé d'un caractère alphanumérique ou de `_` (par exemple dans `C#` ou `foo#bar`) ne produit aucun tag. — INFERRED
4. Un `#` précédé d'un début de ligne, d'un espace blanc (whitespace) ou d'une ponctuation ouvre un tag. — INFERRED
5. Les caractères autorisés dans un tag sont les alphanumériques Unicode, `_`, `-` et `/` ; tout autre caractère termine le tag. — INFERRED
6. Un `#` suivi immédiatement d'un caractère non autorisé ne produit aucun tag. — INFERRED
7. Les tags en doublon sont conservés à cette étape : le dédoublonnage est effectué plus haut, lors du scan. — INFERRED

## Limites et hors périmètre

- La normalisation des tags (minuscules, accents, casse) et la validation sémantique sont hors périmètre.
- La distinction entre tags du frontmatter et tags du corps (et leur fusion) relève du scan de vault, pas de l'extraction unitaire décrite ici.
- Le comportement d'un `#` en fin de fichier (aucun caractère après) n'est pas explicitement documenté.

## Label

Phase 0 : tous les comportements ci-dessus sont documentés à partir du format Markdown, sans observation REA — chacun est étiqueté **INFERRED**.
