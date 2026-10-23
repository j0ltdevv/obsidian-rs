# Objectif

Documenter l'interface utilisateur de l'historique local d'une note : la liste
des snapshots, la restauration par clic, le surlignage du snapshot courant, et le
dossier caché qui les stocke.

## Comportements

### 1. Stockage (dossier caché)

1. Les snapshots sont stockés dans un dossier caché à la racine du vault,
   `<root>/.obsy-history`, donc ignoré par le scan du vault (les dossiers
   dont le nom commence par `.` sont ignorés). — INFERRED
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
6. Les fichiers temporaires (et tout autre fichier caché, commençant par `.`)
   sont ignorés par `list_snapshots`, notamment après un arrêt brutal. —
   INFERRED

### 2. Liste des snapshots (`history_view`)

7. La vue affiche une liste scrollable des snapshots d'une note, un bouton par
   snapshot, dans l'ordre retourné par `list_snapshots` (du plus ancien au plus
   récent). — INFERRED
8. Le texte d'un bouton est le nom court du snapshot (son dernier composant de
   chemin), pas le chemin complet ; un chemin sans composant de fichier est
   affiché tel quel. — INFERRED
9. Cliquer sur le bouton d'indice `i` appelle `on_restore(i)`. — INFERRED
10. Le snapshot `current` (indice passé à la vue, s'il y en a un) est surligné :
    fond `CURRENT_BG` et texte `CURRENT_TEXT`. — INFERRED
11. Un snapshot non courant est neutre, et se grise au survol (fond
    `HOVER_BG`). — INFERRED
12. Une liste vide produit une vue scrollable sans bouton (la hauteur reste
    `Length::Fill`). — INFERRED

### 3. Restauration

13. `restore_snapshot` retourne le contenu exact du snapshot ; le chemin du
    snapshot est confiné au dossier de snapshots de la note, tout chemin extérieur
    étant refusé. — INFERRED
14. `restore_snapshot` accepte aussi bien le chemin complet retourné par
    `list_snapshots` que le simple nom du fichier (relatif au dossier de
    snapshots de la note). — INFERRED
15. Les chemins relatifs sont confinés : rejet des chemins absolus, vides ou
    contenant `..`, avec normalisation des composants `.` redondants. — INFERRED
16. L'écriture de la note restaurée reste à la charge de l'appelant
    (`ops::write_note`) ; `restore_snapshot` ne fait que lire le contenu. —
    INFERRED

## Limites et hors périmètre

- Aucune rétention ni limite de taille : les snapshots s'accumulent sans purge.
- Pas de diff, de comparaison ni de métadonnées de snapshot autres que
  l'horodatage/séquence.
- `history_view` est une vue pure : elle ne lit pas le disque et ne connaît pas
  le contenu des snapshots, seulement leurs chemins et l'indice du courant.
- Le snapshot courant est un indice passé par l'appelant : la vue ne le déduit
  pas du contenu ni de l'état du vault.
- L'ordre d'affichage suit le tri lexical des noms de fichier (qui coïncide avec
  l'ordre chronologique grâce au préfixe milliseconde + séquence) ; aucun tri
  explicite par horodatage n'est effectué dans la vue.

## Label

Phase 3 : tous les comportements ci-dessus sont déduits des modules
`vault::history` et `ui::history_view`, sans observation REA — chacun est
étiqueté **INFERRED**.
