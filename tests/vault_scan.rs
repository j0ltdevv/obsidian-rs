//! Tests d'intégration du scan de vault (`src/vault/scan.rs`).
//!
//! Fixtures : `tests/fixtures/vault-sample/`.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use obsidian_rs::vault::scan::duplicate_names;
use obsidian_rs::vault::{scan_vault, Note, Wikilink};

/// Racine du vault de test.
fn fixture_root() -> &'static Path {
    Path::new("tests/fixtures/vault-sample")
}

/// Retourne la note du scan dont le `name` correspond, sinon erreur.
fn note_par_nom<'a>(notes: &'a [Note], nom: &str) -> anyhow::Result<&'a Note> {
    notes
        .iter()
        .find(|n| n.name == nom)
        .ok_or_else(|| anyhow::anyhow!("note « {nom} » introuvable dans le scan"))
}

/// 1. scan_vault découvre toutes les notes `.md` récursivement,
///    sauf celles des dossiers cachés (`.obsidian`).
#[test]
fn scan_decouvre_toutes_les_notes_md_sauf_dossiers_cachees() -> anyhow::Result<()> {
    let notes = scan_vault(fixture_root())?;

    let chemins: HashSet<String> = notes
        .iter()
        .map(|n| n.path.to_string_lossy().into_owned())
        .collect();

    let attendus: HashSet<String> = [
        "Note déplacement.md",
        "frontmatter-casse.md",
        "Dossier/Brouillon.md",
        "Dossier/index.md",
        "Doublon/A.md",
        "Autre dossier/A.md",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();

    assert_eq!(chemins, attendus, "la note cachée ne doit pas apparaître");
    Ok(())
}

/// 2. Les chemins retournés sont relatifs au root, avec accents et espaces
///    préservés (note « Note déplacement.md », dossier « Autre dossier »).
#[test]
fn chemins_relatifs_preservant_accents_et_espaces() -> anyhow::Result<()> {
    let notes = scan_vault(fixture_root())?;

    let note_deplacement = note_par_nom(&notes, "Note déplacement")?;
    assert_eq!(note_deplacement.path, PathBuf::from("Note déplacement.md"));
    assert!(!note_deplacement.path.is_absolute());

    let note_espace = notes
        .iter()
        .find(|n| n.path == Path::new("Autre dossier/A.md"))
        .ok_or_else(|| anyhow::anyhow!("chemin « Autre dossier/A.md » introuvable"))?;
    assert_eq!(note_espace.path, PathBuf::from("Autre dossier/A.md"));
    assert_eq!(note_espace.name, "A");

    Ok(())
}

/// 3. duplicate_names détecte les doublons de stem (« A » dans deux dossiers).
#[test]
fn duplicate_names_detecte_les_doublons_de_stem() -> anyhow::Result<()> {
    let notes = scan_vault(fixture_root())?;
    assert_eq!(duplicate_names(&notes), vec!["A".to_string()]);
    Ok(())
}

/// 4. Frontmatter YAML invalide → `frontmatter_error = Some(message)`,
///    `frontmatter = None`, sans panique.
#[test]
fn frontmatter_invalide_donne_erreur_sans_panique() -> anyhow::Result<()> {
    let notes = scan_vault(fixture_root())?;

    let note = note_par_nom(&notes, "frontmatter-casse")?;
    assert!(note.metadata.frontmatter.is_none());

    let erreur = note
        .metadata
        .frontmatter_error
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("frontmatter_error doit être Some"))?;
    assert!(
        erreur.contains("frontmatter"),
        "message d'erreur explicite : {erreur}"
    );

    Ok(())
}

/// 5. Les tags sont fusionnés (inline + frontmatter), dédoublonnés,
///    dans l'ordre de première apparition.
#[test]
fn tags_fusionnes_dedoublonnes_ordre_stable() -> anyhow::Result<()> {
    let notes = scan_vault(fixture_root())?;

    // Inline d'abord (#important), puis frontmatter ([projet]).
    let note_deplacement = note_par_nom(&notes, "Note déplacement")?;
    assert_eq!(note_deplacement.metadata.tags, vec!["important", "projet"]);

    // #projet apparaît en inline et en frontmatter → dédoublonné.
    let brouillon = note_par_nom(&notes, "Brouillon")?;
    assert_eq!(brouillon.metadata.tags, vec!["projet", "a/b", "note"]);

    Ok(())
}

/// 6. Les wikilinks sont extraits avec cible et alias.
#[test]
fn wikilinks_extraits_cible_et_alias() -> anyhow::Result<()> {
    let notes = scan_vault(fixture_root())?;

    let note_deplacement = note_par_nom(&notes, "Note déplacement")?;
    assert_eq!(
        note_deplacement.metadata.wikilinks,
        vec![Wikilink {
            target: "Autre".to_string(),
            alias: None,
        }]
    );

    let brouillon = note_par_nom(&notes, "Brouillon")?;
    assert_eq!(
        brouillon.metadata.wikilinks,
        vec![Wikilink {
            target: "Brouillon".to_string(),
            alias: Some("alias".to_string()),
        }]
    );

    Ok(())
}

/// 7. scan_vault sur un chemin inexistant retourne `Err`.
#[test]
fn scan_sur_chemin_inexistant_retourne_err() {
    let chemin = Path::new("/chemin/inexistant/vault-xyz");
    assert!(scan_vault(chemin).is_err());
}
