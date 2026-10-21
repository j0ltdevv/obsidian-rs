# SHIM-SPEC — Spécification du shim Electron/Node pour exécuter le rendu Obsidian contre un moteur Rust

**Date :** 2026-10-21 · **Portée :** document unique, aucune autre modification
**Cible :** `obsidian-1.14.4.asar` — renderer `app.js` (3,94 Mo, 1 ligne minifiée), `enhance.js`, `index.html`, `main.js`, `worker.js`, `sim.js`, `lib/*`
**Référence :** `ANALYSE.md` (analyse statique REA) ; preuves sur la box instabox `hermes` (`/data/home/obsidian-rea/`)

> **Question tranchée :** quel est le shim minimal pour faire tourner le renderer Obsidian contre un moteur Rust, et est-ce plausible un week-end, un mois, un trimestre ?

---

## 0. Méthode et reproductibilité

Analyse **statique inerte** (aucune exécution) de `app.js` + `enhance.js`, refaite pour ce document
sur la box `hermes` (Node v22.23.3), via one-liners `node` qui analysent le JSON REA
(`rea-focused-evidence.json`, 57 Mo → graphe 354 nœuds / 711 arêtes) et comptent des
expressions régulières sur le texte brut de `app.js`.

Commandes réelles utilisées (chacune = un script `.mjs` poussé en base64 puis `node`) :

