# Objectif

Documenter le comportement de l'éditeur de notes : saisie, affichage et
sauvegarde de contenu Markdown brut.

## Comportements

1. L'éditeur est un éditeur de **texte brut** : le contenu est une chaîne de
   caractères unique, sans arbre de rendu ni syntaxe éditables. — INFERRED
2. Les titres Markdown (`# Titre` à `###### Titre`) sont affichés et édités
   comme du texte ordinaire ; aucun niveau de titre n'est reconnu
   structurellement par l'éditeur. — INFERRED
3. Les callouts (`> [!NOTE]`, `> [!WARNING]`, …) sont conservés comme du texte
   brut, bloc par bloc ; l'éditeur ne les distingue pas des autres lignes
   commençant par `>`. — INFERRED
4. Les blocs de code, qu'ils soient délimités par des fences (```) ou par
   indentation, sont traités comme du texte littéral : ni coloration syntaxique,
   ni exécution, ni modification automatique. — INFERRED
5. Les listes (ordonnées ou non) et les tableaux (avec leur ligne séparatrice
   `|---|`) sont édités ligne par ligne ; l'éditeur n'ajoute ni ne supprime de
   lignes lors de la saisie. — INFERRED
6. Les paragraphes adjacents séparés par une ligne vide restent distincts ; une
   ligne vide supprimée fusionne les deux paragraphes qui l'encadraient. — INFERRED
7. Les lignes vides en début, en fin ou au sein du document sont conservées
   telles quelles, y compris en fin de fichier. — INFERRED
8. Les balises HTML présentes dans le contenu sont ignorées : elles ne sont ni
   interprétées ni filtrées, et sont sauvegardées comme du texte ordinaire. — INFERRED
9. La sauvegarde écrit le contenu brut sur le disque de façon atomique
   (`vault::ops::write_note`) ; un échec est signalé par une bannière d'erreur,
   sans perte du tampon d'édition. — INFERRED
10. La prévisualisation affiche le contenu brut dans une police monospace, tel
    qu'enregistré, sans rendu Markdown. — INFERRED

## Limites et hors périmètre

- Aucun rendu WYSIWYG : gras, italique, liens, images et blocs mathématiques ne
  sont pas mis en forme dans l'éditeur.
- La coloration syntaxique des blocs de code, la numérotation automatique des
  listes et l'alignement des tableaux ne sont pas implémentés.
- L'éditeur n'offre pas d'annulation/réaction (undo/redo) au-delà de celle du
  widget sous-jacent, ni de raccourcis clavier propres (la gestion des touches
  relève du module `keymap`).
- Les fichiers autres que `.md` ne sont pas ouverts par l'éditeur.

## Label

Phase 0 : tous les comportements ci-dessus sont déduits du format Markdown et
de l'usage d'un widget texte brut, sans observation REA — chacun est étiqueté
**INFERRED**.
