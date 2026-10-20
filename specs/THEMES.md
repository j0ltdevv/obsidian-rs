# Objectif

Documenter les palettes de thèmes intégrées (`dark`, `light`), leur sélection
par nom, et la vue de sélection UI (pastille de la couleur d'accent, thème
courant surligné).

## Comportements

### 1. Thèmes et sélection par nom

1. Deux thèmes intégrés : `DEFAULT_DARK` (nom `"dark"`) et `DEFAULT_LIGHT` (nom
   `"light"`). — INFERRED
2. `by_name` sélectionne un thème par nom, de façon insensible à la casse
   (`"dark"`, `"DARK"`, `"Dark"`, `"LiGhT"`…). — INFERRED
3. Un nom inconnu (ou vide) retourne `None`, plutôt qu'un thème par défaut : le
   choix de l'utilisateur n'est jamais silencieusement remplacé. — INFERRED
4. Chaque thème expose cinq couleurs (`accent`, `bg`, `fg`, `surface`,
   `border`) dont les composantes RGB sont des octets dans `0..=255`. — INFERRED
5. Le contraste fg/bg est vérifié : le ratio WCAG entre premier plan et fond est
   supérieur à 3.0 pour chacun des deux thèmes. — INFERRED
6. `by_name("dark")` et `by_name("light")` retournent des thèmes distincts. —
   INFERRED

### 2. Vue de sélection (`themes_view`)

7. La vue affiche les thèmes connus (`dark`, `light`) sous forme de boutons, dans
   l'ordre de la liste interne (`THEME_NAMES`). — INFERRED
8. Chaque bouton porte le nom du thème et une pastille circulaire de la couleur
   `accent` du thème ; la pastille est un conteneur sans texte, de rayon
   `DOT_SIZE / 2` (5.0 pixels), avec une bordure transparente. — INFERRED
9. La lookup se fait par `by_name`, qui est insensible à la casse et retourne
   `None` pour un nom inconnu : un thème absent de la liste est simplement
   ignoré, sans fallback silencieux. — INFERRED
10. Cliquer sur un bouton appelle `on_select` avec le nom du thème
    (`theme.name.to_string()`). — INFERRED
11. Le thème actuellement sélectionné est surligné : la comparaison entre le nom
    du thème et `current` est insensible à la casse
    (`eq_ignore_ascii_case`). — INFERRED
12. Le surlignage du thème courant se traduit par un fond très léger de la
    couleur d'accent (alpha 0.12), une bordure de couleur accent (épaisseur
    1.0, rayon 4.0) et un texte coloré avec la couleur d'accent. — INFERRED
13. Un thème inconnu ne surligne rien : si `current` ne correspond à aucun thème,
    tous les boutons restent dans leur état normal. — INFERRED
14. Au survol, un bouton non courant reçoit un fond semi-transparent noir
    (alpha 0.04). — INFERRED
15. La conversion d'un triplet RGB `(0..=255)` en `Color` iced divise chaque
    composante par 255.0 ; l'opacité résultante est 1.0. — INFERRED

## Limites et hors périmètre

- Données pures (`vault::themes`) sans dépendance UI : le module ne sait pas
  appliquer un thème à l'interface.
- Seuls `"dark"` et `"light"` sont reconnus ; pas de construction de thème
  personnalisé au runtime.
- Le calcul de luminance/contraste n'existe que dans les tests ; il n'est pas
  exposé comme API publique.
- La vue de sélection ne gère pas l'application effective du thème : elle ne fait
  qu'émettre un message avec le nom choisi, que l'appelant doit interpréter.
- Les couleurs des thèmes sont des constantes ; aucune lecture de fichier ou de
  préférence n'est réalisée par `vault::themes`.

## Label

Phase 3 : tous les comportements ci-dessus sont déduits des modules
`vault::themes` et `ui::themes_view`, sans observation REA — chacun est étiqueté
**INFERRED**.
