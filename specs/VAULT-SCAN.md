# Objectif

Documenter le comportement du scan récursif d'un vault (ensemble de fichiers Markdown).

## Comportements

1. Le scan parcourt récursivement tous les fichiers `.md` du vault. — INFERRED
2. Les dossiers cachés (dont le nom commence par `.`, par exemple `.obsidian`) sont ignorés. — INFERRED
3. Les chemins retournés sont relatifs au root du vault ; les accents et les espaces dans les noms sont préservés. — INFERRED
4. Un frontmatter invalide produit un champ `frontmatter_error` égal à `Some(message)`, et jamais une panique. — INFERRED
5. Les tags sont fusionnés (frontmatter + inline), dédoublonnés, et ordonnés selon leur première apparition. — INFERRED
6. Les wikilinks sont extraits avec leur cible et leur alias. — INFERRED
7. `duplicate_names` détecte les doublons de stem (nom de fichier sans extension). — INFERRED
8. Le scan d'un chemin inexistant retourne une erreur (`anyhow`). — INFERRED

## Limites et hors périmètre

- Le suivi des liens symboliques, le tri déterministe des résultats de scan et la gestion des fichiers `.md` illisibles (permissions) ne sont pas documentés ici.
- Les types de fichiers autres que `.md` sont exclus du scan.
- Le watcher en temps réel et la ré-indexation incrémentale sont hors périmètre.

## Label

Phase 0 : tous les comportements ci-dessus sont documentés à partir du format Markdown, sans observation REA — chacun est étiqueté **INFERRED**.
