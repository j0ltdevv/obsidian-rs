# Objectif

Documenter le comportement du module `src/vault/vault_archive.rs` : export et
import d'un vault entier en archive ZIP (format standard, chemins relatifs
préservés, écriture atomique).

## Comportements

1. `export_vault_zip(vault_root, dest)` archive tout le contenu de `vault_root` (fichiers et dossiers, y compris les dossiers vides) avec leurs chemins relatifs au root ; les accents et les espaces dans les noms sont préservés. — OBSERVED
2. Les dossiers cachés (dont le nom commence par `.`, par exemple `.obsidian` et `.trash`) sont ignorés à l'export, comme lors du scan ; les fichiers cachés ne sont pas filtrés. — OBSERVED
3. L'export est atomique : l'archive est d'abord écrite dans un fichier `.zip.tmp` du dossier de destination, puis déplacée vers `dest` via `rename` ; si la sortie (ou son temporaire) vit dans le vault, elle est exclue de l'archive pour éviter l'auto-inclusion. — OBSERVED
4. `import_vault_zip(zip_path, vault_root)` extrait les entrées dans `vault_root`, crée les sous-dossiers au besoin, et retourne le nombre de fichiers extraits. — OBSERVED
5. L'import refuse toute entrée non confinée : chemin vide, composant `..` (remontée hors du vault), chemin absolu, préfixe/racine ; une telle entrée produit une erreur `anyhow` et aucun fichier n'est écrit hors de `vault_root`. — OBSERVED
6. L'import n'écrase jamais silencieusement un fichier existant : si la destination existe, un suffixe `-importé` est ajouté avant l'extension (puis `-importé-N` si nécessaire) jusqu'à trouver une destination libre ; au-delà de 10 000 tentatives, erreur documentée. — OBSERVED
7. Un roundtrip export → import restaure la hiérarchie complète du vault, y compris les dossiers vides. — OBSERVED
8. Toutes les erreurs sont des `Result` `anyhow` avec contexte ; aucune panique sur entrée ou fichier invalide. — OBSERVED

## Limites et hors périmètre

- Les liens symboliques ne sont ni suivis ni archivés à l'export (un lien est ignoré), et ne sont jamais recréés à l'import ; l'import écrit les entrées non-répertoire comme fichiers réguliers.
- Les métadonnées (permissions, dates, propriétaire) ne sont ni préservées à l'export ni restaurées à l'import.
- La compression utilisée est celle par défaut de la crate `zip` (`SimpleFileOptions`).
- Les noms d'entrée non UTF-8 sont décodés de façon lossy (remplacés par U+FFFD) ; la ronde parfaite des octets n'est pas garantie pour ces noms.
- Les doublons d'entrées au sein d'une même archive sont traités comme des fichiers distincts (le second prend un suffixe `-importé`).
- Le chiffrement, les archives multi-volumes et les extensions ZIP exotiques sont hors périmètre.

## Label

Phase 0 : tous les comportements ci-dessus sont documentés depuis l'implémentation livrée de `vault_archive.rs` et vérifiés par le harnais de tests (`cargo test`, roundtrip export→import dans des dossiers temporaires) — chacun est étiqueté **OBSERVED**.
