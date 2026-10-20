//! Tests d'intégration des opérations d'écriture du vault (`src/vault/ops.rs`).
//!
//! Chaque test crée un vault temporaire unique sous `std::env::temp_dir()`,
//! nettoyé à la fin via `Drop` (aucune fixture statique nécessaire).

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use obsidian_rs::vault::ops::{create_note, move_note, write_note};

/// Génère un suffixe unique par vault (pid + compteur atomique).
fn unique_suffix() -> String {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    format!("{}-{n}", std::process::id())
}

/// Vault temporaire supprimé à la fin du test (y compris en cas de panique).
struct TempVault {
    root: PathBuf,
}

impl TempVault {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("obsidian-rs-vault-ops-{}", unique_suffix()));
        fs::create_dir_all(&root).expect("création du vault temporaire");
        Self { root }
    }

    fn join(&self, rel: &str) -> PathBuf {
        self.root.join(rel)
    }
}

impl Drop for TempVault {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

/// 1. create_note crée le fichier (contenu exact) et les dossiers
///    intermédiaires ; la `Note` retournée porte le chemin relatif.
#[test]
fn create_note_cree_fichier_et_dossiers_intermediaires() -> anyhow::Result<()> {
    let vault = TempVault::new();
    let rel = Path::new("dossier/sous-dossier/note.md");
    let content = "# Titre\n\nContenu.\n";
    let note = create_note(&vault.root, rel, content)?;

    let fichier = vault.join("dossier/sous-dossier/note.md");
    assert!(fichier.is_file(), "le fichier doit exister");
    assert!(
        vault.join("dossier/sous-dossier").is_dir(),
        "dossiers créés"
    );
    assert_eq!(fs::read_to_string(&fichier)?, content);
    assert_eq!(note.path, rel);
    assert_eq!(note.name, "note");
    Ok(())
}

/// 2. create_note sur un chemin existant → erreur, sans écrasement silencieux.
#[test]
fn create_note_sur_existant_retourne_erreur_sans_ecrasement() -> anyhow::Result<()> {
    let vault = TempVault::new();
    let rel = Path::new("note.md");
    create_note(&vault.root, rel, "# Première version")?;

    let err = create_note(&vault.root, rel, "# Deuxième version").unwrap_err();
    assert!(!err.to_string().is_empty(), "message d'erreur présent");
    assert_eq!(
        fs::read_to_string(vault.join("note.md"))?,
        "# Première version",
        "le contenu d'origine doit être conservé"
    );
    Ok(())
}

/// 3. write_note écrase atomiquement (contenu remplacé), sans laisser de
///    fichier temporaire résiduel.
#[test]
fn write_note_ecrase_atomiquement_sans_temporaire_residuel() -> anyhow::Result<()> {
    let vault = TempVault::new();
    let rel = Path::new("note.md");
    create_note(&vault.root, rel, "# Ancien")?;

    write_note(&vault.root, rel, "# Nouveau contenu")?;
    assert_eq!(
        fs::read_to_string(vault.join("note.md"))?,
        "# Nouveau contenu"
    );

    let residus: Vec<_> = fs::read_dir(&vault.root)?
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().contains(".tmp-"))
        .collect();
    assert!(
        residus.is_empty(),
        "aucun fichier temporaire ne doit subsister"
    );
    Ok(())
}

/// 4. move_note déplace la note : le nouveau chemin existe, l'ancien non.
#[test]
fn move_note_deplace_vers_nouveau_chemin() -> anyhow::Result<()> {
    let vault = TempVault::new();
    create_note(&vault.root, Path::new("source.md"), "# Déplacée")?;

    move_note(
        &vault.root,
        Path::new("source.md"),
        Path::new("cible/destination.md"),
    )?;

    assert!(
        vault.join("cible/destination.md").is_file(),
        "nouveau chemin existe"
    );
    assert!(!vault.join("source.md").exists(), "ancien chemin disparu");
    assert_eq!(
        fs::read_to_string(vault.join("cible/destination.md"))?,
        "# Déplacée",
        "contenu conservé"
    );
    Ok(())
}

/// 5. move_note vers une cible existante → erreur, aucune modification.
#[test]
fn move_note_vers_cible_existante_erreur_sans_modification() -> anyhow::Result<()> {
    let vault = TempVault::new();
    create_note(&vault.root, Path::new("a.md"), "# A")?;
    create_note(&vault.root, Path::new("b.md"), "# B")?;

    move_note(&vault.root, Path::new("a.md"), Path::new("b.md")).unwrap_err();

    assert!(vault.join("a.md").is_file(), "source intacte");
    assert_eq!(
        fs::read_to_string(vault.join("b.md"))?,
        "# B",
        "cible intacte"
    );
    Ok(())
}

/// 6. move_note sur une source inexistante → erreur.
#[test]
fn move_note_source_inexistante_erreur() -> anyhow::Result<()> {
    let vault = TempVault::new();

    move_note(&vault.root, Path::new("absente.md"), Path::new("cible.md")).unwrap_err();

    assert!(!vault.join("cible.md").exists(), "aucune cible créée");
    Ok(())
}

/// 7. create_note avec contenu vide → fichier vide créé.
#[test]
fn create_note_contenu_vide_cree_fichier_vide() -> anyhow::Result<()> {
    let vault = TempVault::new();
    let note = create_note(&vault.root, Path::new("vide.md"), "")?;

    let fichier = vault.join("vide.md");
    assert!(fichier.is_file());
    assert_eq!(fs::read_to_string(&fichier)?, "");
    assert_eq!(note.path, Path::new("vide.md"));
    assert_eq!(note.metadata.frontmatter, None);
    assert!(note.metadata.wikilinks.is_empty());
    assert!(note.metadata.tags.is_empty());
    Ok(())
}

