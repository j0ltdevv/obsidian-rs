# Objectif

Documenter le modèle du Quick Switcher : construction des résultats à partir de
la requête, des récents et du moteur de recherche.

## Comportements

1. Le modèle garde une requête (`query`), les résultats affichés (`results`),
   les notes ouvertes récemment (`recents`, de la plus récente à la moins
   récente, sans doublon) et une limite (`limit`) qui plafonne à la fois
   `results` et `recents`. — INFERRED
2. `update_query` reconstruit `results` : d'abord les récents qui correspondent à
   la requête (dans l'ordre d'utilisation), puis les autres résultats du moteur
   dans leur ordre, sans doublon. — INFERRED
3. Une requête vide est une correspondance universelle : `results` devient les
   récents (ordre d'utilisation) suivis des autres notes dans l'ordre du vault.
   — INFERRED
4. Le moteur de recherche n'est interrogé qu'une seule fois par mise à jour. —
   INFERRED
5. `results` est tronqué à `limit` ; si `limit` vaut 0, `results` reste vide. —
   INFERRED
6. `record_open(index)` enregistre l'ouverture d'une note : elle devient la plus
   récente ; si elle était déjà dans les récents, elle est déplacée en tête ;
   les doublons sont supprimés et la liste tronquée à `limit`. — INFERRED
7. `select(pos)` retourne l'index de la note à la position `pos` dans `results`,
   ou `None` si la position est hors bornes. — INFERRED
8. Le modèle ne valide pas les index : c'est à l'appelant de garantir qu'ils
   correspondent à des notes existantes. — INFERRED

## Limites et hors périmètre

- Modèle en mémoire : les récents ne sont pas persistés entre les sessions.
- Le moteur de recherche est fourni par l'appelant ; le modèle ne calcule pas de
  pertinence.
- Pas de gestion de sélection active ni de navigation clavier hors `select`.

## Label

Phase 2 : tous les comportements ci-dessus sont déduits du module
`vault::quick_switcher`, sans observation REA — chacun est étiqueté
**INFERRED**.
