# Objectif

Documenter le store de marque-pages : un fichier JSON dans
`.obsidian/bookmarks.json`, lu et écrit de façon atomique.

## Comportements

1. Le store est chargé depuis `.obsidian/bookmarks.json` ; fichier absent ou contenu vide → store vide. — INFERRED
2. Un JSON invalide produit une erreur (pas de panique). — INFERRED
3. L'enregistrement (save) écrit de façon atomique : fichier temporaire puis rename, en créant `.obsidian/` si nécessaire. — INFERRED
4. `add` génère un identifiant unique, ajoute le marque-page et le retourne. — INFERRED
5. `remove` retire par id et retourne `true` si un élément a été retiré. — INFERRED
6. `list` retourne les marque-pages dans l'ordre d'ajout ; `get` les retrouve par id. — INFERRED

## Limites et hors périmètre

- Les id sont générés sans dépendance externe (horodatage + compteur) : unicité garantie dans un process, pas cryptographique.
- Le tri, la recherche ou la déduplication par nom sont hors périmètre.
- La synchronisation multi-process (plusieurs instances) n'est pas gérée.

## Label

Phase 2 : comportements déduits de la persistance du plugin Bookmarks
d'Obsidian, sans observation REA — chacun est étiqueté **INFERRED**.
