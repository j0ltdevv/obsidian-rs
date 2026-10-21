//! Tests d'intégration de la corbeille (`obsidian_rs::vault::trash`).
//!
//! Le format testé reproduit exactement celui produit par l'application dans
//! `src/app/mod.rs` (`Message::DeleteConfirm` / `Message::DeleteSelected`) :
//! une note supprimée est déplacée vers `.trash/<horodatage>/<chemin>`.
//! Tous les tests s'exécutent dans un répertoire temporaire, jamais dans le
//! vault du dépôt.

use std::fs;
use std::path::{Path, PathBuf};

use obsidian_rs::vault::trash::{empty_trash, list_trash, restore_trash, TrashEntry, TrashError};

/// Construit un vault de test isolé dans le répertoire temporaire.
fn temp_vault(nom: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("trash-{nom}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("création du vault de test");
    dir
}

/// Simule la suppression d'une note : écrit `contenu` dans `relatif` puis la
/// déplace vers `.trash/<stamp>/<relatif>`, exactement comme l'application.
fn simuler_suppression(vault: &Path, relatif: &Path, stamp: u64, contenu: &str) -> PathBuf {
    let note = vault.join(relatif);
    fs::create_dir_all(note.parent().expect("parent de la note")).expect("dossiers notes");
    fs::write(&note, contenu).expect("écriture de la note");
    let corbeille = vault.join(".trash").join(stamp.to_string()).join(relatif);
    fs::create_dir_all(corbeille.parent().expect("parent de la corbeille"))
        .expect("dossiers corbeille");
    fs::rename(&note, &corbeille).expect("mise à la corbeille");
    corbeille
}

#[test]
fn inventaire_apres_suppression() {
    let vault = temp_vault("inventaire");
    let stamp = 1700000000u64;
    simuler_suppression(
        &vault,
        Path::new("notes/journal.md"),
        stamp,
        "# Journal\ncontenu",
    );
    simuler_suppression(
        &vault,
        Path::new("notes/sous/note.md"),
        stamp + 100,
        "autre",
    );

    let items = list_trash(&vault).expect("inventaire");
    assert_eq!(items.len(), 2);

    // Tri croissant par date de suppression.
    assert_eq!(items[0].deleted_at, stamp);
    assert_eq!(items[0].original_path, PathBuf::from("notes/journal.md"));
    assert_eq!(items[0].size, "# Journal\ncontenu".len() as u64);
    assert_eq!(items[1].deleted_at, stamp + 100);
    assert_eq!(items[1].original_path, PathBuf::from("notes/sous/note.md"));

    fs::remove_dir_all(&vault).unwrap();
}

#[test]
fn restauration_retrouve_la_note_a_son_emplacement_et_contenu() {
    let vault = temp_vault("restauration");
    let contenu = "# Note\ncontenu original";
    let corbeille = simuler_suppression(
        &vault,
        Path::new("archives/travail.md"),
        1700000000,
        contenu,
    );

    let items = list_trash(&vault).expect("inventaire");
    assert_eq!(items.len(), 1);
    restore_trash(&vault, &items[0]).expect("restauration");

    let restauree = vault.join("archives/travail.md");
    assert!(restauree.is_file(), "la note doit être restaurée");
    assert_eq!(
        fs::read_to_string(&restauree).expect("lecture de la note restaurée"),
        contenu
    );
    assert!(
        !corbeille.exists(),
        "l'entrée de corbeille doit disparaître"
    );
    assert!(
        list_trash(&vault)
            .expect("inventaire après restauration")
            .is_empty(),
        "la corbeille doit être vide après restauration"
    );

    fs::remove_dir_all(&vault).unwrap();
}

#[test]
fn restauration_refusee_quand_le_chemin_est_occupe() {
    let vault = temp_vault("occupe");
    let corbeille = simuler_suppression(
        &vault,
        Path::new("note.md"),
        1700000000,
        "version corbeille",
    );

    // Réoccupe le chemin d'origine avec un autre contenu.
    let note = vault.join("note.md");
    fs::write(&note, "version actuelle").expect("écriture de la cible");

    let items = list_trash(&vault).expect("inventaire");
    let err = restore_trash(&vault, &items[0]).expect_err("la restauration doit échouer");
    assert!(
        matches!(err, TrashError::DestinationExists(_)),
        "erreur inattendue : {err}"
    );
    // La cible est intacte, rien n'est écrasé.
    assert_eq!(
        fs::read_to_string(&note).expect("lecture de la cible"),
        "version actuelle"
    );
    assert!(
        corbeille.exists(),
        "l'entrée de corbeille doit être conservée"
    );

    fs::remove_dir_all(&vault).unwrap();
}

#[test]
fn vidage_supprime_le_contenu_de_la_corbeille() {
    let vault = temp_vault("vidage");
    simuler_suppression(&vault, Path::new("a.md"), 1700000000, "a");
    simuler_suppression(&vault, Path::new("b.md"), 1700000001, "b");
    assert_eq!(list_trash(&vault).expect("inventaire").len(), 2);

    empty_trash(&vault).expect("vidage");
    assert!(
        !vault.join(".trash").exists(),
        "la corbeille doit avoir disparu après vidage"
    );
    assert!(
        list_trash(&vault)
            .expect("inventaire après vidage")
            .is_empty(),
        "l'inventaire doit être vide après vidage"
    );

    fs::remove_dir_all(&vault).unwrap();
}

#[test]
fn entree_corrompue_donne_une_erreur_sans_panique() {
    let vault = temp_vault("corrompue");
    // Un dossier d'horodatage qui n'est pas un nombre : entrée illisible.
    let corrompu = vault.join(".trash").join("pas_un_nombre").join("note.md");
    fs::create_dir_all(corrompu.parent().expect("parent")).expect("dossiers");
    fs::write(&corrompu, "x").expect("écriture de l'entrée corrompue");

    let err = list_trash(&vault).expect_err("l'inventaire doit échouer");
    assert!(
        matches!(err, TrashError::Unreadable(_)),
        "erreur inattendue : {err}"
    );

    fs::remove_dir_all(&vault).unwrap();
}

#[test]
fn restauration_refusee_pour_un_chemin_piege() {
    let vault = temp_vault("piege");
    fs::create_dir_all(vault.join(".trash")).expect("corbeille");

    let remontee = TrashEntry {
        original_path: PathBuf::from("../hors.txt"),
        deleted_at: 1700000000,
        size: 0,
    };
    let err = restore_trash(&vault, &remontee).expect_err("le chemin `..` doit être refusé");
    assert!(
        matches!(err, TrashError::InvalidRelativePath(_)),
        "erreur inattendue : {err}"
    );

    let absolue = TrashEntry {
        original_path: PathBuf::from("/etc/passwd"),
        deleted_at: 1700000000,
        size: 0,
    };
    let err = restore_trash(&vault, &absolue).expect_err("le chemin absolu doit être refusé");
    assert!(
        matches!(err, TrashError::InvalidRelativePath(_)),
        "erreur inattendue : {err}"
    );

    fs::remove_dir_all(&vault).unwrap();
}

#[cfg(unix)]
#[test]
fn lien_symbolique_piege_refuse() {
    use std::os::unix::fs::symlink;

    let vault = temp_vault("symlink");
    // Fichier extérieur au vault, cible du lien piégé.
    let exterieur = std::env::temp_dir().join(format!("trash-exterieur-{}", std::process::id()));
    fs::write(&exterieur, "secret").expect("écriture du fichier extérieur");

    let stamp_dir = vault.join(".trash").join("1700000000");
    fs::create_dir_all(&stamp_dir).expect("dossier d'horodatage");
    let piege = stamp_dir.join("evasion.md");
    symlink(&exterieur, &piege).expect("création du lien symbolique");

    // L'inventaire refuse de suivre le lien et ne lit pas le fichier extérieur.
    let err = list_trash(&vault).expect_err("l'inventaire doit refuser le lien");
    assert!(
        matches!(err, TrashError::SymlinkEntry(_)),
        "erreur inattendue : {err}"
    );

    // La restauration refuse aussi une entrée manuelle pointant sur un lien.
    let entree = TrashEntry {
        original_path: PathBuf::from("evasion.md"),
        deleted_at: 1700000000,
        size: 0,
    };
    let err = restore_trash(&vault, &entree).expect_err("la restauration doit refuser le lien");
    assert!(
        matches!(err, TrashError::SymlinkEntry(_) | TrashError::Unreadable(_)),
        "erreur inattendue : {err}"
    );

    let _ = fs::remove_file(&exterieur);
    fs::remove_dir_all(&vault).unwrap();
}
