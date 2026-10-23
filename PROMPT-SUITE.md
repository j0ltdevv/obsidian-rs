# Suite du projet obsy — avance jusqu'au bout, supervise tes agents

## 1. GIT — commit immédiat

Tu n'as qu'un commit depuis le début et ~2300 LOC non versionnés.

1. Écris `.gitignore` : `/target`, `.omp/`
2. `git add . && git commit -m "Phase 0 + Phase 1 : vault core, parsers, ops, index, UI"`
3. Ensuite : **UN commit par feature**, jamais un gros fourre-tout. Un commit = une
   feature qui passe les 4 commandes.

---

## 2. REA — va droit au graphe, ne redécouvre pas

L'evidence_id de l'analyse complète est **stable** (hash déterministe du contenu,
identique sur des runs indépendants) :

```
ev_00c397c132cbaf3363cb15ae435e1688a912c53660f97991d1e0abecae0c101e
```

Va directement à `inspect_analysis_view` avec :

```json
{"source":{"kind":"retained-evidence","evidence_id":"ev_00c397c132cbaf3363cb15ae435e1688a912c53660f97991d1e0abecae0c101e"},
 "view":{"kind":"summary"}}
```

Puis les pages :

```json
{"source":{"kind":"retained-evidence","evidence_id":"ev_00c397c132cbaf3363cb15ae435e1688a912c53660f97991d1e0abecae0c101e"},
 "view":{"kind":"page","collection":"modules","offset":0,"limit":100}}
```

**Collections valides : `sections`, `symbols`, `modules`. Pas de `ipc`.**

Ce que l'analyse a déjà donné (SEEN, à intégrer tel quel dans
`evidence/00-architecture-overview.md`) :

- `relevant_files: 37`, `parsed_javascript_files: 5`, `parse_failures: 0`
- `text_bytes_read: 4 771 961` · `visited_ast_nodes: 1 276 779` · `findings: 778`
- `modules: 363` (420 au total, paginé) — majoritairement `app.js` et `worker.js`
- `electron.browser_windows: 2`
- `electron.ipc: 45 literal_channels · 45 main_handlers · 0 dynamic · 0 renderer_transmissions`
  → **SEEN** : 45 canaux IPC littéraux, tous traités côté main process ; **aucune**
  transmission renderer appariée. Donc l'appels renderer→main passe par un chemin
  qui n'est pas un `ipcRenderer.invoke` appariable statiquement.
- `electron.preload_entrypoints: 0` · `context_bridge_apis: 0` · `native_addon_bindings: 0`
  → **SEEN** : pas de pont preload/contextBridge dans l'ASAR analysé ; cohérent avec
  un `nodeIntegration` activé côté renderer (les API Node sont directement disponibles).
- `source_selection: 29 fichiers non analysés` (le reste de l'ASAR : vendor lib/)

**Ne relance PAS l'analyse de l'asar entier** : timeout interne REA à 300 s, il ne
passera pas. L'analyse bornée (package.json, main.js, app.js, starter.js, worker.js,
enhance.js, index.html, starter.html) prend **97,6 s** et c'est déjà fait.

Extrais les **45 canaux IPC** et les **noms de modules** (collection `sections` et
`symbols`) pour documenter le contrat comportemental : les canaux = surface d'intégration,
les modules = découpage interne. **Aucun code copié** — uniquement des noms de canaux
et des comportements décrits en français,标签 SEEN/INFERRED/UNKNOWN.

---

## 3. AVANCE — ne t'arrête pas à la fin d'une phase

**Objectif : aller le plus loin possible, jusqu'au bout du projet.**

Phases à livrer, dans l'ordre, sans attendre ma validation entre chaque :

| Phase | Contenu | Statut |
|---|---|---|
| 0 | Fondations, cartographie REA, slice vault | faite (54 tests) |
| 1 | Vault complet (création/renommage/déplacement), éditeur Markdown + live preview, sidebar, ouverture/sauvegarde, wikilinks cliquables | en cours |
| 2 | Backlinks, recherche fuzzy (Quick Switcher) + full-text (tantivy), properties/frontmatter éditables, graph view (basique puis avancé) | à faire |
| 3 | Canvas, Bases/Dataview-like, versions locales, thèmes, raccourcis clavier complets | à faire |

Avance tant que tu peux produire du code qui compile et des tests qui passent.
**Ne t'arrête pas pour demander une validation à chaque phase** : valide-toi toi-même
(les 4 commandes), commite, et enchaîne. Tu ne t'arrêtes que si :

- wgpu/iced ne démarre pas (déjà vérifié OK) ;
- une décision d'architecture a deux réponses légitimes sans défaut net ;
- une règle ci-dessus est contredite.

