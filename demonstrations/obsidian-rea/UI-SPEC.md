# UI-SPEC — Spécification visuelle implémentable de l'interface d'Obsidian

**Objet.** Description mesurée de l'apparence de l'interface d'Obsidian 1.14.4,
suffisante pour qu'un implémentateur qui n'a **jamais vu Obsidian** puisse reproduire à
l'identique la structure, les dimensions, les couleurs et les états d'une fenêtre
d'édition.

**Clean-room.** Ce document ne contient **aucune ligne de code, de CSS, de SVG ni de
texte propriétaire** d'Obsidian. Il ne contient que des **mesures** (pixels, ratios,
couleurs résolues) et des **descriptions** de comportement, obtenues en observant
l'application **en fonctionnement** (voir protocole ci-dessous). Les noms de variables
CSS cités sont ceux de l'API de thématisation publique d'Obsidian, déjà référencés par
`src/ui/theme_vars.rs` — ce sont des **identifiants de jeton**, pas du code.

---

## 0. Protocole de mesure (à relire avant de croire une valeur)

| Élément | Valeur |
| --- | --- |
| Application | Obsidian **1.14.4** (build Arch `obsidian` : `/usr/lib/obsidian/app.asar` + ASAR mis à jour `~/.config/obsidian/obsidian-1.14.4.asar`) |
| Moteur | Electron 43, `nodeIntegration` actif (le renderer expose `window.app`) |
| Plateforme | Linux x86-64, thème **sombre par défaut** (`body.theme-dark`), locale **française** |
| Rendu | SwiftShader (ANGLE) sous `Xvfb :99`, 1600×1000 ; **fenêtre 1024×800**, `devicePixelRatio = 1` |
| Vault | coffre de mesure jetable (`/tmp/obsidian-ui-probe/vault`), 6 notes, frontmatter, tags, wikilinks (résolu et non résolu), liste, tâche, citation, code, tableau, callout |
| Méthode | Chrome DevTools Protocol sur le renderer (`Runtime.evaluate`) : `getBoundingClientRect()` + `getComputedStyle()` sur des éléments repérés par classe CSS ; survols et ouvertures de menus simulés par `Input.dispatchMouseEvent` / `app.commands.executeCommandById` |
| Relevé | 2026-10-21, 11 passes de mesure, 980 variables CSS énumérées sur `body` |

**Règle de lecture des sources.** Chaque valeur porte une source :

- **[M]** *mesuré* — relevée sur l'application en fonctionnement (chiffre exact ou valeur
  CSS résolue par le moteur) ;
- **[R]** *dépôt* — reprise telle quelle d'un fichier existant de `obsy`
  (`src/ui/theme_vars.rs`, `src/vault/themes.rs`, `src/vault/editor.rs`) ;
- **[I]** *inféré* — déduit de plusieurs mesures, ou déduit d'une version antérieure ;
  **à revérifier** avant d'en faire une référence.

Les valeurs marquées **[I]** ou **[I/mesuré divergent]** ne doivent pas être traitées
comme des mesures.

### 0.1 Environnement de référence et réserves

- Mesures prises à **DPR = 1**. En HiDPI, Obsidian reste en unités CSS : les mêmes
  valeurs s'appliquent, mais le rendu est plus net. **[M]** pour l'unité, **[I]** pour
  l'absence d'arrondi physique.
- Le coffre de l'utilisateur (`/home/jolt/ddd`) et sa configuration
  (`~/.config/obsidian/obsidian.json`) ont été **restaurés à l'identique** après
  mesure (`diff` vide sur `.obsidian/`, `obsidian.json` remis à sa valeur initiale).
  Aucun fichier d'Obsidian n'a été copié dans le dépôt.
- Les libellés d'interface cités sont ceux de la **locale française** du binaire ;
  Obsidian les traduit, la géométrie ne change pas. **[M]**

---

## 1. Jetons

### 1.1 Couleurs — thème sombre par défaut

Les variables sont déclarées sur `body` (et non `:root`) : un implémentateur doit les
attacher à l'élément racine de l'application. **[M]**

