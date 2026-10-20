# Objectif

Documenter l'édition batch de propriétés frontmatter : application séquentielle,
erreurs et préservation du document.

## Comportements

1. `apply_edits` applique les éditions séquentiellement, dans l'ordre donné, en
   déléguant chacune à `meta::set_frontmatter_property` ou
   `meta::remove_frontmatter_property`. — INFERRED
2. Chaque édition réussie est écrite sur disque (fichier temporaire + `rename`
   atomique via `ops::write_note`) avant que la suivante ne soit tentée. —
   INFERRED
3. Au premier edit invalide, l'erreur est retournée et les edits suivants ne
   sont pas exécutés ; les edits déjà appliqués ne sont pas annulés (pas de
   rollback, le lot n'est pas atomique). — INFERRED
4. `Set` affecte une clé : elle est créée si absente, remplacée sinon. —
   INFERRED
5. La modification d'une clé existante préserve sa position ; une nouvelle clé
   est ajoutée en fin de bloc. — INFERRED
6. Si le document n'a pas de frontmatter, un bloc est créé en tête ; le corps
   Markdown (tout ce qui suit le bloc) est préservé tel quel. — INFERRED
7. `Remove` supprime la clé du frontmatter ; une clé absente n'est pas une
   erreur (aucune écriture). — INFERRED
8. Une liste d'éditions vide réussit toujours, sans lire ni réécrire la note,
   même si le chemin n'existe pas. — INFERRED
9. Erreurs possibles : `root` n'est pas un répertoire, `rel_path`
   vide/absolu/contenant `..`, note illisible, frontmatter YAML invalide ou qui
   n'est pas un mapping, échec d'une écriture atomique. — INFERRED

## Limites et hors périmètre

- Absence de transaction : pas de tout-ou-rien ; l'appelant qui en a besoin doit
  relire l'état avant de réessayer, ou appliquer une édition unique.
- L'ordre de présentation du YAML n'est garanti que pour les clés existantes ;
  pas de tri général des clés.
- La validation sémantique des valeurs (types métier, défauts) est hors
  périmètre.

## Label

Phase 2 : tous les comportements ci-dessus sont déduits des modules
`vault::properties` et `vault::meta`, sans observation REA — chacun est
étiqueté **INFERRED**.
