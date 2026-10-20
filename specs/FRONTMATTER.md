# Objectif

Documenter le comportement de parsing du frontmatter : le bloc YAML optionnel placé en tête d'un document Markdown, délimité par deux lignes `---`.

## Comportements

1. Un document qui ne commence pas par une ligne `---` (un BOM UTF-8 en tête est toléré) n'a pas de frontmatter : le parseur retourne `Ok(None)`. — INFERRED
2. Un bloc valide commence par une première ligne `---`, contient du YAML, puis une ligne de fermeture `---` (une ligne dont le trim est exactement `---`) : le contenu est parsé en un mapping YAML (`Ok(Some(Value))`). — INFERRED
3. Du YAML invalide dans le bloc produit l'erreur `InvalidYaml`. — INFERRED
4. Un bloc ouvrant sans ligne de fermeture produit l'erreur `Unterminated`. — INFERRED
5. Une ligne `---` à l'intérieur du contenu du bloc ne ferme pas le bloc. — INFERRED
6. Un bloc vide (`---` immédiatement suivi de `---`) est parsé en `Value::Null`. — INFERRED
7. Le corps du document situé après le bloc de fermeture n'est pas parsé comme YAML. — INFERRED

## Limites et hors périmètre

- Seul le frontmatter en tête de document est pris en compte ; aucun bloc YAML ailleurs dans le fichier n'est traité.
- Le format exact de la ligne d'ouverture (trim toléré ou non), les variations d'encodage autres que le BOM UTF-8, et les délimiteurs alternatifs (ex. `+++`) ne font pas partie des comportements documentés.
- La conversion du YAML vers des types métier (validation des clés, typage des valeurs, défauts) est hors périmètre.
- L'écriture, la sérialisation ou la modification du frontmatter sont hors périmètre.

## Label

Phase 0 : tous les comportements ci-dessus sont documentés à partir du format Markdown, sans observation REA — chacun est étiqueté **INFERRED**.
