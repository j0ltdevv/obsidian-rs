# Objectif

Documenter le comportement de l'extraction des embeds `![[...]]` : parsing,
classification par type et résolution vers les notes du vault.

## Comportements

1. Seuls les **embeds** `![[...]]` sont extraits ; un wikilink `[[...]]`
   sans `!` n'est pas un embed et est ignoré. — INFERRED
2. Le contenu d'un embed ne traverse pas les lignes : aucun `]]` trouvé avant
   la fin de la ligne (ou du document) → le fragment est ignoré. — INFERRED
3. La cible est le texte entre `![[` et `]]`, avant toute section et tout
   alias, trimmé. — INFERRED
4. L'**alias** suit le premier `|`, est trimmé, et est ignoré s'il est vide. — INFERRED
5. La **section** suit le premier `#` de la cible (heading ou identifiant de
   bloc `^id`), est trimmée, et est ignorée si elle est vide. L'ordre canonique
   est `cible#section|alias`. — INFERRED
6. Le **type** d'un embed est déduit de l'extension de la cible, comparée
   insensible à la casse : `png`, `jpg`, `jpeg`, `gif`, `svg`, `webp`,
   `bmp` → image ; `pdf` → PDF ; toute autre extension → autre ; cible sans
   extension → note. Seule la dernière extension du nom de fichier compte
   (après le dernier `/` et le dernier `.`). — INFERRED
7. Les embeds sont retournés dans l'ordre d'apparition dans le document. — INFERRED
8. Les **blocs de code** délimités par trois backticks ou tildes ( ``` / ~~~ )
   sont ignorés : ni leur contenu ni leurs délimiteurs ne produisent d'embed.
   Un bloc non refermé s'étend jusqu'à la fin du document. — INFERRED
9. `resolve_embed` délègue à l'index uniquement pour un embed de type **note**
   **sans section** ; tout autre embed (image, PDF, autre, ou avec une section)
   retourne une liste vide. — INFERRED
10. La délégation retourne **toutes** les notes dont le stem correspond à la
    cible, doublons homonymes compris (comportement de `NoteIndex::resolve`). — INFERRED

## Limites et hors périmètre

- L'ordre alternatif `cible|alias#section` n'est pas supporté : la section
  n'est reconnue qu'avant l'alias.
- Le code en ligne (`` `![[x]]` ``) et les blocs de code indentés (4 espaces)
  ne sont pas traités comme du code : un embed y figurant serait extrait.
- Une référence de bloc sans `#` (`![[note^id]]`) garde `^id` dans la cible ;
  elle n'est pas traitée comme une section.
- Le caractère d'échappement `\![[...]]` n'est pas géré : l'embed est tout de
  même extrait.
- La section n'est pas validée ni résolue : un embed avec section retourne
  toujours une liste vide, sans vérifier que le heading ou le bloc existe.
- La résolution ne porte que sur des notes ; images, PDF et autres retournent
  toujours une liste vide.
- Pas de rendu ni de lecture des fichiers cibles : seul le nom est analysé.

## Label

Phase 0 : tous les comportements ci-dessus sont déduits de la syntaxe Markdown
des embeds et de l'API de `NoteIndex`, sans observation REA — chacun est
étiqueté **INFERRED**.