| Jeton | Valeur mesurée | Source |
| --- | --- | --- |
| `--background-primary` | `#1C1C1C` | **[M]** `body` |
| `--background-secondary` | `#282828` | **[M]** (fonds des docks, ruban, barre d'état) |
| `--background-modifier-border` | `#333333` | **[M]** (séparateurs, bordures de bloc de code, de tableau) |
| `--background-modifier-hover` | `color-mix(in oklch, white 6.7%, transparent)` → rendu mesuré `oklch(0.999994 0.0000497986 none / 0.067)` | **[M]** |
| `--background-modifier-border-hover` | `#3f3f3f` | **[M]** (variable) |
| `--background-modifier-border-focus` | `#555555` | **[M]** (variable) |
| `--background-modifier-box-shadow` | `rgba(0, 0, 0, 0.3)` | **[M]** (variable) |
| `--text-normal` | `#dadada` | **[M]** |
| `--text-muted` | `#b3b3b3` | **[M]** (texte d'interface : barre d'état, onglets, arbre) |
| `--text-faint` | `#666666` | **[M]** (icônes inactives, texte de remplacement) |
| `--text-accent` | `hsl(255, 89.8 %, 75.9 %)` → `#A68AF9` | **[M]** |
| `--text-on-accent` | `white` | **[M]** |
| `--accent` / `--interactive-accent` | `hsl(258, 88 %, 66 %)` → `#8A5CF5` | **[M]** |
| `--accent-hover` / `--text-accent-hover` | `hsl(255, 89.8 %, 75.9 %)` → `#A68AF9` | **[M]** |
| `--color-accent-2` | `hsl(253, 92.4 %, 85.1 %)` | **[M]** (variable) |
| `--code-background` | `#232323` | **[M]** (blocs de code **et** code inline) |
| `--code-border-color` | `#333333` | **[M]** |
| `--code-normal` | `#dadada` | **[M]** |
| `--code-comment` / `--code-punctuation` | `#666666` / `#b3b3b3` | **[M]** (variables) |
| `--code-keyword` / `--code-string` / `--code-function` / `--code-number` | `#fa99cd` / `#44cf6e` / `#e0de71` / `#e9973f` | **[M]** (variables) |
| `--divider-color` | `#333333` | **[M]** (variable) |
| `--checkbox-border-color` | `#666666` | **[M]** |
| `--checkbox-color` | `hsl(258, 88 %, 66 %)` | **[M]** |
| `--link-color` / `--link-color-hover` | `#A68AF9` / `hsl(253, 92.4 %, 85.1 %)` | **[M]** |
| `--link-unresolved-opacity` | `0.7` | **[M]** (variable) — le wikilink non résolu est **la même couleur**, en opacité réduite |
| `--tag-background` | accent à 10 % d'opacité | **[M]** |
| `--tag-border-color` | accent à 15 % d'opacité | **[M]** |
| `--menu-background` / `--menu-border-color` | `#282828` / `#3f3f3f` | **[M]** (variables) |
| `--modal-background` | `#1C1C1C` | **[M]** (variable) |
| `--prompt-background` / `--prompt-border-color` | `#1C1C1C` / `#3f3f3f` | **[M]** (variables) |
| `--tooltip-background` / `--tooltip-color` | noir à 90 % / `#FAFAFA` | **[M]** (variables) |
| `--status-bar-background` / `--status-bar-text-color` | `#282828` / `#b3b3b3` | **[M]** (variables) |
| `--ribbon-background` | `#282828` | **[M]** (variable) |
| `--tab-container-background` | `#282828` | **[M]** (variable) |
| `--tab-background-active` | `#1C1C1C` | **[M]** (onglet de note actif) |
| `--tab-text-color` / `--tab-text-color-active` | `#666666` / `#b3b3b3` | **[M]** (variables) |

**Contraste utile :** le fond de l'application est `#1C1C1C` (28,28,28) et celui des
docks `#282828` (40,40,40) : **12 niveaux** d'écart seulement. Les surfaces « secondaries »
se distinguent donc par la *luminosité*, pas par la teinte. **[M]**

### 1.2 Typographie

#### 1.2.1 Polices

| Jeton | Valeur | Source |
| --- | --- | --- |
| `--font-text` | pile système : 3 noms de famille **littéralement `??`** (espaces réservés des polices embarquées, non exploitables) puis `ui-sans-serif, -apple-system, BlinkMacSystemFont, system-ui, "Segoe UI", "Google Sans Flex", Roboto, "Inter Variable", "Inter", "Apple Color Emoji", "Segoe UI Emoji", "Segoe UI Symbol", sans-serif` | **[M]** |
| `--font-interface` | identique à `--font-text` (3 × `??` en tête) | **[M]** |
| `--font-monospace` | 2 × `??` puis `ui-monospace, SFMono-Regular, "Cascadia Mono", "Roboto Mono", "DejaVu Sans Mono", "Liberation Mono", Menlo, Monaco, "Consolas", "Source Code Pro", monospace` | **[M]** |
| Polices réellement chargées par l'app | `Inter` (chargée), `Source Code Pro`, `Flow Circular` | **[M]** `document.fonts` |
| Police effective du texte | **Inter** est installée sur la machine (`fc-match Inter` → `Inter.ttc`), donc la pile retombe dessus | **[M]** + **[I]** (l'ordre exact des 3 entrées `??` est masqué par l'application) |

> **Consigne d'implémentation :** ne pas tenter de deviner les 3 noms masqués. Utiliser
> une pile `Inter, system-ui, -apple-system, "Segoe UI", Roboto, sans-serif` et
> `ui-monospace, "Cascadia Mono", "Roboto Mono", "DejaVu Sans Mono", monospace`. **[I]**

#### 1.2.2 Échelle de tailles

| Jeton | Valeur | Usage observé | Source |
| --- | --- | --- | --- |
| `--font-text-size` | `16px` | corps de note | **[M]** |
| `--font-ui-medium` | `15px` | titres d'onglets, en-tête de vue, suggestions | **[M]** |
| `--font-ui-small` | `13px` | interface (arbre, barre d'état, docks) | **[M]** |
| `--font-ui-smaller` | `12px` | barre d'état, raccourcis | **[M]** |
| `--font-ui-large` | `20px` | (variable) | **[M]** |
| `--font-small` / `--font-smaller` / `--font-smallest` | `0.933em` / `0.875em` / `0.8em` | | **[M]** |
| `--code-size` | `0.875em` (14 px sur un corps 16 px) | code inline et bloc | **[M]** |

**Attention — divergence avec le dépôt.** `src/vault/editor.rs` (`typography`) fixe
`CODE_SIZE_EM = 0.9` et `INTERFACE_FONT_SIZE_PX = 13` ; la valeur mesurée du code est
`0.875em` (14 px), l'interface est à 15 px sur les titres d'onglet. Voir §5.3.

#### 1.2.3 Interlignes et corps

| Contexte | police / interligne | Source |
| --- | --- | --- |
| Corps de note (édition **et** lecture) | `16px / 24px` → ratio **1,5** | **[M]** (`--line-height-normal: 1.5`) |
| Ligne de liste (édition) | `16px / 24px`, `padding-left 21px`, `padding-top/bottom 1.2px` | **[M]** |
| Élément de liste (lecture) | `16px / 24px`, `padding-left 26.7px`, `padding 1.2px 0` | **[M]** |
| Bloc de code | `14px / 21px` (ratio 1,5) | **[M]** |
| Code inline | `14px / 21px`, fond `#232323`, padding `2.1px 4.2px`, rayon 4 px | **[M]** |
| Tableau | `16px / 20.8px` (ratio 1,3) | **[M]** |
| En-tête d'onglet de note | `15px / 19.5px` | **[M]** |
| Élément d'arbre de fichiers | `13px / 16.9px` | **[M]** |
| Barre d'état | `12px / 12px`, `font-variant-numeric: tabular-nums` | **[M]** |
| Suggestion (palette de commandes) | `15px` | **[M]** |
| Jeton de raccourci | `12px` | **[M]** |
| Nom du coffre (profil) | `13px`, graisse **500** | **[M]** |

> Le dépôt (`src/vault/editor.rs`) annonce `LINE_HEIGHT = 1.6` et
> `HEADING_LINE_HEIGHT = 1.4` : **les deux divergent de la mesure** (1,5 mesuré ; 1,2 à
> 1,5 selon le niveau de titre). Voir §5.3.

#### 1.2.4 Titres

Tailles en `em` **relatives au corps 16 px** ; valeurs mesurées en px réels.

| Niveau | taille (em) | px mesuré | interligne (em) | px mesuré | graisse | interlettrage | Source |
| --- | --- | --- | --- | --- | --- | --- | --- |
| H1 | 1,618 | 25,888 | 1,2 | 31,07 | 700 | −0,015 em (−0,388 px) | **[M]** |
| H2 | 1,462 | 23,392 | 1,2 | 28,07 | **680** | −0,011 em (−0,257 px) | **[M]** |
| H3 | 1,318 | 21,088 | 1,3 | 27,41 | **660** | −0,008 em (−0,169 px) | **[M]** |
| H4 | 1,188 | 19,008 | 1,4 | 26,61 | **640** | −0,005 em (−0,095 px) | **[M]** |
| H5 | 1,076 | 17,216 | 1,5 | 25,82 | **620** | −0,002 em (−0,034 px) | **[M]** |
| H6 | 1,0 | 16 | 1,5 | 24 | **600** | 0 | **[M]** |

Points à retenir :

- Les graisses **décroissent de 700 à 600** avec le niveau : le H2 est plus « lourd »
  relatively qu'un H1 à 600. Une pile de polices variable (Inter Variable) rend ces
  graisses intermédiaires ; une pile statique ne les rend pas exactement.
- L'interlettrage est **négatif** et proportionnel : `−0,015 em` au H1 jusqu'à `0` au H6.
- Marges verticales mesurées (lecture) : H1 `margin-bottom 16px` ; H2 `margin-top 40px`,
  `margin-bottom 16px` ; H3→H6 `margin-top 16px` ; paragraphe `margin 16px 0`. **[M]**
- **Titre en ligne** (`inline-title`, le nom de la note affiché comme premier élément du
  corps) : `1.618em` = **25,888 px**, graisse 700, interligne 1,2 (31,07 px),
  `margin-bottom 0.5em` = **12,944 px**. **[M]**

### 1.3 Rayons, bordures, ombres

| Jeton / élément | Valeur | Source |
| --- | --- | --- |
| `--radius-s` | 4 px | **[M]** / **[R]** `theme_vars.rs` |
| `--radius-m` | 8 px | **[M]** / **[R]** |
| `--radius-l` | 12 px | **[M]** / **[R]** |
| `--radius-xl` | 24 px | **[M]** (variable) |
| rayon d'un élément d'arbre | 8 px | **[M]** |
| rayon d'un bouton icône (28×24) | 4 px | **[M]** |
| rayon d'un onglet (inactif) / actif | 4 px / `10px 10px 0 0` | **[M]** |
| rayon de la palette de commandes | 12 px | **[M]** |
| rayon d'un bloc de code | 4 px | **[M]** / **[R]** `CODE_RADIUS_PX` |
| rayon d'une case à cocher | 4 px | **[M]** |
| rayon d'un jeton (tag) | 28 px (`2em`) | **[M]** |
| rayon d'un jeton de raccourci | 24 px (`2em`) | **[M]** |
| rayon de la barre d'état | `8px 0 0 0` (arrondi **seulement en bas à gauche**) | **[M]** |
| rayon d'une propriété (frontmatter) | 6 px | **[M]** |
| bordures | 1 px, couleur `#333333` (tableau, code, propriétés) ; 2 px accent (citation) | **[M]** |
| `--shadow-s` | `0 1px 2px rgba(0,0,0,.121), 0 3.4px 6.7px rgba(0,0,0,.179), 0 15px 30px rgba(0,0,0,.3)` | **[M]** (variable) |
| `--shadow-l` | `0 1.8px 7.3px rgba(0,0,0,.071), 0 6.3px 24.7px rgba(0,0,0,.112), 0 30px 90px rgba(0,0,0,.2)` | **[M]** (variable) |
| ombre d'un champ de saisie | `inset 0 .5px .5px .5px rgba(255,255,255,.09), 0 2px 4px rgba(0,0,0,.15), 0 1px 1.5px rgba(0,0,0,.1), 0 1px 2px rgba(0,0,0,.2)` | **[M]** (variable `--input-shadow`) |
| barre de défilement | largeur **12 px**, rayon 12 px, pouce `white 10 %`, pouce actif `white 20 %` (mesuré : écart `offsetWidth − clientWidth` = 12 sur le scroller de l'éditeur) | **[M]** |

### 1.4 Icônes

| Jeton | Valeur | Source |
| --- | --- | --- |
| `--icon-s` / `--icon-m` | 16 px | **[M]** |
| `--icon-l` | 18 px | **[M]** |
| `--icon-xl` | 32 px | **[M]** |
| `--icon-xs` | 14 px | **[M]** |
| épaisseur de trait (`--icon-stroke`) | 2 px (mesuré sur les SVG du ruban et des onglets de dock) | **[M]** |
| opacité des icônes | 0,85 (1 en survol / actif) | **[M]** |
| couleur des icônes | `#b3b3b3` (inactives), `#dadada` (survol), accent (active) | **[M]** / variables `--icon-color*` |

---

## 2. Gabarits

Une fenêtre mesure **1024×800** dans les conditions de mesure. La structure, de gauche
à droite et de haut en bas :

```
┌───────────────────────────────────────────────────────────────────────────┐
│ [◧]  ┌ barre d'onglets du dock gauche (40 px) ┐┌ barre d'onglets principale ┐│  y=0..40
│ 44px └──────────────────────────────────────┘└────────────────────────────┘│
│  ▣    ┌─ dock gauche (300 px) ─────────────┐┌─ zone principale ──────────┐  │
│  ▣    │ onglets icônes (40 px)              ││ onglets de note (40 px)    │  │
│  ▣    │ en-tête explorateur (40 px)         ││ en-tête de vue (38 px)     │  │
│  ▣    │ arbre de fichiers                   ││ surface d'édition         │  │
│  ▣    │ …                                   ││ (largeur de ligne 700 px)  │  │
│  ▤    ├────────────────────────────────────┤│                            │  │
│  ▣    │ profil du coffre (42 px)           ││                    ┌───────┤  │
└────────────────────────────────────────────┴────────────────────┴───────┴──┘
                              barre d'état (27 px) collée en bas à droite → y=773..800
```

> Le dock droit (300 px) est replié par défaut ; il s'insère à droite de la zone
> principale et réduit celle-ci. **[M]**

### 2.1 Bandeau supérieur (barre d'onglets)

| Élément | Mesure | Source |
| --- | --- | --- |
| Hauteur | **40 px** (bandeau continu de `y=0` à `y=40` sur toute la largeur) | **[M]** |
| Fond | `#282828` | **[M]** |
| Padding du conteneur d'onglets principal | `0 8px` quand le dock droit est ouvert ; `padding-right: 134px` quand il est replié (réservation pour les commandes de fenêtre) | **[M]** |
| Coin haut-gauche | bouton de bascule du dock gauche, zone cliquable **44×39**, icône interne 28×24 (SVG 16×16) | **[M]** |
| Le coin haut-gauche du ribbon (0..44 × 0..39) est donc le bouton de bascule, pas une icône d'action | **[M]** |

### 2.2 Ruban (colonne d'icônes à gauche)

| Élément | Mesure | Source |
| --- | --- | --- |
| Largeur | **44 px** (`--ribbon-width: 44px`) | **[M]** |
| Position | `x=0..44`, `y=40..800` (le ruban **débute sous le bandeau d'onglets**) | **[M]** |
| Fond | `#282828` (identique aux docks) | **[M]** |
| Padding | `8px 4px 12px` | **[M]** |
| Zone cliquable d'une icône | **28×24**, rayon 4 px, centrée dans la colonne (marge latérale 3,5 px) | **[M]** |
| Pas vertical des icônes | **30 px** (24 + 6 d'écart ; `gap: 6px` mesuré) | **[M]** |
| SVG | 16×16, trait 2 px, opacité 0,85 | **[M]** |
| Rail bas | zone de réglages du dock, ancrée en bas (`y=788`), vide par défaut | **[M]** |
| Actions observées (7) | sélecteur rapide, vue graphique, nouveau canvas, note quotidienne, insérer un modèle, palette de commandes, nouvelle base | **[M]** (`aria-label` FR) |

> L'ordre exact des icônes dépend des plugins installés ; la colonne, elle, est fixe :
> 7 entrées mesurées sur ce binaire. **[M]** pour la colonne, **[I]** pour l'ordre.

### 2.3 Dock gauche (barre latérale)

| Élément | Mesure | Source |
| --- | --- | --- |
| Largeur | **300 px** (par défaut sur ce coffre ; variable `--sidebar-left-toggle-inner-width-open: 24%` pour le mode « glissant ») | **[M]** / **[M]** (variable) |
| Fond | `#282828` | **[M]** |
| Barre d'onglets du dock | 40 px de haut ; onglets **icônes seules** (libellé masqué : `--sidebar-tab-text-display: none`) | **[M]** |
| Onglet de dock | zone 28×25, icône SVG 16×16 trait 2 px, **pas de 31 px** (x = 52, 83, 114) | **[M]** |
| Onglet de dock actif | fond `white 6,7 %` (opacité 0,067), couleur d'icône `#b3b3b3` | **[M]** |
| Onglets observés | Explorateur de fichiers, Rechercher, Signets | **[M]** |
| En-tête de l'explorateur de fichiers | 300×40 à `y=40`, padding 8 px, **fond transparent** (le fond du dock transparaît) | **[M]** |
| Boutons de l'en-tête | barre de 284×24 ; boutons 28×24 à `x = 120, 150, 180, 210, 240` | **[M]** |
| Actions de l'en-tête | nouvelle note, nouveau dossier, modifier l'ordre de tri, auto-révéler le fichier actuel, tout développer (+ champ de recherche, masqué par défaut) | **[M]** (`aria-label` FR) |
| Note | **le champ de recherche de l'en-tête est présent dans le DOM mais masqué** ; la colonne des boutons est alignée à gauche, pas à droite | **[M]** |

#### 2.3.1 Arborescence de fichiers

| Élément | Mesure | Source |
| --- | --- | --- |
| Élément (dossier ou fichier) | hauteur **24,9 px**, `x=56`, largeur 276 px (300 − 2×12) | **[M]** |
| Police | 13 px / 16,9 px, couleur `#b3b3b3` | **[M]** |
| Padding | `4px 8px 4px 24px` (gauche = place de l'icône de disclosure) | **[M]** |
| Rayon | 8 px | **[M]** |
| Marge basse entre éléments | 2 px | **[M]** |
| Icône | 16×16 (SVG) | **[M]** |
| Retrait d'un niveau | +16 px (12 px de marge d'enfant + 4 px) | **[M]** / `--nav-item-children-margin-start: 12px` |

#### 2.3.2 Profil du coffre (bas du dock)

| Élément | Mesure | Source |
| --- | --- | --- |
| Bloc | 300×41,9 px à `y=758.1` (ancré en bas), bordure haute 1 px `#333333`, padding 8 px | **[M]** |
| Icône du coffre | 16×16, couleur `#666666` | **[M]** |
| Nom du coffre | 13 px, graisse 500, couleur `#dadada` | **[M]** |
| Actions | 2 boutons 28×24 alignés à droite (changer de coffre, menu) | **[M]** |

### 2.4 Zone principale

| Élément | Mesure | Source |
| --- | --- | --- |
| Barre d'onglets de note | 40 px de haut, fond `#282828`, padding gauche 8 px | **[M]** |
| Onglet de note (au repos, un seul onglet) | **200 px de large** (`--tab-width: 200px`), 34 px de haut, rayon `10px 10px 0 0`, padding `1px 4px 4px 4px` | **[M]** |
| Onglet interne | 192×29, rayon 4 px, padding `0 3px 0 6px`, gap 2 px | **[M]** |
| Titre de l'onglet | 15 px / 19,5 px, couleur `#b3b3b3` | **[M]** |
| Onglet actif | fond `#1C1C1C` (`--tab-background-active`) | **[M]** |
| Bouton de fermeture | 20×20, visible **uniquement sur l'onglet actif** | **[M]** |
| Comportement en manque de place | l'onglet se rétrécit (29,5 px observés pour 9 onglets dans 380 px) ; largeur max 320 px (`--tab-max-width`) | **[M]** |
| En-tête de vue (titre de la note) | bandeau de `y=40` à `y=78` (**38 px**), fond **transparent**, conteneur de titre 228×37 | **[M]** |
| Titre de la note dans l'en-tête | 13 px, `#b3b3b3` | **[M]** |
| Boutons de l'en-tête | 5 : navigation arrière, navigation avant, signet, bascule lecture/édition, « Plus d'options » | **[M]** |
| Visibilité de l'en-tête | `display:none` au repos, `display:flex` au survol de la zone — mode « navigation flottante » (`body.is-floating-nav`) | **[M]** |
| Surface d'édition | de `y=78` à `y=800` | **[M]** |

#### 2.4.1 Surface d'édition (mode édition / *live preview*)

| Élément | Mesure | Source |
| --- | --- | --- |
| Marge intérieure du défilement | **32 px** sur les quatre côtés | **[M]** |
| Largeur de ligne | **700 px maximum** (`--file-line-width: 700px`), centrée dans la division | **[M]** |
| Bloc de propriétés (frontmatter) | pleine largeur de la colonne de texte, padding `8px 0`, marge basse 32 px, bordure haute 1 px `#333333` | **[M]** |
| Lignes de propriétés | 29 px de haut ; colonne clé 144 px (9 em), colonne valeur alignée à 144 px de la colonne clé | **[M]** |
| Texte des propriétés | 16 px / 24 px, couleur `#b3b3b3` | **[M]** |
| Bloc de code | fond `#232323`, bordure 1 px `#333333`, rayon 4 px, padding `12px 16px` | **[M]** |
| Citation | bordure gauche **2 px** accent (`#8A5CF5`), padding gauche 24 px, aucun fond | **[M]** |
| Liste | retrait 21 px (édition) / 26,7 px (lecture) | **[M]** |
| Case à cocher | 16×16, bordure 1 px `#666666`, rayon 4 px, marge `0 3.33px 0 11.33px` | **[M]** |

#### 2.4.2 Surface de lecture (aperçu)

Mêmes valeurs de base que l'édition, avec en plus :

| Élément | Mesure | Source |
| --- | --- | --- |
| Marge intérieure | 32 px | **[M]** |
| Largeur de ligne | 700 px max | **[M]** |
| Colonne de texte | 604 px dans une division de 680 px | **[M]** |
| Titre affiché | le H1 de la note est rendu **comme titre en ligne** (25,888 px, 700) | **[M]** |
| Propriétés (frontmatter) | affichées **au-dessus** du titre, sous forme de tableau clé/valeur (29 px par ligne) | **[M]** |
| Lien interne | couleur `#A68AF9`, **souligné** | **[M]** |
| Lien externe | couleur `#A68AF9`, souligné | **[M]** |
| Tag | 14 px, couleur `#A68AF9`, fond accent 10 %, padding `3.5px 9.1px`, rayon 28 px | **[M]** |
| Tableaux | `16px / 20.8px`, cellules padding `4px 8px`, bordures 1 px `#333333`, en-têtes en graisse 600 | **[M]** |
| Callout | fond accent 10 %, bordure accent 25 %, padding `12px 12px 12px 24px`, rayon 4 px, titre de couleur callout (`#027AFF` pour « note ») | **[M]** |

### 2.5 Dock droit (panneau secondaire)

| Élément | Mesure | Source |
| --- | --- | --- |
| Largeur | **300 px** (identique au dock gauche) | **[M]** |
| Position | à droite de la zone principale (`x=724..1024` sur 1024) | **[M]** |
| Fond | `#282828` | **[M]** |
| Poignée de redimensionnement | **3 px** de large, bord gauche du dock | **[M]** |
| Barre d'onglets | 40 px, icônes 28×25, pas de 31 px | **[M]** |
| Onglets observés | Rétroliens, Liens sortants, Mots-clés, Toutes les propriétés, Plan | **[M]** |
| État vide | texte centré « La barre latérale est vide, essayez de faire glisser un onglet ici. » | **[M]** |

> Un panneau d'**aperçu** (lecture) d'une note s'obtient en ouvrant la note dans le dock
> droit (glisser-déposer) : ce n'est pas un composant distinct, c'est le même composant
> note. **[I]** (déduit du modèle de divisions ; la mesure du panneau droit a porté sur
> les onglets Rétroliens/Plan).

### 2.6 Barre d'état (bas)

| Élément | Mesure | Source |
| --- | --- | --- |
| Position | **fixe, collée en bas à droite** : `x=683.5..1024`, `y=773..800` (pas de barre pleine largeur) | **[M]** |
| Hauteur | **27 px** (minimum 18 px) | **[M]** |
| Fond | `#282828` | **[M]** |
| Bordure | 1 px `#333333` **en haut et à gauche** (`1px 0 0 1px`) | **[M]** |
| Rayon | `8px 0 0 0` | **[M]** |
| Padding / gap | `4px 8px 4px 4px` / 4 px | **[M]** |
| Éléments | hauteur 18 px, padding `3px 4px`, rayon 4 px, 12 px, `#b3b3b3`, chiffres tabulaires | **[M]** |
| Contenu observé | « 2 rétroliens », « 3 propriétés », icône d'état de l'éditeur, « 8 mots » / « 52 caractères » (2 segments), icône de synchronisation | **[M]** |

---

## 3. États

| État | Rendu | Source |
| --- | --- | --- |
| **Survol d'un élément d'arbre** | fond `oklch(white 6,7 % / 0.067)` ≈ `rgba(255,255,255,0.067)`, texte `#b3b3b3` → **`#dadada`** | **[M]** (survol simulé) |
| **Élément d'arbre actif** (note ouverte) | même fond clair (6,7 %), texte `#dadada` | **[M]** |
| **Survol d'un onglet de note** | fond appliqué à l'onglet **interne** (pas à l'en-tête), texte inchangé `#b3b3b3` | **[M]** |
| **Onglet de note actif** | fond `#1C1C1C`, bouton de fermeture visible | **[M]** |
| **Survol d'une icône du ruban** | fond `rgba(255,255,255,0.067)`, opacité de l'icône 1 | **[M]** |
| **Onglet de dock actif** | fond `rgba(255,255,255,0.067)` | **[M]** |
| **Note ouverte** | onglet présent dans la barre d'onglets ; élément d'arbre correspondant marqué actif | **[M]** |
| **Note vide** | **aucun texte de remplacement n'a été observé** (`.cm-placeholder` absent du DOM) ; la zone affiche le bloc « Propriétés / Ajouter une propriété ». Le comportement exact du curseur dans une note vide reste **non mesuré** | **[M]** (absence) / **[I]** |
| **Note modifiée (non enregistrée)** | **non observé** : l'enregistrement est quasi instantané, la classe d'indicateur n'a pas pu être capturée. Un conteneur `workspace-tab-header-status-container` existe dans le DOM (0×0 au repos) | **[M]** (structure), **[I]** (apparence du point) |
| **Focus** | l'anneau de focus natif est `2px solid light-dark(#0060df, #0df)` ; les variables existent mais le rendu n'a pas été capturé | **[M]** (variable), **[I]** (apparence) |
| **Sélection multiple dans l'arbre** | `--nav-item-background-selected` = accent 15 % ; **non observée** | **[M]** (variable) |
| **Wikilink résolu** | couleur `#A68AF9` ; en lecture : souligné ; en édition : **non souligné** | **[M]** |
| **Wikilink non résolu** | **même couleur** `#A68AF9`, **opacité 0,7** en lecture ; en édition : couleur identique, aucune marque distincte observée | **[M]** |
| **Tag** | couleur `#A68AF9`, fond accent 10 %, 14 px, capsule (rayon 28 px) ; en édition le tag est rendu en 14 px / 21 px, sans capsule | **[M]** |
| **Frontmatter / propriétés** | tableau clé/valeur, 29 px par ligne, clé en 144 px, texte `#b3b3b3` 16 px, bordure haute 1 px `#333333`, marge basse 32 px | **[M]** |
| **Case à cocher de tâche** | 16×16, bordure 1 px `#666666`, rayon 4 px ; cochée : accent + texte barré (`--checklist-done-decoration: line-through`) | **[M]** (côté non coché), **[M]** (variable) |
| **Citation** | bordure gauche 2 px accent, aucune autre décoration | **[M]** |
| **Liste à puces** | puce de 0,3 em, couleur `#666666` | **[M]** (variable) |
| **Syntaxe colorée** | commentaire `#666666`, chaîne `#44cf6e`, mot-clé `#fa99cd`, fonction `#e0de71`, opérateur `#fb464c` | **[M]** (variables) |

### 3.1 Palette de commandes (raccourcis visibles)

| Élément | Mesure | Source |
| --- | --- | --- |
| Modale | **700 × 560** px, centrée (x=162, y=120 sur 1024×800) | **[M]** |
| Fond / rayon / bordure | `#1C1C1C` / 12 px / 1 px `#3f3f3f` | **[M]** |
| Champ de saisie | 49 px de haut, police 15 px (`--prompt-input-height: 40px` + marges) | **[M]** |
| Ligne de suggestion | 662 × **31,5 px**, 15 px, texte `#dadada` | **[M]** |
| Suggestion sélectionnée | fond `rgba(255,255,255,0.067)` | **[M]** |
| Jeton de raccourci | 12 px, fond `rgba(255,255,255,0.067)`, texte `#dadada`, rayon 24 px, padding `2px 6px` | **[M]** |
| Raccourcis observés | `Ctrl + ;`, `Ctrl + 1`, `Ctrl + 2`, `Ctrl + Shift + Tab`, `Ctrl + Page Up`, `Ctrl + Tab` | **[M]** |

---

## 4. Comportements

### 4.1 Redimensionnabilité

| Comportement | Mesure / constat | Source |
| --- | --- | --- |
| Poignées de redimensionnement | **3 px**, sur le bord droit du dock gauche (`x=341..344`) et le bord gauche du dock droit (`x=724..727`), hauteur pleine | **[M]** |
| Les docks sont redimensionnables à la souris | poignée entre dock et zone principale | **[M]** |
| Le ruban ne se redimensionne pas | largeur fixe 44 px | **[M]** |
| La barre d'onglets ne se redimensionne pas | 40 px | **[M]** |
| Plages de largeur du dock | variables `--sidebar-left-toggle-inner-width: 8.33%` (replié) et `-open: 24%` (ouvert) — en pratique le coffre de mesure est resté à **300 px** | **[M]** (variable), **[M]** (300 px) |
| Zone de texte | largeur de ligne fixe 700 px, la colonne de texte se centre dans la division | **[M]** |
| La barre d'état ne se redimensionne pas | largeur intrinsèque, alignée à droite | **[M]** |

### 4.2 Onglets

| Comportement | Constat | Source |
| --- | --- | --- |
| Les notes ouvertes deviennent des onglets dans la barre principale | 9 onglets coexistaient pendant la mesure | **[M]** |
| Un onglet par note ; pas de regroupement | — | **[M]** |
| Les onglets se rétrécissent quand la place manque | 29,5 px observés | **[M]** |
| Largeur nominale / maximale | 200 px / 320 px | **[M]** |
| Fermeture | bouton 20×20 sur l'onglet actif | **[M]** |
| Docks : onglets icônes de 28×25, libellés masqués | `--sidebar-tab-text-display: none` | **[M]** |
| Les divisions (dock droit, aperçus) acceptent le glisser-déposer d'un onglet | message d'état vide explicite | **[M]** (message) |

### 4.3 Raccourcis et affordances visibles

| Action | Raccourci / indice | Source |
| --- | --- | --- |
| Palette de commandes | `Ctrl + P` (ouverture mesurée par simulation de touches) | **[M]** |
| Changer d'onglet de note | `Ctrl + Tab` / `Ctrl + Shift + Tab` | **[M]** (jetons de la palette) |
| Note précédente / suivante | `Ctrl + Page Up` / `Ctrl + Page Down` | **[M]** (jeton) |
| Basculer dock gauche / droit | `Ctrl + 1` / `Ctrl + 2` | **[M]** (jeton) |
| Vérification orthographique | `Ctrl + ;` | **[M]** (jeton) |
| Basculer lecture / édition | clic sur le bouton de l'en-tête de vue ; infobulle : « Éditer (Ctrl/Cmd + Clic pour éditer dans un nouvel onglet) / Cliquer pour lire / Ctrl+Clique pour ouvrir dans un nouvel onglet » | **[M]** (`aria-label` mesuré) |
| Bandeau d'onglets / en-tête de vue | masqués par défaut, apparaissent au survol (mode « navigation flottante ») | **[M]** |

### 4.4 Autres comportements visibles

| Comportement | Constat | Source |
| --- | --- | --- |
| La barre d'état suit la largeur de la fenêtre (ancrée à droite) | x = 683,5 pour 1024 de large | **[M]** |
| Le ruban est ancré sous le bandeau d'onglets, pas sous le bord haut de la fenêtre | `y=40` | **[M]** |
| Les boutons de l'en-tête de l'explorateur n'occupent pas toute la largeur | boutons alignés à gauche, champ de recherche masqué | **[M]** |
| Une note s'ouvre dans la division principale et y reste | — | **[M]** |
| Le titre de la note s'affiche dans le corps (titre en ligne) et non dans l'en-tête | `--inline-title-size: 1.618em` | **[M]** |

---

## 5. Écarts avec le dépôt, et points à trancher

### 5.1 Constantes du dépôt qui ne correspondent pas aux mesures

Toutes les valeurs ci-dessous ont été **mesurées** sur Obsidian 1.14.4 ; la colonne
« dépôt » indique ce que `obsy` utilise aujourd'hui.

| Sujet | Mesuré (Obsidian 1.14.4) | Dépôt (`src/…`) | Écart |
| --- | --- | --- | --- |
| Fond principal | `#1C1C1C` | `#1E1E1E` (`theme_vars.rs`) | 2 niveaux |
| Fond secondaire | `#282828` | `#262626` (`theme_vars.rs`) | 2 niveaux |
| Texte discret (`--text-muted`) | `#b3b3b3` | `#A0A0A0` (`theme_vars.rs`) | 19 niveaux |
| Interligne du corps | **1,5** (16/24) | `1.6` (`editor.rs`) | +0,1 |
| Interligne des titres | 1,2 / 1,2 / 1,3 / 1,4 / 1,5 / 1,5 | `1.4` uniforme (`editor.rs`) | non uniforme |
| Taille H1 | 1,618 em (25,888 px) | 1,6 em (`editor.rs`) | ≈ 0,29 px |
| Tailles H2→H6 | 1,462 / 1,318 / 1,188 / 1,076 / 1,0 | 1,4 / 1,2 / 1,1 / 1,0 / 1,0 | jusqu'à 0,99 px |
| Graisses des titres | 700 / 680 / 660 / 640 / 620 / 600 | non modélisées | à ajouter |
| Interlettrage des titres | −0,015 → 0 em | non modélisé | à ajouter |
| Taille du code | 0,875 em (14 px) | `0.9` em (`editor.rs`) | 0,4 px |
| Retrait de citation | padding gauche **24 px** | `20 px` (`editor.rs`) | 4 px |
| Espacement des paragraphes | **16 px** (1 rem) | `0.75` em = 12 px (`editor.rs`) | 4 px |
| Retrait de liste | 21 px (édition) / 26,7 px (lecture) | `2.0` em (`editor.rs`) | non uniforme selon le mode |
| Case à cocher | **16 px** (= 1 em) | `1.15` em (`editor.rs`) | 2,4 px |
| Police d'interface | pile système (Inter en tête de liste utile) | `"Inter"` seul (`theme_vars.rs`) | pile à préciser |
| Interligne de la table | 1,3 | non modélisé | à ajouter |
| `--link-unresolved-opacity` | 0,7 | non modélisé | à ajouter |

> Ces écarts sont **des divergences de fait, pas des jugements** : soit les constantes du
> dépôt ont été relevées sur une version antérieure d'Obsidian, soit elles ont été
> approximées. Le tableau est la base d'une décision à prendre par le propriétaire ;
> ce document ne modifie aucun code.

### 5.2 Ce que la spec ne couvre pas (à observer ensuite)

1. **Thème clair** — non mesuré (la session était en `theme-dark`). Variables
   prévisibles : mêmes noms, valeurs inversées. À relever en basculant
   `body.theme-light`.
2. **Animations et transitions** — `--input-transition: none`, `--button-transition: none`,
   `--menu-transition: none` sont « none » dans le thème par défaut, mais Obsidian anime
   quand même certaines surfaces (apparition de la palette, fondu du bandeau). Durées
   non mesurées.
3. **Theme HiDPI** — toutes les mesures sont en unités CSS à DPR 1. À revérifier à DPR 2
   (arrondis, épaisseur des traits 2 px, largeur de scrollbar).
4. **Plugins et vues secondaires** — Canvas, Bases, Kanban, Graph, Table, Pétales
   (canvas), et l'interface des plugins community : non mesurés.
5. **Surcharges utilisateur** (`.obsidian/snippets/*.css`) — le dépôt les sait déjà lire
   (`src/vault/css_snippets.rs`) ; la cascade de surcharge (variables → classes) n'a pas
   été observée.
6. **Boîtes de dialogue et modales** — seul le cas « palette de commandes » a été mesuré.
   Les modales de réglages (largeur `--modal-width: 90vw`, rayon 12 px) restent à
   vérifier.
7. **Icônes exactes** — le jeu d'icônes (Lucide) n'a pas été relevé ; seules les
   dimensions (16 px, trait 2 px) sont mesurées.
8. **Curseur, sélection de texte, règles de géométrie (rulers), guides d'indentation** —
   `--indentation-guide-width: 1px`, couleur `white 12 %` (variable) ; non mesuré à
   l'écran.
9. **Notifications et infobulles** — `--tooltip-radius: 12px`, `--tooltip-padding: 8px 12px`,
   `--tooltip-font-size: 13px` (variables) ; non mesuré.
10. **Le bandeau de titre de la fenêtre système** — sous Xvfb il n'existe pas ; le
    comportement « fenêtre frameless » d'Obsidian sur Linux (intégration Wayland) n'a pas
    pu être observé.
11. **Édition collaborative, mode hors-ligne, comptes Obsidian Sync** — hors périmètre.
12. **Le behaviors de la palette sur écran tactile / petite largeur** — non mesuré.

### 5.3 Hypothèses explicites (non mesurées) portées par cette spec

- Les 3 noms de famille `??` en tête de `--font-interface` / `--font-text` /
  `--font-monospace` sont des espaces réservés aux polices embarquées par Obsidian ;
  la pile système qui suit suffit à reproduire l'apparence sur une machine qui a
  Inter. **[I]**
- L'« aperçu » (panneau de prévisualisation) d'une note est le même composant que
  l'éditeur, monté dans le dock droit. **[I]**
- Le point « note modifiée » est un petit disque dans l'onglet ; sa taille et sa
  couleur n'ont pas été capturées. **[I]**
- L'intervalle de 30 px entre icônes du ruban et celui de 31 px entre onglets de dock
  viennent d'une mesure ponctuelle (une seule fenêtre, DPR 1) : à re-mesurer à
  DPR 2. **[M]** + **[I]** pour la stabilité.

---

## 6. Comment reproduire la mesure

1. Lancer l'application réelle en mode mesurable : sous X, avec un port de débogage CDP
   ouvert, en forçant la plateforme X11 (`--ozone-platform=x11`) et un rendu logiciel
   (`--in-process-gpu --enable-unsafe-swiftshader --use-gl=angle --use-angle=swiftshader`).
   Sans GPU utilisable, le processus s'arrête (`GPU process isn't usable`) ; sans
   plateforme X11 forcée, il échoue (`Failed to initialize Wayland platform`).
2. Se connecter au renderer et mesurer avec `getBoundingClientRect()` +
   `getComputedStyle()` ; filtrer sur `rect.width > 0` pour ne garder que les éléments
   réellement visibles (les onglets inactifs gardent un nœud à taille nulle).
3. Les survols se simulent par `Input.dispatchMouseEvent` de type `mouseMoved` aux
   coordonnées de l'élément, **puis** la relecture du style calculé : `:hover` n'est
   pas lisible par `getComputedStyle` sans vrai survol de la souris.
4. Les vues se pilotent par `app.commands.executeCommandById(...)`
   (`app:toggle-right-sidebar`, `markdown:toggle-preview`, `app:toggle-ribbon`), et les
   notes par `app.workspace.openLinkText(...)`.
5. Les mesures de cette spec ont été prises avec un coffre de mesure jetable ; le
   coffre et la configuration de l'utilisateur ont été restaurés à l'identique.

---

## Annexe — Índice des sources

| Source | Apport |
| --- | --- |
| `src/ui/theme_vars.rs` | Les 21 variables CSS déjà modélisées, leurs valeurs par défaut, l'ordre de chargement (défaut → thème du coffre → snippets) |
| `src/vault/themes.rs` | Palettes `DEFAULT_DARK` / `DEFAULT_LIGHT`, chemins de thème |
| `src/vault/editor.rs` (`typography`) | Constantes typographiques du dépôt (à confronter à §5.1) |
| `demonstrations/obsidian-rea/ANALYSE.md` | Analyse REA statique de 1.14.4 : comportements de liens, tags, frontmatter (aucune donnée visuelle) |
| `demonstrations/obsidian-rea/BOOT-CONTRACTS.md` | Conditions de démarrage du renderer (utiles pour rejouer l'application) |
| Mesure CDP du 2026-10-21 | Toutes les valeurs marquées **[M]** |
