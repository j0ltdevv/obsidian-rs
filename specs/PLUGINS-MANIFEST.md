# Objectif

Documenter la lecture et la validation des manifests de plugins Obsidian
(`.obsidian/plugins/<id>/manifest.json`) : parsing strict, vérification des
champs requis et de l'identifiant, et énumération des plugins présents dans un
vault. Le module est un module vault pur (aucune dépendance iced) ; il traite le
manifest comme des *données* uniquement et ne charge ni n'exécute aucun code.

## Comportements

1. `parse_manifest` accepte un objet JSON valide dont `id`, `name` et `version`
   sont présents et non vides, et produit un `PluginManifest`. Les champs
   optionnels (`description`, `author`, `minAppVersion`, `isDesktopOnly`,
   `main`) valent `None` lorsqu'ils sont absents. — INFERRED
2. Un champ requis `id`, `name` ou `version` manquant ou vide produit
   `PluginManifestError::ChampsManquants` avec la liste de tous les champs en
   cause (par exemple `["name", "version"]`). — INFERRED
3. Un `id` contenant au moins un caractère hors de `[a-zA-Z0-9_-]` produit
   `PluginManifestError::DossierInvalide` ; un `id` composé uniquement de
   caractères autorisés est accepté. — INFERRED
4. Un JSON syntaxiquement invalide, qui n'est pas un objet, ou dont un champ a
   un type inattendu produit `PluginManifestError::Json`. — INFERRED
5. Les clés inconnues du JSON sont ignorées ; la sérialisation utilise les noms
   de champs Obsidian (`minAppVersion`, `isDesktopOnly`). — INFERRED
6. `load_manifest` parcourt `.obsidian/plugins/*/manifest.json` et retourne une
   entrée `(chemin relatif du dossier du plugin, manifest)` par plugin, le
   chemin étant relatif à la racine du vault (par exemple
   `.obsidian/plugins/mon-plugin`). — INFERRED
7. L'ordre des entrées retournées est déterministe : tri par nom de dossier de
   plugin. — INFERRED
8. Si le dossier `.obsidian/plugins` n'existe pas, `load_manifest` retourne une
   liste vide, sans erreur. — INFERRED
9. Un dossier de plugin sans `manifest.json` (ou dont `manifest.json` n'est pas
   un fichier régulier) produit `PluginManifestError::ChampsManquants` avec un
   message mentionnant le dossier. — INFERRED
10. Le premier problème rencontré arrête le parcours (mode fail-fast) ; les
    problèmes suivants ne sont pas rapportés. — INFERRED
11. Les fichiers présents directement sous `.obsidian/plugins` (non-dossiers)
    sont ignorés. — INFERRED
12. Toute erreur d'E/S (dossier illisible, fichier illisible…) est propagée sous
    `PluginManifestError::Io`. — INFERRED
13. `plugin_dir` retourne `vault_root/.obsidian/plugins/<id>` (concaténation
    brute) ; `id` n'y est ni validé ni nettoyé. — INFERRED

## Limites et hors périmètre

- Le manifest est parsé uniquement comme données : `main` n'est ni chargé ni
  exécuté, et aucune dépendance du plugin n'est résolue.
- Aucune écriture, modification, création ou suppression de fichier ou de
  dossier (le module est en lecture seule).
- Pas de hot-reload ni de détection de changements du manifest après lecture.
- L'ordre des erreurs agrégées n'est pas garanti au-delà du mode fail-fast
  documenté ci-dessus.

## Label

Tous les comportements ci-dessus sont étiquetés **INFERRED** : spécification de
conception du module de lecture des manifests, sans observation REA.
