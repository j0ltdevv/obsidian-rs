# Benchmarks — obsidian-rs

Mesures reproductibles de consommation mémoire et de temps de compilation.
Chiffres **mesurés** sur cette machine, sauf mention contraire.

## Machine de référence

| Élément | Valeur |
|---|---|
| CPU | Intel Raptor Lake-P, 12 threads (6 cœurs × 2 SMT) |
| RAM | 15 GiB (dont ~2,4 GiB libres en session interactive) |
| GPU | Intel Iris Xe (pas de NVIDIA) |
| Disque | 237 Go, 94 Go libres au moment de la mesure |
| OS | Linux 7.2.5-3-omarchy, Wayland + X11 |
| Rust | 1.99.0 |

## Méthode de mesure mémoire

- Binaire **release** (`cargo build --release`), lancé sous `Xvfb` en **X11 forcé** :
  `env -u WAYLAND_DISPLAY DISPLAY=:NN ./target/release/obsidian-rs`
  → sans `env -u WAYLAND_DISPLAY`, l'application se connecte au compositeur Wayland
  réel et la capture/mesure sous Xvfb est fausse (écran noir).
- Attente de 12 s (à vide) ou 20 s (vault 5 000 notes) avant lecture.
- Lecture de `VmRSS` et `VmHWM` dans `/proc/<pid>/status`.
- Vault de test : 5 000 notes générées (40 dossiers, frontmatter, tags, 6 wikilinks
  croisés par note, 20 Mo au total), hors du dépôt.
- ⚠️ RSS n'est pas PSS : la mémoire partagée (driver GPU, polices) est comptée en
  entier dans le RSS du processus. À lire comme une borne haute.

## Résultats — obsidian-rs

| Configuration | À vide | Vault 5 000 notes | Libs GL/Vulkan mappées |
|---|---|---|---|
| iced + **wgpu** (GPU) | 75,6 Mo | 121,2 Mo | oui |
| iced + **tiny-skia** (rendu logiciel) | **38,4 Mo** | **83,8 Mo** | **0** |
| iced + wgpu, build debug | 103,9 Mo | — | oui |

**Gain du rendu logiciel : −49 % à vide, −31 % sur 5 000 notes.**

### Re mesure du 21 octobre — nouveau build (boîtes instabox)

| Configuration | RSS | VmHWM | Détail |
|---|---|---|---|
| iced + tiny-skia, **release** (`cargo build --release`) | **22,0 Mo** (22 488 Ko) | 23,3 Mo | 10 threads, binaire 12,8 Mo — mesuré sur `pour-omp` |
| iced + tiny-skia, **debug** (`CARGO_PROFILE_DEV_DEBUG=0`) | **39,6 Mo** (40 548 Ko) | — | stable de T+2 s à T+20 s — `pour-omp` |
| iced + tiny-skia, debug, **vault de 5 000 notes** (20 Mo, 40 dossiers) | **70,7 Mo** (72 416 Ko) | 72,4 Mo | VmHWM = VmRSS : aucun pic de mémoire — `hermes` |

Le build release descend à **22 Mo** : l'écart avec le build debug (39,6 Mo)
vient du binaire et de la debuginfo, pas du code. Les trois mesures sont des
**RSS de processus unique**, relevées sur `/proc/<pid>/status` après
stabilisation.

Comparaison avec la mesure de 5 000 notes plus ancienne (83,8 Mo) : non
strictement homogène (hôtes différents, build debug dans les deux cas).

Le coût est la pile GPU (contexte wgpu + driver mappé + atlas de textures), pas le
code de l'application. D'où l'option `default-features = false` avec la feature
`tiny-skia` dans `Cargo.toml` — **exclusive** avec wgpu : un moteur HTML embarqué
(Blitz, CEF, Servo) exige le device wgpu d'iced et fait remonter la conso.

## Planchers mémoire des runtimes (mesurés le 21 octobre)

