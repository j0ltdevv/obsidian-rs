# Objectif

Documenter les palettes de thèmes intégrées (dark/light) et leur sélection par
nom.

## Comportements

1. Deux thèmes intégrés : `DEFAULT_DARK` (nom `"dark"`) et `DEFAULT_LIGHT`
   (nom `"light"`). — INFERRED
2. `by_name` sélectionne un thème par nom, de façon insensible à la casse
   (`"dark"`, `"DARK"`, `"Dark"`, `"LiGhT"`…). — INFERRED
3. Un nom inconnu (ou vide) retourne `None`, plutôt qu'un thème par défaut : le
   choix de l'utilisateur n'est jamais silencieusement remplacé. — INFERRED
4. Chaque thème expose cinq couleurs (`accent`, `bg`, `fg`, `surface`,
   `border`) dont les composantes RGB sont des octets dans `0..=255`. —
   INFERRED
5. Le contraste fg/bg est vérifié : le ratio WCAG entre premier plan et fond est
   supérieur à 3.0 pour chacun des deux thèmes. — INFERRED
6. `by_name("dark")` et `by_name("light")` retournent des thèmes distincts. —
   INFERRED

## Limites et hors périmètre

- Données pures, sans dépendance UI : le module ne sait pas appliquer un thème à
  l'interface.
- Seuls `"dark"` et `"light"` sont reconnus ; pas de construction de thème
  personnalisé au runtime.
- Le calcul de luminance/contraste n'existe que dans les tests ; il n'est pas
  exposé comme API publique.

## Label

Phase 2 : tous les comportements ci-dessus sont déduits du module
`vault::themes`, sans observation REA — chacun est étiqueté **INFERRED**.
