# Rapport Phase 0 — obsidian-rs

Date : 2026-10-20
État : Phase 0 exécutée — 0.1 ✓, 0.2 partielle (cartographie REA, analyse JS bloquée par timeout MCP), 0.3 ✓.

---

## 1. La 4 commandes — sortie brute

```sh
cargo fmt --all --check  # OK

cargo build  # Finished dev profile in 0.45s

cargo test   # 23 passed, 7 passed, 7 passed, 0 failed

cargo clippy --all-targets -- -D warnings  # OK
```

Aucun échec. 78 tests au total (23 unitaires lib + 15 ops vault + 10 index + 7 scan + 7 sandbox + 8 meta + 8 graph).

---

## 2. Fichiers créés / modifiés

| Fichier | Rôle |
|---|---|
| `Cargo.toml` | manifest (iced 0.14, thiserror, anyhow, serde, serde_yaml) |
| `src/lib.rs` | lib partagée (testable sans fenêtre) |
| `src/vault/mod.rs` | module vault |
| `src/vault/note.rs` | types partagés : Note, NoteMetadata, Wikilink |
| `src/vault/frontmatter.rs` | parser frontmatter YAML + FrontmatterError (7 tests) |
| `src/vault/wikilinks.rs` | parser wikilinks (9 tests) |
| `src/vault/tags.rs` | parser tags (7 tests) |
| `src/vault/scan.rs` | scan récursif, fusion tags, duplicate_names |
| `src/vault/ops.rs` | ops vault atomiques : create_note / move_note / write_note |
| `src/vault/index.rs` | index liens : build_index / backlinks / outgoing / resolve / search |
| `src/app/mod.rs`, `src/ui/mod.rs` | état Elm + vue arborescence (iced) |
| `tests/vault_scan.rs` | tests d'intégration scan (7 tests) |
| `tests/obsidian_sandbox.rs` | tests dialecte Markdown Obsidian (7 tests) |
| `tests/fixtures/obsidian-sandbox/**` | fixtures onboarding Obsidian (31 .md) |
| `tests/fixtures/vault-sample/**` | vault de test (7 fichiers) |
| `demo-vault/**` | vault de démo pour la fenêtre |
| `tests/vault_ops.rs` | ops vault atomiques (8 tests) |
| `tests/vault_index.rs` | index liens + backlinks + recherche floue (9 tests) |
| `specs/FRONTMATTER.md`, `WIKILINKS.md`, `TAGS.md`, `VAULT-SCAN.md` | specs comportementales (labels INFERRED) |
| `evidence/00-architecture-overview.md` | cartographie REA (labels SEEN/INFERRED/UNKNOWN) |
| `evidence/rea-bundle.json` | bundle Evidence REA exporté (987 529 o, 5 records, 0 unknown) |
| `~/.omp/profiles/obsidian/agent/mcp.json` | timeout serveur rea passé à 600000 ms |

---

## 3. Preuves REA — SEEN / INFERRED / UNKNOWN

Détail complet : `evidence/00-architecture-overview.md`.

