//! Tests de l'interface de la corbeille, sans fenêtre ni pixels.
//!
//! Les fonctions testées sont la logique pure du panneau : mise en forme des
//! lignes (`trash_rows`), conversions date/taille, messages de conflit de
//! restauration, et décision d'ouverture de la confirmation de vidage.
//! Les deux derniers tests vérifient le câblage des `Message` de la
//! corbeille sur un vault temporaire (restauration, refus d'un vidage non
//! confirmé, vidage après confirmation explicite).

use std::path::{Path, PathBuf};

use obsidian_rs::app::{
    boot, format_deleted_at, human_size, restore_conflict_message, should_open_trash_confirm,
    update, Message, SidebarTab,
};
use obsidian_rs::ui::trash_view::trash_rows;
use obsidian_rs::vault::trash::{list_trash, TrashEntry};

/// Construit une entrée de corbeille pour les tests.
fn entry(path: &str, deleted_at: u64, size: u64) -> TrashEntry {
    TrashEntry {
        original_path: PathBuf::from(path),
        deleted_at,
        size,
    }
}

/// Vault temporaire isolé (nettoyé à la fin du test).
fn temp_vault(nom: &str) -> PathBuf {
    let mut dir = std::env::temp_dir();
    dir.push(format!("obsidian-rs-{nom}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("création du vault temporaire");
    dir
}

/// Simule la suppression d'une note : déplace `relatif` vers
/// `.trash/<stamp>/<relatif>`, exactement comme l'application.
fn simuler_suppression(vault: &Path, relatif: &Path, stamp: u64) {
    let dest = vault.join(".trash").join(stamp.to_string()).join(relatif);
    std::fs::create_dir_all(dest.parent().unwrap()).expect("création du dossier corbeille");
    std::fs::rename(vault.join(relatif), &dest).expect("déplacement vers la corbeille");
}

#[test]
fn lignes_formatees_chemin_date_taille() {
    let rows = trash_rows(&[entry("dossier/note.md", 0, 1024)]);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].path, "dossier/note.md");
    assert_eq!(rows[0].date, "1970-01-01 00:00");
    assert_eq!(rows[0].size, "1.0 Ko");
}

#[test]
fn taille_humaine_o_ko_mo_go() {
    assert_eq!(human_size(0), "0 o");
    assert_eq!(human_size(512), "512 o");
    assert_eq!(human_size(1024), "1.0 Ko");
    assert_eq!(human_size(1536), "1.5 Ko");
    assert_eq!(human_size(1048576), "1.0 Mo");
    assert_eq!(human_size(1073741824), "1.0 Go");
}

#[test]
fn date_suppression_civile() {
    assert_eq!(format_deleted_at(0), "1970-01-01 00:00");
    assert_eq!(format_deleted_at(86_400), "1970-01-02 00:00");
    // 2024-01-01T00:00:00Z
    assert_eq!(format_deleted_at(1_704_067_200), "2024-01-01 00:00");
    // Milieu de journée : heures/minutes préservées.
    assert_eq!(
        format_deleted_at(1_704_067_200 + 13 * 3600 + 42 * 60),
        "2024-01-01 13:42"
    );
}

#[test]
fn message_conflit_restauration() {
    use obsidian_rs::vault::trash::TrashError;

    let destination = TrashError::DestinationExists("notes/a.md".to_string());
    let message = restore_conflict_message(&destination).expect("message attendu");
    assert!(message.contains("existe déjà"), "message : {message}");
    assert!(message.contains("notes/a.md"), "message : {message}");

    // Une erreur d'E/S générique n'a pas de message de conflit dédié.
    let io = TrashError::Io(std::io::Error::other("disque"));
    assert!(restore_conflict_message(&io).is_none());
}

#[test]
fn decision_ouverture_confirmation_vidage() {
    // Sans entrée, on n'ouvre jamais la confirmation (rien à vider).
    assert!(!should_open_trash_confirm(0, false));
    // Avec des entrées et aucune confirmation en attente : on ouvre.
    assert!(should_open_trash_confirm(3, false));
    // Confirmation déjà en attente : on ne ré-ouvre pas.
    assert!(!should_open_trash_confirm(3, true));
    assert!(!should_open_trash_confirm(0, true));
}

#[test]
fn onglet_corbeille_selectionne_charge_l_inventaire() {
    let (mut app, _task) = boot();
    let _ = update(&mut app, Message::SetSidebarTab(SidebarTab::Corbeille));
    assert_eq!(app.sidebar_tab, SidebarTab::Corbeille);
}

#[test]
fn restauration_via_message_remet_la_note_en_place() {
    let dir = temp_vault("restore-ui");
    std::fs::write(dir.join("note.md"), "contenu").unwrap();
    simuler_suppression(&dir, Path::new("note.md"), 1000);

    let (mut app, _task) = boot();
    app.vault_root = dir.clone();
    app.trash_entries = list_trash(&dir).unwrap();
    assert_eq!(app.trash_entries.len(), 1);

    let entry = app.trash_entries[0].clone();
    let _ = update(&mut app, Message::TrashRestore(entry));

    assert!(
        dir.join("note.md").exists(),
        "la note doit revenir à sa place"
    );
    assert!(!dir.join(".trash").join("1000").join("note.md").exists());

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn vidage_non_confirme_refuse_puis_confirme_supprime() {
    let dir = temp_vault("vide-ui");
    std::fs::write(dir.join("note.md"), "contenu").unwrap();
    simuler_suppression(&dir, Path::new("note.md"), 2000);

    let (mut app, _task) = boot();
    app.vault_root = dir.clone();
    app.trash_entries = list_trash(&dir).unwrap();
    assert!(!app.trash_entries.is_empty());

    // Vidage sans demande préalable : aucun effet.
    let _ = update(&mut app, Message::TrashEmptyConfirm);
    assert!(!app.trash_empty_confirm);
    assert!(!app.trash_entries.is_empty());
    assert!(dir.join(".trash").exists(), "aucun vidage silencieux");

    // Demande de vidage → confirmation en attente, puis vidage effectif.
    let _ = update(&mut app, Message::TrashEmptyRequest);
    assert!(app.trash_empty_confirm);
    let _ = update(&mut app, Message::TrashEmptyConfirm);
    assert!(!app.trash_empty_confirm);
    assert!(!dir.join(".trash").exists(), "la corbeille doit être vidée");
    assert!(app.trash_entries.is_empty());

    let _ = std::fs::remove_dir_all(&dir);
}
