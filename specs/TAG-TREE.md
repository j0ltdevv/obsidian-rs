# Objectif

Documenter la construction de l'arbre hiérarchique de tags à partir des
tags plats (`a/b/c`).

## Comportements

1. Chaque tag est découpé sur `/` ; un chemin `a/b/c` crée les nœuds `a` > `b` > `c`. — INFERRED
2. Les segments vides (slashes répétés) sont ignorés ; un tag ne contenant que des segments vides est ignoré. — INFERRED
3. `count` d'un nœud = nombre de feuilles distinctes qui le traversent (un tag compte pour 1 à chaque niveau). — INFERRED
4. Les frères sont triés par ordre alphabétique. — INFERRED
5. Une liste vide produit un arbre vide. — INFERRED
6. Un tag sans `/` est une racine directe. — INFERRED
7. Les doublons d'un même chemin ne comptent qu'une fois par niveau. — INFERRED

## Limites et hors périmètre

- La normalisation de la casse des tags n'est pas effectuée ici.
- Le filtre par tag (sélection d'un nœud) relève de l'UI, pas de ce module.
- Les comptes sont dérivés de la liste de tags passée en entrée, pas d'un rescan.

## Label

Phase 2 : comportements déduits du panneau Tags d'Obsidian, sans
observation REA — chacun est étiqueté **INFERRED**.
