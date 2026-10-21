# ADR-001 — Stack d'interface

**Date :** 2026-10-21 · **Statut :** accepté · **Décideur :** propriétaire.

## Décision

Deux interfaces sont livrées, toutes deux alimentées par le **même moteur Rust**
(`src/vault/` + `src/engine/`) :

1. **Notre front HTML/CSS** (`web/`) dans une coquille **Tauri 2** (webview
   système) — interface **primaire** : notre code, robuste aux mises à jour
   d'Obsidian, apparence reproduite à partir des mesures (`UI-SPEC.md`).
2. **Mode « compat »** : l'**interface réelle d'Obsidian** (leur renderer
   Electron) pilotée par notre **shim** + notre moteur — fidélité exacte,
   dépend de l'application Obsidian **installée** chez l'utilisateur.
   La chaîne d'acceptation (284 caractères) est **lue à l'exécution** depuis
   cette installation ; elle n'est **jamais copiée** dans le dépôt.

L'UI **native iced** (22 Mo) reste le **filet léger** (build de repli).

## Contexte et mesures

- Budget mémoire : **200-400 Mio** (décision du propriétaire), fidélité
  visuelle prioritaire.
- Mesuré le 21/10/2026 (PSS de l'arbre de processus complet, sauf mention) :

  | Stack | PSS | RSS |
  |---|---|---|---|
  | iced + tiny-skia (release) | — | 22,0 Mo |
  | Tauri 2 + WebKitGTK, page vide | 286 Mo | 467 Mo |
  | Tauri 2 + WebKitGTK + CodeMirror 6 ouvert | 387 Mo | 559 Mo |
  | Electron 45 + UI Obsidian chargée | **298 Mo** | 606 Mo |
  | Chromium simple, page vide | ~400 Mo | 843 Mo cumulés |

  → la voie « Chromium simple + shim » est écartée (hors budget avant même
  d'afficher l'UI) ; Electron et notre webview tiennent tous deux dans le
  budget en PSS.

## Périmètre

- `src/vault/` + `src/engine/` : cœur partagé (parsing, index, liens,
  recherche, service HTTP).
- `web/` + `src-tauri/` : front principal (HTML/CSS + coquille).
- `shim/` : pont vers leur renderer (mode compat).
- `src/ui/` (iced) : filet natif.

## Conséquences

`AGENTS.md` décrit un éditeur en iced ; la voie Tauri/webview etend ce choix.
Le propriétaire doit mettre à jour `AGENTS.md` s'il entérine cette direction.
