# Manuel utilisateur

Client de prise de notes obsidian-like en Rust + iced. Ce manuel couvre
l'usage courant : navigation, édition, liaison, recherche, graphe, canvas et
personnalisation.

## Démarrage

```sh
cargo run --bin obsy
```

L'application ouvre le vault de démonstration (`demo-vault`). Pour ouvrir un
autre vault, pointez `DEMO_VAULT` (src/app/mod.rs) vers votre dossier — la
structure attendue est un dossier contenant des `.md`.

## L'espace de travail

- **Ribbon** (gauche) : icônes Lucide — actualiser, graphe, palette de
  commandes, recherche rapide, nouvelle note, sidebar gauche/droite, réglages.
- **Sidebar gauche** : onglets Fichiers / Recherche / Tags / Backlinks /
  Calendrier.
- **Éditeur** : note active ; la prévisualisation Markdown est à droite.
- **Barre d'état** (bas) : compteur de mots/lignes + thème courant.
- Les **panneaux sont redimensionnables** (tirez les séparateurs). La
  disposition est persistée dans `.obsidian/pane_layout.json`.

## Édition

- Cliquez une note dans Fichiers pour l'ouvrir dans un **onglet**. Plusieurs
  onglets peuvent être ouverts ; les modifications non enregistrées sont
  préservées quand vous changez d'onglet.
- `Ctrl+S` sauvegarde ; l'indicateur `●` signale du non enregistré.
- `Ctrl+F` recherche/remplace dans la note (◀/▶ naviguent entre occurrences).
- Renommer / supprimer : boutons du panneau éditeur. La suppression passe par
  la **corbeille** `.trash`.

## Liaisons & recherche

- `[[wikilink]]`, `[[note|alias]]`, `[[note#titre]]`, `![[embed]]` sont
  résolus (clic → ouvre la note).
- `Ctrl+K` : recherche rapide (notes récentes). `Ctrl+P` : palette de
  commandes.
- Recherche full-text (onglet Recherche) : opérateurs `path:`, `tag:`,
  `file:`, `line:N`, `-mot` (exclusion).
- Le **Plan** (outline) liste les titres ; cliquer sur un titre place le
  curseur de l'éditeur à la ligne correspondante.

## Graphe & Canvas

- Graphe : filtres par dossier/tag, profondeur locale (1/2/3), groupes
  colorés, glisser-déposer des nœuds, clic sur un tag → filtre.
- Canvas : cartes éditables, auto-layout, import/export
  `.obsidian/canvas.json`.

## Personnalisation

- Thèmes clair/sombre : `.obsidian/app.json` (`theme`), persisté ; le thème
  est construit depuis les tokens du vault.
- Snippets CSS : variables de couleurs (`--background-primary`, …) appliquées.
- Plugins : registre natif (activés/désactivés via la vue Plugins).
- Police Inter embarquée (OFL).

## Raccourcis

| Touche | Action |
|---|---|
| Ctrl+K | Quick Switcher |
| Ctrl+S | Sauvegarder |
| Ctrl+G | Graphe |
| Ctrl+P | Palette de commandes |
| Ctrl+F | Rechercher/remplacer dans la note |
| Ctrl+Shift+F | Recherche globale (vault) |
| Échap | Fermer l'overlay courant |

## CLI compagnon

```sh
cargo run --bin obsy-cli -- notes
cargo run --bin obsy-cli -- grep "motif"
cargo run --bin obsy-cli -- stats
```
