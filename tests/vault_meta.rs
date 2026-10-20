//! Tests d'intégration du frontmatter (`src/vault/meta.rs`).
//!
//! Chaque test crée un vault temporaire unique sous `std::env::temp_dir()`,
//! nettoyé à la fin via `Drop` (aucune fixture statique nécessaire).

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use obsidian_rs::vault::meta::{
    read_frontmatter, remove_frontmatter_property, set_frontmatter_property,
};
use obsidian_rs::vault::ops::create_note;

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
        let root = std::env::temp_dir().join(format!("obsidian-rs-vault-meta-{}", unique_suffix()));
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

fn yaml_str(s: &str) -> serde_yaml::Value {
    serde_yaml::Value::String(s.to_string())
}

fn yaml_seq(items: &[&str]) -> serde_yaml::Value {
    serde_yaml::Value::Sequence(items.iter().map(|s| yaml_str(s)).collect())
}

/// 1. set_ clé existante → valeur remplacée, corps intact.
#[test]
fn set_cle_existante_remplace_valeur_et_preserve_corps() -> anyhow::Result<()> {
    let vault = TempVault::new();
    create_note(
        &vault.root,
        Path::new("note.md"),
        "---\ntitle: Ancien\n---\nCorps",
    )?;

    let note = set_frontmatter_property(
        &vault.root,
        Path::new("note.md"),
        "title",
        &yaml_str("Nouveau"),
    )?;

    let fm = note.metadata.frontmatter.expect("frontmatter présent");
    assert_eq!(fm.get("title"), Some(&yaml_str("Nouveau")));
    assert_eq!(note.path, PathBuf::from("note.md"));
    assert_eq!(
        fs::read_to_string(vault.join("note.md"))?,
        "---\ntitle: Nouveau\n---\nCorps"
    );
    Ok(())
}

/// 2. set_ clé absente → ajoutée en fin du bloc, corps intact.
#[test]
fn set_cle_absente_ajoute_en_fin_de_bloc() -> anyhow::Result<()> {
    let vault = TempVault::new();
    create_note(
        &vault.root,
        Path::new("note.md"),
        "---\ntitle: Bonjour\ncorps: x\n---\nCorps",
    )?;

    let note = set_frontmatter_property(
        &vault.root,
        Path::new("note.md"),
        "nouveau",
        &yaml_str("oui"),
    )?;

    let fm = note.metadata.frontmatter.expect("frontmatter présent");
    let serde_yaml::Value::Mapping(map) = fm else {
        panic!("mapping attendu");
    };
    let cles: Vec<String> = map
        .keys()
        .map(|k| k.as_str().expect("clé chaîne").to_string())
        .collect();
    assert_eq!(cles, vec!["title", "corps", "nouveau"]);
    assert_eq!(
        fs::read_to_string(vault.join("note.md"))?,
        "---\ntitle: Bonjour\ncorps: x\nnouveau: oui\n---\nCorps"
    );
    Ok(())
}

/// 3. set_ sur note sans frontmatter → bloc créé, corps intact.
#[test]
fn set_sans_frontmatter_cree_bloc_et_preserve_corps() -> anyhow::Result<()> {
    let vault = TempVault::new();
    create_note(&vault.root, Path::new("note.md"), "# Titre\n\nTexte")?;

    set_frontmatter_property(&vault.root, Path::new("note.md"), "tags", &yaml_seq(&["a"]))?;

    assert_eq!(
        fs::read_to_string(vault.join("note.md"))?,
        "---\ntags:\n- a\n---\n# Titre\n\nTexte"
    );
    Ok(())
}

/// 4. set_ sur YAML invalide → Err, fichier NON modifié.
#[test]
fn set_sur_yaml_invalide_erreur_sans_modification() -> anyhow::Result<()> {
    let vault = TempVault::new();
    let original = "---\ntitle: [non fermée\n---\nCorps";
    create_note(&vault.root, Path::new("note.md"), original)?;

    let err = set_frontmatter_property(&vault.root, Path::new("note.md"), "title", &yaml_str("X"))
        .unwrap_err();
    assert!(!err.to_string().is_empty(), "message d'erreur présent");
    assert_eq!(fs::read_to_string(vault.join("note.md"))?, original);
    Ok(())
}