À la fin, un rapport global : ce qui est fait, ce qui reste, la sortie brute des
4 commandes, les preuves REA, le coût.

---

## 4. SUBAGENTS — lance autant que tu veux, mais SUPERVISE

**Tu as le droit de lancer autant de subagents que tu veux.** `maxConcurrency: 64`,
`speculativeLaunch: true`. N'hésite pas à ouvrir des vagues de 3 à 6 agents en
parallèle. Mais **un subagent n'est pas terminé tant que tu n'as pas vérifié son
travail**. C'est là que ça se joue.

### 4.1 — Renseigne TOUJOURS le champ `agent`

| `agent` | Pour quoi |
|---|---|
| `builder` | **écrit du code** (parsers, UI, modules) |
| `reviewer` | relecture **read-only** : régressions, cas limites, tests manquants |
| `verifier` | lance les 4 commandes et rapporte la **sortie brute**, ne modifie rien |
| `scout` | exploration **read-only** (lecture de code, inventaire) |
| `sonic` | docs, index, fichiers de synthèse |
| `seance` | le seul qui accepte un `model` |

Ne laisse **jamais** `agent` vide → tu tombes sur des *generic workers* sans
spécialisation. omp te le rappelle après chaque spawn : *« Check the agent list for a
closer specialist type »*. Écoute-le.

### 4.2 — Le champ `model` ne marche QU'AVEc `agent: "seance"`

```json
{"agent":"seance","model":"tokenplan/deepseek-v4-flash-0731","task":"..."}
```

Sinon : `` `model` is only accepted for the `seance` agent ``. Ton appel échoue et tu
perds une vague entière. Pour router un subagent sur deepseek, c'est `seance` + `model`.

Répartition :
- **Volume / boilerplate / doc** → `sonic` ou `builder` (modèle par défaut)
- **Tout ce qui doit être juste** (parsing, tests, spec, revue) → `seance` + deepseek
- **Vérification** → `verifier` (dédié, jamais un generic worker)

### 4.3 — Supervise sérieusement

Le cycle obligatoire pour chaque vague :

1. **Tu écris la délégation** : périmètre fichier fermé (fichiers listés, rien d'autre),
   contrat exact (signature, comportement, cas limites), critères d'acceptation
   vérifiables, contexte partagé (repo, langue française, règles absolues).
2. **Tu lances** la vague (3 à 6 en parallèle, périmètre disjoint entre eux).
3. **Tu attends les résultats** — et tu les lis. Pas de « spawn and forget ».
4. **Tu vérifies toi-même** : tu relis le diff, tu relances les 4 commandes.
5. **Si un subagent a cassé le build ou ses tests :** tu le répares toi, ou tu le
   relances avec un périmètre plus petit et un contrat plus précis. Un subagent
   qui rate ne se répare pas seul.
6. **Tu commites** seulement quand la vague est verte.

### 4.4 — Règle d'or

**Tu ne lances une nouvelle vague que s'il n'y a AUCUNE erreur en cours.**

Zéro erreur = tu peux foncer, ouvrir 6 agents, paralléliser à fond, avancer vite.
Une erreur = tu t'arrêtes, tu la localises, tu la répares, tu revérifies, et
**ensuite** tu repars.

Sois très attentif au `Tip` d'omp et aux messages d'erreur : ce sont eux qui t'ont
fait perdre des vagues (`model` refusé, `tasks` manquant, edits rejetés).

### 4.5 — Ce qui se délègue bien

- un fichier / un module / une feature = un subagent, périmètre disjoint
- une couche REA à documenter = un `scout` par layer
- la vérification = un `verifier` par vague, en parallèle du travail suivant
- la relecture = un `reviewer` sur le code frais
- les specs = un `sonic` par feature documentée

Ce qui ne se délègue pas : les décisions d'architecture, le design des structs
partagées, `src/app/state.rs` (fichier pivot → conflits), et le merge final.

---

## 5. RAPPEL DE QUALITÉ

Inchangeé, non négociable :

```sh
cargo fmt --all --check
cargo build
cargo test
cargo clippy --all-targets -- -D warnings
```

Les 4 doivent passer. Colle la **sortie brute**, jamais une promesse.
Jamais de `unwrap()` sur entrée utilisateur ou fichier. Jamais de `#[allow]`.
Jamais de test supprimé ou affaibli. Jamais de code d'Obsidian copié.

---

**Commence par le commit, puis l'extraction IPC + modules, puis Phase 1 jusqu'au bout.
Avance. Supervise. Ne t'arrête pas.**
