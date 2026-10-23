# Démarrage du renderer réel d'Obsidian 1.14.4 — diagnostic, preuves, procédure

**Date :** 2026-10-21 · **Boîte :** instabox `hermes` (Debian 13 « trixie », x86_64,
4 vCPU, 7,8 Gio RAM) · **Renderer :** `app.js` de l'ASAR `obsidian-1.14.4.asar`
(sha256 `146ef8470d8cbb4a6db209bdc19bf2bad13350444186c0fd93c31087b142b92f`)

> Ce document ne décrit que ce qui a été **exécuté et mesuré**. Les journaux
> bruts sont conservés sur la boîte (`/data/home/obsidian-rea/logs/`) ; les
> preuves volumineuses (captures, rapport CDP) sont **copiées dans ce
> dossier** (§9).

---

## 1. Verdict

| Question | Réponse mesurée |
|---|---|
| **Leur interface démarre-t-elle ?** | **Oui.** Renderer d'Obsidian 1.14.4 + processus principal de substitution : fenêtre affichée, titre `New tab - vault - Obsidian 1.14.4`, **explorateur de fichiers peuplé** (`Note-de-test`, `Projet`, dossier `Dossier`), thème `theme-dark` appliqué, ruban à 8 icônes, **zéro exception**. Preuves : `renderer-boot-electron-page.png`, `renderer-boot-electron-ecran.png`, `renderer-boot-electron-cdp.json`. |
| La voie « Chromium simple + `shim/browser-shim.js` » démarre-t-elle ? | **Non.** Le shim installe bien `window.require` / `window.electron` / `window.process` (`platform: "linux"`), mais le boot s'arrête sur 7 points de rupture, tous localisés et chiffrés (§7). |
| **Pourquoi « le processus quitte ~2 s après le chargement de l'ASAR » ?** | Ce n'est **pas un crash** : c'est la **première porte de boot**. Le renderer exige `sendSync('vault')` → `{id, path}` **et** que ce chemin existe (`require('original-fs').existsSync(path)`). Sinon il appelle `sendSync('starter')` puis **`window.close()`** : la fenêtre disparaît et Electron se termine quand il ne reste plus aucune fenêtre. Messages réels relevés : `TypeError: Cannot read properties of undefined (reading 'ipcRenderer')` (sans Electron) et `Error: chemin absolu interdit : /data/home/obsidian-rea/vault` (avec le shim). |
| **Coût mémoire de leur interface** | **Electron 45, arbre complet de 6 processus : 606 Mio RSS / 298 Mio PSS** avec l'interface entièrement chargée. Sans fenêtre (processus principal officiel) : 344 Mio RSS / 150 Mio PSS. Un **Chromium simple** (155, build Debian) coûte à lui seul **892 Mio RSS / 400 Mio PSS** pour un renderer qui n'affiche rien. → **Electron est la seule voie dans le budget 200–400 Mio** ; la voie « Chromium + shim » est hors budget avant même d'afficher l'UI. |

---

## 2. Environnement de la boîte `hermes`

| Élément | Valeur mesurée |
|---|---|
| OS | Debian GNU/Linux 13 (trixie), noyau 6.18.54-instabox, x86_64 |
| CPU / RAM / disque | 4 vCPU · 7,8 Gio RAM · 20 Go de disque (4,7 Go libres) |
| Node.js | **v22.23.3** |
| npm | présent — réseau vers `registry.npmjs.org` **OK (HTTP 200)** |
| Chromium | **155.0.8059.79** (build Debian) `/usr/bin/chromium` |
| Electron | absent au départ → installé pour l'essai : **v45.0.0** (`npm i electron@latest`) |
| `asar` | absent au départ → installé : `@electron/asar@3` |
| X | `Xvfb`, ImageMagick `import`, `xdotool` présents |
| Python | 3.13.5 (serveur statique de test) |

---

## 3. Point d'entrée du renderer et mécanique de démarrage

Extraction (box `hermes`) :