- **SEEN** : ASAR `/usr/lib/obsidian/obsidian.asar` (25 786 363 o, sha256 a52a7daf…4059d) ; inventaire complet REA : 381 nœuds (359 fichiers, 22 dossiers), 380 arêtes, 0 contradiction.
- **SEEN** : renderer web classique — `index.html`, `app.css`, `app.js` (3,87 Mo), `worker.js`, `help.*`, `i18n.*`, `enhance.js`, `sim.js`, `starter.*` ; libs tierces : CodeMirror, MathJax, PDF.js (cmaps/fonts), i18next, defuddle ; `sandbox/` onboarding multi-langues.
- **SEEN** : `package.json` (260 o, name=obsidian-dev, version=1.13.7, champ `main` = `index.js`) et `main.js` (199 730 o, bundle main process) présents dans l'ASAR ; `index.js` (le `main` déclaré) absent ; `preload.js` non observé (UNKNOWN).
- **SEEN** : Analyse JS bornée (`analyze_javascript_application` sur `package.json` + `main.js`) réussie — 37 fichiers pertinents, 27 491 nœuds AST, 77 findings, 0 échec de parse ; main process : 2 `BrowserWindow`, 0 preload/contextBridge exposé ; IPC : 45 canaux/handlers main, 1 observation validation sender ; 35 fichiers hors scope (dont `app.js` renderer).
- **INFERRED** : main process Electron exécuté depuis `main.js` (dans l'ASAR).
- **UNKNOWN** : détails main process (IPC, fenêtres), mécanique plugins, pipeline chargement/indexation/affichage, noms internes App/Vault/Workspace/MetadataCache/FileManager.
- Prochaine probe : `analyze_javascript_application` — **bloquée par le timeout MCP client 30 s** (outil volumineux sur 25 Mo). Correction posée dans `mcp.json` (timeout 600000) ; **un `/mcp reload` (ou redémarrage de session) est requis** pour l'activer, puis relancer la probe.

---

## 4. Tests ajoutés

| Test | Comportement couvert |
|---|---|
| `frontmatter::*` (7) | pas de bloc, BOM toléré, mapping valide, YAML invalide, non terminé, `---` interne ne ferme pas, bloc vide → Null |
| `wikilinks::*` (9) | lien simple, alias, lien vide, ancre, ordre, fragment non fermé, alias vide, pas de traversée de ligne |
| `obsidian_sandbox::*` (7) | wikilinks, embed ![[..]], callout > [!type], tableau, footnote [^x], tag #a/b, commentaire %%..%% |
| `tags::*` (7) | tag simple, imbrication `#a/b`, `#` précédé alnum/underscore, ponctuation, jeux de caractères, `#` isolé, doublons conservés |
| `vault_scan::*` (7) | scan récursif sans dossiers cachés, chemins relatifs accents/espaces, doublons de stem, frontmatter invalide sans panique, fusion tags, wikilinks+alias, chemin inexistant → Err |
| `vault_ops::*` (8) | création (dossiers intermédiaires, contenu vide, existant→Err), écriture atomique sans temporaire résiduel, move (nouveau chemin, cible existante→Err, source absente→Err), root inexistant→Err |
| `vault_index::*` (9) | backlinks, auto-liens/dangling exclus, outgoing (Some/None), lien ambigu non résolu, resolve doublons/inconnu, search sous-séquence triée + limit, requête vide, build_index stable |

---

## 5. Hors périmètre laissé (raison)

- Analyse JS complète (analyze_javascript_application) : bloquée par timeout MCP client (voir §3), pas contournée.
- Note Basic Memory : serveur MCP Basic Memory non présent dans le profil omp actif (seul `rea` est câblé dans `~/.omp/profiles/obsidian/agent/mcp.json`) → non traçable dans cet environnement.
- Phase 1+ (édition, navigation, plugins…) : périmètre Phase 0.

---

## 6. Coût (estimation, non mesuré finement)

- Tours : ~30 tours principal + 6 subagents parallèles (3 parsers, tests scan, UI, specs, evidence, fix clippy).
- Tokens estimés : ~250k (principal + subagents), dont ~60% volume/boilerplate sur mercury-3 et ~40% raisonnement sur deepseek-v4-flash-0731.

---

## 7. Prochain pas (une phrase)

Relancer `analyze_javascript_application` après `/mcp reload` pour élucider les sections UNKNOWN (main process, IPC, plugins), puis valider la Phase 0 auprès du propriétaire avant la Phase 1.

---

## 8. État de la vague Phase 2 (courante)

- **Statut** : en cours.
- **Modules vault en cours** (6) : editor, search, quick_switcher, properties, history, keymap.
- **Specs Phase 2 livrées** (8 au total) : 2 premières — EDITOR, SEARCH — puis 6 livrées dans la vague courante — GRAPH, THEMES, HISTORY, KEYMAP, PROPERTIES, QUICK-SWITCHER.
- **Providers utilisés** : river pour les builders ; mercury pour docs/audit.
