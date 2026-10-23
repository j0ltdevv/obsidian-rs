# Analyse REA ciblée — Obsidian 1.14.4

**Date :** 2026-10-21 · **Méthode :** REA (rea-agents 6.7.0), analyse statique JavaScript/Electron
**Validation :** reverse-engineering officielle validée par le propriétaire (cf. note dans `AGENTS.md`). Règlement clean-room maintenu : *déduction de comportement, aucune ligne copiée*.

## Cible et identité des preuves

| Artefact | SHA-256 / ID |
|---|---|
| `obsidian-1.14.4.asar` (27,4 Mo, ASAR réellement chargé par l'app) | root `146ef8470d8cbb4a6db209bdc19bf2bad13350444186c0fd93c31087b142` |
| Preuve ciblée (app.js + enhance.js seulement) | `ev_7efd1cd556266e00f6db866808e5dba3500922cf303054f3f6de76ac6bce6332` |
| Inventaire | `agm_3d65731ebf232070d80f930f144d0d22d429b491f96ac68ebedb81b7` |
| Fichier local | `demonstrations/obsidian-rea/rea-focused-evidence.json.gz` (9,3 Mo) |

- 2 fichiers analysés sur 38 inventoriés (`--source-path` app.js, enhance.js) ; 1 127 105 nœuds AST visités ; 195 modules webpack ; 22 757 nœuds du graphe sémantique ; 0 échec de parsing.
- Le bundle `app.js` fait ~3,94 Mo sur **une seule ligne minifiée** — identifiants mangled, seules les chaînes littérales survivent.
- Exécution sur la box instabox `hermes` (Node v22.23.3) pour ne pas saturer l'hôte local.

## Observations (faites, static-only)

Occurrences de chaînes littérales dans `app.js` :

| Chaîne | Occurrences |
|---|---|
| `frontmatter` | 130 |
| `metadataCache` | 161 |
| `[[` (littéraux/regex) | 2 033 |
| `hashtag` | 13 |
| `wikilink` | 6 |
| `yaml` | 87 |
| `vault` | 796 |

Clusters d'offsets (fenêtre ±150 car., nœuds lexicaux voisins) — traduction fonctionnelle :

### 1. Frontmatter (charge maximale pour obsy)
Deux familles de zones :
- **zone pipeline markdown** (~763 000 ; ~1 455 000) : nœuds voisins `addTransformer`, `hProperties`, `position`, `children`, `plugins`, `getInfo` → un *transformer* de pipeline remark/unified expose le frontmatter comme métadonnées structurées du document (descripteurs de propriétés, positions d'AST).
- **zone éditeur HyperMD** (~2 143 000) : littéraux `FRONT_MATTER`, `FRONT_MATTER_END`, modalités d'état éditeur (`fallbackMode`, `exitChecker`, `startsWith`) → reconnaissance de la délimitation du bloc frontmatter dans l'éditeur (début/fin), mode de repli.
- **zone décorations CodeMirror** (~1 719 000) : `frontmatterEnd`, `decorations`, `facet` → rendu masqué/plié de la frontière.

### 2. Cache de métadonnées
- Hub massif (~1 718 000–1 830 000, 44 hits) : `vault`, `fileCache`, `mtime`, `trigger`, `switch`, `toLowerCase` → index par fichier avec timestamp, invalidation/rafraîchissement événementiel.
- **Tags** : littéraux `_cachedTags`, `fromFrontMatter`, `getProps`, `_cachedProps`, `getFileCache`, `basename` (~2 208 000) → déduction : les tags d'un fichier agrègent tags *du corps* et tags *issus du frontmatter* (concat), avec cache de propriétés.
- **Résolution de liens** : `getFirstLinkpathDest` (plusieurs zones, notamment ~1 572 000 et ~1 686 000 avec `subpath`, `msgFileOrFolderNotFou...` = message « fichier/dossier non trouvé ») et `fileToLinktext`, `evaluateFormula`, `instanceof`, `concat` (~2 374 000) → les liens se **résolvent** depuis un *linkpath* vers une destination via le cache, le `subpath` porte l'ancre (titre/bloc) ; à l'inverse `fileToLinktext` produit le texte de lien pour un fichier donné.

### 3. YAML
- Concentré dans un seul grand bloc (~654 000–722 583, 76/87 hits) : `items`, `contents`, `offset`, `throw`, `typeof` → **un parseur YAML embarqué** (implémentation autonome, pas une dépendance externe visible) — style js-yaml.
- Un micro-cluster (~12 800) évoque des types de liste mdast (`listItem`, `paragraph`, `strong`, `table`, `thematicBreak`, `definition`, `footnoteDefinition`) → zone parseur-markdown (micromark/mdast).

### 4. Wikilinks (moins direct)
Pas de « cluster parseur wikilinks » sous ce nom : les 6 occurrences sont des zones **UI/commandes** (`insertWikilink`, `mobileToolbarCommands`, réglages `linkStyle`, `showFullPath`, `embed`, `heading`, `attach`).
- Déduction : l'insertion wikilink est une commande d'éditeur ; la syntaxe elle-même est traitée au niveau des parseurs de liens génériques (les 2 033 littéraux `[[` incl. regex) et la résolution passe par `getFirstLinkpathDest`/`subpath` (cf. §2).

### 5. Hashtags
- Zone HyperMD (~2 140 000–2 155 000) : `hmdLinkType`, `formatting`, `quote`, `header`, `inline` → les hashtags sont tokenisés comme des types de lien inline dans l'éditeur, avec formatage.

## Comportements déduits ( ras pour les tests de `src/vault/`)

1. Frontmatter = bloc YAML délimité en tête ; reconnaissable par indices début/fin distincts (états éditeur séparés), exposé ensuite aux métadonnées via un transformer de pipeline.
2. Tags d'une note = union corps (hashtag inline) + frontmatter ; mécanisme explicite de fusion depuis le frontmatter.
3. Index metadata par fichier **avec mtime** et triggers — invalide/enrichit de façon incrémentale (modèle exact de la fonctionnalité backlinks/tags de `obsy`).
4. Résolution de lien : `linkpath` + `subpath` (ancre) → destination ; chemin de revers : fichier → texte de lien. Le message « file or folder not found » est distinct.
5. Wikilink : pas un parseur dédié nommé ; l'UI fournit une commande d'insertion, les réglages portent sur le style d'insertion (embed vs lien, chemin complet vs basename ? non tranché — inconnu).

## Limitations explicites (à ne pas surinterpréter)

- Analyse **statique inerte** : aucune exécution ; les relations dynamiques restent inconnues.
- Seuls 2/38 fichiers analysés ; les relations via les sources non sélectionnées sont inconnues.
- Les frontières webpack repérées (197) couvrent le bootstrap et un segment du bundle ; les zones à features tombent **hors plages granulaires** — la localisation par module n'est pas possible pour le reste du bundle.
- Identifiants mangled : les noms observés sont des littéraux, pas des noms de symboles d'origine.
- Les plages `[[` (2 033) n'ont pas été désambiguïsées (littéraux UI/regex/échappements mélangés).

## État de l'environnement d'analyse

- Box instabox `hermes` : `/data/home/obsidian-rea/` ({`obsidian-1.14.4.asar`, `app.js`, `rea-focused-evidence.json`, `rea-evidence-89mo-partiel.json` (95 Mo partiel du run full + `provider_timeout` sur mermaid.min.js, scripts d'extraction `extract-keypoints.js`, `map-modules.js`, `cluster-summary.js`)}).
- Local : preuve gz + logs ; le vieux JSON tronqué local (142 Mo, sessions fantômes) reste à purger.
- Aucun code de `obsy` modifié par cette analyse ; aucun build/test à repasser (pas de fichier Rust touché).

## Traces sémantiques (post-rapport) — exécutées, résultats

Traces lancées sur la box à partir de la preuve authentifiée (`trace-application-feature` et `trace-javascript-semantics`, REA 6.7.0) ; fichiers JSON de traces conservés côté box (`/data/home/obsidian-rea/sem-sem-*.json`, `trace-module-appjs.json`).

| Trace | Seed | Résultat |
|---|---|---|
| module-level | `module "app.js"` | **OK** : 197 modules matchés, 354 nœuds, 711 arêtes, 73 chemins terminaux (graphe app) |
| seed littéral | `"getFirstLinkpathDest"`, `"fromFrontMatter"`, `"fileCache"`, `"FRONT_MATTER"` (kind string) | `no-match` — le graphe **ne conserve pas les littéraux** ; le no-match ne prouve pas l'absence |
| `Im` (frontmatter transformer) | semantic-node, callers + provenance | `partial` : **1 nœud (seed seul)**, 13 limitations → appelants/récipients inconnus |
| `tg` (parseur YAML) | semantic-node, callers | `partial` : seed seul, relations inconnues |
| fusion tags frontmatter | semantic-node, callers | `partial` : seed seul |
| hub metadataCache | semantic-node, provenance | `partial` : seed seul + **20 000 unknowns** typés `object-flow` (receiver non résolu, ex. lecture de propriété `/charAt` @144997, `ambiguous-target`) |

**Lecture honnête :** les traces **ne levant rien de plus** sur les relations intra-bundle — le código minifié garde ses récepteurs non résolus, donc les relations d'appel autour des zones features restent inconnues de la preuve. Aucune déduction du rapport n'a pu être infirmée ; aucune n'est passée au niveau « observé par graphe ». Le degré de certitude du rapport est inchangé : littéraux **observés**, comportements **déduits** (inférés).

## Suite possible (au choix)

1. **Traces ciblées** : `rea trace-application-feature` sur les seeds `getFirstLinkpathDest` / `fromFrontMatter` pour sécuriser la déduction des comportements de résolution.
2. **Tests clean-room** : écrire les fixtures `tests/fixtures/` frontmatter/tags/links reflétant les comportements §1–4 (déduits, jamais copiés) et les faire passer dans `src/vault/`.
3. **Purge** : supprimer le JSON fantôme local et éventuellement la copie `app.js`/partielle côté box après validation du rapport.
