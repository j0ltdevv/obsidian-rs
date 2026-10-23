# Shim navigateur — pont synchrone vers le moteur `obsy`

Ce dossier contient le **pont synchrone** entre le renderer d'Obsidian (le
bundle `app.js` de l'ASAR 1.14.4) et le moteur Rust `obsy`
(`src/engine`).

- `browser-shim.js` — script **non-module** à charger dans la page **avant**
  `app.js`. Il installe `window.require`, `window.process` et `window.electron`.
- `test.html` — banc d'essai manuel dans un navigateur.
- `test-sync.mjs` — test automatique (node, sans jsdom) qui démarre le moteur
  Rust, charge le shim et vérifie les comportements.
- `package.json` — uniquement pour `npm test` (le shim n'a aucune dépendance).

---

## 1. Pourquoi un pont synchrone

Le renderer d'Obsidian n'est pas isolé : il n'y a **ni `preload`, ni
`contextBridge`, ni `ipcMain`** dans l'ASAR. Le renderer appelle donc
directement, dans la page :

| Surface | Volume mesuré (analyse statique, `demonstrations/obsidian-rea/SHIM-SPEC.md`) |
|---|---|
| `ipcRenderer.sendSync` | 54 appels, 31 canaux distincts |
| `ipcRenderer.send` | 10 appels, 8 canaux |
| `ipcRenderer.invoke` | 1 appel (`register-cli`) |
| `fs` / `original-fs` synchrones | `readFileSync`, `existsSync`, `readdirSync`, `statSync` |
| `path` | `basename`, `join`, résolution |
| `crypto` | `scrypt` (+ `Buffer`) |

`sendSync` et `fs.readFileSync` doivent **rendre la main dans la pile
d'appel**. L'IPC de Tauri est asynchrone (aucun `invoke-sync`) : il ne peut
pas servir ce chemin. Le pont passe donc par un **`XMLHttpRequest` en mode
synchrone** vers le moteur Rust local.

## 2. Mécanisme

```
page Obsidian                     moteur Rust
─────────────                     ───────────
window.require('fs')              ┌──────────────────────────┐
  .readFileSync('Note.md','utf8')  │  POST /sync              │
        │                         │  {"op":"fs.readFileSync", │
        │  XHR synchrone ────────► │   "args":["Note.md",      │
        │                         │          "utf8"]}        │
        │                         │                          │
        │                         │  src/engine/sync.rs      │
        │                         │  → paths::normalize_      │
        │                         │    relative + ensure_     │
        │                         │    inside                │
        │  ◄────────────────────  │  {"ok":true,"value":…}   │
        ▼                         └──────────────────────────┘
   valeur de retour
```

- **Requête** : `POST http://127.0.0.1:<port>/sync`, corps JSON
  `{"op":"<opération>","args":[…]}`, `Content-Type: text/plain;charset=UTF-8`.
- **Réponse** : `{"ok":true,"value":…}` ou
  `{"ok":false,"error":"message en français","code":"<code machine>"}`.
- Le champ `code` permet au shim de distinguer une absence
  (`not_found`) d'un refus de confinement (`path_refused`).
- Seule une requête **malformée** (JSON invalide, champ `op` absent) renvoie
  un statut HTTP 4xx ; une opération refusée reste un `200` avec `ok:false`.

### Configuration de la page hôte

```html
<script>
  window.__OBSIDIAN_ENGINE_PORT = 27123;   // port du moteur
  // window.__OBSIDIAN_ENGINE_ORIGIN = 'http://127.0.0.1:27123'; // optionnel
</script>
<script src="browser-shim.js"></script>
<script src="app.js"></script>   <!-- l'ASAR d'Obsidian, après le shim -->
```

Le shim lit `window.__OBSIDIAN_ENGINE_PORT` (défaut `27123`) et
`window.__OBSIDIAN_ENGINE_ORIGIN` (défaut `http://127.0.0.1:<port>`) **au
moment de son installation** : les deux globales doivent donc être définies
avant `<script src="browser-shim.js">`.

> Le shim **écrase** `window.require` : ne pas le charger sur une page qui
> utilise RequireJS/AMD (`window.require` y a une autre signification).

### Ce que le shim installe

| Globale | Contenu |
|---|---|
| `window.require(id)` | `fs`, `original-fs` (alias), `path`, `crypto`, `os`, `electron`, `util`, `events`, `buffer` |
| `window.process` | `platform`, `arch`, `env`, `cwd()`, `versions` (`node`, `electron`, `chrome`, `v8`), `resourcesPath`, `argv`, `execPath` |
| `window.electron` | `ipcRenderer` (`sendSync`, `send`, `invoke`, `on`, `once`, `removeListener`, `removeAllListeners`), `shell`, `clipboard`, `dialog`, `app` (`getPath`, `getVersion`), `nativeTheme`, `webFrame`, `safeStorage`, `systemPreferences`, `screen`, `Menu`, `remote` (alias `@electron/remote`) |
| `window.Buffer` | polyfill minimal (`Uint8Array` + `TextEncoder`/`TextDecoder`) : `from`, `alloc`, `concat`, `isBuffer`, `byteLength`, `toString` |
| `window.__OBSIDIAN_ENGINE__` | diagnostic : `port`, `origin`, `endpoint`, `call(op, args)`, `modules` |

## 3. Opérations supportées par le moteur (`POST /sync`)

Implémentées dans `src/engine/sync.rs`.

| Opération | Arguments | Valeur renvoyée |
|---|---|---|
| `fs.readFileSync` | `[chemin, "utf8"]` | contenu du fichier (chaîne) |
| `fs.writeFileSync` | `[chemin, données, "utf8"]` | `{"bytes":N}` — écriture **atomique** (fichier temporaire + `rename`) |
| `fs.existsSync` | `[chemin]` | `true` / `false` |
| `fs.readdirSync` | `[chemin]` ou `[chemin, {"withFileTypes":true}]` | liste de noms, ou liste de descripteurs (`name`, `isFile`, `isDirectory`, `isSymbolicLink`) — triée par nom |
| `fs.statSync` | `[chemin]` | `{size, mtime, isFile, isDirectory, isSymbolicLink, readonly}` |
| `fs.mkdirSync` | `[chemin]` ou `[chemin, {"recursive":true}]` | `null` |
| `fs.unlinkSync` | `[chemin]` | `null` |
| `fs.renameSync` | `[origine, destination]` | `null` |
| `path.join` | `[…fragments]` | chaîne jointe (normalisation lexicale, sémantique Node) |
| `path.dirname` | `[chemin]` | dossier parent |
| `path.basename` | `[chemin]` ou `[chemin, ".md"]` | nom du fichier |
| `path.extname` | `[chemin]` | extension (`""` si aucune) |
| `path.resolve` | `[…fragments]` | chemin **absolu**, relatif à la racine du vault |
| `crypto.randomUUID` | `[]` | UUID version 4 (RFC 4122) |
| `ipc.vault.health` | `[]` | corps de `GET /health` |
| `ipc.vault.list` | `[]` | corps de `GET /files` |
| `ipc.vault.read` | `[chemin]` | corps de `GET /file?path=…` |
| `ipc.vault.write` | `[chemin, contenu]` | corps de `PUT /file?path=…` |
| `ipc.vault.metadata` | `[chemin]` | corps de `GET /metadata?path=…` |
| `ipc.vault.backlinks` | `[chemin]` | corps de `GET /backlinks?path=…` |
| `ipc.vault.search` | `[requête]` ou `[requête, limite]` | corps de `GET /search?q=…&limit=…` |
| `ipc.vault.graph` | `[]` | corps de `GET /graph` |
| `ipc.vault.root` | `[]` | racine du vault (chemin absolu) |

Les opérations `ipc.vault.*` **réutilisent les handlers existants** de
`src/engine/api.rs` (via `api::route`) : pas de duplication du confinement,
du parsing ni des messages d'erreur.

### Confinement des chemins

Tout chemin qui touche au disque passe par **la même logique que le reste du
moteur** (`src/engine/paths.rs`), sans exception :

1. `paths::normalize_relative` — chemin non vide, non absolu, sans composant
   `..` ;
2. `paths::ensure_inside` — le chemin canonisé doit rester sous la racine du
   vault (un lien symbolique sortant est donc refusé).

Exemples refusés (toutes opérations `fs.*` confondues) :
`../../etc/passwd`, `/etc/passwd`, `../evade.md`, `..`.

Les opérations `path.*` sont des **calculs de chaînes purs** : elles
n'ouvrent aucun fichier, donc aucune résolution de chemin n'est effectuée et
le confinement ne s'y applique pas. Leur résultat ne peut servir qu'à une
opération `fs.*`, qui est, elle, confinée.

## 4. Coût et limites du XHR synchrone

### Coût

- Chaque appel = **une connexion TCP** vers `127.0.0.1` (une requête par
  connexion, `Connection: close`), un aller-retour HTTP/1.1 et un JSON.
- **Mesuré** sur la box de compilation (4 vCPU, moteur en build *debug*,
  boucle locale, via `node shim/test-sync.mjs`) : **0,7 à 1,2 ms par
  aller-retour** (200 appels synchrones en 139 à 242 ms selon la charge de
  la machine, qui compilait en parallèle). Le navigateur ajoute à ce coût
  le traitement HTTP de `XMLHttpRequest`, mais pas d'aller-retour
  supplémentaire.
- Le poste de travail reste en revanche **bloqué pendant toute la durée de
  l'appel** (le renderer ne peint plus, ne traite pas les entrées) : le
  coût perçu par l'utilisateur est le *blocage*, pas le temps de calcul.
- Le moteur sérialise les requêtes derrière un verrou : deux XHR
  synchrones ne se chevauchent pas.
- Conséquence pratique : un renderer qui émet des centaines d'appels `fs` au
  démarrage aura un démarrage visibly saccadé. Le remède n'est pas un cache
  (les données doivent rester fraîches), mais **moins d'appels** : regrouper
  les lectures côté renderer, ou passer par `ipc.vault.*` qui renvoie des
  structures plus grosses pour moins d'aller-retours.

### Limites

- **Le XHR synchrone est déprécié** par les navigateurs (avertissement
  console) et **interdit** dans certains contextes (worker de module,
  `fetch` n'a pas d'équivalent synchrone). Chromium (Electron) et WebKit
  (WebView2/Tauri) le supportent encore.
- **Pas de préflight possible** : un XHR synchrone ne peut pas déclencher de
  requête `OPTIONS` (le préflight est toujours asynchrone). C'est pourquoi le
  shim envoie `Content-Type: text/plain;charset=UTF-8` — un type CORS
  « safelisted » — : la requête est alors une *simple request*, sans
  préflight. Un `Content-Type: application/json` ferait **échouer** tous les
  appels.
- **Same-origin ou CORS** : si la page n'est pas servie par le moteur, la
  requête est cross-origin et le navigateur bloque la réponse sans
  `Access-Control-Allow-Origin`. Le moteur envoie donc
  `Access-Control-Allow-Origin: *` (et `Access-Control-Allow-Methods: POST,
  OPTIONS`) sur `/sync` : c'est sans risque, le moteur n'écoute que sur
  `127.0.0.1`. Si cette politique CORS est un problème (extensions
  d'entreprise, `file://` strict), **servir la page depuis le moteur Rust**
  rend la requête same-origin et supprime tout besoin d'en-tête CORS.
- Une page ouverte en `file://` peut être bloquée par le navigateur pour
  toute requête `file://` → `http://` : dans ce cas, servir la page (et
  l'ASAR) depuis le moteur ou depuis un petit serveur statique local.

## 5. Ce qui n'est **pas** implémenté

| Manque | Conséquence | Piste |
|---|---|---|
| `crypto.scrypt` | le renderer ne peut pas dériver sa clé de chiffrement (`scrypt` lève une erreur explicite) | implémenter l'opération côté moteur (argon2/scrypt en Rust) |
| Canaux IPC du renderer sans effet de bord | `update`, `check-update`, `insider-build`, `relaunch`, `copy-asar`, `starter`, `sandbox`, `adblock-*`, `set-icon`, `cli`, `set-language`, `trash`, `help`, `terms`, `policy`… renvoient une valeur par défaut | implémenter ces canaux côté moteur, ou faire tourner le `main.js` d'Obsidian |
| `ipcRenderer.send` / `invoke` vers le moteur | `send` est distribué aux listeners **locaux** de la page ; `invoke` résout la même table de canaux. Aucun message n'atteint le moteur | ajouter un canal asynchrone (`POST /sync` en mode asynchrone, ou une file d'événements SSE) |
| Événements poussés par le moteur | `ipcRenderer.on` ne reçoit rien du moteur | ajouter un flux d'événements (SSE ou WebSocket) côté moteur |
| `remote.dialog` | stub : les boîtes de dialogue renvoient `canceled: true` | implémenter l'ouverture de fichiers côté moteur (l'API `PUT /file` existe déjà) |
| `remote.clipboard` | stub : lecture vide, écriture ignorée | pont presse-papiers (X11/Wayland) ou `navigator.clipboard` |
| `remote.shell` | stub : l'ouverture d'URL / l'affichage dans un dossier ne font rien | implémenter côté moteur (ou via le `main.js` d'Obsidian) |
| `remote.safeStorage` | stub : pas de chiffrement | implémenter côté moteur (ou via le `main.js` d'Obsidian) |
| `remote.screen` | stub : un seul écran 1280×800 | implémenter côté moteur (ou via le `main.js` d'Obsidian) |
| `remote.Menu` | stub | implémenter côté moteur (ou via le `main.js` d'Obsidian) |
| Binaire | `fs.readFileSync` renvoie du texte UTF-8 (le binaire est décodé, puis re-encodé) | opération `fs.readFileSync` avec encodage `base64` + `Buffer` |
| `fs.copyFileSync`, `fs.cpSync` | absents | opération de copie côté moteur |
| `path.relative`, `path.parse`, `path.format` | absents | calculs lexicaux locaux (comme `path.normalize`, déjà présent) |
| Serveur d'assets | le moteur ne sert pas l'ASAR (index.html, app.js, workers, `lib/*`) | tranche M1 : servir l'ASAR depuis le moteur |
| `process.env` réel | valeurs factices (`HOME`, `TMPDIR`, `LANG`) | passer l'environnement réel du moteur |

## 6. Tester

### Test automatique (node, sans jsdom)

```sh
# sur la box de compilation (le binaire du moteur est requis)
cargo build --bin obsy-engine
node shim/test-sync.mjs
```

`test-sync.mjs` :

1. crée un vault temporaire (`Note.md`, `dossier/Autre.md`) ;
2. démarre le binaire `obsy-engine` sur ce vault (port éphémère) ;
3. charge `browser-shim.js` dans un contexte `vm` node, avec un
   `XMLHttpRequest` factice qui interroge **réellement** le moteur (la requête
   part dans un *worker* : le fil principal est bloqué par `Atomics.wait`,
   la boucle d'événements node ne pourrait pas traiter la réponse) ;
4. vérifie lecture, existence, `path.*`, refus de remontée hors du vault,
   écriture, dossier, `randomUUID`, `ipcRenderer.sendSync`, et le fait que
   toutes les requêtes sont **synchrones** (`open(…, false)`) et en
   `text/plain` ;
5. arrête le moteur et supprime le vault temporaire.

### Test manuel (navigateur)

```sh
obsy-engine --root <vault> --port 27123
# puis ouvrir shim/test.html?port=27123
```

La page affiche le résultat de chaque vérification. Si le navigateur bloque
les requêtes `file://` → `http://`, servir le dossier `shim/` avec un petit
serveur statique local (`python3 -m http.server`) et ouvrir
`http://127.0.0.1:<port>/test.html?port=27123`.
