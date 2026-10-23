# REFERENCES-UI — Verso & SilverBullet comme références d'architecture pour notre UI HTML/CSS

**Date :** 2026-10-21 · **Périmètre :** document unique (`demonstrations/obsidian-rea/REFERENCES-UI.md`), aucun autre fichier du dépôt modifié, aucun commit, aucun `cargo`.
**Question à laquelle ce document répond :** quelle architecture d'éditeur et de runtime adopter pour que notre UI cesse d'être « laide » tout en tenant le budget RAM.

> **Avertissement clean-room (à lire avant toute citation).**
> Verso est **GPL-3.0**. SilverBullet est **MIT**. Ce document **n'a repris aucun code, aucun CSS, aucun identifiant** de l'un ou l'autre : uniquement des décisions d'architecture, des noms de bibliothèques et des chiffres publiés, avec leur URL. Les techniques listées en §6 sont décrites en behaviour, à réimplémenter par nous. `obsy` reste MIT et clean-room vis-à-vis d'Obsidian (règle inchangée).

---

## 1. Synthèse

| | **Verso** | **SilverBullet** |
|---|---|---|
| Projet | Éditeur Markdown mono-fichier/dossier (Cargo.toml : v0.2.2 ; le site affiche encore « v0.1.0 ») | Base de connaissances programmable (Markdown + Lua) |
| Licence | **GPL-3.0** (`LICENSE`, `src-tauri/Cargo.toml` : `license = "GPL-3.0-only"`) | **MIT** (`LICENSE.md`) |
| Shell applicatif | **Tauri v2** (webview système) | **SilverBullet Server** = serveur Rust + client web ; **SilverBullet Desktop** = produit commercial (Tauri v2.0→2.11, **Electron depuis 2.12**) — shell **absent du dépôt OSS** |
| Langages | Rust (backend) + TypeScript/React 19 (frontend) | Rust (serveur) + TypeScript/Preact (client) |
| Framework front | Vite + Tailwind CSS v4 | ESBuild + SCSS (aucun framework UI ; Preact) |
| Éditeur WYSIWYG | **Tiptap v3** (ProseMirror) + `@tiptap/markdown` | **Pas de WYSIWYG** : *live preview* CodeMirror 6 (décorations + widgets) |
| Éditeur source | **CodeMirror 6** (`@codemirror/lang-markdown` + `language-data`) | **CodeMirror 6** (même base) |
| Recherche | **Tantivy 0.22** (index par dossier, sur disque) | **Tantivy** (`server-search/`) + **recherche sémantique optionnelle (Candle)** |
| Stockage | Fichiers `.md` dans un dossier choisi par l'utilisateur | *Space* = dossier de `.md` sur disque, servi par API HTTP, sync optionnelle |
| Thèmes | Variables CSS (~59 propriétés) + classe `.dark` + `prefers-color-scheme` | SCSS + `space-style` (CSS modifiable par l'utilisateur depuis l'espace) |
| Mesure RAM publiée | **Aucune** (claim marketing « moins qu'un onglet de navigateur ») | Serveur : **158 → 70 Mo mesuré** (2.0→2.1) ; **client desktop : aucun chiffre**, retour d'expérience Tauri vs Electron |

**Le fait le plus important du document** : SilverBullet, maintained par une personne qui a livré les deux, **a quitté Tauri pour Electron en 2.12** en expliquant que le gain mémoire de Tauri **n'était pas réel** en multi-fenêtre (§4.3). C'est la donnée la plus directement transposable à notre décision de runtime.

---

## 2. Verso — éditeur Markdown Rust + Tauri (GPL-3.0)

Sources : site <https://verso.imzl.com/>, dépôt <https://github.com/zhoulianglen/verso> (branche `master`, clone local effectué pour cette étude ; ~40 étoiles, 10 issues ouvertes, licence GPL-3.0 détectée par l'API GitHub).

### 2.1 Architecture

Deux moitiés, communication par commandes Tauri (`invoke()`) :

- **Backend Rust** (`src-tauri/src/lib.rs`, ~2 200 lignes) : lecture/écriture des `.md`, `walkdir` pour l'arborescence, `notify 6` pour le watching, `tantivy 0.22` pour la recherche, plugins Tauri `fs`/`dialog`/`opener`/`os`/`updater`/`clipboard-manager`/`single-instance`, `tokio` pour l'asynchrone, `chrono`, `regex`, `base64`.
- **Frontend TypeScript/React 19** (`src/`, ~516 Ko de sources) : un seul `Editor.tsx` de 2 141 lignes porte l'essentiel de l'édition ; contexte `NotesContext` (données + actions séparés) et `ThemeContext` (thème, typographie, direction du texte, largeur de page) ; Radix UI pour les primitives, `sonner` pour les toasts, `tippy.js` pour les infobulles.

**Modèle de données** : pas de base de données. Les notes sont des fichiers `.md` dans un dossier ; le nom de fichier dérive du titre (sanitisé) ; le premier `# Titre` sert de titre affiché. Un `config.json` dans `app_data` mémorise le dernier dossier, un `settings.json` les réglages par dossier (dans `<dossier>/.verso/settings.json`).

**Cycle de vie d'une note** : `list_notes` → `read_note` → édition en mémoire → `save_note` (délai de 300 ms) → réindexation Tantivy → événement `file-change` vers le frontend pour les modifications externes.

### 2.2 L'éditeur riche : Tiptap v3 + `@tiptap/markdown`

C'est le point le plus instructif de Verso pour nous.

- **WYSIWYG** : Tiptap v3 (donc **ProseMirror**) avec `StarterKit` (titres 1-6), `CodeBlockLowlight` (coloration des blocs de code via `lowlight`), `Link`, `Image`, `TaskList`/`TaskItem` (imbriqués), `TableKit`, `HighlightMark`, `Frontmatter` (extensions maison), `CodeBlockLowlight` désactivé dans StarterKit, `Markdown.configure({})`.
- **Le module markdown de Tiptap v3 est le cœur de la conversion** : la sérialisation passe par le *manager* de l'extension Markdown :
  `manager.serialize(editor.getJSON())` → chaîne Markdown ; le post-traitement consiste à remplacer les `&nbsp;` insérés par Tiptap et à reconvertir les URLs d'assets Tauri en chemins relatifs.
- **Mode source** : CodeMirror 6 avec `@codemirror/lang-markdown`, `@codemirror/language-data` (grammaires Lezer pour la coloration des blocs de code) et une extension Lezer **maison** pour la syntaxe `==surlignage==` (`MarkdownConfig.parseInline` personnalisé). Le style de coloration est un `HighlightStyle` minimaliste piloté par variables CSS.
- **Transition source ↔ WYSIWYG** (bouton `Cmd+Shift+M`) : à l'entrée en mode source, le Markdown est sérialisé depuis le document ProseMirror ; à la sortie, le texte source est re-parsé par Tiptap (`editor.commands.setContent`). C'est une **conversion de document à document, avec perte possible** (position du curseur non conservée, idempotence non garantie), et non une vue « live » du même texte.

**Ce que ça nous dit** : Verso a choisi le modèle *« document riche ↔ Markdown »* (un seul document ProseMirror, deux représentations). C'est exactement le modèle de l'**onglet « Lecture »** d'Obsidian, et il est **destructif pour la source** : aller-retour source → riche → source peut reformater le fichier. Notre exigence (Obsidian-like, fichiers .md non destructifs) interdit ce modèle pour l'édition.

### 2.3 Recherche, fichiers, assets, thèmes

- **Recherche** : Tantivy 0.22, schéma `id` (STRING|STORED), `title` (TEXT|STORED), `content` (TEXT|STORED), `modified` (i64 INDEXED|STORED) ; `QueryParser` sur title+content ; **repli sur requête préfixe** (`requête*`) si le parsing échoue ; top 20 résultats avec score ; **repli en mémoire** (titre + aperçu) si Tantivy échoue ; index **par dossier**, rangé dans `app_data/search_indexes/<hash du chemin>`. `IndexWriter` ouvert avec un budget de **50 Mo** (`writer(50_000_000)`).
- **Watching** : `notify` (inotify) avec debounce **500 ms par fichier**, carte de debounce purgée après 5 s ; les événements sont poussés au frontend qui ignore la note en cours d'édition pour éviter les conflits d'écriture.
- **Assets/images** : un dossier `assets/` est créé à la demande dans le dossier des notes ; les images collées ou insérées y sont écrites et **référencées par chemin relatif** (`assets/xxx.png`) — donc Markdown portable. Tauri est compilé avec la feature `protocol-asset` pour servir les fichiers locaux.
- **Thèmes** : clair/sombre/système ; bascule d'une classe `.dark` sur `<html>` ; le mode système écoute `matchMedia("(prefers-color-scheme: dark)")` ; Tailwind v4 avec `@custom-variant dark` ; ~59 variables CSS (`--color-text`, `--color-text-muted`, `--editor-base-font-size`…) ; réglages de typographie (police, taille, interligne, graisse) injectés en variables CSS.

### 2.4 Coût mémoire rapporté : **absent**

C'est un point à assumer franchement :

- Le site dit seulement : *« Uses less memory than a browser tab »* (page « Philosophy », <https://verso.imzl.com/>). **Aucune mesure, aucun contexte** (build, taille du vault, process ou arbre de processus).
- Recherche `memory` dans les issues du dépôt : **0 résultat** (10 issues ouvertes au total : n°1 « lOpening a large Markdown file lags », n°2/n°4 redimensionnement de fenêtre, n°3/n°9 images, n°5 fenêtre fantôme, n°6 largeur, n°7 lecteur réseau, n°10/§11 titres/noms de fichiers).
- Aucun benchmark mémoire dans le dépôt (pas de `bench/`, pas de section « performance » du CLAUDE.md : celui-ci ne mentionne que des *debounces* — 300 ms autosave, 150 ms recherche, 500 ms watcher, 1000 ms git).

**Conclusion Verso sur la RAM** : **aucun chiffre published**. Le seul coût mémoire *lisible dans le code* est le budget d'écriture Tantivy de 50 Mo, qui est une RAM allouée par l'indexeur, pas par le vault. Il ne faut pas recopier ce réglage tel quel dans notre budget.

### 2.5 Licence GPL-3.0 — conséquences concrètes pour nous

- Ce que nous pouvons faire : **étudier** l'architecture, lire le code pour comprendre une technique, s'inspirer des choix (Tiptap pour le WYSIWYG, Tantivy pour la recherche, `assets/` pour les pièces jointes, convention du dossier de configuration). Les *idées* et les *choix d'architecture* ne sont pas protégés par le droit d'auteur ; le README et la documentation de Verso sont utilisables comme sources de citation.
- Ce que nous ne pouvons pas faire : **copier** du code Rust, du TypeScript/React, du CSS, ni transposer un fichier de Verso dans notre projet en « réécriture » — une traduction/adaptation reste une œuvre dérivée GPL-3.0. Transposer une expression régulière, une fonction de parsing ou un bloc CSS de Verso dans `obsy` (MIT) contaminerait notre licence.
- Ce qui est **sans risque** : utiliser directement les **bibliothèques** que Verso utilise, car leurs licences sont indépendantes — Tiptap (MIT), CodeMirror 6 (MIT), Tantivy (Apache-2.0), Tauri (MIT/Apache-2.0), React (MIT), lowlight (MIT). Nous n'héritons d'aucune obligation GPL en consommant Tiptap ou Tantivy.
- **Règle de conduite proposée** : Verso reste une **source de lecture**. Toute technique transposée est réécrite à partir de la **documentation publique de la bibliothèque** (Tiptap/CM6/Tantivy), pas à partir du code de Verso. En cas de doute : on ne transpose pas, on réinvente.

---

## 3. SilverBullet — base de connaissances Rust + client web (MIT)

Sources : dépôt <https://github.com/silverbulletmd/silverbullet> (MIT, 6 364 étoiles), docs <https://docs.silverbullet.md/>, blog <https://no.silverbullet.md/>, forum <https://community.silverbullet.md/>.

### 3.1 Architecture — attention, la formulation courante est fausse

La description « SilverBullet = Rust + Tauri » est **inexacte** pour la version open source. Dans le dépôt OSS (`main`, commit du 2026-10-21) :

- **Aucun fichier Tauri** (recherche `*tauri*` dans l'arbre complet : zéro résultat).
- Le dépôt contient : `client/` (TypeScript, ~109 entrées), `server/`, `server-common/`, `server-runtime-chrome/` (Chrome headless pour exécuter Space Lua côté serveur), `server-search/`, `server-merge/`, `plugs/`, `libraries/`, `bin/`, `bench/`.
- **Serveur** : Rust (workspace Cargo) ; sert le bundle client, l'API HTTP du space, l'authentification ; le client est un **navigateur** (PWA) qui parle au serveur.
- **Client** : TypeScript + **Preact** + **CodeMirror 6** + **ESBuild** ; pas de React, pas de Tailwind, pas de Tauri.
- **SilverBullet Desktop** (l'app native) est un **produit commercial** (gratuit en perso, payant en pro) : son shell **n'est pas dans le dépôt OSS**. Il a été construit sur **Tauri v2** (versions 2.0→2.11) puis **basculé sur Electron en 2.12** (<https://no.silverbullet.md/2-12>). Seuls le *backend Rust* et le *client web* sont open source.

**Modèle de données** : un *space* est un **dossier de fichiers Markdown** sur disque ; le serveur l'expose via une API HTTP ; le client synchronise (service worker) et peut se synchroniser avec un serveur distant ; l'index (recherche, tags, liens, objets) est construit côté serveur et servi au client ; Space Lua permet de programmer des commandes, templates et widgets ; `space-style` permet de styler l'UI depuis le space.

### 3.2 L'éditeur : *live preview* CodeMirror, pas de WYSIWYG

C'est l'autre grande architecture, et elle est **très différente** de celle de Verso — et beaucoup plus proche de ce que fait Obsidian.

- **Pas de ProseMirror, pas de Tiptap.** L'éditeur est CodeMirror 6, et le Markdown est parsé par **Lezer** (`@lezer/markdown`), *étendu* par SilverBullet avec ses propres types de nœuds : `WikiLink` (avec page/alias/dimensions), `@mention`, tags, footnotes, tables, listes de tâches, frontmatter, blocs HTML, marqueurs de conflit, et surtout la **syntaxe personnalisée** (définie par l'utilisateur) et les **directives Lua**.
- **Le *live preview* est fait par des décorations CodeMirror** : des `StateField` calculent des `DecorationSet` et remplacent des portions de source par des `WidgetType` — par exemple le frontmatter est remplacé par un widget de résumé cliquable (`client/codemirror/syntax/frontmatter_preview.ts`), un wikilink devient un widget qui affiche l'état de la cible (fichier manquant / ambigu / valide), les blocs de code ont un widget de copie, les blocs `iframe`/HTML/Lua ont leurs widgets. Le source reste la vérité ; le rendu est une **surcouche visuelle**.
- **Deux parseurs Markdown coexistent** : (1) Lezer, pour l'édition en direct ; (2) un parseur maison `client/markdown_parser/parse_tree.ts` (~8 500 lignes au total sur le dossier parser+renderer) qui produit un arbre `ParseTree`, utilisé par `client/markdown_renderer/` pour le rendu HTML complet (transclusions, widgets, liens, sanitation HTML, tables justifiées, slots).
- **Thèmes** : SCSS (`client/styles/`, ~28 fichiers) + variables + `space-style` (le CSS du space est appliqué par priorité). C'est le degré de personnalisation le plus abouti des deux projets.

**Ce que ça nous dit** : SilverBullet a fait le chemin inverse de Verso — *« le source reste la vérité, on masque la syntaxe et on rend par-dessus »* — qui est précisément le mode **Live Preview** d'Obsidian. C'est l'approche qui préserve les fichiers `.md` (contrainte que Verso s'autorise à ne pas respecter).

### 3.3 Recherche

- `server-search/` : **Tantivy** (index du space) ; feature optionnelle `semantic` based on **candle-core / candle-nn / tokenizers** (embeddings + recherche sémantique), activable via `default = ["semantic"]`.
- Le 2.1 a ajouté des options pour réduire le coût : `config.set("index.search.enabled", false)` (désactive la recherche plein texte), `config.set("index.paragraph.all", false)` (n'indexe pas les paragraphes non taggués) — la doc parle explicitement de « processing and storage » économisés.
- **Pour notre budget** : la recherche sémantique (candle) est **hors budget** (modèle ML embarqué). Le plain Tantivy reste envisageable, mais notre `src/vault/search.rs` fait déjà un index inversé **en mémoire** (`FullTextIndex`, `build_text_index`, `search_text`) — à garder tant que le vault reste sous quelques milliers de notes.

### 3.4 Coût mémoire rapporté — ce qui existe, et ce qui n'existe pas

**Serveur (mesuré, tierce partie)** — une mesure publiée dans le fil de sortie de la 2.1 (<https://community.silverbullet.md/t/silverbullet-2-1-released/3265>, message de Xiè Chén-Zhú du 2025-10-05, sur fly.io) :

| | 2.0.0 (Deno) | 2.1.7 (Go, base Alpine) |
|---|---|---|
| Image Docker | 121 Mo | 21 Mo (−82,6 %) |
| **Mémoire serveur** | **158 Mo** | **70 Mo** (−55,7 %) |

Et l'annonce de la 2.1 elle-même : *« The server has been rewritten in Go (previously Deno). This uses significantly less memory (think: 10x) »* — la « 10x » est une **estimation de l'auteur**, la 158→70 Mo est la **mesure**.

Autre mesure isolée (2024, conteneur Docker de la v1) : `117.3MiB / 955.6MiB` de RAM pour le conteneur SilverBullet (post « Would 1gb ram suffice? », <https://community.silverbullet.md/t/would-1gb-ram-suffice/1152>). La documentation d'installation indique : *« the server needs very limited resources (a few hundred megabytes of RAM is sufficient) »* (<https://github.com/silverbulletmd/silverbullet/blob/main/docs/Install.md>).

**Serveur (2026, passage au Rust)** : l author's blog « On Tech Stacks » (2026-06-11) dit seulement *« Likely memory use is slightly less too »* — **pas de chiffre**. Le gain asserted porte sur la taille des binaires et la disparition du GC Go.

**Client desktop** : **aucun chiffre publié**. Ce qui existe, c'est un retour d'expérience très argumenté (à citer tel quel) :

> *« Resource use — while the download and app size of a Tauri app is significantly smaller, its memory usage is actually not. If you have a single window open it's probably a bit cheaper, but personally I have often 2-5 windows open and each window uses a lot of additional RAM. With Electron it's easier to share these resources so the memory usage can actually be cut down to be lower. »*
> — Zef Hemel, « SilverBullet 2.12 », <https://no.silverbullet.md/2-12>

Dans le même fil : *« Supporting multiple browser engines remains a bit of a pain… Windows got the best SilverBullet+ experience because their browser engine is Edge (Chrome), everybody else gets the slower Webkit »*, et *« Linux — no offense — is a pain in the buttocks to support… I shipped with a bunch of hacks to disable GPU based rendering »*.

Et la formulation d'appoint du même auteur sur les apps Electron : *« each weighing in about half a gigabyte of diskspace as that same amount in memory use »* — **énoncé d'auteur, pas une mesure**.

**Conclusion SilverBullet sur la RAM** : côté **serveur**, on a des chiffres réels (158 → 70 Mo, 117 Mo en conteneur). Côté **client desktop** (la partie qui nous intéresse : la fenêtre), **aucune mesure n'est publiée** — ni pour Tauri, ni pour Electron. Le seul enseignement structurant est **qualitatif** : le passage de Tauri à Electron a été décidé par l'auteur pour des raisons de stabilité multi-fenêtre et de développement, et il constate que **Tauri n'a pas tenu sa promesse mémoire**.

### 3.5 Licence MIT — conséquences

- `LICENSE.md` : MIT. Réutilisation, modification, redistribution autorisées, avec **conservation de l'avis de droit** ; aucune obligation de rendre les modifications (copyleft absent).
- Notre projet est **clean-room vis-à-vis d'Obsidian**, mais rien n'interdit d'utiliser du code MIT : on peut même porter une partie du client SilverBullet (MIT) dans notre projet, à condition de garder l'avis de droit.
- **Réserve de la règle maison** : notre architecture vise un **moteur Rust + UI HTML/CSS** ; le client SilverBullet est TypeScript/Preact. Le «porting » serait de toute façon une réécriture. Recommandation : s'inspirer des **techniques** (§6) et, si on veut réutiliser du code, le faire **explicitement** et **attribué**, dans un fichier dédié et signalé dans l'historique — pas de « fusion silencieuse » de code tiers dans le cœur du moteur.

### 3.6 Ce que SilverBullet n'est pas (à ne pas copier comme architecture)

- L'architecture « serveur HTTP + client web » est **incompatible** avec notre objectif *application locale sans daemon* (nous n'avons pas de serveur, et le SHIM-SPEC de notre projet suppose un échange direct page ↔ moteur).
- Le moteur Space Lua, le system de plugs, la sync collaborative, la recherche sémantique : **hors périmètre** pour nous.

---

## 4. Notre cas : (a) source colorée vs (b) WYSIWYG

### 4.1 Ce que nous avons déjà (à ne pas jeter)

| Brique | Fichier | État |
|---|---|---|
| Rendu Markdown en segments stylés | `src/vault/live_preview.rs` (1 560 l.) | `SegmentStyle` : **37 variantes** (titres 1-6, gras, italique, gras-italique, barré, code inline, bloc de code, citation, lien, URL nue, wikilink, tag, cases à cocher, puces, numéros, règle, tableau, footnote, callout, callout-titre, maths inline/display, embed, embed image, embed PDF) |
| Fragmentation de la source | `src/vault/editor.rs` (1 374 l.) | `SourceKind` : **18 variantes** ; `colorize_source` garantit des fragments **ordonnés et contigus** (la concaténation redonne la source à l'identique) et réutilise les mêmes détecteurs que `live_preview` |
| Rendu de la source colorée | `src/ui/markdown.rs` (183 l.) | `token_color(kind)` → `StyledSpan` → widget `iced::widget::text::Rich` |
| Thèmes | `src/ui/theme_vars.rs` | Variables CSS analysées depuis les thèmes Obsidian + CSS snippets |
| Index de recherche en mémoire | `src/vault/search.rs` | `FullTextIndex` (inversé, en mémoire) |
| Pont page ↔ moteur | `src/engine/` (`api.rs`, `http.rs`, `sync.rs`, `json.rs`) | Déjà en place pour la voie webview/shim |

Autrement dit : **notre moteur sait déjà produire les deux formes** (source fragmentée, rendu en segments). Ce qui manque, c'est le *transport* vers une UI web et le *retour* de l'édition.

### 4.2 Option (a) — source colorée, façon « Live Preview » d'Obsidian

**Principe** : le document affiché est la source ; la coloration vient de `colorize_source` ; le curseur et les frappes restent dans le texte.

- **Coût RAM** : côté webview, c'est le plancher de notre banc (une page HTML minimale). Si on écrit **notre** coloration côté moteur Rust (on a déjà les 18 `SourceKind` → 37 `SegmentStyle`), on n'embarque **ni CodeMirror, ni ProseMirror, ni Tiptap** dans la page : le JS de la page peut se limiter à un `contenteditable`/`textarea` + un peu de glue. Coût marginal : ~0 Mo côté webview au-delà du webview lui-même.
- **Fidélité Obsidian** : le mode Live Preview d'Obsidian **masque** la syntaxe au survol/focus. Une simple source colorée ne masque rien → c'est le mode **« Source »** d'Obsidian, pas le Live Preview. Pour masquer, il faut un surlignage « inline » (le bloc sous le curseur est rendu, le reste reste source) : c'est faisable **sans ProseMirror**, en renvoyant du moteur la **plage de lignes à rendre** (on a déjà `Segment`/`SegmentStyle`).
- **Briques manquantes** : (1) mapping `SegmentStyle` → classes CSS ; (2) protocole incrémental de mise à jour (ligne/plage modifiée) ; (3) mapping position curseur ↔ offset source (on a `cursor_line_col`, `occurrence_line_col` dans `editor.rs`) ; (4) navigation des wikilinks (on a `src/vault/wikilinks.rs`) ; (5) gestion du focus « ligne courante rendue ».
- **Risques** : l'édition dans une zone rendue (ex. tableau ou liste) demande de re-sérialiser ; l'aller-retour peut normaliser le Markdown. → **on ne réécrit pas la source** : on n'édite que dans la source, et on rend à côté.

### 4.3 Option (b) — WYSIWYG (document riche éditable)

**Deux archétypes, et ils ne coûtent pas pareil** :

- **B1 — « Document ProseMirror » (archétype Verso / Tiptap)** : un document riche est la vérité ; le Markdown est une **projection**. Pour l'éditer, il faut ProseMirror (JS) ; le moteur Rust devient un simple (de)serializeur. Coût : la page embarque ProseMirror + Tiptap (~200-400 Ko de JS minifié, non mesuré chez nous) **et** on introduit une **double vérité** (arbre JS ↔ moteur Rust) avec un risque de dérive de formatage. **Et** le modèle n'est pas destructif-secure : le Markdown peut être réécrit à l'enregistrement.
- **B2 — « Rendu HTML éditable » (le vrai « Lecture » d'Obsidian)** : un document HTML (ou notre `Segment` sérialisé en HTML) est éditable dans la page ; à l'enregistrement, on **re-sérialise en Markdown**. Coût : il faut un **sérialiseur Markdown↔HTML/segments sans perte** — c'est le vrai travail, et il est **côté moteur Rust** (notre `live_preview.rs` rend bien, mais ne sérialise pas). C'est faisable chez nous (on a déjà 37 styles de segment), mais c'est un chantier de plusieurs semaines avec un risque de round-trip imparfait.

**Verdict pour notre budget** :

- **(a) est réalisable aujourd'hui** : nos briques existent, le surcoût RAM est quasi nul (côté moteur : on réutilise `colorize_source` ; côté rendu : un widget de texte, que ce soit `iced` ou une page web), la fidélité « Obsidian » est bonne en mode Source et *approximable* en Live Preview (masquage de la syntaxe sur la ligne courante).
- **(b1) est hors budget de complexité** (double vérité, réécriture possible du fichier, dépendance JS lourde) **et** hors budget RAM (ajoute un framework d'édition JS à un webview déjà mesuré à 395 Mo PSS / 576 Mo RSS pour CodeMirror 6 sur WebKitGTK — banc interne, cf. `docs/BENCHMARKS.md`).
- **(b2) est la seule voie « fidèle » à moyen terme**, mais elle exige un **round-trip Markdown↔segments sans perte**, qui est notre brique manquante n°1.

**Recommandation** : **construire (a) d'abord, en gardant (b2) comme evolve**. Concrètement : le moteur produit deux flux — (i) les 18/37 tokens de coloration, (ii) un rendu segmenté des plages « rendues » (ligne courante, blocs focus) — et la zone éditable reste du texte. Le passage à (b2) consistera à remplacer la zone rendue par du contenu riche éditable (HTML en webview, ou `iced::widget::text::Rich` + `Canvas` en natif) en conservant le même protocole de segments ; le moteur aura alors besoin d'un sérialiseur Markdown→segments→Markdown (aller-retour) et d'un « mapping offset↔segment ».

**Le piège à documenter** : la décision de runtime. SilverBullet, qui avait choisi Tauri, l'a **abandonné pour Electron** en 2.12 en invoquant la stabilité multi-fenêtre et en constatant que le gain mémoire de Tauri n'était pas au rendez-vous. Nos propres mesures vont dans le même sens, et plus loin :

| Banc interne (21/10/2026, `docs/BENCHMARKS.md`) | PSS | RSS |
|---|---|---|
| UI native iced + tiny-skia, release, page Markdown réelle | **22,0 Mo** | 22,5 Mo (1 process) |
| Tauri 2 + WebKitGTK, **page HTML vide** | **286 Mo** | 467 Mo (3 process) |
| Tauri 2 + WebKitGTK + CodeMirror 6, page éditeur réelle | **395 Mo** | 576 Mo (arbre complet) |
| Chromium, page HTML vide | — | 843 Mo cumulés (7 process) |

Autrement dit : **le webview est le coût, pas le framework d'édition** — WebKitGTK seul vaut 286 Mo PSS pour une page *vide*. Notre conclusion de banc est déjà posée : *« une UI web coûte plus cher qu'Electron lui-même ; le budget < 150 Mo exclut donc les webviews »* (`docs/BENCHMARKS.md`). **Cela ne condamne pas (a) ni (b) : cela condamne l'hypothèse « notre UI en HTML/CSS rendue par un webview »** — les options (a)/(b) sont aussi réalisables en rendu natif, puisque nos briques de coloration/segmentation sont **côté moteur Rust**, indépendantes du moteur de rendu. Ce qui change si on reste natif : le « masquage de la syntaxe » et les widgets se font en `iced` (textures, `Canvas`), pas en DOM — donc chaque technique de §5.1 a un coût d'implémentation supérieur en natif qu'en web, mais un coût RAM nul.

---

## 5. Techniques transposables

### 5.1 Éditeur et rendu

| Technique | Origine | Ce qu'elle coûte chez nous |
|---|---|---|
| **Masquage de la syntaxe par décorations** (remplacer une plage de source par un widget) | SilverBullet (CM6) | **Élevée** : protocole de plages + curseur ; mais ré-utilisable pour l'aperçu en ligne et les callouts. Évite ProseMirror. |
| **Double parseur** (rapide pour l'édition, complet pour le rendu) | SilverBullet (Lezer + parseur maison) | **Moyenne** : notre `editor.rs` (source) et `live_preview.rs` (rendu) sont déjà deux passes cohérentes — l'architecture existe, il faut la Delta entre les deux. |
| **Document ↔ Markdown via une bibliothèque d'édition** (Tiptap `@tiptap/markdown`) | Verso | **Élevée** + risque : round-trip non garanti, double vérité, réécriture du fichier. MIT, donc utilisable legally, mais pas dans notre budget. |
| **Mode source = un second éditeur léger, pas une conversion** | Verso (CodeMirror à côté du Tiptap) | **Moyenne** : si on fait (a), pas de second éditeur — un seul éditeur texte, et le rendu est en surcouche. |
| **Coloration par tokens, style par variables CSS** | Verso (HighlightStyle sur variables) | **Faible** : on a déjà `token_color` ; il suffit d'émettre des classes CSS au lieu de couleurs RGB. |
| **Rendu des blocs de code avec une coloration à la demande** | Verso (`lowlight`) / SilverBullet | **Moyenne** : coloration paresseuse au scroll ; notre moteur a déjà `math.rs`/blocs de code, il manque le coloriseur. |

### 5.2 Backend, recherche, fichiers, thèmes

| Technique | Origine | Ce qu'elle coûte chez nous |
|---|---|---|
| **Index Tantivy par dossier, sur disque** | Verso (0.22) | **Moyenne** : +~10-15 Mo de RAM si on réduit le budget d'écriture (Verso a mis 50 Mo — **à ne pas reprendre tel quel**), + un crate Apache-2.0 ; à n'envisager qu'au-delà de quelques milliers de notes (notre `FullTextIndex` en mémoire suffit en dessous). |
| **Repli de recherche si l'index échoue** | Verso | **Faible** : on a déjà un fallback possible ; bon filet de sécurité. |
| **Watching avec debounce par fichier + suppression des propres écritures** | Verso (`notify`, 500 ms) | **Faible, mais absent chez nous** : le crate `notify` **ne figure pas** dans notre `Cargo.toml` (aucun watching de fichiers today) ; ~1 journée si l'on veut l'outside-edit. |
| **Assets : dossier `assets/` + chemins relatifs dans le Markdown** | Verso | **Faible** : on a déjà `src/vault/attachments.rs` ; l'important est de garder des chemins **relatifs** pour la portabilité. |
| **Thèmes par variables CSS + classe `.dark` + `prefers-color-scheme`** | Verso / SilverBullet | **Faible** : on a déjà `theme_vars.rs` ; côté webview, on émet le même jeu de variables. |
| **Recherche sémantique (embeddings)** | SilverBullet (candle) | **Interdite** pour nous : poids du modèle >> budget RAM. |
| **Indexer seulement ce qui est utile** (`index.search.enabled`, `index.paragraph.all`) | SilverBullet | **Faible** : knobs utiles si on introduit un index. |

---

## 6. Les 3 questions à trancher avant de coder

**Q1 — Quel est le contrat d'édition : le `.md` est-il intouchable, ou peut-il être réécrit par l'éditeur ?**
→ C'est la question qui sépare (a) de (b1). Si la réponse est « le fichier ne doit jamais être réformatté par l'UI », **Tiptap/ProseMirror est écarté d'emblée** et l'éditeur reste un éditeur de texte avec rendu en surcouche (archétype SilverBullet/Obsidian Live Preview). Si la réponse est « la forme exacte du fichier peut changer », l'archétype Verso (document ↔ Markdown) devient possible — mais alors la round-trip doit être vérifiée sur des corpus réels.

**Q2 — Quelle fidélité exacte du mode « Live Preview » d'Obsidian est visée, et est-elle atteignable sans ProseMirror ?**
→ Spécifier le niveau 0/1/2 : **(0)** source colorée (= mode Source d'Obsidian, nous l'avons déjà) ; **(1)** la **ligne courante** est rendue, le reste reste source (Live Preview d'Obsidian, atteignable par décorations, comme SilverBullet) ; **(2)** blocs entiers rendus (tableaux, callouts, maths) en permanence. Le niveau 1 exige un protocole de plages + une gestion fine du curseur ; le niveau 2 exige en plus un **round-trip** (option (b2)). **Chiffrer ce niveau avant d'estimer le travail** — c'est lui qui décide si on fait de l'architecture.

**Q3 — Le runtime : webview ou rendu natif, et quel budget RAM est-on prêt à défendre ?**
→ Rappel de mesure interne (`docs/BENCHMARKS.md`, 21/10/2026) : UI native iced+tiny-skia **22,0 Mo** (1 process) ; Tauri 2 + WebKitGTK **286 Mo PSS pour une page HTML vide**, 395 Mo PSS / 576 Mo RSS avec CodeMirror 6 ; Chromium 843 Mo cumulés. Rappel de terrain : SilverBullet a **quitté Tauri pour Electron** en 2.12 en constatant que le gain mémoire de Tauri ne se matérialisait pas en multi-fenêtre (<https://no.silverbullet.md/2-12>).
Trois options, et il faut en choisir une **avant** d'écrire du code d'éditeur :
**(i)** **rester natif** (iced + tiny-skia) et retravailler le design — 22 Mo, zéro webview, et nos briques (coloration, segments) sont déjà en place : c'est l'option que les chiffres soutiennent ;
**(ii)** **webview** (Tauri/WebKitGTK ou Chromium) + **notre** coloration sans framework JS d'édition — coût = le webview seul (286 Mo PSS mini) : incompatible avec « < 150 Mo », compatible avec un budget « plutôt < 150 Mo mais on cède si l'UI est fidèle » ;
**(iii)** webview + framework d'édition (Tiptap/CM6) — on paie le webview *et* le framework, et on réintroduit une double vérité (arbre JS ↔ moteur Rust) : le pire des deux mondes pour nous.
**Question de formulation à trancher** : « < 150 Mo » est-il une **contrainte dure** (→ (i) est la seule réponse) ou un **objectif** que l'UI peut faire dépasser (→ (ii), en assumant ~300-400 Mo, est défendable) ? C'est cette réponse, et non un choix technique, qui décide de tout le reste.

---

## 7. Sources

**Verso**
- Site : <https://verso.imzl.com/> (features, « Philosophy », claim mémoire non mesuré)
- Dépôt : <https://github.com/zhoulianglen/verso> (branche `master`) — `CLAUDE.md` (guide de développement : stack, commandes Tauri, schéma de recherche, debounces), `package.json` (Tiptap 3, CM6, Radix, Tailwind 4), `src-tauri/Cargo.toml` (Tauri 2, tantivy 0.22, notify 6, tokio), `src-tauri/src/lib.rs` (index Tantivy, watching, assets, commandes), `src/components/editor/Editor.tsx` (Tiptap + bascule source), `src/components/editor/MarkdownSource.tsx` (CM6 + extension Lezer `==highlight==`)
- Licence : `LICENSE` (GPL-3.0) ; `src-tauri/Cargo.toml` : `license = "GPL-3.0-only"`
- Issues (10, aucune sur la mémoire) : <https://github.com/zhoulianglen/verso/issues>

**SilverBullet**
- Dépôt : <https://github.com/silverbulletmd/silverbullet> (`main`) — `README.md` (client TS/CM6/Preact + ESBuild, serveur Rust, crates `server/`, `server-common/`, `server-runtime-chrome/`), `client/markdown_parser/` (parseur Lezer étendu + `parse_tree.ts`), `client/markdown_renderer/` (rendu HTML), `client/codemirror/syntax/` (décorations live preview : frontmatter, wikilinks, tables, tasks…), `client/styles/` (SCSS), `server-search/Cargo.toml` (Tantivy + candle/tokennizers optionnels), `LICENSE.md` (MIT)
- Docs : <https://docs.silverbullet.md/> et <https://docs.silverbullet.md/Install> (« a few hundred megabytes of RAM is sufficient »)
- Blog : « SilverBullet 2.12: Growing With You » (<https://no.silverbullet.md/2-12>) — **Tauri → Electron**, « its memory usage is actually not [lower] » ; « On Tech Stacks » (<https://no.silverbullet.md/tech-stacks>) — passage Deno→Go→Rust, Electron ≈ 500 Mo, Tauri = webview système
- Forum : sortie 2.1 et **mesure 158 → 70 Mo** (<https://community.silverbullet.md/t/silverbullet-2-1-released/3265>), conteneur 117,3 MiB (<https://community.silverbullet.md/t/would-1gb-ram-suffice/1152>), tenue à l'échelle 5k/25k notes (<https://community.silverbullet.md/t/speed-usability-issues-at-scale-with-thousands-of-notes/4413>), annonce SilverBullet+ (<https://community.silverbullet.md/t/announcing-silverbullet-beta-silverbullet-desktop-edition/4037>)

**Mesures internes au projet (rappel, non re-mesurées ici)**
- `docs/BENCHMARKS.md` (21/10/2026) : iced+tiny-skia **release 22,0 Mo RSS** (22,5 Mo PSS, 1 process, 10 threads, binaire 12,8 Mo, mesuré sur `pour-omp`) ; debug 39,6 Mo ; debug + vault 5 000 notes 70,7 Mo ; **Tauri 2 + WebKitGTK, page HTML vide : 286 Mo PSS / 467 Mo RSS** (87,6 Mo Tauri + 176 Mo `WebKitWebProcess` + 29 Mo réseau) ; **Tauri 2 + WebKitGTK + CodeMirror 6 : 395 Mo PSS / 576 Mo RSS** (arbre complet : 173/341/62 Mo RSS) ; Chromium page vide : 843 Mo RSS cumulés (7 process) ; Electron (Obsidian) 180-250 Mo = **rapport tiers, non mesuré chez nous**.
- La présente étude **n'a produit aucune mesure RAM nouvelle** : aucun binaire de Verso ou SilverBullet n'a été construit ni lancé dans le cadre de cette mission (contrainte « pas de cargo »).

**Ce qui n'a pas été trouvé, et qui reste donc inconnu**
- Verso : **aucune** mesure de mémoire publiée (site, issues, dépôt) ; le claim « moins qu'un onglet de navigateur » n'est étayé par rien.
- SilverBullet : **aucune** mesure de mémoire du **client desktop** (ni Tauri, ni Electron) ; les seuls chiffres disponibles concernent le **serveur** (158 → 70 Mo ; 117 Mo en conteneur) et sont donc **transposables au serveur, pas à la fenêtre**.
