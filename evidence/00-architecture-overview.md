# 00 — Vue d'ensemble de l'architecture

Doc d'architecture de l'artefact **Obsidian** (client de type Obsidian en Rust + iced, clean-room). Ce document résume les constats d'inventaire générés par REA (outils MCP) et les inférences/limites qui en découlent.

## Conventions de labels

Chaque affirmation porte un label de statut :

- **SEEN** — observé directement via l'ASAR / l'inventaire REA.
- **INFERRED** — déduit de constats SEEN (raisonnement explicite), non directement observé.
- **UNKNOWN** — non observé, non vérifiable à ce stade.

---

## 1. Méthode

- **SEEN** — Artefact analysé : `/usr/lib/obsidian/obsidian.asar`.
- **SEEN** — Hash SHA-256 : `a52a7daf1e2460bae03de80f2816604bd16a56cd374fbe5ce8d1a9ef5604059d`.
- **SEEN** — Taille : 25 786 363 octets.
- **SEEN** — Format : asar, hash vérifié.
- **SEEN** — Inventaire complet REA (source_complete=true) : **381 nœuds** (359 fichiers, 22 dossiers), **380 arêtes**, 0 contradiction d'intégrité.
- **SEEN** — Exploration réalisée via les outils MCP de REA, inventaire paginé complet.

---

## 2. Architecture globale

- **SEEN** — L'arborescence contient des fichiers web classiques (`index.html`, `app.css`, `app.js`, `help.html`, etc.), un `worker.js`, et des bundles JavaScript → **client Electron** (couche renderer HTML/JS).
- **SEEN** — `package.json` présent dans l'ASAR (260 o, name=obsidian-dev, version=1.13.7) : champ `main` = `index.js`.
- **SEEN** — `main.js` présent dans l'ASAR (199 730 o) : bundle minifié du main process Electron (require child_process/fs/original-fs/net/os/path/url/util).
- **SEEN** — `index.js` (le `main` déclaré) est ABSENT de l'ASAR → le point d'entrée réel diffère de la déclaration.
- **UNKNOWN** — `preload.js` : aucun fichier preload observable ; les ponts contextBridge restent à élucider.
- **INFERRED** — Le main process Electron est exécuté depuis `main.js` contenu dans l'ASAR.
- **UNKNOWN** — Détails du main process non vérifiables ici : mécanique IPC, gestion des fenêtres, relations preload/renderer.

---

## 3. Modules et responsabilités