/// 8. Les opérations échouent proprement (Err) sur un root inexistant,
///    sans panique et sans effet de bord.
#[test]
fn operations_echouent_proprement_sur_root_inexistant() {
    let root =
        std::env::temp_dir().join(format!("obsidian-rs-vault-ops-absente-{}", unique_suffix()));
    assert!(!root.exists());

    assert!(create_note(&root, Path::new("note.md"), "contenu").is_err());
    assert!(write_note(&root, Path::new("note.md"), "contenu").is_err());
    assert!(move_note(&root, Path::new("note.md"), Path::new("cible.md")).is_err());

    assert!(!root.exists(), "aucun effet de bord sur un root inexistant");
}

// 9. create_note rejette les chemins absolus (confinement).
#[test]
fn create_note_chemin_absolu_rejete() -> anyhow::Result<()> {
    let vault = TempVault::new();
    let absolu = vault.root.join("note.md");

    let err = create_note(&vault.root, &absolu, "contenu").unwrap_err();
    assert!(
        err.to_string().contains("absolu"),
        "message d'erreur explicite : {}",
        err
    );
    assert!(!vault.join("note.md").exists(), "aucun fichier créé");
    Ok(())
}

// 10. create_note rejette les chemins relatifs avec ``..`` (remontée hors du
//     vault).
#[test]
fn create_note_chemin_avec_parent_dir_rejete() -> anyhow::Result<()> {
    let vault = TempVault::new();
    let hors_vault = vault.root.parent().unwrap().join("hors-vault.md");

    let err = create_note(&vault.root, Path::new("../hors-vault.md"), "contenu").unwrap_err();
    assert!(
        err.to_string().contains(".."),
        "message d'erreur explicite : {}",
        err
    );
    assert!(!hors_vault.exists(), "aucun fichier hors du vault");
    Ok(())
}

// 11. move_note rejette les chemins absolus et ``..`` (confinement).
#[test]
fn move_note_chemins_non_confines_rejetes() -> anyhow::Result<()> {
    let vault = TempVault::new();
    create_note(&vault.root, Path::new("source.md"), "# Source")?;

    let absolu = vault.root.join("cible.md");
    assert!(move_note(&vault.root, Path::new("source.md"), &absolu).is_err());
    assert!(move_note(
        &vault.root,
        Path::new("source.md"),
        Path::new("../cible.md")
    )
    .is_err());

    assert!(vault.join("source.md").is_file(), "source intacte");
    assert!(!vault.join("cible.md").exists(), "aucune cible créée");
    Ok(())
}

// 12. write_note rejette les chemins absolus et ``..`` (confinement).
#[test]
fn write_note_chemins_non_confines_rejetes() -> anyhow::Result<()> {
    let vault = TempVault::new();
    create_note(&vault.root, Path::new("note.md"), "# Existant")?;

    let absolu = vault.root.join("autre.md");
    assert!(write_note(&vault.root, &absolu, "contenu").is_err());
    assert!(write_note(&vault.root, Path::new("../hors.md"), "contenu").is_err());

    assert_eq!(fs::read_to_string(vault.join("note.md"))?, "# Existant");
    Ok(())
}

// 13. write_note sur une note inexistante → erreur, sans créer de fichier.
#[test]
fn write_note_sur_note_inexistante_erreur() -> anyhow::Result<()> {
    let vault = TempVault::new();

    let err = write_note(&vault.root, Path::new("absente.md"), "contenu").unwrap_err();
    assert!(!err.to_string().is_empty(), "message d'erreur présent");
    assert!(!vault.join("absente.md").exists(), "aucun fichier créé");
    Ok(())
}

// 14. move_note vers un lien symbolique mort → erreur, sans écrasement.
#[cfg(unix)]
#[test]
fn move_note_vers_lien_symbolique_mort_erreur_sans_ecrasement() -> anyhow::Result<()> {
    use std::os::unix::fs::symlink;

    let vault = TempVault::new();
    create_note(&vault.root, Path::new("source.md"), "# Source")?;
    let lien_mort = vault.join("lien-mort.md");
    symlink("cible-absente.md", &lien_mort)?;

    let err = move_note(
        &vault.root,
        Path::new("source.md"),
        Path::new("lien-mort.md"),
    )
    .unwrap_err();
    assert!(!err.to_string().is_empty(), "message d'erreur présent");

    assert!(vault.join("source.md").is_file(), "source intacte");
    let meta = fs::symlink_metadata(&lien_mort)?;
    assert!(
        meta.file_type().is_symlink(),
        "le lien symbolique mort est conservé, non écrasé"
    );
    Ok(())
}

// 15. create_note vers un lien symbolique mort → erreur, sans écrasement.
#[cfg(unix)]
#[test]
fn create_note_vers_lien_symbolique_mort_erreur_sans_ecrasement() -> anyhow::Result<()> {
    use std::os::unix::fs::symlink;

    let vault = TempVault::new();
    let lien_mort = vault.join("lien-mort.md");
    symlink("cible-absente.md", &lien_mort)?;

    let err = create_note(&vault.root, Path::new("lien-mort.md"), "contenu").unwrap_err();
    assert!(!err.to_string().is_empty(), "message d'erreur présent");

    let meta = fs::symlink_metadata(&lien_mort)?;
    assert!(
        meta.file_type().is_symlink(),
        "le lien symbolique mort est conservé, non écrasé"
    );
    Ok(())
}