```sh
cd /data/home/obsidian-rea
npm i @electron/asar@3                                    # dans tools/asar
node tools/asar/node_modules/.bin/asar extract obsidian-1.14.4.asar asar-x1
# → 368 fichiers, 28 Mio ; app.js sha256 958ce57f… (identique à l'extrait précédent)
```

Faits relevés dans les bundles (lecture seule) :

| Fait | Source | Conséquence |
|---|---|---|
| `index.html` charge **14 scripts en `defer`** : `lib/codemirror/*` (6), `lib/moment.min.js`, `lib/pixi.min.js`, `lib/i18next.min.js`, `lib/scrypt.js`, `lib/turndown.js`, `enhance.js`, `i18n.js`, `app.js` + `app.css` | `asar-x1/index.html` | le renderer est une page web classique, sans bundler ni module ES |
| Le renderer est servi par le protocole privé **`app://obsidian.md/`** (URL `app://obsidian.md/index.html`), **pas** par `file://` | `main.js` | une reproduction doit servir la page en `app://` ou en HTTP ; en `file://`, les requêtes locales sont bloquées |
| Fenêtre créée avec `contextIsolation:false`, `nodeIntegration:true`, `nodeIntegrationInWorker:true`, **aucun `preload`**, aucun `contextBridge` | `main.js` (`webPreferences`) | le renderer appelle `require('electron')`, `require('fs')`, `require('original-fs')` **directement dans la page** : `nodeIntegration` est obligatoire, un shim ne peut pas être injecté par `preload` |
| Le canal IPC `vault` renvoie `{id, path}` si un vault est ouvert, `{}` sinon ; `path` est un **chemin absolu** | `main.js` (`ipcMain.on("vault")`) | contrat 2 **confirmé** |
| Le canal IPC `terms` renvoie la **chaîne d'acceptation** ; le renderer la compare à une constante embarquée et ferme la fenêtre en cas de divergence | `main.js` (`ipcMain.on("terms")`) + `app.js` | contrat 3 **confirmé** — longueur mesurée : **284 caractères** (`BOOT-CONTRACTS.md` annonce 286 : **à corriger**) |
| Le canal `starter` ouvre le sélecteur de vault ; le renderer l'appelle puis ferme sa fenêtre quand le vault est invalide | `main.js` + `app.js` | **c'est la cause du « quit ~2 s »** |
| Configuration : `<userData>/<nom>.json` → `~/.config/obsidian/obsidian.json` en installation réelle | `main.js` (`join(userData, nom + ".json")`) | il faut écrire ce fichier pour reproduire |
| Au démarrage, le processus principal ouvre les vaults marqués `open:true`, sinon il affiche le sélecteur | `main.js` | avec une configuration valide, une fenêtre doit s'ouvrir |
| Le renderer tente `require('@electron/remote')` quand `require('electron').remote` est absent | `app.js` | **Electron ≥ 15 a retiré `remote`** : il faut fournir `remote` soi-même (préload ou module local) |