1. `node /tmp/probe2.mjs` — structure des JSON (clés `normalized_result.graph.nodes/edges` ; l'évidence 89 Mo = timeout `provider_timeout` avec `partial_observation`, non exploitable).
2. `node /tmp/grep1.mjs` — comptage d'occurrences des API Electron et des specifiers Node dans `app.js`.
3. `node /tmp/pat1.mjs` — extraction des sites `require(...)`, `ipcRenderer.invoke/send/on/once/removeListener` avec leur offset.
4. `node /tmp/ctx.mjs` — contexte ±160 car. de chaque `require(...)`, `"electron"`, `process.x`.
5. `node /tmp/ipc2.mjs` — tous les canaux `ipcRenderer.*` (par méthode), `window.require("...")`, accès `.remote.<membre>`.
6. `node /tmp/preload.mjs` — nœud `electron-preload`, nœuds `endpoint`, `worker`.
7. `node /tmp/idx.mjs` — nœuds `storage` (clés localStorage), liste `artifact`/`javascript-asset`.
8. `node /tmp/asar.mjs` — liste complète des 38 entrées ASAR, recherche `window.electron=`.
9. `node /tmp/fin.mjs` — `Qy.isDesktopApp`, lectures `window.electron.*`, `electron.remote.*`, `window.process.*`, compteurs finaux `contextBridge`/`ipcMain`/`preload`/`nodeIntegration`.

Toutes les conclusions de ce document portent une preuve (offset / occurrence / littéral) ci-dessous.

---

## 1. Fait de faisabilité clé : `nodeIntegration` direct, pas de preload bridge

Vérifié à partir des preuves exactes :

- **`contextBridge` : 0 occurrence** ; **`ipcMain` : 0 occurrence** (comptage `grep1.mjs`).
- **Aucun script preload dans l'ASAR** : les 38 entrées comprennent `app.js`, `main.js`, `worker.js`, `sim.js`, `i18n.js`, `starter.js`, `lib/*`… — **pas de `preload.js`**. Les 8 occurrences du mot `preload` dans `app.js` sont toutes étrangères (attribut HTML `preload`, `<video preload="metadata">`, méthode `preload()` du cache de métadonnées) — voir `fin.mjs`/`pre2.mjs`.
- Le renderer passe par **`window.require`** (require global de page) et **`window.process`**, **`window.electron`** :

  - `@795100` : `e=window.require("original-fs").readFileSync(ub,"utf8")`
  - `@1366824` : `(n=window.require&&window.require("crypto"))?…n.scrypt(Buffer.from(e,"utf8"),…)`
  - `@1536032` : `var e=window.require("path"),t=electron.ipcRenderer.sendSync("vault").path,…`
  - `@565970`, `@566236`, `@566390` : `(t=window.require)?(n=t("fs"),…)`, `t("fs").promises.mkdir`, `t("fs").existsSync`
  - `@1254973` : `var t=window.process.platform,…window.process.arch`
  - `@819119` : `cw=process.versions.electron,…Qy.resourcePathPrefix=e…` (via `rw(e=>…)` qui fait `iw("electron")`)

- Helper d'acquisition d'Electron, `@817737` :
  ```js
  var nw=window.require; function iw(e){return nw&&nw(e)}
  function rw(e){var t=iw("electron");t&&e(t)}
  ```
  et `@818883` (fonction `lw`) :
  ```js
  if(!(t=iw("electron"))) throw new Error("Not electron"); return n=t.ipcRenderer,…n.once(r,…)
  ```

**Conclusion :** le renderer suppose `window.require` (gateway Node) et `window.electron` présents dans la page. C'est l'équivalent d'un `nodeIntegration:true` **sans** `contextBridge`. Le shim DOIT donc fournir un `require` de type Node dans la page (ou réécrire ces accès). C'est la part la plus délicate — et le point de faisabilité n°1.

---

## 2. Surface API Electron référencée (comptages bruts `app.js`)

| API | Occurrences | Offsets significatifs | Usage observé |
|---|---|---|---|
| `ipcRenderer` | 68 | 1249075, 1250043, 2615401, 2985978, 3391369, 3553743, 3558622, 3884889, 3918544, 819404 | `invoke`/`send`/`sendSync` |
| `contextBridge` | **0** | — | absent |
| `ipcMain` | **0** | — | absent (côté main uniquement) |
| `require(` | 6 | 795100, 1366824, 1536032, 1537428, 3351379, 3359451 | specifiers `original-fs`, `crypto`, `path` |
| `process.` | 10 | 792413, 819119, 1254980, 2303366, 3720371 | `platform`, `env.LOG`, `versions.electron`, `arch` |
| `window.electron.*` | (lectures) | 1536367, 2802865, 2616211, 3931987 | `.ipcRenderer`, `.webFrame`, `.remote` |
| `electron.remote.*` (ou `.remote.*`) | ~30 | 796262, 820089, 1227427, 2826706 | `dialog`, `clipboard`, `Menu`, `screen`, `nativeTheme`, `safeStorage`, `shell`, `systemPreferences`, `app`, `webContents`, `getCurrentWebContents`, `getCurrentWindow`, `session` |
| `webFrame` | 6 | 2616227, 2922236, 3855102, 3918947 | `.getZoomLevel()` / `setFrameZoomLevel` |
| `screen` | 84 | 2826706 | `remote.screen.getAllDisplays` (reste = DOM/UI) |
| `clipboard` | 148 | 819404, 1475308 | `remote.clipboard.readImage/write` |
| `dialog` | 135 | 796262, 3101837 | `remote.dialog.showSaveDialog{…}`, `showOpenDialogSync` |
| `Notification` | 4 | — | API web (pas Electron) |
| `nativeTheme` | 2 | 3900979, 3923501 | `remote.nativeTheme.shouldUseDarkColors`, `removeAllListeners("update")` |
| `app.` | 1086 | 820193 | → `remote.app.getLocale()` (le reste = faux positifs objet `app`) |
| `BrowserWindow` | 0 | — | non référencé directement |
| `session` | 10 | 820089 | `remote.getCurrentWebContents().session` |
| `protocol` | 16 | — | interne (non tranché) |
| `net` | 2 | — | interne |
| `Menu` | 5 | 1227427 | `remote.Menu.buildFromTemplate`, `.popup` |
| `crashReporter`/`globalShortcut`/`Tray` | 0 | — | absents |

**Modules requis via `window.require` / `require`** (preuves `ctx.mjs` + `ipc2.mjs`) :

| Specifier | Occurrences | Usages observés |
|---|---|---|
| `original-fs` (alias webpack de `fs`) | 3 directs + 4 `window.require` | `readFileSync`, `promises.writeFile`, `existsSync` |
| `fs` (via `window.require`) | @565970/566236/566390 | `promises.mkdir`, `existsSync`, read |
| `path` | 2 directs | `basename`, join, résolution de chemins de vault |
| `crypto` | 1 | `scrypt` (dérivation clé) |
| `electron` | 8 | `ipcRenderer`, `remote`, `clipboard`, `webFrame` |
| `@electron/remote` | fallback @2615163 | `if(!i.remote) try{i.remote=n("@electron/remote")}` |

---

## 3. Contrat IPC (canaux réellement utilisés) — c'est le contrat main-process

Extrait par `ipc2.mjs` (chaîne littérale passée à `ipcRenderer.*`), avec occurrence.

**`ipcRenderer.sendSync(...)` — 31 canaux distincts, 54 appels :**

`vault`(3) · `vault-list`(3) · `vault-open`(2) · `frame`(3) · `update`(3) · `disable-update`(3) ·
`insider-build`(4) · `check-update`(2) · `latest-public-version`(1) · `version`(2) · `relaunch`(1) ·
`set-icon`(2) · `get-icon`(1) · `copy-asar`(1) · `help`(1) · `terms`(1) · `policy`(1) ·
`is-quitting`(1) · `is-closing`(1) · `cli`(2) · `set-language`(1) · `documents-dir`(1) ·
`desktop-dir`(1) · `file-url`(1) · `trash`(1) · `starter`(2) · `sandbox`(2) ·
`get-sandbox-vault-path`(1) · `adblock-lists`(2) · `adblock-frequency`(2) · `disable-gpu`(2)

**`ipcRenderer.send(...)` — 8 canaux distincts, 10 appels :**

`create-browser-session`(2) · `render-menu`(2) · `set-menu`(1) · `update-menu-items`(1) ·
`open-url`(1) · `open-file`(1) · `insider-build`(1) · `check-update`(1) ·
(+ `print-to-pdf`, `request-url` vus en contexte @819404/@819119)

**`ipcRenderer.invoke(...)` — 1 canal :** `register-cli`(@3556705)

**`ipcRenderer.on/once/removeListener` : aucun littéral détecté** (les écouteurs passent vraisemblablement par des noms dynamiques — non déterminable statiquement).

> Le shim doit implémenter **40 canaux distincts** (1 invoke + 8 send + 31 sendSync). C'est le contrat de RPC principal avec le « main » Rust.

---

## 4. Workers et assets statiques à servir

**Workers web** (pas `worker_threads`, nœuds `worker`, `preload.mjs`) :
- `new Worker("worker.js",{name:"Metadata Cache Worker"})` — parseur de métadonnées
- `new Worker("/sim.js",{name:"Graph Worker"})` — graphe
- `new Worker(e,t)` / `new Worker(URL.createObjectURL(new Blob([e],…)))` — workers blob dynamiques (3 autres sites)

**Assets statiques extraits de l'ASAR** (38 entrées, `asar.mjs`) devant être servis comme fichiers :
`index.html`(1445 o), `app.js`, `starter.html`, `starter.js`, `quicklook/index.html`, `help.html`,
`i18n.js`, `worker.js`, `sim.js`, `main.js`, `package.json`, et `lib/*` :
`prism.min.js`, `mermaid.min.js`(2,96 Mo), `pdfjs/*`, `codemirror/*`, `mathjax/*`, `reveal/*`,
`defuddle.full.js`, `pixi.min.js`, `turndown.js`, `readability.js`, `scrypt.js`, `moment.min.js`, `i18next.min.js`.

**Storage navigateur** (nœuds `storage`, `idx.mjs`) : clés `localStorage` — `communityPluginSortOrder`,
`mobile-selected-vault`, `site-theme`, `communityThemeSortOrder`, `most-recently-installed-version`,
`spellcheck-languages`, `vim`, `history-show-diff`, `local-themes`, `obsidian` ; `sessionStorage` :
`obsidian-uri` ; `indexedDB.open` (sync mobile). Ce sont des API DOM standard, pas Electron — fournies par le navigateur hôte (Chromium/WebView).

---

## 5. Conclusion de faisabilité

### Fait central
Le renderer n'est **pas** isolé : il appelle en direct `window.require`, `window.process`, `window.electron`
(ipcRenderer + remote + webFrame). Il n'y a ni `contextBridge` ni preload. Le shim doit donc soit
(a) fournir `window.require` + `window.electron` dans la page, soit (b) patcher `app.js`. L'option (a)
est la seule réaliste (aucune modification du bundle Obsidian).

### Milestone 1 (« must have » — lire/écrire une note, lister les fichiers, cache de métadonnées, résolution de liens)
Surface minimale exigée :
1. **`window.require`** avec : `fs`/`original-fs` (`readFileSync`, `promises.writeFile`, `promises.mkdir`, `existsSync`), `path` (`basename`/join), `crypto` (`scrypt`+`Buffer`). → **4 modules Node, ~10 fonctions**.
2. **`window.electron`** minimum : `ipcRenderer.sendSync` pour `vault`/`vault-list`/`vault-open` (+ les 3 canaux retournent le chemin/id du vault), `webFrame.getZoomLevel` (tolérant). → **3 canaux + 1 objet-global**.
3. **`window.process`** : `platform`, `arch`, `versions.electron`. → **3 propriétés**.
4. **Serveur d'assets** : `index.html` + `app.js` + `i18n.js` + `worker.js` + `sim.js` + les `lib/*` chargés au démarrage (au minimum `prism`, `mermaid`, `codemirror`, `pdfjs`, `mathjax`). → **~15 fichiers**.
5. **localStorage** de base (la page en fait un usage constant).

**Estimation M1 : ~1–2 semaines** d'ingénierie ciblée (Rust ↔ JS bridge, serveur d'assets, 3 canaux IPC, 4 modules Node émulés).

