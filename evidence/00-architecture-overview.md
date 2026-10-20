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