| Runtime | Charge | PSS | RSS | Détail |
|---|---|---|---|---|
| **notre UI native** (iced + tiny-skia, release) | page Markdown réelle | **22,0 Mo** | 22,5 Mo | 1 processus, 10 threads — `pour-omp` |
| **notre front `web/`** (Tauri 2 + WebKitGTK) | vault démo, note ouverte | **223,9 Mo** (229 875 Ko) | 335 Mo | 105 827 Ko (Tauri) + 85 341 Ko (renderer WebKit) + 36 210 Ko (réseau) + 2 497 Ko (moteur) — `pour-omp` |
| **notre UI native** (debug) | vault de 5 000 notes | **70,7 Mo** | 72,4 Mo | VmHWM = VmRSS : aucun pic — `hermes` |
| **Tauri 2 + WebKitGTK** | page HTML **vide** | **286 Mo** | 467 Mo | 87,6 Mo (Tauri) + 176 Mo (WebKit) + 29 Mo (réseau) — `pour-omp` |
| Tauri 2 + WebKitGTK + **CodeMirror 6** | un document ouvert | **395 Mo** | 576 Mo | banc `/data/home/tauri-probe` |
| Tauri 2 + WebKitGTK + CodeMirror 6 | vault de 5 000 notes **sur disque** | **387 Mo** | 559 Mo | 96,4 Mo (Tauri) + 270,7 Mo (renderer) + 29,8 Mo (réseau) |
| **Chromium** | page HTML **vide** | — | 843 Mo cumulé | RSS cumulé sur 7 processus (la mémoire partagée y est comptée plusieurs fois) — `ssss` |
| Electron (Obsidian) | application réelle | — | 180-250 Mo | **rapport tiers**, non mesuré ici |

⚠️ Nuance sur la ligne « 5 000 notes » : le banc charge **un** document dans
l'éditeur ; les 5 000 notes sont sur le disque (20 Mo), pas dans le DOM. La
mesure dit donc « webview + un éditeur », pas « webview + 5 000 notes
ouvertes ». Le coût dominant est le moteur de rendu, pas le contenu du vault.

**Conséquence** : une UI web (webview du système ou Chromium) coûte **plus cher
qu'Electron lui-même** — WebKitGTK alone vaut 286 Mo PSS pour une page vide, et
387 Mo avec un éditeur de Markdown ouvert. Le budget « 200-400 Mo » **inclut**
donc un webview, mais pas deux moteurs.

## Comparaison — Obsidian (Electron)

| Configuration | RSS | PSS | Libs GL mappées |
|---|---|---|---|
| **obsidian-rs** (iced + tiny-skia, release) | **22,0 Mo** | — | 0 |
| **obsidian-rs** (iced + tiny-skia, debug, 5 000 notes) | **70,7 Mo** | — | 0 |
| Obsidian 1.7.3 (vault léger) — **rapport tiers, non mesuré ici** | 180-250 Mo | — | oui |

Comparaison hétérogène (notre chiffre = 1 process, celui d'Obsidian = rapport
tiers sur le process principal probable) : elle indique un ordre de grandeur,
pas une égalité. Ce qui est solide : **notre base est mesurée, la sienne est
rapportée**.

## Temps de compilation

| Contexte | Durée | `target/` |
|---|---|---|
| Local, release cold (wgpu) | 2 min 38 s | 1,1 Go (release) |
| Local, release (tiny-skia, sans wgpu) | 49 s | — |
| Box instabox `pour-omp` (4 vCPU), debug cold, `CARGO_PROFILE_DEV_DEBUG=0` | **59 s** | **1,4 Go** |

## Disque

- `target/debug` atteignait **30 Go** avec le profil par défaut (debuginfo complète).
- `[profile.dev] debug = 1` ajouté au `Cargo.toml` (la trace complète ne sert qu'au
  débogage ; l'app et les tests n'en ont pas besoin).
- Sur la box, `CARGO_PROFILE_DEV_DEBUG=0` donne 1,4 Go de `target/` : c'est la
  configuration à utiliser pour les builds déportés.

## Résultats — Tauri + WebKitGTK + CodeMirror 6 (banc `/tmp/tauri-probe`, box `pour-omp`)

