# Contrats de boot du renderer d'Obsidian 1.14.4

**Date :** 2026-10-21 · **Source :** analyse statique de l'ASAR réel
(`obsidian-1.14.4.asar`, sha256 `146ef8470d8cbb4a6db209bdc19bf2bad13350444186c0fd93c31087b142b92f`),
extrait sur la box instabox `hermes` (`/data/home/obsidian-rea/`), puis recopié sur
`pour-omp` (`/data/home/obsidian-spike/asar-x1`, 368 fichiers, 28 Mo).

Ces contrats ont été déduits par lecture du bundle **avant** tout essai
d'exécution. Ils sont conservés ici parce que le harnais qui les avait produits
(spike Tauri) n'a jamais réussi à démarrer et a été supprimé.

> **Statut : contrats confirmés par exécution le 2026-10-21** (ils
> étaient des déductions statiques jusqu'alors). Le renderer a été lancé sur la
> box `hermes` : Electron 45 avec un processus principal de substitution →
> **leur interface démarre** (explorateur peuplé, thème appliqué, 0
> exception). Voir `RENDERER-BOOT.md` pour le diagnostic complet, les causes
> réelles des échecs et les points de rupture restants.

## Entrée du renderer

`index.html` charge 11 scripts en `defer` :
`lib/codemirror/*`, `moment`, `pixi`, `i18next`, `scrypt`, `turndown`,
`enhance.js`, `i18n.js`, `app.js` — plus `app.css`.

Ressources attendues : `lib/*` (pdfjs, mathjax, mermaid, reveal, prism),
`public/fonts`, `public/images`, `i18n/*.txt`, `worker.js`, `sim.js`.

## Les 7 contrats

| # | Contrat | Conséquence si violé |
|---|---|---|
| 1 | `window.require('electron')` doit renvoyer `{ipcRenderer, remote, webFrame}` **et** `window.electron` doit exister (nodeIntegration actif : 0 contextBridge, 0 preload) | boot avorté |
| 2 | `ipcRenderer.sendSync('vault')` doit renvoyer `{id, path}`. Si `fs.existsSync(path)` est faux → le renderer appelle `sendSync('starter')` puis `window.close()` | **point de rupture n°1** |
| 3 | `sendSync('version')` → chaîne ; `remote.app.getVersion()` → build ; `sendSync('policy')` → objet ; `sendSync('terms')` → **exactement** la chaîne d'acceptation de **284 caractères** (longueur mesurée le 21/10 sur Obsidian 1.14.4 ; « I understand and agree that I am not allowed to distribute the Obsidian application… ») | `window.close()` |
| 4 | `process.platform` doit valoir `'linux'` (sinon `Qy.isLinux` est faux) ; `process.versions.electron` est lu au démarrage (branche `electron.clipboard` vs `electron.remote.clipboard`) | comportement silently faux |
| 5 | **La liste des fichiers ne passe pas par un canal IPC** : l'adaptateur fs fait `fs.promises.readdir/stat/lstat/readFile` sur `path.join(basePath, chemin)` | l'explorateur de fichiers est vide |
| 6 | Modules requis au boot : `original-fs`, `path`, `url`, `os` (optionnel), `electron` ; `crypto.scrypt` seulement pour les vaults chiffrés ; `btime` optionnel | boot avorté |
| 7 | Surface IPC mesurée : 31 canaux `sendSync` + 8 `send` + 1 `invoke` = **40 canaux distincts**, plus ~20 membres `electron.remote` | shim incomplet |

## Ce que ça implique pour un shim

- Le contrat 5 est le plus structurant : l'explorateur de fichiers passe par
  l'adaptateur `fs` du renderer, donc par le shim `fs` — pas par un canal IPC.
  C'est ce qui rend le pont synchrone (`POST /sync`) nécessaire.
- Les contrats 2 et 3 sont des **points de rupture uniques** : un seul octet de
  différence sur la chaîne d'acceptation suffit à faire fermer la fenêtre.
- Le contrat 7 confirme la surface déjà chiffrée dans `SHIM-SPEC.md`
  (40 canaux) : les deux documents sont cohérents.

## Relancer l'essai

Les contrats ci-dessus ont été **confirmés par exécution** le 2026-10-21 sur
la box instabox `hermes` : voir `RENDERER-BOOT.md` (diagnostic complet, causes
réelles des échecs, points de rupture restants, mesures mémoire) et
`tools/` (harnais de test : processus principal Electron de substitution,
preload `remote`, sonde CDP, mesure RSS/PSS). Pour la surface requise côté
shim : `SHIM-SPEC.md` et `shim/browser-shim.js` (pont synchrone déjà écrit,
23 opérations, 25 vérifications au vert).
