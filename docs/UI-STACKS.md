# Choix du stack d'interface — obsy

Comparaison des quatre voies possibles pour une interface fidèle à Obsidian,
avec les **coûts mémoire mesurés ou sourcés**. Le projet a deux contraintes :
la fidélité visuelle d'Obsidian, et une mémoire qui reste sous les 400 Mo.

## Les quatre voies

| Voie | RAM (contexte) | Fidélité | Code | Fragilité |
|---|---|---|---|---|
| **Rendu natif — iced + tiny-skia** (la nôtre) | **22,0 Mo** release · **70,7 Mo** sur 5 000 notes (mesuré ici) | approchée : reconstruction au pixel près possible, mais bornée par ce qu'iced dessine | notre code | aucune |
| **Rendu natif — Slint** | ~30 Mo pour une app réelle en renderer logiciel (Linux, release, source tierce) | déclaratif `.slint` (proche du CSS), panneaux redimensionnables, widgets maison | notre code | réécriture complète de l'UI ; chiffres à re-mesurer | 
| **Webview — notre front HTML/CSS + moteur Rust** | **286 Mo** PSS page vide · **387 Mo** PSS avec un éditeur ouvert (mesuré ici) | **pixel-parseille** : Obsidian est du HTML/CSS | notre code | celle de n'importe quel front web |
| **Leur renderer (Electron)** | 180-250 Mo (rapport tiers, jamais mesuré ici) ; Electron mesuré 443-644 Mo sur apps chargées | **exacte** (c'est leur code) | dépend de 40 canaux IPC internes | **forte** : casse à chaque mise à jour d'Obsidian |

## Pourquoi le webview, si la fidélité prime

- Obsidian **est** une application HTML/CSS : reproduire son apparence ne demande
  pas d'inventer un moteur de rendu, mais d'écrire du CSS avec les bonnes
  valeurs — et ces valeurs sont **mesurées** (`UI-SPEC.md`).
- Notre moteur Rust est déjà un service HTTP (23 opérations, chemins confinés) :
  il sert de source de vérité sans changement d'architecture.
- 387 Mo est dans le budget annoncé (200-400 Mo).

## Pourquoi ne pas choisir le webview plus tôt

À 150 Mo maximum, le webview était **exclu** : 286 Mo PSS pour une page HTML
vide, c'est-à-dire plus qu'Electron. La mesure a tranché avant toute
implémentation.

## Ce que disent les autres applications (recherche du 21 octobre)

- **Zed** (GPUI, natif) : ~420 MiB au repos (mesuré, PSS, arbre complet, avec
  ses serveurs de langage) contre 744 MiB pour VS Code — le natif est bien
  moins coûteux qu'Electron, mais un éditeur complet reste lourd en charge.
- **Rnote** (Rust + GTK4, canvas natif) : prouve qu'une surface de notes peut
  être dessinée **sans HTML**.
- **Logseq** : garde son graphe Datalog **en mémoire dans le renderer** et
  plafonne vers 200 Mo de vault — l'index et les requêtes doivent vivre dans
  le cœur natif, pas dans l'interface. C'est déjà notre architecture.
- **Joplin** : 1,5 Go de RAM rapportés avec une base de 5 Mo — le renderer
  Electron, pas la couche de données, est le plafond mémoire.
- **SilverBullet** : serveur Rust + client web ; les greffons tournent dans des
  Web Workers sandboxés sur un QuickJS/WASM intégré — le modèle d'extension à
  copier le jour où il faudra des plugins, sans jamais embarquer un second
  moteur.

## Ce qu'il ne faut pas faire

- Ne **pas** garder le graphe ou l'index complet dans la mémoire de l'interface
  (le Mur de Logseq).
- Ne **pas** réécrire le document entier comme une seule chaîne à chaque
  frappe (Marque noire de MarkText) : le moteur travaille par fragments
  (`SourceToken`).
- Ne **pas** embarquer un second moteur de rendu à côté du premier.

## Sources

- Mesures locales : `docs/BENCHMARKS.md` (21 octobre 2026, boîtes instabox).
- Spécification d'apparence : `demonstrations/obsidian-rea/UI-SPEC.md`.
- Références d'architecture : `demonstrations/obsidian-rea/REFERENCES-UI.md`.
- Étude des frameworks : Slint (slint.dev), egui (github.com/emilk/egui),
  GPUI (github.com/zed-industries/zed), Rnote (github.com/flxzt/rnote),
  SilverBullet (v2.silverbullet.md), Logseq, Joplin, Tauri 2 (tauri.app).
