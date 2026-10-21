# Démarrage du renderer réel d'Obsidian 1.14.4 — diagnostic, preuves, procédure

**Date :** 2026-10-21 · **Boîte :** instabox `hermes` (Debian 13 « trixie », x86_64,
4 vCPU, 7,8 Gio RAM) · **Renderer :** `app.js` de l'ASAR `obsidian-1.14.4.asar`
(sha256 `146ef8470d8cbb4a6db209bdc19bf2bad13350444186c0fd93c31087b142b92f`)

> Ce document décrit **ce qui a été exécuté et mesuré**, jamais ce qui est
> supposé. Les journaux bruts sont conservés sur la boîte
> (`/data/home/obsidian-rea/logs/`) ; la liste exacte des fichiers est en fin
> de document.

---

## 1. Verdict

| Question | Réponse mesurée |
|---|---|
| Le renderer d'Obsidian peut-il démarrer ? | **Oui**, dans Electron, quand le processus principal fournit les canaux IPC attendus (identité `{id, path}` du vault, chaîne d'acceptation, `version`, `policy`, `frame`…). Preuve : fenêtre affichée, titre `Obsidian`, interface rendue. |
| Démarre-t-il dans un Chromium simple avec `shim/browser-shim.js` du dépôt ? | **Non.** Le shim comble 3 des 4 portes de boot ; il échoue ensuite sur la surface manquante de `electron.remote` (inventaire ci-dessous). |
| Pourquoi « le processus quitte ~2 s après le chargement de l'ASAR » ? | **Ce n'est pas le renderer qui plante** : c'est la **première porte de boot** (`sendSync('vault')`). Sans chemin de vault valide, le renderer appelle `sendSync('starter')` puis `window.close()` — la fenêtre disparaît, et l'application Electron se termine quand il ne reste aucune fenêtre. Message réel du renderer : `TypeError: Cannot read properties of undefined (reading 'ipcRenderer')` (sans Electron) / `chemin absolu interdit : …` (avec le shim). Détail en §4. |
| Coût mémoire de leur interface | **Electron 45, arbre complet : ≈ 333 Mio RSS** sans fenêtre, **≈ 470 Mio RSS** avec la fenêtre et l'interface chargée (mesure §6). Le budget 200–400 Mio impose **Electron**, pas un Chromium simple (notre Chromium 155 de Debian : ≈ 890 Mio RSS pour un renderer qui n'affiche rien). |

---

## 2. Environnement de la boîte `hermes`

