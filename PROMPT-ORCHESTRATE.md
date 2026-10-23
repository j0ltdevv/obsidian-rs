orchestrate

Continuité du projet obsy. Tu es en Phase 1 terminée / Phase 2 démarrée. Applique les règles suivantes PRIORITAIREMENT sur ton rythme actuel.

## 1. RÈGLE DE PARALLÉLISME — elle prime sur tout le reste

1. MINIMUM 6 AGENTS PAR VAGUE. Cible 8. Jamais 4, 3, 2 ou 1. Si tu crois n'avoir que 4 unités de travail indépendantes, c'est que tu n'as pas assez découpé. Le découpage fait partie de ton travail.

2. AVANT DE LANCER, fais l'inventaire du reste à faire et découpe-le en ≥6 ensembles de fichiers DISJOINTS. Écris cette liste dans ton todo. Un sous-agent = un ensemble.

3. MINIMUM 4 AGENTS EN VOL à tout moment. Avant toute nouvelle vague, compte ceux qui tournent. Si tu en as moins de 4 en vol et qu'il reste du travail délégable, lance. La file ne doit jamais être vide.

4. PENDANT QUE LES AGENTS TOURNENT, tu prépares la vague SUIVANTE (contrats, périmètres, specs). Tu ne relis un résultat que quand la vague est complète — et à ce moment tu lances la suivante AVANT de relire. Jamais d'attente passive.

5. RÉSERVE POUR TOI, personne d'autre n'y touche même dans une autre vague : src/lib.rs, src/vault/mod.rs, src/app/state.rs, Cargo.toml, AGENTS.md, .gitignore. Tu intègres les `pub mod` et les mod declarations toi-même après la vague. C'est ce qui évite les conflits de merge.

6. AUCUN agent ne touche un fichier déjà modifié par un autre agent de la même vague. Si deux unités se recouvrent, fusionne-les en une seule tâche.

7. 1 `verifier` PAR VAGUE, lancé EN PARALLÈLE de la vague de code, jamais après. 1 `reviewer` sur le code frais quand il y a du code à relire.

8. `agent` TOUJOURS renseigné : builder (écrit du code), reviewer (relecture read-only), verifier (4 commandes, sortie brute, ne modifie rien), scout (exploration read-only), sonic (docs/specs/index), seance (le seul qui accepte `model`). Pour router sur deepseek : {"agent":"seance","model":"tokenplan/deepseek-v4-flash-0731"}.

9. Ne te bloque pas sur la capture d'écran. La preuve visuelle via XWayland/import échoue (ImageMagick absent). Ce n'est PAS bloquant : passe au test fonctionnel (assertion sur l'état, capture si l'outil marche, sinon note "preuve visuelle non disponible" dans le rapport et passe). Ne bloque jamais le projet sur une capture.

## 2. GIT

Un commit par feature, chaque commit doit passer les 4 commandes. Jamais de gros commit fourre-tout. Vérifie que .gitignore couvre /target et .omp/ (mais garde .omp/agents/ versionné : ce sont tes agents).

## 3. REA — va droit au graphe, ne redécouvre pas

evidence_id STABLE (hash déterministe, identique sur des runs indépendants) :
ev_00c397c132cbaf3363cb15ae435e1688a912c53660f97991d1e0abecae0c101e

inspect_analysis_view avec :
{"source":{"kind":"retained-evidence","evidence_id":"ev_00c397c132cbaf3363cb15ae435e1688a912c53660f97991d1e0abecae0c101e"},"view":{"kind":"summary"}}
puis {"view":{"kind":"page","collection":"modules","offset":0,"limit":100}}

Collections valides : sections, symbols, modules. PAS de "ipc".

Faits déjà établis (SEEN) à intégrer tels quels dans evidence/00-architecture-overview.md :
- 363 modules (420 paginés), 778 findings, 0 parse failure, 5 fichiers JS parsés
- 45 canaux IPC littéraux, 45 main_handlers, 0 dynamic, 0 renderer_transmissions
  => 45 canaux traités côté main process, AUCUNE transmission renderer appariable statiquement
- electron.preload_entrypoints 0, context_bridge_apis 0, native_addon_bindings 0
  => pas de pont preload/contextBridge dans l'ASAR, cohérent avec nodeIntegration direct
- 2 browser_windows, 1 sender_validation_observation
- text_bytes_read 4 771 961, visited_ast_nodes 1 276 779, 29 fichiers non analysés (vendor lib/)

NE RELANCE PAS l'analyse de l'asar entier : timeout interne REA à 300 s, il ne passera pas. L'analyse bornée (package.json, main.js, app.js, starter.js, worker.js, enhance.js, index.html, starter.html) a pris 97,6 s et l'evidence est déjà produite.

Extrais les 45 canaux IPC et les noms de modules (collections sections et symbols) pour documenter le CONTRAT COMPORTEMENTAL : les canaux = surface d'intégration, les modules = découpage interne. Décris en français, labels SEEN/INFERRED/UNKNOWN. AUCUN code copié.

## 4. AVANCE

Objectif : aller le plus loin possible jusqu'au bout du projet. Phases 1 → 2 → 3 livrées d'affilée, sans attendre de validation entre chaque. Tu te valides toi-même (les 4 commandes), tu commites, tu enchaînes.

Phase 2 : Backlinks, recherche fuzzy (Quick Switcher) + full-text (tantivy), properties/frontmatter éditables, graph view (basique puis avancé).
Phase 3 : Canvas, Bases/Dataview-like, versions locales, thèmes, raccourcis clavier complets.

Tu ne t'arrêtes que si : wgpu/iced ne démarre pas (déjà OK), décision d'architecture à deux réponses sans défaut net, ou règle ci-dessus contredite.

## 5. QUALITÉ — inchangé

cargo fmt --all --check
cargo build
cargo test
cargo clippy --all-targets -- -D warnings

Les 4 doivent passer. Colle la sortie brute. Jamais de unwrap() sur entrée utilisateur ou fichier, jamais de #[allow], jamais de test supprimé ou affaibli, jamais de code d'Obsidian copié.

RAPPORT EN FIN DE VAGUE (pas entre chaque agent) : les 4 commandes, fichiers touchés, ce qui reste.

Commence : ouvre une vague de 6-8 agents sur la Phase 2, avec un verifier en parallèle.
