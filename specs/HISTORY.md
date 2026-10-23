# Objectif

Documenter les versions locales (snapshots) des notes du vault.

## Comportements

1. Les snapshots sont stockés dans un dossier caché à la racine du vault,
   `<root>/.obsy-history`, donc ignoré par le scan du vault. —
   INFERRED
2. Chaque note possède son propre sous-dossier, reflétant son arborescence
   relative : `dossier/note.md/`. — INFERRED
3. Le nom d'un snapshot commence par l'horodatage système en millisecondes,
   suivi d'un numéro de séquence : `<millis>-<séquence>.md` ; la séquence
   garantit unicité et ordre même pour deux snapshots pris dans la même
   milliseconde. — INFERRED
4. L'écriture est atomique : le contenu est écrit dans un fichier temporaire
   unique (créé en `create_new`), puis lié sous son nom définitif. — INFERRED
5. Aucun snapshot n'est écrasé ni supprimé : le lien définitif échoue si le nom
   existe déjà, même en cas d'accès concurrent. — INFERRED
6. Les dossiers intermédiaires sont créés à la demande ; la racine du vault doit
   exister et être un dossier (sinon erreur). — INFERRED
7. `list_snapshots` retourne les snapshots d'une note du plus ancien au plus
   récent ; liste vide si aucun snapshot n'existe. — INFERRED
8. `restore_snapshot` retourne le contenu exact du snapshot ; le chemin du
   snapshot est confiné au dossier de snapshots de la note, tout chemin extérieur
   étant refusé. — INFERRED
9. Les chemins relatifs sont confinés : rejet des chemins absolus, vides ou
   contenant `..`, avec normalisation des composants `.` redondants. —
   INFERRED

## Limites et hors périmètre

- L'écriture de la note restaurée reste à la charge de l'appelant
  (`ops::write_note`) ; `restore_snapshot` ne fait que lire le contenu.
- Aucune rétention ni limite de taille : les snapshots s'accumulent sans purge.
- Pas de diff, de comparaison ni de métadonnées de snapshot autres que
  l'horodatage/séquence.

## Label

Phase 2 : tous les comportements ci-dessus sont déduits du module
`vault::history`, sans observation REA — chacun est étiqueté **INFERRED**.
