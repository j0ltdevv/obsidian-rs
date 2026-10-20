# Objectif

Documenter les raccourcis clavier par défaut et les actions qu'ils
déclenchent.

## Comportements

1. Six actions sont définies : ouvrir le sélecteur de notes (`OpenSwitcher`),
   enregistrer la note (`SaveNote`), basculer la vue du graphe (`ToggleGraph`),
   créer une nouvelle note (`NewNote`), basculer la barre latérale
   (`ToggleSidebar`), annuler/fermer (`Escape`). — INFERRED
2. La table par défaut mappe : `Ctrl+K` → OpenSwitcher, `Ctrl+S` → SaveNote,
   `Ctrl+G` → ToggleGraph, `Ctrl+N` → NewNote, `Ctrl+B` → ToggleSidebar,
   `Escape` → Escape. — INFERRED
3. Chaque combinaison est unique : une action correspond à exactement un
   raccourci, et inversement. — INFERRED
4. Les modificateurs sont triés lexicographiquement (`Alt` < `Ctrl` < `Shift`),
   ce qui rend la représentation canonique et déterministe. — INFERRED
5. La liste des raccourcis est triée par action (`KeyAction`), pour un ordre
   déterministe indépendant de l'ordre d'écriture. — INFERRED
6. Le module expose des données pures, sans dépendance iced ni état applicatif ;
   la couche UI traduit ces données en événements clavier. — INFERRED

## Limites et hors périmètre

- Pas de remappage ni de raccourcis personnalisés documentés : la table est
  fixe.
- `Escape` est le seul raccourci sans modificateur.
- La traduction effective en événements clavier (couche UI) est hors périmètre.

## Label

Phase 2 : tous les comportements ci-dessus sont déduits du module
`vault::keymap`, sans observation REA — chacun est étiqueté **INFERRED**.