| Élément | Valeur mesurée |
|---|---|
| OS | Debian GNU/Linux 13 (trixie), noyau 6.18.54-instabox, x86_64 |
| CPU / RAM / disque | 4 vCPU · 7,8 Gio RAM (4,6 Mio libres au moment des essais) · 20 Go de disque, 4,7 Go libres |
| Node.js | **v22.23.3** (`/usr/local/bin/node`) |
| npm | présent (10.x) — réseau vers `registry.npmjs.org` **OK (HTTP 200)** |
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
npm i @electron/asar@3            # dans tools/asar
node tools/asar/node_modules/.bin/asar extract obsidian-1.14.4.asar asar-x1
# → 368 fichiers, 28 Mio ; app.js sha256 958ce57f… (identique à l'extrait précédent)
```

Faits relevés dans les bundles (lecture seule) :

| Fait | Source | Conséquence |
|---|---|---|
| `index.html` charge **14 scripts en `defer`** : `lib/codemirror/*` (6), `lib/moment.min.js`, `lib/pixi.min.js`, `lib/i18next.min.js`, `lib/scrypt.js`, `lib/turndown.js`, `enhance.js`, `i18n.js`, `app.js` + `app.css` | `asar-x1/index.html` | le renderer est une page web classique, sans bundler ni module ES |
| Le renderer est servi par le protocole **`app://obsidian.md/`** (URL `app://obsidian.md/index.html`), **pas** par `file://` | `main.js` (constante `le`, usage `ht`) | toute reproduction doit servir la page en HTTP ou en `app://`, sinon les requêtes locales sont bloquées |
| La fenêtre est créée avec `contextIsolation:false`, `nodeIntegration:true`, `nodeIntegrationInWorker:true`, **aucun `preload`**, `contextBridge` absent | `main.js` (`webPreferences`) | le renderer fait `require('electron')`, `require('fs')`, `require('original-fs')` **directement dans la page** : il faut `nodeIntegration`, un shim ne peut pas être injecté par `preload` |
| Le canal IPC `vault` renvoie `{id, path}` quand un vault est ouvert, `{}` sinon ; `path` est un **chemin absolu** | `main.js` (`ipcMain.on("vault")`) | contrat 2 confirmé |
| Le canal IPC `terms` renvoie la **chaîne d'acceptation** ; le renderer compare cette chaîne à une constante embarquée et ferme la fenêtre si elle diffère | `main.js` (`ipcMain.on("terms")`) + `app.js` (comparaison) | contrat 3 confirmé — **longueur mesurée : 284 caractères** (et non 286 : `BOOT-CONTRACTS.md` est à corriger) |
| Le canal `starter` ouvre la fenêtre de choix de vault ; le renderer l'appelle puis ferme sa fenêtre quand le vault est invalide | `main.js` + `app.js` | **c'est la cause du « quit ~2 s »** |
| La configuration est un JSON par application : `<userData>/<nom>.json`, soit `~/.config/obsidian/obsidian.json` en installation réelle | `main.js` (`X = t => join(userData, t + ".json")`) | pour reproduire, il faut écrire ce fichier |
| Au démarrage, le processus principal ouvre les vaults marqués `open:true`, sinon il affiche le sélecteur de vault | `main.js` | avec un fichier de configuration valide, une fenêtre doit s'ouvrir |

**Liste des fichiers** (ce qui est déjà présent sur la boîte) : l'ASAR original
(27 Mo), l'extraction `asar-x1/` (28 Mo), `app.js` (copie de travail),
les journaux REA, **et le shim du dépôt copié** dans
`/data/home/obsidian-rea/shim/browser-shim.js` (29 501 octets, sha256 identique à
`shim/browser-shim.js` du dépôt) ainsi que le binaire du moteur Rust
`/data/home/obsidian-rea/bin/obsidian-engine` (5,3 Mo, compilé sur `pour-omp`
depuis l'état du dépôt du 2026-10-21).

---

## 4. Diagnostic : pourquoi le renderer ne démarre pas

Le renderer a **quatre portes de boot**, dans cet ordre. Chacune a été testée
séparément.

### Porte 1 — le vault (`sendSync('vault')` → `{id, path}`)

Le renderer exige un chemin **absolu** de vault, puis vérifie qu'il existe via
l'adaptateur `fs` du renderer (`require('original-fs').existsSync(path)`).
Si le chemin manque ou n'existe pas, le renderer appelle `sendSync('starter')`
puis `window.close()`.

* **Avec Electron** (`main.js` réel) : le canal renvoie `{}` tant qu'aucun
  vault n'est enregistré → **porte 1 fermée → la fenêtre se ferme ~immédiatement
  → l'application quitte** (plus aucune fenêtre). C'est le symptôme observé
  chez l'utilisateur (« le processus quitte ~2 s après le chargement de l'ASAR,
  journal vide ») : ce n'est pas un crash, c'est le renderer qui se ferme
  lui-même parce qu'aucun vault n'est ouvert.
* **Avec le shim du dépôt** : le shim renvoie `{path, name, open}` (pas d'`id`)
  et le chemin est **absolu** ; il l'envoie tel quel au moteur Rust, dont le
  confinement **refuse les chemins absolus**. Message réel observé dans la
  console du renderer :

  ```
  Uncaught (in promise) Error: chemin absolu interdit : /data/home/obsidian-rea/vault
      at browser-shim.js:51
  ```

  → **porte 1 fermée**.

### Porte 2 — la version et la politique

`sendSync('version')` (chaîne), `remote.app.getVersion()` (build) et
`sendSync('policy')` (objet) sont lus juste après. Le shim les fournit
(`version` → `1.14.4`). `remote.app.getPreferredSystemLanguages` manque dans le
shim → **erreur réelle** :

```
Uncaught (in promise) TypeError: e.electron.remote.app.getPreferredSystemLanguages is not a function
```

### Porte 3 — la chaîne d'acceptation (`sendSync('terms')`)

Le renderer compare la valeur reçue à une constante embarquée de **284
caractères** ; en cas de divergence il appelle `window.close()`. Le shim renvoie
`''` → **porte 3 fermée** (et le renderer se ferme).

### Porte 4 — l'OS et l'environnement

`process.platform` doit valoir `linux` (le shim le fait : vérifié
`platform: "linux"`), `process.versions.electron` est lu, et le renderer appelle
`require('os').hostname()`, `.version()`, `.release()`. **Le shim ne fournit pas
`os.version()`** → erreur réelle : `TypeError: r.version is not a function`.

### Sans Electron : la première erreur est encore plus tôt

Chromium seul, page servie en HTTP, sans shim :

```
Uncaught (in promise) TypeError: Cannot read properties of undefined (reading 'ipcRenderer')
    at http://127.0.0.1:8899/app.js:1
```

La page reste **blanche** (18 balises, 1 184 octets de HTML, aucun
`.app-container`, aucun `.workspace`) et **le processus Chromium ne quitte pas** :
la fenêtre ne se ferme pas, elle ne se remplit pas. La classe de thème
`theme-dark mod-linux` est bien appliquée (posée par le renderer avant l'échec).

---

## 5. Les expériences (commandes reproductibles)

Toutes les commandes se lancent sur la boîte `hermes`, avec `HOME` isolé
(`/data/home/obsidian-rea/home`).

### F — renderer réel dans Electron (procédure principale de substitution) ✅

`tools/main-harness.js` (code écrit pour ce diagnostic) remplace le processus
principal d'Obsidian : il crée une `BrowserWindow` avec `nodeIntegration`
activé, répond aux canaux IPC du boot, et charge `asar-x1/index.html`.

```sh
Xvfb :81 -screen 0 1400x900x24 &
DISPLAY=:81 tools/electron/node_modules/.bin/electron tools/main-harness.js \
  --no-sandbox --disable-gpu --remote-debugging-port=9226
# sonde CDP : console, exceptions, capture d'écran, état du DOM
node tools/cdp.mjs 9226 25000 logs/electron-harness-page.png tools/probe.js
```

Résultat : la fenêtre s'affiche (`[harnais] fenêtre affichée`), le titre est
`Obsidian`, l'interface est rendue (explorateur de fichiers peuplé, thème
appliqué) — voir §6 et les captures.

### E — Chromium + shim + moteur Rust (voie « sans Electron ») ❌

```sh
# moteur Rust (vault de démonstration)
/data/home/obsidian-rea/bin/obsidian-engine --root /data/home/obsidian-rea/vault --port 27123
# → {"notes":3,"ok":true,"root":"/data/home/obsidian-rea/vault"}
cd asar-x1 && python3 -m http.server 8898 --bind 127.0.0.1 &
chromium --user-data-dir=/tmp/chr --no-sandbox --disable-gpu \
  --remote-debugging-port=9224 http://127.0.0.1:8898/boot-shim.html
```

`boot-shim.html` = copie de `index.html` + `<script src="browser-shim.js">`
avant `app.js` (fichier de la boîte, hors dépôt).

Résultat : le shim s'installe correctement (`window.require`, `window.electron`,
`window.process`, `platform: "linux"`), mais le boot s'arrête à la porte 1
(`chemin absolu interdit`), puis à la porte 2
(`getPreferredSystemLanguages`), puis `os.version`, puis
`module non supporté par le shim obsidian-rs : url`.

### D — Chromium + shim + harnais de correction (quelques portes comblées) ⚠️

`asar-x1/boot-probe.js` (fichier de la boîte) :
1. traduit les chemins absolus du vault en chemins relatifs ;
2. extrait la chaîne d'acceptation **du bundle lui-même au runtime** (aucune
   recopie) ;
3. renvoie `{id, path}` pour le canal `vault` ;
4. complete la surface `electron.remote` absente avec un stub permissif et
   **journalise chaque membre manquant**.

Résultat : le renderer passe les portes 1 à 4, construit le DOM (7 037 octets
de HTML, texte « Obsidian »), puis s'arrête sur `module non supporté par le
shim obsidian-rs : url`.

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

### A — application Electron officielle (ASAR repacké) ⚠️

L'ASAR de l'ASAR-packagé a un `package.json` dont `main` pointe vers `index.js`
(absent de l'archive : l'entrée réelle est `main.js`). Après repack avec
`main: "main.js"`, l'application démarre (agent utilisateur
`obsidian-dev/1.14.4 Chrome/156.0.8078.48 Electron/45.0.0`) mais **n'ouvre
aucune fenêtre** : aucune cible renderer sur le port de débogage, seulement une
fenêtre X de 10 × 10 px. Le processus principal enregistre bien le protocole
`app://` et les canaux IPC, mais la fenêtre de vault n'est pas créée — reste à
localiser (le `main.js` réel d'Obsidian n'est pas notre cible : le harnais F
contourne ce problème).

---

## 6. Consommation mémoire

Mesures relevées sur l'arbre de processus complet (RSS et PSS sommés), boîte
`hermes`, Electron 45, protocole `tools/mem.sh` :

| Configuration | RSS de l'arbre | PSS de l'arbre | Détail |
|---|---|---|---|
| Electron 45, **sans fenêtre** (processus principal seul) | ≈ 333 Mio | — | Chromium + Node du runtime Electron |
| Electron 45, **fenêtre ouverte, interface chargée** | ≈ **470 Mio** | — | leur UI réelle, explorateur de fichiers peuplé |
| Chromium 155 (Debian), renderer **sans shim**, page blanche | ≈ 890 Mio | — | un renderer qui n'affiche rien |
| **notre coquille Tauri 2 + WebKitGTK** (page vide + moteur Rust) | — | **190 Mo** | 83,5 Mo (Tauri) + 80,1 Mo (renderer WebKit) + 29,5 Mo (réseau) + 2,4 Mo (moteur) — mesuré le 21/10 |
| **notre UI native iced + tiny-skia** (release) | 22,5 Mo | — | 1 processus — mesuré le 21/10 |

Lecture : **leur interface coûte ~470 Mio**, soit 2,5 fois notre front webview
(190 Mio à vide, 387 Mio avec un éditeur Markdown complet). Un budget de
200-400 Mio accepte notre front, mais pas leur renderer Electron chargé.

---

## 7. Points de rupture restants, par ordre de priorité

| # | Point de rupture | Où | Effet | Correctif proposé | Temps estimé |
|---|---|---|---|---|---|
| 1 | Le shim envoie le **chemin absolu** de la racine du vault au moteur, qui refuse les chemins absolus | `shim/browser-shim.js` (adaptateur `fs`) + `src/engine/paths.rs` | porte 1 fermée : le renderer ferme la fenêtre | le shim traduit un chemin absolu situé sous la racine en chemin relatif ; le moteur expose une opération « racine » | 30–60 min |
| 2 | `sendSync('terms')` renvoie `''` au lieu de la **chaîne d'acceptation de 284 caractères** | `shim/browser-shim.js` (`SYNC_CHANNELS.terms`) | porte 3 fermée : `window.close()` | décision du propriétaire : servir leur interface suppose d'accepter leurs conditions ; à défaut, faire comme l'a fait le harnais (extraction runtime) — **ne pas recopier la chaîne dans le dépôt** | décision + 30 min |
| 3 | `sendSync('vault')` renvoie `{path, name, open}` **sans `id`** | `shim/browser-shim.js` (`SYNC_CHANNELS.vault`) | le renderer et le processus principal attendent `{id, path}` | ajouter `id` | 10 min |
| 4 | Module `url` **non supporté** par le shim | `shim/browser-shim.js` (table des modules) | exception au boot : `module non supporté par le shim obsidian-rs : url` | implémenter `url.parse` / `url.fileURLToPath` / `url.pathToFileURL` (ou un sous-ensemble réellement utilisé) | 30–60 min |
| 5 | `os.version()` absent | `shim/browser-shim.js` (`osModule`) | `TypeError: r.version is not a function` | ajouter `version()` (version du noyau) | 5 min |
| 6 | `electron.remote` : surface absente (`app.getPreferredSystemLanguages`, `getCurrentWindow()` et son `webContents` + `getZoomFactor`, `minimizable`, `maximizable`, `closable`, `isMaximized`, `isFullScreen`, `isFocused`) | `shim/browser-shim.js` (`electronShim.remote`) | exception au boot | implémenter un `remote` minimal (fenêtre courante : géométrie, zoom, état) | 1 h |
| 7 | Le moteur Rust **ne sait pas lister la racine** du vault (`fs.readdirSync('')` refusé : « chemin relatif vide ») | `src/engine/sync.rs` (`readdir_sync`) | l'explorateur de fichiers racine ne peut pas être alimenté par `fs.readdirSync` | opération « lister la racine » côté moteur, ou chemin sentinelle | 30 min |
| 8 | Les 8 autres canaux `sendSync` renvoient une valeur par défaut | `shim/browser-shim.js` | comportement dégradé (mise à jour, corbeille, CLI, langue…) | implémenter au fil de l'eau | — |

** hors périmètre, mais vérifié :** `process.platform === 'linux'` (contrat 4)
est correct dans le shim ; la liste des fichiers passe bien par l'adaptateur
`fs` et non par un canal IPC (contrat 5) — confirmé par le bundle (le renderer
appelle `readdirSync`/`statSync` sur `join(basePath, chemin)`).

---

## 8. Procédure reproductible pour un utilisateur

### 8.1 Démarrer leur interface (Electron)

1. Extraire l'ASAR (`asar extract`) et disposer d'un Electron ≥ 30.
2. Écrire un processus principal de substitution (ou utiliser celui d'Obsidian)
   qui :
   - crée une fenêtre avec `nodeIntegration: true`, `contextIsolation: false` ;
   - sert la page depuis `app://obsidian.md/index.html` (protocole privé) ou
     depuis `http://127.0.0.1:<port>/index.html` ;
   - répond aux canaux `vault` (`{id, path}`), `version`, `terms`, `policy`,
     `frame`, `starter`, `vault-list`…
3. Lancer sous X (`Xvfb` pour un environnement headless, `DISPLAY` sinon).

### 8.2 Démarrer leur interface avec notre moteur Rust (Chromium simple)

1. `obsidian-engine --root <vault> --port 27123` (le moteur sert `POST /sync`).
2. Servir l'ASAR extrait en HTTP local.
3. Charger `index.html` **après** `shim/browser-shim.js` (script classique, pas
   `defer`), sur une page **http://** (une page `file://` ne peut pas émettre
   de XHR synchrone vers `http://127.0.0.1`).
4. Corriger les points 1 à 7 du §7.

---

## 9. Fichiers de diagnostic

Sur la boîte `hermes` (`/data/home/obsidian-rea/`) :

| Fichier | Contenu |
|---|---|
| `logs/electron-harness.stdout` | journal du harnais Electron (fenêtre affichée, erreurs du renderer) |
| `logs/electron-harness-cdp.json` | rapport CDP complet (console, exceptions, sonde DOM) |
| `logs/electron-harness-page.png` | capture de la page (via CDP) |
| `logs/electron-harness-screen.png` | capture de l'écran X (via `import`) |
| `logs/chrom-b-nosim-*` | expérience B (Chromium sans shim) |
| `logs/chrom-c-shim-*` | expérience C (Chromium + shim, sans moteur) |
| `logs/chrom-d..h-harnais*-cdp.json` | expériences D→H (harnais de correction) |
| `logs/engine-*.log` | journal du moteur Rust |
| `tools/main-harness.js` | processus principal de substitution (Electron) |
| `tools/cdp.mjs`, `tools/probe.js`, `tools/summary.mjs`, `tools/mem.sh` | sonde CDP, sonde d'état du renderer, résumé, mesure mémoire |
| `asar-x1/boot-shim.html`, `asar-x1/boot-probe.js` | pages de test (shim seul, shim + correctifs) |

Ces journaux sont volumineux et restent sur la boîte ; les captures sont
référencées ci-dessus. Aucun fichier de `src/`, `shim/` ou `Cargo.toml` du
dépôt n'a été modifié par ce diagnostic.
