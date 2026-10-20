# Objectif

Documenter l'extraction des alias de notes depuis le frontmatter YAML et
leur utilisation pour résoudre des liens alternatifs.

## Comportements

1. La clé `aliases` du frontmatter, quand elle vaut une chaîne, produit un seul alias (trimé). — INFERRED
2. Quand elle vaut une séquence de chaînes, chaque élément chaîne (trimé) produit un alias, dans l'ordre d'apparition. — INFERRED
3. Les éléments non-chaîne d'une séquence sont ignorés. — INFERRED
4. Clé absente, frontmatter absent, ou valeur ni chaîne ni séquence → aucune liste vide. — INFERRED
5. Un alias vide (après trim) est ignoré. — INFERRED
6. La résolution d'un alias est insensible à la casse. — INFERRED
7. Un alias peut être partagé par plusieurs notes ; la résolution retourne toutes les notes correspondantes. — INFERRED
8. Un alias inconnu retourne une liste vide. — INFERRED

## Limites et hors périmètre

- La normalisation des alias (diacritiques, espacements internes) est hors périmètre.
- L'intégration des alias dans la résolution de wikilinks de l'index global n'est pas documentée ici (module `aliases` autonome).
- La clé n'est reconnue qu'en minuscules exactes (`aliases`).

## Label

Phase 2 : comportements déduits du format frontmatter d'Obsidian, sans
observation REA — chacun est étiqueté **INFERRED**.