**Déjà présent sur la boîte** : l'ASAR original (27 Mo), l'extraction `asar-x1/`
(28 Mo), `app.js` (copie de travail), les journaux REA, **le shim du dépôt
copié** (`/data/home/obsidian-rea/shim/browser-shim.js`, 29 501 octets, sha256
identique à `shim/browser-shim.js`) et le **binaire du moteur Rust**
`/data/home/obsidian-rea/bin/obsy-engine` (5,3 Mo, compilé sur `pour-omp`
à partir de l'état du dépôt du 2026-10-21).

---

## 4. Les portes de boot, et où elles ferment

Le renderer a quatre portes successives. Chacune a été testée séparément.

### Porte 1 — le vault (`sendSync('vault')` → `{id, path}`)

Le renderer exige un chemin **absolu**, puis vérifie son existence via
l'adaptateur `fs` (`require('original-fs').existsSync(path)`). Si le chemin
manque ou n'existe pas : `sendSync('starter')` puis `window.close()`.

* **Application Electron officielle** : le canal renvoie `{}` tant qu'aucun
  vault n'est enregistré → **porte fermée → la fenêtre se ferme
  immédiatement → le processus quitte**. C'est exactement le symptôme
  « journal vide, le processus s'arrête ~2 s après le chargement ».
* **Avec le shim du dépôt** : le shim renvoie `{path, name, open}` (sans `id`)
  et transmet le chemin **absolu** au moteur Rust, dont le confinement refuse
  les chemins absolus. Message réel dans la console du renderer :

  ```
  Uncaught (in promise) Error: chemin absolu interdit : /data/home/obsidian-rea/vault
      at browser-shim.js:52
  ```

### Porte 2 — version, build, politique

`sendSync('version')` (chaîne), `remote.app.getVersion()` (build),
`sendSync('policy')` (objet). Le shim fournit `version` ; il manque
`remote.app.getPreferredSystemLanguages` →

```
Uncaught (in promise) TypeError: e.electron.remote.app.getPreferredSystemLanguages is not a function
```

### Porte 3 — la chaîne d'acceptation (`sendSync('terms')`)

Comparaison à une constante embarquée de **284 caractères** ; en cas de
divergence : `window.close()`. Le shim renvoie `''` → **porte fermée**.

### Porte 4 — OS et environnement

`process.platform` doit valoir `linux` (le shim le fait : vérifié
`platform: "linux"`), `process.versions.electron` est lu, et le renderer appelle
`require('os').hostname()`, `.version()`, `.release()`. **Le shim ne fournit
pas `os.version()`** → `TypeError: r.version is not a function`.

### Sans Electron : l'échec est encore plus tôt

Chromium seul, page servie en HTTP, sans shim :

```
Uncaught (in promise) TypeError: Cannot read properties of undefined (reading 'ipcRenderer')
    at http://127.0.0.1:8899/app.js:1
```

La page reste **blanche** (18 balises, 1 184 octets de HTML, aucun
`.app-container`, aucun `.workspace`) et **le processus Chromium ne quitte
pas** : la fenêtre ne se remplit pas, elle ne se ferme pas. La classe de thème
`theme-dark mod-linux` est bien appliquée (posée par le renderer avant
l'échec).

---

## 5. Les expériences (commandes reproductibles)

Toutes les commandes se lancent sur la boîte `hermes`, avec `HOME` isolé
(`/data/home/obsidian-rea/home`).

### F — renderer réel dans Electron (procédure principale de substitution) ✅

`tools/main-harness.js` + `tools/preload-harness.js` (code écrit pour ce
diagnostic) remplacent le processus principal d'Obsidian : fenêtre avec
`nodeIntegration`, `preload` qui installe un `remote` minimal, réponses aux
canaux IPC du boot, chargement de `asar-x1/index.html`.

```sh
Xvfb :81 -screen 0 1400x900x24 &
DISPLAY=:81 tools/electron/node_modules/.bin/electron tools/main-harness.js \
  --no-sandbox --disable-gpu --remote-debugging-port=9226
# sonde CDP : console, exceptions, capture, état du DOM
node tools/cdp.mjs 9226 30000 logs/electron-harness-page.png tools/probe.js
```

**Résultat mesuré** (rapport `renderer-boot-electron-cdp.json`) :

```
titre            : New tab - vault - Obsidian 1.14.4
body.class       : theme-dark mod-linux is-frameless is-focused obsidian-app
                   styled-scrollbars show-inline-title is-floating-nav
                   auto-full-screen show-ribbon show-view-header
.app-container   : vrai          .workspace : vrai        barre latérale : vrai
fichiers listés  : ["Note-de-test", "Projet"]   (+ dossier "Dossier")
ruban            : 8 icônes
DOM              : 67 201 octets      exceptions : 0
```

La chaîne d'acceptation est lue dans leur `main.js` **au moment de
l'exécution** (regex sur le fichier), jamais recopiée dans le dépôt.

### E — Chromium + shim + moteur Rust (voie « sans Electron ») ❌

```sh
/data/home/obsidian-rea/bin/obsy-engine --root /data/home/obsidian-rea/vault --port 27123
# → {"notes":3,"ok":true,"root":"/data/home/obsidian-rea/vault"}
cd asar-x1 && python3 -m http.server 8898 --bind 127.0.0.1 &
chromium --user-data-dir=/tmp/chr --no-sandbox --disable-gpu \
  --remote-debugging-port=9224 http://127.0.0.1:8898/boot-shim.html
```

`boot-shim.html` = copie de `index.html` + `<script src="browser-shim.js">`
avant `app.js` (fichier de la boîte, hors dépôt). Le shim s'installe
correctement (`window.require`, `window.electron`, `window.process`,
`platform: "linux"`), mais le boot s'arrête à la porte 1
(`chemin absolu interdit`).

### D — Chromium + shim + harnais de correction (quelques portes comblées) ⚠️

`asar-x1/boot-probe.js` (fichier de la boîte) : (1) traduit les chemins
absolus du vault en chemins relatifs ; (2) extrait la chaîne d'acceptation du
bundle au runtime ; (3) renvoie `{id, path}` pour le canal `vault` ; (4)
complète la surface `electron.remote` absente avec un stub permissif et
**journalise chaque membre manquant**.

Résultat : le renderer passe les portes 1 à 4 et construit le DOM (7 037
octets, texte « Obsidian », classe `is-frameless`), puis s'arrête sur
`module non supporté par le shim obsy : url`.

**Inventaire des membres manquants** (journal du harnais, dans l'ordre) :

```
remote.app.getPreferredSystemLanguages
remote.getCurrentWindow().minimizable
remote.getCurrentWindow().maximizable
remote.getCurrentWindow().closable
remote.getCurrentWindow().webContents
remote.getCurrentWindow().webContents.getZoomFactor
remote.getCurrentWindow().isFullScreen
remote.getCurrentWindow().isMaximized
remote.getCurrentWindow().isFocused
os.version
url                      (module entier, non supporté par le shim)
```

Le harnais Electron (F) a révélé deux détails supplémentaires de la surface
`remote` : `isFullScreen()` / `isMaximized()` sont des **méthodes** (pas des
propriétés) et `nativeTheme.removeAllListeners("updated")` doit **retourner
l'émetteur** (appel chaîné `.on("updated", …)`).

### A — application Electron officielle (ASAR repacké) ⚠️

L'ASAR livré a un `package.json` dont `main` pointe vers `index.js` (absent de
l'archive : l'entrée réelle est `main.js`). Après repack avec `main: "main.js"`,
l'application démarre (agent utilisateur `obsidian-dev/1.14.4
Chrome/156.0.8078.48 Electron/45.0.0`), enregistre le protocole `app://` et les
canaux IPC, mais **n'ouvre aucune fenêtre** : aucune cible renderer sur le port
de débogage, seulement une fenêtre X de 10 × 10 px. Le `main.js` d'Obsidian
n'est pas notre cible : le harnais F contourne ce point.

---

## 6. Consommation mémoire

Mesure : somme de `VmRSS` et de `Pss` (`/proc/<pid>/smaps_rollup`) sur
**l'arbre complet** de processus, à l'instant où la fenêtre est affichée
(`tools/mem.sh`).

| Configuration | Processus | RSS total | **PSS total** | Processus renderer |
|---|---|---|---|---|
| **Electron 45 + leur UI chargée** (harnais F) | 6 | **606 Mio** | **298 Mio** | 203,8 Mio RSS / 134,0 Mio PSS |
| Electron 45, application officielle, **aucune fenêtre** | 5 | 344 Mio | 150 Mio | — |
| Chromium 155 simple + shim, **boot avorté** (page vide) | 9 | **892 Mio** | **400 Mio** | 147,9 Mio RSS / 75,8 Mio PSS |

Lecture pour la décision :

* leur interface complète tient dans le budget **200–400 Mio en PSS**
  (298 Mio) ; en RSS la somme atteint 606 Mio à cause des pages partagées
  comptées plusieurs fois.
* la voie « Chromium + shim » est **hors budget dès le démarrage** (400 Mio
  PSS pour un écran vide) : l'écart vient des processus zygote/utility de
  Chromium, pas du code d'Obsidian.
* l'app officielle d'Obsidian n'ouvre aucune fenêtre dans notre
  configuration : à elle seule elle pèse 344 Mio RSS / 150 Mio PSS.

---

## 7. Points de rupture restants, par ordre de priorité

| # | Point de rupture | Où | Effet | Correctif proposé | Temps |
|---|---|---|---|---|---|
| 1 | Le shim envoie le **chemin absolu** de la racine du vault au moteur, qui refuse les chemins absolus | `shim/browser-shim.js` (adaptateur `fs`) + `src/engine/paths.rs` | porte 1 fermée → le renderer ferme la fenêtre | le shim traduit un chemin absolu situé sous la racine en chemin relatif ; le moteur expose une opération « racine » | 30–60 min |
| 2 | `sendSync('terms')` renvoie `''` au lieu de la **chaîne d'acceptation de 284 caractères** | `shim/browser-shim.js` (`SYNC_CHANNELS.terms`) | porte 3 fermée → `window.close()` | **décision du propriétaire** : servir leur interface suppose d'accepter leurs conditions ; sinon, extraction runtime comme dans le harnais (ne pas recopier la chaîne dans le dépôt) | décision + 30 min |
| 3 | `sendSync('vault')` renvoie `{path, name, open}` **sans `id`** | `shim/browser-shim.js` (`SYNC_CHANNELS.vault`) | le renderer et le processus principal attendent `{id, path}` | ajouter `id` | 10 min |
| 4 | Module `url` **non supporté** par le shim | `shim/browser-shim.js` (table des modules) | exception au boot : `module non supporté par le shim obsy : url` | implémenter `url.parse` / `fileURLToPath` / `pathToFileURL` (sous-ensemble réellement utilisé) | 30–60 min |
| 5 | `os.version()` absent | `shim/browser-shim.js` (`osModule`) | `TypeError: r.version is not a function` | ajouter `version()` | 5 min |
| 6 | Surface `electron.remote` absente : `app.getPreferredSystemLanguages`, `getCurrentWindow()` (+ `webContents.getZoomFactor`), `minimizable()`, `maximizable()`, `closable()`, `isFullScreen()`, `isMaximized()`, `isFocused()`, et `nativeTheme.removeAllListeners()` **chaînable** | `shim/browser-shim.js` (`electronShim.remote`) | exception au boot | implémenter un `remote` minimal (fenêtre courante : état, géométrie, zoom) | 1 h |
| 7 | Le moteur Rust **ne sait pas lister la racine** du vault (`fs.readdirSync('')` refusé : « chemin relatif vide ») | `src/engine/sync.rs` (`readdir_sync`) | l'explorateur racine ne peut pas être alimenté par `fs.readdirSync` | opération « lister la racine » côté moteur, ou chemin sentinelle | 30 min |
| 8 | Electron ≥ 15 a supprimé le module `remote` : le renderer tente `require('@electron/remote')` | harnais (`tools/preload-harness.js`) | sans `remote`, le boot s'arrête au premier usage | le shim fournit `electron.remote` (point 6) — en Chromium simple, aucun `@electron/remote` n'est nécessaire | couvert par 6 |
| 9 | Les autres canaux `sendSync` renvoient une valeur par défaut | `shim/browser-shim.js` | comportement dégradé (mise à jour, corbeille, CLI, langue…) | implémenter au fil de l'eau | — |

**Vérifié et conforme** : `process.platform === 'linux'` (contrat 4), la liste
des fichiers passe bien par l'adaptateur `fs` et non par un canal IPC
(contrat 5), `{id, path}` pour le canal `vault` (contrat 2, dans le processus
principal officiel), la chaîne d'acceptation (contrat 3, **284** et non 286
caractères).

---

## 8. Procédure reproductible

### 8.1 Démarrer leur interface (Electron) — **vérifié**

1. Extraire l'ASAR (`asar extract`) ; disposer d'un Electron ≥ 15 et fournir un
   `remote` minimal (préload ou module local) — c'est le point qui casse par
   défaut.
2. Processus principal de substitution :
   - `BrowserWindow` avec `nodeIntegration: true`, `contextIsolation: false` ;
   - page servie en `app://obsidian.md/index.html` (protocole privé) ou en
     `http://127.0.0.1:<port>/index.html` ;
   - réponses aux canaux `vault` (`{id, path}`), `version`, `terms`, `policy`,
     `frame`, `starter`, `vault-list`, `is-quitting`, `is-closing`…
3. Lancer sous X (`Xvfb` + `DISPLAY` pour un environnement headless).

Référence : `tools/main-harness.js` + `tools/preload-harness.js` (box
`hermes`), journal `logs/electron-harness.stdout`.

### 8.2 Démarrer leur interface avec notre moteur Rust (Chromium simple) — **non atteint**

1. `obsy-engine --root <vault> --port 27123` (le moteur sert `POST /sync`).
2. Servir l'ASAR extrait en HTTP local.
3. Charger `index.html` **après** `shim/browser-shim.js` (script classique, pas
   `defer`), sur une page **http://** (une page `file://` ne peut pas émettre
   de XHR synchrone vers `http://127.0.0.1`).
