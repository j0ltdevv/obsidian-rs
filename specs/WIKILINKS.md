# Objectif

Documenter l'extraction des wikilinks (liens `[[…]]`) dans le texte d'un document.

## Comportements

1. `[[Cible]]` est extrait avec la cible `Cible` et aucun alias. — INFERRED
2. `[[Cible|Alias]]` est extrait avec la cible et l'alias, tous deux trimés (espaces en tête et en fin retirés). — INFERRED
3. `[[]]` est conservé comme lien avec une cible vide. — INFERRED
4. `[[#Titre]]` est extrait avec la cible `#Titre` : l'ancre est conservée telle quelle, sans transformation. — INFERRED
5. Plusieurs liens dans un même texte sont retournés dans leur ordre d'apparition. — INFERRED
6. Un `[[` sans `]]` fermant produit un fragment ignoré : aucun lien n'est émis. — INFERRED
7. Un alias vide (`[[X|]]`) ne produit pas d'alias. — INFERRED
8. Un lien ne traverse jamais une ligne : un saut de ligne (`\n`) casse le lien. — INFERRED

## Limites et hors périmètre

- La résolution des cibles vers des fichiers du vault, la gestion des fragments/ancres au-delà de leur conservation textuelle, et le rendu des liens sont hors périmètre.
- Le cas de plusieurs séparateurs `|` dans un même lien (alias multiples) n'est pas documenté.
- Les wikilinks contenus dans le frontmatter ou dans les blocs de code ne font pas l'objet d'un comportement distinct ici.

## Label

Phase 0 : tous les comportements ci-dessus sont documentés à partir du format Markdown, sans observation REA — chacun est étiqueté **INFERRED**.