- **SEEN** — `app.js` (3 876 459 o) : bundle renderer, UI + logique applicative.
- **SEEN** — `main.js` (199 730 o) : main process Electron (bundle minifié).
- **SEEN** — `worker.js` (239 283 o) : worker (traitement en arrière-plan).
- **SEEN** — `enhance.js` (13 346 o) / `sim.js` (17 693 o) : petits modules auxiliaires.
- **SEEN** — `help.js` (377 345 o) : aide ; `i18n.js` (133 013 o) + `i18n/` (~60 fichiers `.txt` de locales, ~80–210 ko) : internationalisation.
- **SEEN** — `lib/` : bibliothèques tierces :
  - `codemirror/` (codemirror.js 400 ko, modes.min.js 562 ko, vim.js 249 ko, markdown.js 44 ko, cm-addons.js 58 ko, overlay.js, meta.min.js) → éditeur de code/markdown (CodeMirror).
  - `mathjax/` (output/chtml/fonts/woff-v2/*.woff) → rendu mathématique.
  - `pdfjs/` (cmaps/ ~300 .bcmap, iccs/, images/ .svg, standard_fonts/ .pfb, pdf.min.mjs 566 ko, pdf.viewer.min.mjs 488 ko, pdf.worker.min.mjs 1,13 Mo) → rendu PDF.
  - `i18next.min.js` (41 ko) → internationalisation.
  - `defuddle.full.js` (750 ko) → extraction/clonage de contenu.
- **SEEN** — `sandbox/` : contenu d'onboarding multi-langues (ex. `Start here.md`, `Formatting/*`, `Guides/*`, `Adventurer/*`, …) — chemins d'exemple de structure uniquement, **aucun contenu réutilisé**.
- **UNKNOWN** — Les noms internes « App / Vault / Workspace / MetadataCache / FileManager » ne sont **pas observés** dans l'inventaire → non élucidés.

---

## 4. Chargement / indexation / affichage

- **UNKNOWN** — Non élucidé sans analyse du code JS (déroulé du chargement, indexation, pipeline d'affichage).
- **UNKNOWN** — Prochain pas : `analyze_javascript_application` une fois le timeout MCP levé (voir § 7).

---

## 5. Système de plugins

- **SEEN** — Indice : présence du dossier `sandbox/` (contenu d'onboarding).
- **UNKNOWN** — La mécanique réelle des plugins n'est pas élucidée.

---

## 6. Points d'entrée

- **SEEN** — `index.html` (1 445 o) + `app.js` (renderer) : points d'entrée de la couche renderer.
- **SEEN** — Point d'entrée main process : `main.js` (dans l'ASAR) ; `package.json` déclare `index.js` (absent).

---

## 7. Prochaines probes REA

- **UNKNOWN** — `analyze_javascript_application` : **bloqué par timeout MCP client 30 s**.
- **SEEN** — Configuration posée : `mcp.json` timeout `600000` ; **reload requis** via `/mcp reload`.
- Une fois levé, l'analyse JavaScript permettra d'élucider les sections 4, 5 et les détails du main process (section 2).

## 7bis. Résultats de l'analyse JS (portée main.js + package.json)

- **SEEN** — Analyse JS bornée exécutée via `analyze_javascript_application` sur `package.json` + `main.js` : réussie.
- **SEEN** — 37 fichiers pertinents analysés ; 27 491 nœuds AST ; 77 findings ; 0 échec de parse.
- **SEEN** — Main process : 2 `BrowserWindow` créées ; 0 preload / contextBridge exposé.
- **SEEN** — IPC : 45 opérations / canaux littéraux / handlers main ; 1 observation de validation sender.
- **SEEN** — 35 fichiers hors scope (dont `app.js`, renderer).

## 7ter. Analyse JS complète (tous les fichiers d'application)

- **SEEN** — `relevant_files: 37`, `parsed_javascript_files: 5`, `parse_failures: 0`, `text_bytes_read: 4 771 961`, `visited_ast_nodes: 1 276 779`, `findings: 778`, `modules: 363` (420 en paginé).
- **SEEN** — `electron.browser_windows: 2` ; `ipc: 45 literal_channels, 45 main_handlers, 0 dynamic, 0 renderer_transmissions`.
- **SEEN** — `preload_entrypoints: 0`, `context_bridge_apis: 0`, `native_addon_bindings: 0`.
- **INFERRED** — Aucune transmission renderer appariée + aucun pont preload/contextBridge → le renderer accède vraisemblablement aux API Node directement (`nodeIntegration` activé) ; non prouvé dynamiquement.
- **SEEN** — `source_selection: 29 fichiers non analysés` (vendor `lib/`) ; l'analyse complète de l'ASAR entier dépasse le timeout interne REA (300 s) et n'est pas relancée.
- **SEEN** — Les 45 canaux IPC littéraux sont tous traités côté main process (liste complète : voir synthèse `evidence/01-canaux-ipc.md`).

## 8. Modules et découpage (analyse app.js)

- **SEEN** — La collection `modules` (app.js) pagine 228 lignes ; la quasi-totalité sont des chunks du bundle renderer (`javascript-module` dans `app.js`), sans noms internes exploitables.
- **SEEN** — Dépendances assets référencées : `lib/scrypt.js`, `lib/prism.min.js` (highlight), `lib/mathjax/tex-chtml-full.js`, `lib/defuddle.full.js`, `sim.js`, `starter.js`, `i18next.min.js`, `lib/pdfjs/*`.
- **UNKNOWN** — Les collections `sections`/`symbols` ne s'appliquent pas à l'Evidence JavaScript (réservées à `inspect_binary_layout`) → découpage interne symbolique non accessible par cette voie.

- **SEEN** — Libs analysées (12 fichiers, 4 308 539 o, 1 024 125 nœuds AST, 913 findings, 0 échec parse) :
  - CodeMirror (`lib/codemirror/*`, 7 fichiers, 1 330 731 o) : 0 module → scripts globaux (noyau, modes, vim, markdown, addons) [SEEN] ; attribution fine [INFERRED].
  - i18next (`lib/i18next.min.js`, 41 340 o) : script global [SEEN].
  - Defuddle (`lib/defuddle.full.js`, 750 220 o) : **seul bundle multi-modules** (64 modules) [SEEN].
  - PDF.js (`lib/pdfjs/*`, 3 fichiers ESM `.mjs`, 2 186 248 o) : noyau/viewer/worker [SEEN] ; nature worker [INFERRED].
- **SEEN** — Aucune intégration Electron détectée dans les libs (0 fenêtre/preload/IPC) ; 0 child-process.
- **UNKNOWN** — Versions exactes des libs, exports/API détaillés, graphe complet (détail `complete` bloqué par budget transport stdio ~10,5 MiB ; heap worker épuisé sur `pdf.viewer.min.mjs`).
