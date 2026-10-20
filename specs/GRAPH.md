# Objectif

Documenter le graphe non orienté des notes du vault : nœuds, arêtes issues
des wikilinks résolus, et requêtes (voisins, composantes, degré).

## Comportements

1. Les nœuds du graphe sont les notes du vault, dans l'ordre du vault : chaque
   nœud est identifié par l'indice de sa note (un nœud par note). — INFERRED
2. Une arête non orientée existe entre deux notes si au moins l'une des deux
   possède un wikilink résolu vers l'autre ; elle relie la paire une seule fois,
   indépendamment du sens du lien. — INFERRED
3. Les wikilinks dangling (cible inexistante) et ambigus (plusieurs cibles
   possibles) sont ignorés : ils ne produisent aucune arête. — INFERRED
4. Les boucles (wikilink d'une note vers elle-même) sont ignorées : aucun nœud
   n'est jamais relié à lui-même. — INFERRED
5. Les arêtes sont stockées triées par paire (a < b) et dédoublonnées. —
   INFERRED
6. `neighbors(node)` retourne les voisins directs, dans l'ordre croissant ; un
   nœud inconnu retourne une liste vide. — INFERRED
7. `components()` retourne (nombre de composantes connexes, taille de la plus
   grande), calculés par parcours BFS ; les nœuds isolés comptent comme des
   composantes de taille 1. — INFERRED
8. `degree(node)` retourne le degré du nœud (nombre d'arêtes incidentes) ; un
   nœud inconnu retourne 0. — INFERRED
9. L'ordre des nœuds est toujours l'ordre du vault ; le graphe ne réordonne ni
   ne renomme les notes. — INFERRED

## Limites et hors périmètre

- Graphe simple : au plus une arête par paire, pas de multi-arêtes ni de poids.
- Graphe non orienté : le sens du wikilink n'est pas conservé.
- Pas de persistance : le graphe est reconstruit en mémoire à partir des notes
  scannées.
- La résolution des wikilinks (dangling/ambigu) relève de l'index ; le graphe
  ne fait que consommer ses résultats.

## Label

Phase 2 : tous les comportements ci-dessus sont déduits du module
`vault::graph`, sans observation REA — chacun est étiqueté **INFERRED**.
