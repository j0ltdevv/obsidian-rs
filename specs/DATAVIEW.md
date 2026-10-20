# Objectif

Documenter le filtrage et le tri des notes par champ de frontmatter (style
« Dataview ») : conjonction de conditions, tri stable ascendant/descendant,
exclusion des notes sans champ, et conversion numérique des valeurs.

## Comportements

### 1. Modèle de données

1. Une condition porte sur un champ du frontmatter YAML d'une note : `Query`
   associe un `field` (nom du champ) à un `FieldCmp` (la comparaison à
   appliquer). — INFERRED
2. `FieldCmp` propose quatre comparaisons : `Eq(String)` (égalité sur la forme
   texte), `Contains(String)` (sous-chaîne sur la forme texte), `Gt(f64)`
   (strictement supérieur, numérique) et `Lt(f64)` (strictement inférieur,
   numérique). — INFERRED
3. La valeur d'un champ est lue dans `note.metadata.frontmatter` : un champ est
   absent si le frontmatter est `None`, s'il n'est pas un mapping YAML, ou si la
   clé est absente du mapping. — INFERRED

### 2. Filtrage (`filter_notes`)

4. `filter_notes(notes, queries)` retourne les indices des notes qui satisfont
   **toutes** les requêtes (conjonction logique, AND) ; l'ordre d'origine des
   notes est conservé. — INFERRED
5. Une requête vide ne filtre rien : toutes les notes sont retenues, dans leur
   ordre d'origine. — INFERRED
6. Une note qui ne possède pas le champ d'une requête ne satisfait jamais cette
   requête, donc est exclue du résultat. — INFERRED
7. `Eq` compare la forme texte de la valeur à la chaîne attendue ; `Contains`
   vérifie que la forme texte contient la sous-chaîne. — INFERRED
8. `Gt` et `Lt` comparent numériquement : la valeur doit être lisible en `f64`
   (nombre YAML ou chaîne numérique) et la borne est strictement exclue. —
   INFERRED
9. Une valeur `Value::Null` (ex. `title: `) est présente mais vide : elle
   satisfait `Eq("")` et `Contains("")`, mais diffère d'une chaîne non vide. —
   INFERRED
10. Les séquences et mappings sont sérialisés en YAML compact (ex.
    `[rust, test]`) pour leur forme texte, afin que le contenu reste lisible. —
    INFERRED

### 3. Tri (`sort_by_field`)

11. `sort_by_field(notes, field, ascending, indices)` réordonne `indices` selon
    la valeur du champ `field` dans les notes. — INFERRED
12. `ascending = true` : valeurs croissantes, puis les notes sans le champ. —
    INFERRED
13. `ascending = false` : valeurs décroissantes, puis les notes sans le champ
    (les notes sans champ restent donc en fin de liste quel que soit le sens).
    — INFERRED
14. Les notes qui partagent la même valeur conservent leur ordre relatif : le tri
    est stable. — INFERRED
15. Un `indices` vide ne fait rien. — INFERRED
16. Un indice hors de `notes` provoque un `panic` (accès direct à
    `notes[left]`/`notes[right]`). — INFERRED

### 4. Conversion numérique

17. La comparaison numérique tente de lire chaque valeur en `f64` : un
    `Value::Number` est lu directement, une `Value::String` est parsée ; toute
    autre forme (booléen, séquence, mapping, `null`, chaîne non numérique) n'est
    pas numérique. — INFERRED
18. Si les deux valeurs sont numériques, la comparaison est numérique ; sinon,
    c'est une comparaison lexicale sur leur forme texte. — INFERRED
19. Un nombre YAML et une chaîne numérique égaux (ex. `5` et `"5"`) sont
    considérés comme égaux numériquement et conservent leur ordre d'origine
    (stabilité). — INFERRED
20. Une valeur non numérique est exclue par `Gt`/`Lt` (la comparaison échoue) et
    est traitée comme du texte par le tri. — INFERRED

## Limites et hors périmètre

- Le filtrage et le tri portent uniquement sur le frontmatter YAML : pas de
  filtrage sur le contenu, les tags, les wikilinks ou les métadonnées du scan.
- Les comparaisons `Eq` et `Contains` sont sensibles à la casse (elles comparent
  des chaînes brutes) ; seul le tri texte utilise l'ordre lexical par défaut.
- `sort_by_field` modifie son argument `indices` en place et ne retourne rien ;
  elle ne valide pas les indices (panic sur hors-borne).
- `filter_notes` retourne des indices dans l'ordre du vault, pas dans l'ordre
  d'une quelconque clé de tri : le tri est une opération distincte.
- Les listes de valeurs (ex. `tags: [a, b]`) ne sont pas éclatées : une
  condition `Contains("rust")` porte sur la chaîne sérialisée, pas sur chaque
  élément.
- Aucune syntaxe de requête textuelle n'est parsée : les `Query` sont construits
  à la main par l'appelant.

## Label

Phase 3 : tous les comportements ci-dessus sont déduits du module
`vault::dataview`, sans observation REA — chacun est étiqueté **INFERRED**.