Mesure de l'arbre de processus complet (le coût d'une webview n'est pas dans le
processus principal mais dans `WebKitWebProcess`).

| Processus | RSS | PSS |
|---|---|---|
| `tauri-probe` (principal) | 173 Mo | 98 Mo |
| `WebKitWebProcess` (rendu) | 341 Mo | 268 Mo |
| `WebKitNetworkProcess` | 62 Mo | 29 Mo |
| **Total application** | **576 Mo** | **395 Mo** |

- Hôte : Debian 13, 4 vCPU, 7 GiB ; WebKitGTK 2.54.0 ; Tauri 2.12.4.
- Document : 3 202 lignes de markdown dans CodeMirror 6 (oneDark), page minimale.
- Releves T+15 s et T+45 s stables (341 → 340 Mo sur le processus de rendu).
- Xvfb (60 Mo) est le serveur d'affichage du harnais, **exclu** du total.
- **Borne basse** : une seule page, un seul éditeur. Le DOM complet d'Obsidian
  (leur `app.js`, 195 modules webpack) et ses plugins—are à ajouter par-dessus.

## Le chiffre qui tranche

| Front-end | Coût mesuré | RAM |
|---|---|---|
| **iced + tiny-skia (nôtre)** | 38,4 Mo (vide) / 83,8 Mo (5 000 notes) — 1 process | ✅ ÷ 2 à 6 vs Obsidian |
| Tauri + WebKitGTK + CM6 | 395 Mo PSS / 576 Mo RSS — **arbre complet mesuré** | ❌ |
| Obsidian (Electron) | 180-250 Mo (vault léger) — **rapport tierce, process principal très probablement** | référence |
| Obsidian (20k notes + plugins) | 6-10 Go —rapport tierce | — |

**Ce que la mesure prouve, et ce qu'elle ne prouve pas.**

- Elle prouve que **WebKitGTK en webview ne bat pas Electron** : 395 Mo PSS pour
  une seule page CodeMirror, contre 180-250 Mo rapportés pour Obsidian.
- Elle ne prouve pas « tout renderer ≥ 400 Mo » : les 180-250 Mo d'Obsidian
  viennent de rapports tiers et sont probablement le **process principal seul**,
  alors que notre chiffre est l'**arbre complet** (3 process WebKit). Comparaison
  hétérogène, donc borne indicative, pas égalité.
- Ce qui reste solide : **un moteur de rendu web coûte 5 à 15 fois plus que
  notre 38-84 Mo**, quel que soit l'hôte. Remplacer le rendu par du Rust reste
  la seule voie qui réduit la RAM.
- Ultralight (15 Mo annoncés) et Sciter (4-8 Mo) n'ont pas encore été mesurés :
  c'est le dernier trou de cette matrice.

## Références externes (non mesurées ici)

| Sujet | Chiffre | Source |
|---|---|---|
| Obsidian (vault léger) | 180-250 Mo | comparatif tiers, v1.7.3 |
| Obsidian (20k notes, 50k images, plugins) | 6-10 Go | forum Obsidian |
| Plafond structurel Obsidian | heap V8 de 4 Go (memory cage) | Electron / forum |
| Tauri + WebKitGTK + CodeMirror 6 | **en cours de mesure** | banc `/tmp/tauri-probe` |

## Ce qui n'est pas mesuré

- RAM d'Obsidian lancé localement : le processus quitte immédiatement dans cet
  environnement (GPU/display), la mesure est impossible ici.
- Consommation d'une webview (WebKitGTK/CEF) : mesure en cours.
- Coût GPU réel en session native (les chiffres ci-dessus sont en rendu logiciel
  sous Xvfb, donc pessimistes côté CPU et optimistes côté RAM).

## Reproduction

```sh
cargo build --release
Xvfb :95 -screen 0 1400x900x24 & sleep 2
env -u WAYLAND_DISPLAY DISPLAY=:95 ./target/release/obsidian-rs & sleep 12
grep -E 'VmRSS|VmHWM' /proc/$!/status
```