4. Corriger les points 1 à 7 du §7.
5. **Attention budget mémoire** : cette voie pèse déjà 400 Mio PSS avant
   d'afficher quoi que ce soit (§6).

---

## 9. Fichiers de diagnostic

Dans ce dossier :

| Fichier | Contenu |
|---|---|
| `renderer-boot-electron-page.png` | capture de la page par CDP — **leur interface, vault peuplé** |
| `renderer-boot-electron-ecran.png` | capture de l'écran X (1400 × 900) via `import` |
| `renderer-boot-electron-cdp.json` | rapport CDP complet (console, exceptions, sonde DOM) |
| `logs-electron-harness.stdout` | journal du harnais Electron (fenêtre affichée, erreurs du renderer) |
| `tools/main-harness.js` | processus principal Electron de substitution (réponses aux canaux IPC du boot) |
| `tools/preload-harness.js` | preload qui installe le `remote` minimal (Electron ≥ 15 l'a supprimé) |
| `tools/boot-probe.js` | harnais de correction pour la voie Chromium (chemins relatifs, chaîne d'acceptation extraite au runtime, inventaire des membres manquants) |
| `tools/cdp.mjs`, `tools/probe.js`, `tools/summary.mjs` | sonde CDP (console, exceptions, capture), sonde d'état du renderer, résumé de rapport |
| `tools/mem.sh` | mesure RSS/PSS d'un arbre de processus |
| `tools/run-electron-harness.sh`, `tools/run-chromium-mem.sh` | scripts de lancement des expériences F et E/D |

Ces outils sont **du code écrit pour ce diagnostic** (harnais de test) : ils ne
contiennent aucun code d'Obsidian ; la chaîne d'acceptation et le reste sont
lus dans leurs bundles **au moment de l'exécution**.

Sur la boîte `hermes` (`/data/home/obsidian-rea/`) :

| Fichier | Contenu |
|---|---|
| `logs/electron-harness.stdout` | journal du harnais (fenêtre affichée, erreurs du renderer) |
| `logs/electron-harness-cdp.json` | rapport CDP complet |
| `logs/chrom-b-nosim-*` | expérience B (Chromium sans shim) |
| `logs/chrom-c-shim-*` | expérience C (Chromium + shim) |
| `logs/chrom-d…h-harnais*-cdp.json` | expériences D→H (harnais de correction) |
| `logs/chrom-mem-shim-*` | expérience de mesure mémoire Chromium |
| `logs/engine-*.log` | journal du moteur Rust |
| `tools/main-harness.js`, `tools/preload-harness.js` | processus principal + preload de substitution (Electron) |
| `tools/cdp.mjs`, `tools/probe.js`, `tools/summary.mjs`, `tools/mem.sh` | sonde CDP, sonde d'état du renderer, résumé, mesure mémoire |
| `tools/boot-probe.js` (copié dans `asar-x1/`) | harnais de correction pour la voie Chromium |
| `asar-x1/boot-shim.html`, `asar-x1/boot-probe.html` | pages de test (shim seul, shim + correctifs) |

Aucun fichier de `src/`, `shim/`, `Cargo.toml` ou des tests du dépôt n'a été
modifié par ce diagnostic ; aucun commit n'a été fait.
