# Objectif

Documenter le comportement de la recherche rapide de notes : correspondance,
scoring et ordre des résultats.

## Comportements

1. La recherche porte sur le **nom** des notes (stem du fichier, sans
   extension), et non sur leur contenu. — INFERRED
2. La correspondance est une **sous-séquence** : chaque caractère de la requête
   doit apparaître dans le nom, dans l'ordre, mais pas nécessairement de façon
   contiguë. — INFERRED
3. La recherche est **insensible à la casse** : la requête et le nom sont
   normalisés en minuscules avant comparaison. — INFERRED
4. Les caractères de ponctuation et les espaces de la requête sont traités comme
   des caractères ordinaires à faire correspondre ; ils ne sont ni supprimés ni
   tokenisés. — INFERRED
5. Le score d'une correspondance décroît avec la somme des positions des
   caractères appariés : une correspondance plus précoce est mieux classée. — INFERRED
6. Un bonus de **couverture** est appliqué : plus la requête est longue par
   rapport au nom, plus le score est élevé. — INFERRED
7. Un **bonus de préfixe** est appliqué quand le nom entier commence par la
   requête exacte. — INFERRED
8. Les résultats sont triés par score décroissant, puis limités à une constante
   (`SEARCH_LIMIT`, 10 par défaut). — INFERRED
9. Une requête vide retourne **toutes** les notes, dans l'ordre du vault, avec
   un score nul. — INFERRED
10. L'ordre est **stable** : à score égal, l'ordre d'origine des notes dans le
    vault est préservé. — INFERRED
11. Une requête qui n'est sous-séquence d'aucun nom retourne une liste vide. — INFERRED

## Limites et hors périmètre

- Pas de *stemming* : « écrire » ne correspond pas à « écriture ».
- Pas de classement sémantique (pas d'analyse du contenu, pas de similarité
  vectorielle, pas de backlinks dans le score).
- Pas de recherche floue au sens de la distance d'édition : un caractère manquant
  ou ajouté dans la requête casse la correspondance.
- La recherche ne porte pas sur le contenu des notes, ni sur les tags, ni sur le
  frontmatter.
- Les doublons de nom (notes homonymes dans des dossiers différents) sont tous
  retenus, chacun avec son propre score.

## Label

Phase 0 : tous les comportements ci-dessus sont déduits de l'algorithme de
sous-séquence et de la fonction de score, sans observation REA — chacun est
étiqueté **INFERRED**.
