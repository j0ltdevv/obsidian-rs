# Objectif

Documenter l'inventaire des pièces jointes (fichiers non-note du vault) et
l'import atomique d'une pièce jointe externe dans le vault.

## Comportements

1. Une pièce jointe est un fichier dont l'extension (minuscules) appartient à une liste fixe : png, jpg, jpeg, gif, svg, webp, bmp, ico, pdf, mp4, webm, ogg, mp3, wav, flac, zip, epub. — INFERRED
2. Le scan est récursif et ignore les dossiers cachés (commençant par `.`, dont `.obsidian`). — INFERRED
3. Les fichiers `.md` ne sont jamais des pièces jointes. — INFERRED
4. Le scan retourne chemin relatif, nom, extension en minuscules et taille en octets, triés par chemin. — INFERRED
5. L'import copie le fichier source dans le dossier `attachments` à la racine du vault, de façon atomique (temporaire puis rename). — INFERRED
6. En cas de collision de nom, un suffixe numérique (`-1`, `-2`, …) est ajouté avant l'extension ; aucun fichier n'est écrasé. — INFERRED
7. Un fichier source absent est une erreur. — INFERRED
8. L'import retourne le chemin relatif créé. — INFERRED

## Limites et hors périmètre

- Pas de déduplication par contenu (hash) ; la collision est gérée par le nom.
- Le déplacement/renommage de pièces jointes existantes est hors périmètre.
- La taille retournée est la taille logique (`len()`), pas la taille sur disque.

## Label

Phase 2 : comportements déduits des conventions de vault Obsidian, sans
observation REA — chacun est étiqueté **INFERRED**.
