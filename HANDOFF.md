# Handoff — obsidian-rs

Fiche de reprise compacte. **Source de vérité : ce dépôt local** (`/home/jolt/obsidian-rs`, git).
La copie dans Instabox est un bac à sable jetable : ne jamais y éditer.

## État

- Version `0.2.0`, tag `v0.2.0`, worktree propre.
- **531 tests verts** (+2 tests `#[ignore]` = benchmarks), `cargo clippy --all-targets -- -D warnings` clean.
- Portes qualité : `cargo fmt --all` → `build` → `test` → `clippy -D warnings`.
- Travailler par petits commits verts ; jamais de `#[allow]`, jamais de `unwrap()` sur entrée/fichier.
- Écriture fichier atomique (temp + rename) partout.

## Livré (UI façon Obsidian)

Ribbon d'icônes Lucide (SVG embarqués) · barre d'état · sidebar gauche à onglets
(Fichiers/Recherche/Tags/Backlinks/Calendrier/Pièces jointes) · multi-onglets de notes
(tampons non sauvegardés préservés) · sidebar droite (Backlinks + Plan) · panneaux
redimensionnables `pane_grid` avec disposition persistée · thème iced construit depuis
les tokens du vault · police Inter embarquée (OFL) · graphe zoom molette + pan + noms/quadrillage.

Modules clés : `src/app/mod.rs` (état + I/O), `src/ui/` (rendu pur), `src/vault/` (logique testable),
`src/ui/theme.rs` (tokens), `assets/` (Inter + Lucide + licences, cf. `THIRD-PARTY.md`).

## Vérification visuelle (débloquée)

Le local ne peut pas capturer la fenêtre (pas de WM, wgpu sans GPU → capture noire).
Instabox `hermes` fournit un agent **OpenCode v2.1.2** avec **vision gratuite** :

```sh
# 1. copier le dépôt (bac à sable)
tar czf /tmp/repo.tgz --exclude=target --exclude=.git -C . . \
  && instabox ssh hermes -- 'cat > /tmp/repo.tgz' < /tmp/repo.tgz \
  && instabox ssh hermes -- 'mkdir -p /data/home/obsidian-rs && tar xzf /tmp/repo.tgz -C /data/home/obsidian-rs'

# 2. build (SSH : pas de limite de temps ; `exec` coupe à 60 s)
instabox ssh hermes -- 'cd /data/home/obsidian-rs && cargo build --locked'

# 3. lancer sous Xvfb + capturer
instabox ssh hermes -- 'Xvfb :99 -screen 0 1280x800x24 & sleep 2; DISPLAY=:99 /data/home/obsidian-rs/target/debug/obsidian-rs & sleep 8; WID=$(DISPLAY=:99 xdotool search --onlyvisible --name . | head -1); DISPLAY=:99 import -window $WID /tmp/cap.png'

# 4. décrire la capture (modèle gratuit AVEC vision)
instabox ssh hermes -- 'cd /tmp && opencode run -m opencode/minimax-m3.1-flash-free "Décris cette UI: @cap.png"'

# 5. rapatrier si besoin
instabox files hermes /tmp/cap.png
```

Clé Zen : dans le Vault instabox (`OPENCODE_API_KEY`), jamais dans le dépôt.
Modèles : `opencode models`. Vision gratuite constatée : `minimax-m3.1-flash-free`.
(Quota gratuit **par modèle** ; certains sont épuisés.)

## Reste à faire (7 items)

Faisables et vérifiables :
- **Chiffrement optionnel du vault** (clé locale, AEAD) — touche tous les chemins lecture/écriture.
- **Accessibilité** : contraste AA déjà testé (`themes.rs`) ; reste la navigation clavier complète.
- **i18n fr/en** : extraire les chaînes (refactor large, risque de régression).

Hors environnement (ne pas coder de stub, ne pas cocher) :
- Packaging `.deb`/AppImage, tray/login, alerte de mise à jour (réseau), télémétrie.

Bloqués (décision d'architecture, hors solo) :
- Sync multi-appareils, app mobile, compatibilité plugins tiers JS/CSS.

## Pièges connus

- `cargo run` est ambigu (2 binaires) → `cargo run --bin obsidian-rs`.
- `iced::time::every` exige la feature `tokio` (non activée).
- `Content::text()` renvoie `String` → passer `&…`.
- `text_editor::Cursor` est une **struct** (`position`, `selection`), pas l'enum souris.
- `pane_grid` : ratios par défaut 50/25/12.5/12.5 sans `state.resize`.
- Delta de drag du graphe : publier en **espace monde** (diviser par le zoom).
