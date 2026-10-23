# MODE-COMPAT — L'interface réelle d'Obsidian sur notre moteur Rust

**Date :** 2026-10-21 · **Boîte de test :** instabox `hermes` (Debian 13,
Electron 45, moteur `obsy-engine` compilé)
**État :** fonctionnel — voir « Preuves » plus bas.

> **Ce qui est prouvé, exactement** : leur renderer réel (extrait de l'ASAR
> installé chez l'utilisateur) démarre sous notre lanceur, passe **toutes** les
> portes de boot, et affiche **notre vault** : l'explorateur de fichiers montre
> les notes de notre moteur, et **un fichier caché présent sur le disque
> n'apparaît pas** (notre moteur l'ignore, le disque, non) → la provenance des
> données est **démontrée**, pas supposée.
>
> **Ce qui n'est pas fait** : la surveillance du vault hors de l'application
> (`fs.watch` est un observateur inerte), et le mode compat n'est pas encore
> empaqueté pour un poste utilisateur (Electron doit être présent).

---

## 1. Ce que fait le lanceur

`shim/electron-main-compat.js` (processus principal Electron) :

1. résout l'ASAR d'Obsidian **installé chez l'utilisateur** (argument
   `--asar`, sinon détection système) — rien n'est copié dans le dépôt ;
2. lance **notre moteur Rust** (`obsy-engine --root <vault> --port N`) et
   attend `/health` ;
3. sert le renderer d'Obsidian **en HTTP local** (un `file://` ne permet pas
   les XHR synchrones du shim vers le moteur), en réécrivant `index.html` pour
   injecter, **avant `app.js`** : la passerelle `window.__OBSIDIAN_COMPAT__`
   (surface `remote` + chaîne d'acceptation déjà lue) puis
   `shim/browser-shim.js` ;
4. répond aux canaux IPC du boot que le shim ne couvre pas ;
5. arrête le moteur à la sortie (`will-quit`).

Le shim (`shim/browser-shim.js`) fournit `window.require`, `window.process`,
`window.electron` (avec `remote`), le module `fs`/`original-fs` **ponté vers
notre moteur**, `path`, `url`, `os`, `crypto`, `util`, `events`, `buffer` et
`@electron/remote`.

---

## 2. Les portes de boot (dans l'ordre de leur bundle)

| # | Porte | État |
|---|---|---|
| 1 | `sendSync('vault')` → `{id, path}` et `fs.existsSync(path)` vrai | oui — le shim répond `{id, path, name, open}` ; `existsSync` passe par le moteur |
| 2 | `sendSync('version')`, `remote.app.getVersion()`, `sendSync('policy')`, `remote.app.getPreferredSystemLanguages()` | oui |
| 3 | `sendSync('terms')` = **exactement** la constante comparée par le renderer | oui — lue **à l'exécution** dans l'ASAR, localisée **par la comparaison** (`"…" !== sendSync("terms")`) — jamais recopiée |
| 4 | `process.platform === 'linux'`, `process.versions.electron`, `os.hostname()/version()/release()` | oui |
| 5 | la liste des fichiers passe par l'adaptateur `fs` | oui — via notre moteur |

Le renderer exige aussi `require('@electron/remote')` (retiré d'Electron 45) :
le shim sert cette surface.

---

## 3. Preuves

**Boot** : la fenêtre s'ouvre avec le titre
`New tab - vault-compat - Obsidian 1.14.4`, sans exception.

**Données** (sonde CDP sur le DOM de leur application) :

```
titre    : "New tab - vault-compat - Obsidian 1.14.4"
fichiers : ["brouillon", "note ephemere", "projet"]
cache    : []            ← .cache-cache.md est sur le disque, pas dans l'interface
fermeture: aucune
```

Le vault de test contenait `brouillon.md`, `note ephemere.md`, `projet.md`
**et** `.cache-cache.md`. Le moteur ignore les fichiers commençant par un point
(son scan), le disque, non : leur interface montre les trois notes et **pas**
le fichier caché → **les données affichées viennent de notre moteur**.

**Mémoire** : 373 Mo PSS pour l'arbre Electron (processus principal + renderer
+ utilitaires), dont **2,4 Mo** pour notre moteur Rust.

**Preuve textuelle** (texte du DOM de leur application, extrait par CDP) :

```
New tab - vault-compat - Obsidian 1.14.4
vault-compat
brouillon
note ephemere
projet
New tab
New note      Ctrl + N
New base
New canvas
Go to file    Ctrl + O
```

Ce sont leurs libellés (« New note », « New base », « New canvas », « Go to
file ») et nos noms de notes : leur interface, nos données. La capture d'écran
correspondante reste à produire (la sonde DOM ci-dessus fait foi).

---

## 4. Les cinq bugs trouvés en l'exécutant vraiment

Aucun n'était visible à la lecture du code ; tous sont apparus à l'exécution.

1. **Faute de frappe** dans le nom de la fonction de réécriture de l'index
   (`reeecrireIndex` au lieu de `reecrireIndex`) → l'index n'était jamais
   servi, la page ne chargeait pas.
2. **Le gestionnaire HTTP ne répondait pas en cas d'exception** : la requête
   restait pendante (aucun octet) et l'échec était invisible. Il répond
   désormais toujours (500 + message réel) et journalise chaque requête.
3. **`require('@electron/remote')` absent du shim** → le renderer mourait à
   son chargement (Electron 45 a retiré ce module).
4. **La chaîne d'acceptation n'est pas de 284 caractères** mais ~345 : la
   capturer par sa longueur la tronquait, la comparaison échouait, et le
   renderer faisait `window.close()`. Elle est désormais localisée **par la
   comparaison** qu'il fait — on récupère le littéral entier sans le recopier.
5. **`fs.watch` et `fs.promises.lstat` manquants** → leur écran d'erreur
   « An error occurred while loading Obsidian ». Ajoutés (`watch` est un
   observateur inerte : le moteur n'expose pas encore de flux d'événements).

---

## 5. Limites connues

- **Pas de surveillance du vault** : une note modifiée hors de l'application
  n'est pas détectée (`fs.watch` inerte).
- **Electron doit être installé** sur le poste de l'utilisateur (le mode compat
  ne l'embarque pas) ; notre front `web/` n'a pas cette dépendance.
- Le lanceur est **notre** code ; il remplace le processus principal
  d'Obsidian (qui reste le chemin officiel et le seul supporté par Obsidian).
- La chaîne d'acceptation est lue dans l'ASAR de l'application installée :
  l'utilisateur doit donc **avoir Obsidian installé** et avoir accepté ses
  conditions (c'est le cas de qui l'a installé).

---

## 6. Reproduction

```sh
# 1. moteur
cargo build --bin obsy-engine
# 2. Electron (installé par l'utilisateur) + leur application
Xvfb :90 -screen 0 1400x900x24 &
DISPLAY=:90 electron shim/electron-main-compat.js \
  --vault /chemin/du/vault \
  --asar ~/.config/obsidian/obsidian.asar \
  --engine-bin ./target/debug/obsy-engine
```

Le test automatisé du shim (41 vérifications) :
`OBSIDIAN_ENGINE_BIN=./target/debug/obsy-engine node shim/test-sync.mjs`.