/// 5. remove_ clé présente → supprimée ; clé absente → Ok sans changement.
#[test]
fn remove_cle_presente_supprime_absente_ok_sans_changement() -> anyhow::Result<()> {
    let vault = TempVault::new();
    create_note(
        &vault.root,
        Path::new("note.md"),
        "---\na: 1\nb: deux\n---\nCorps",
    )?;

    let note = remove_frontmatter_property(&vault.root, Path::new("note.md"), "a")?;
    let fm = note.metadata.frontmatter.expect("frontmatter présent");
    assert_eq!(fm.get("a"), None);
    assert_eq!(fm.get("b"), Some(&yaml_str("deux")));
    assert_eq!(
        fs::read_to_string(vault.join("note.md"))?,
        "---\nb: deux\n---\nCorps"
    );

    let avant = fs::read_to_string(vault.join("note.md"))?;
    remove_frontmatter_property(&vault.root, Path::new("note.md"), "absente")?;
    assert_eq!(fs::read_to_string(vault.join("note.md"))?, avant);
    Ok(())
}

/// 6. write non effectué si valeur identique (contenu retourné identique).
#[test]
fn set_valeur_identique_n_ecrit_pas() -> anyhow::Result<()> {
    let vault = TempVault::new();
    create_note(
        &vault.root,
        Path::new("note.md"),
        "---\ntitle: X\n---\nCorps",
    )?;
    let avant = fs::read_to_string(vault.join("note.md"))?;

    let note =
        set_frontmatter_property(&vault.root, Path::new("note.md"), "title", &yaml_str("X"))?;
    assert_eq!(
        note.metadata
            .frontmatter
            .expect("frontmatter présent")
            .get("title"),
        Some(&yaml_str("X"))
    );
    assert_eq!(fs::read_to_string(vault.join("note.md"))?, avant);
    Ok(())
}

/// 7. Round-trip : set_ puis read_frontmatter → valeurs attendues ; contenu
///    YAML valide.
#[test]
fn round_trip_set_puis_read_frontmatter() -> anyhow::Result<()> {
    let vault = TempVault::new();
    create_note(&vault.root, Path::new("note.md"), "Bonjour")?;

    set_frontmatter_property(
        &vault.root,
        Path::new("note.md"),
        "titre",
        &yaml_str("Ma note"),
    )?;
    set_frontmatter_property(
        &vault.root,
        Path::new("note.md"),
        "publiée",
        &serde_yaml::Value::Bool(true),
    )?;

    let fm = read_frontmatter(&vault.root, Path::new("note.md"))?.expect("frontmatter présent");
    assert_eq!(fm.get("titre"), Some(&yaml_str("Ma note")));
    assert_eq!(fm.get("publiée"), Some(&serde_yaml::Value::Bool(true)));

    // Le contenu reste un YAML valide : un second parse donne la même valeur.
    let relu = read_frontmatter(&vault.root, Path::new("note.md"))?.expect("frontmatter présent");
    assert_eq!(relu, fm);
    Ok(())
}

/// 8. multi-ligne/scalaire simple (ex. liste) préservé.
#[test]
fn liste_multiligne_preservee() -> anyhow::Result<()> {
    let vault = TempVault::new();
    create_note(
        &vault.root,
        Path::new("note.md"),
        "---\ntags:\n  - a\n  - b\n---\nCorps",
    )?;

    let note = set_frontmatter_property(
        &vault.root,
        Path::new("note.md"),
        "status",
        &yaml_str("fait"),
    )?;
    let fm = note.metadata.frontmatter.expect("frontmatter présent");
    assert_eq!(fm.get("tags"), Some(&yaml_seq(&["a", "b"])));
    assert_eq!(
        fs::read_to_string(vault.join("note.md"))?,
        "---\ntags:\n- a\n- b\nstatus: fait\n---\nCorps"
    );
    Ok(())
}
