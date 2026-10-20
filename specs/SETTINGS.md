# Objectif

Documenter la persistance des paramètres de l'application
(`.obsidian/app.json`) et des raccourcis clavier personnalisés
(`.obsidian/hotkeys.json`), en format JSON simple aligné sur les conventions
Obsidian.

## Comportements

### 1. Paramètres d'application (`AppSettings`)

1. `AppSettings` expose trois champs : `theme` (String, défaut `"dark"`),
   `vim_mode` (bool, défaut `false`) et `spellcheck` (bool, défaut
   `false`). — INFERRED
2. Les champs absents prennent leur valeur par défaut via `#[serde(default)]` ;
   les champs inconnus sont ignorés. — INFERRED
3. `load_app_settings` lit `.obsidian/app.json` : fichier absent ou vide →
   `AppSettings::default()` ; JSON invalide → erreur. — INFERRED
4. `save_app_settings` écrit `.obsidian/app.json` de façon atomique
   (temporaire + `rename`), en créant `.obsidian/` si nécessaire. — INFERRED

### 2. Raccourcis clavier (`Hotkeys`)

5. `Hotkey` est une paire `command` (String) et `keys` (liste de String) ;
   `Hotkeys` est une liste de `Hotkey`. — INFERRED
6. `load_hotkeys` lit `.obsidian/hotkeys.json` : fichier absent ou vide →
   liste vide ; JSON invalide → erreur. — INFERRED
7. `save_hotkeys` écrit `.obsidian/hotkeys.json` de façon atomique
   (temporaire + `rename`), en créant `.obsidian/` si nécessaire. — INFERRED
8. Aucune écriture ne laisse de fichier temporaire résiduel. — INFERRED

## Limites et hors périmètre

- Le module ne fait que lire/écrire les fichiers ; il n'applique pas les
  paramètres (thème, mode Vim, correcteur) à l'interface.
- Seuls les champs `theme`, `vim_mode` et `spellcheck` sont gérés ; tout autre
  contenu est rejeté comme JSON invalide.
- Aucune valeur par défaut d'Obsidian n'est recopiée au-delà des trois champs
  documentés ci-dessus.

## Label

Phase 3 : tous les comportements ci-dessus sont déduits du module
`vault::settings`, sans observation REA — chacun est étiqueté **INFERRED**.
