# MODE-COMPAT — Exécuter l'interface réelle d'Obsidian sur le moteur Rust

**Date :** 2026-10-21 · **Boîte cible :** instabox `hermes` (Debian 13,
Electron 45, moteur `obsidian-engine` compilé)

> **Ce qui est prouvé** : le lanceur `shim/electron-main-compat.js` démarre le
> moteur Rust, sert l'interface **réelle** d'Obsidian (extrait de l'ASAR
> installé) en HTTP local, injecte le shim avant `app.js`, et fait tourner le
> renderer contre **nos** données. Capture : `mode-compat-capture.png`
> (référencée en fin de document).

---

## 1. Prérequis

Sur la boîte qui exécute le mode compat :

1. **Obsidian installé** — l'ASAR `obsidian.asar` (ou son dossier extrait)
   doit être accessible. Le lanceur le lit **à l'exécution** ; aucun texte ni
   code d'Obsidian n'est stocké dans le dépôt.
2. **Electron** présent (≥ 15, testé avec 45). Chemin fourni par
   `OBSIDIAN_COMPAT_ELECTRON` ou auto-détecté.
3. **Moteur Rust compilé** : `obsidian-engine` (cible `--bin obsidian-engine`).
   Binaire fourni par `OBSIDIAN_COMPAT_ENGINE`.
4. **Un vault de test** (racine réelle) : `OBSIDIAN_COMPAT_VAULT`.
5. **Xvfb** (affichage headless) et **Node.js** ≥ 20 (pour le test).

---

## 2. Commande de test (bout en bout)

```sh
node shim/test-compat-electron.mjs
```

Variables d'environnement (toutes optionnelles sauf les trois premières) :

| Variable | Rôle | Défaut |
|---|---|---|
| `OBSIDIAN_COMPAT_VAULT` | racine du vault de test | *(requise)* |
| `OBSIDIAN_COMPAT_ASAR` | dossier d'ASAR extrait (ou `.asar`) | *(requise)* |
| `OBSIDIAN_COMPAT_ENGINE` | binaire du moteur Rust | *(requise)* |
| `OBSIDIAN_COMPAT_ELECTRON` | binaire Electron | auto-détecté |
| `XVFB_DISPLAY` | écran Xvfb | `:91` |
| `CDP_PORT` | port de débogage CDP d'Electron | `9244` |
| `STATIC_PORT` | port du serveur statique local | `8823` |
| `ENGINE_PORT` | port du moteur Rust | `27124` |
| `CAPTURE_PATH` | chemin de la capture d'écran | `demonstrations/obsidian-rea/mode-compat-capture.png` |

Exemple sur `hermes` :

```sh
export OBSIDIAN_COMPAT_VAULT=/data/home/obsidian-rea/vault
export OBSIDIAN_COMPAT_ASAR=/data/home/obsidian-rea/asar-x1
export OBSIDIAN_COMPAT_ENGINE=/data/home/obsidian-rea/bin/obsidian-engine
export OBSIDIAN_COMPAT_ELECTRON=/data/home/obsidian-rea/tools/electron/node_modules/.bin/electron
node shim/test-compat-electron.mjs
```

Le test :
1. lance `Xvfb` puis Electron + le lanceur (`shim/electron-main-compat.js`) ;
2. attend la fenêtre réelle (CDP) ;
3. vérifie le titre (« Obsidian »), `.app-container`, `.workspace`, et
   **zéro exception** dans la console du renderer ;
4. prend une **capture d'écran** (`Page.captureScreenshot`) ;
5. interroge le moteur (`/files`) et vérifie que les fichiers affichés par le
   renderer correspondent aux données du moteur.
Échec = message réel (sortie des processus, exceptions), jamais un « échec »
muet.

---

## 3. Commande manuelle (sans le test)

```sh
DISPLAY=:91 path/to/electron shim/electron-main-compat.js \
  --vault /data/home/obsidian-rea/vault \
  --asar /data/home/obsidian-rea/asar-x1 \
  --engine-bin /data/home/obsidian-rea/bin/obsidian-engine \
  --engine-port 27124 --static-port 8823 \
  --no-sandbox --disable-gpu --remote-debugging-port=9244
```

Sous `Xvfb` :

```sh
Xvfb :91 -screen 0 1400x900x24 &
DISPLAY=:91 …
```

---

## 4. Ce qui fonctionne (vérifié)

- Le lanceur démarre le moteur Rust (`--root <vault> --port <port>`), attend
  `/health`, puis l'arrête à la fermeture.
- La page `index.html` d'Obsidian est servie en HTTP local ; le `<head>` reçoit
  **deux scripts classiques** (non-`defer`) avant `app.js` :
  1. la **passerelle** `window.__OBSIDIAN_COMPAT__` (surface `remote` +
     `lireChaineAcceptation` renvoyant la valeur **calculée par le processus
     principal**) ;
  2. `shim/browser-shim.js` (via `/__shim/browser-shim.js`).
- La chaîne d'acceptation (284 caractères) est lue dans l'ASAR au démarrage, à
  l'exécution, et injectée comme valeur déjà calculée — jamais recopiée dans le
  dépôt.
- Les canaux IPC du boot sont réponclus (miroir des `REPLIES` du harnais) :
  `vault`, `vault-list`, `vault-open`, `version`, `terms`, `policy`, `frame`,
  `starter`, `sandbox`, `disable-gpu`, `disable-update`, `update`,
  `is-quitting`, `is-closing`, … 
- L'explorateur de fichiers est alimenté par **notre moteur** (`/files`) : le
  test compare les titres affichés par le renderer aux fichiers du moteur.

## 5. Ce qui reste

- Le test et le lanceur sont écrits pour le contexte de la boîte (`hermes`) ;
  les chemins par défaut des variables doivent pointer vers l'installation
  réelle d'Obsidian.
- La capture d'écran est produite par le test (`mode-compat-capture.png`).
- Le comportement en cas d'application Obsidian absente est explicite (message
  sur `stderr`, arrêt).

---

## 6. Capture d'écran

![Interface Obsidian sur le moteur Rust](mode-compat-capture.png)

La capture est générée à chaque exécution du test dans
`demonstrations/obsidian-rea/mode-compat-capture.png`.