### Plus tard (« needed later »)
- Menu (set-menu/update-menu-items/render-menu) + `remote.Menu`.
- Dialogues (`remote.dialog.showSaveDialog/showOpenDialogSync`), `remote.clipboard`, `remote.shell`, `remote.safeStorage`, `remote.systemPreferences`.
- Thème natif (`remote.nativeTheme.shouldUseDarkColors`).
- Écran multi-display (`remote.screen.getAllDisplays`).
- Mise à jour (check-update/latest-public-version/insider-build/relaunch), browser-sessions (create-browser-session), adblock, sandbox, copy-asar.
- Sync/mobile (`indexedDB`, `mobile-selected-vault`).
- Export PDF (print-to-pdf), capture (`remote.getCurrentWebContents().capturePage` + `original-fs.promises.writeFile`).
- Ouverture externe (`open-file`, `open-url`, `shell.showItemInFolder`), corbeille (`trash`).

**Estimation complète : ~2–3 mois** d'ingénierie (fidélité Electron complète).

**Verdict :** un **week-end : non** ; un **mois : M1 seulement** ; un **trimestre : le shim complet crédible**. La faisabilité est **réelle** (toutes les surfaces sont statiquement identifiées, aucune API obscure ou non cataloguée n'est requise pour M1), mais le coût est dominé par la fidélité des modules Electron `remote`/menu/dialogues et par le maintien d'un `require` Node dans la page.

---

## 6. Charge estimée — par catégorie (estimé, non mesuré)

| Catégorie | Nombre de surfaces distinctes | Détail |
|---|---|---|
| Électron `ipcRenderer` | 3 méthodes / **40 canaux** | invoke(1) + send(8) + sendSync(31) |
| Électron `remote`/`@electron/remote` | ~15 membres sur ~13 objets | clipboard, dialog, Menu, nativeTheme, screen, session, shell, app, safeStorage, systemPreferences, webContents, getCurrentWebContents, getCurrentWindow |
| Électron `webFrame` | 2 | getZoomLevel / setFrameZoomLevel |
| Électron global `window.electron` | 1 objet | ipcRenderer + remote + webFrame |
| Node `fs`/`original-fs` | ~5 | readFileSync, promises.writeFile, promises.mkdir, existsSync, readonly |
| Node `path` | ~3 | basename, join, resolve |
| Node `crypto` | 1 | scrypt (+ Buffer) |
| Node `process` | 3 props | platform, arch, env, versions.electron |
| Workers web | 2 nommés + 3 blob | worker.js, sim.js |
| Serveur d'assets | ~38 entrées ASAR | index.html, app.js, lib/* |
| Storage navigateur | ~10 clés localStorage + indexedDB + sessionStorage | fourni par l'hôte |

Total surfacique approximatif : **~40 canaux IPC + ~20 membres Electron + ~13 fonctions Node + ~38 assets**. Coût dominé par Electron remote/menu/dialogues + le bridge du `require` Node.

---

## 7. Limites de l'analyse statique (non déterminable)

- **Comportement dynamique** : `ipcRenderer.on/once`, requires à specifier dynamique, canaux construits par concaténation → non capturés (non déterminable sans exécution).
- **`window.electron` / `electronWindow`** : l'affectation de `window.electron` n'apparaît pas dans `app.js` (0 hit de `window.electron=`). Elle est vraisemblablement injectée par le processus main (via `webContents.executeJavaScript` ou équivalent) → à confirmer par trace runtime (non déterminable ici).
- **Seuls `app.js` + `enhance.js` analysés** (2/38) : les relations via `main.js`, `worker.js`, `sim.js`, `starter.js` restent inconnues. En particulier le comportement du main (`main.js`, 203 Ko) — qui est justement ce que le shim remplace — n'est pas détaillé ici.
- **L'évidence full-ASAR (`rea-evidence-89mo-partiel.json`) est un timeout** (`provider_timeout` sur `mermaid.min.js`) : pas de graphe complet.
- **Les comptages bruts** (`screen`, `app.`, `session`, etc.) incluent des faux positifs (identifiants mangled, objets `app`, DOM). Seuls les offsets cités dans les tableaux sont des usages vérifiés.
- **Aucune exécution** : le poids réel (perf, dépendances runtime) est hors portée.
- **`protocol`/`net`/`url`/`stream`** internes au bundle : non tranchés (probablement du code bundlé, pas des API Electron).

---

## 8. Feuille de route suggérée (pour décision)

1. **Semaine 1** : serveur d'assets ASAR + `window.require` (fs/path/crypto) + `window.process` + canaux `vault`/`vault-list`/`vault-open` + `window.electron.ipcRenderer.sendSync` stub → **objectif : la page `index.html` charge `app.js` sans erreur et ouvre un vault**.
2. **Semaine 2** : `worker.js` (Metadata Cache Worker) + `sim.js` fonctionnels → **cache de métadonnées + résolution de liens observables**.
3. **Mois 2–3** : menu/dialogues/clipboard/thème/update/screen/safeStorage + `@electron/remote` complet.

> Le présent document ne modifie aucun code Rust ; il n'est qu'une spécification de décision. Toutes les mesures ci-dessus sont **statiques** (occurrences textuelles) — les estimations de charge sont **estimées, non mesurées**.
