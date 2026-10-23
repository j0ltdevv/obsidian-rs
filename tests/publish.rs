//! Tests d'intégration de l'export statique HTML (`src/vault/publish.rs`).
//!
//! Chaque test écrit dans un répertoire temporaire unique et le nettoie.

use std::path::{Path, PathBuf};

use obsy::vault::publish::{html_filename, publish_site};

/// Crée un répertoire temporaire unique pour un test.
fn temp_dir(tag: &str) -> PathBuf {
    let mut dir = std::env::temp_dir();
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    dir.push(format!("obsidian-publish-{tag}-{unique}"));
    dir
}

/// Écrit un fichier (création des dossiers parents au besoin).
fn write_file(path: &Path, content: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(path, content).unwrap();
}

/// Lit un fichier texte.
fn read_file(path: &Path) -> String {
    std::fs::read_to_string(path).expect("lecture du fichier généré")
}

// 1. Une note simple → la page HTML contient le titre et le texte.
#[test]
fn note_simple_contient_titre_et_texte() -> anyhow::Result<()> {
    let base = temp_dir("simple");
    let vault = base.join("vault");
    let dest = base.join("site");
    write_file(
        &vault.join("Bonjour.md"),
        "# Bonjour\nCeci est le texte de la note.\n",
    );

    let report = publish_site(&vault, &dest)?;
    assert_eq!(report.pages, 1);

    let html = read_file(&dest.join("Bonjour.html"));
    assert!(html.contains("Bonjour"), "le titre doit apparaître");
    assert!(
        html.contains("Ceci est le texte de la note."),
        "le texte doit apparaître"
    );
    assert!(dest.join("index.html").exists());
    assert!(dest.join("style.css").exists());

    let index = read_file(&dest.join("index.html"));
    assert!(index.contains("Bonjour.html"));
    assert!(index.contains("Bonjour"));

    let _ = std::fs::remove_dir_all(&base);
    Ok(())
}

// 2. Un wikilink vers une note existante → lien relatif vers `cible.html`.
#[test]
fn wikilink_devient_lien_relatif() -> anyhow::Result<()> {
    let base = temp_dir("wikilink");
    let vault = base.join("vault");
    let dest = base.join("site");
    write_file(&vault.join("A.md"), "Voir [[B]].\n");
    write_file(&vault.join("B.md"), "Contenu de B.\n");

    publish_site(&vault, &dest)?;

    let html = read_file(&dest.join("A.html"));
    assert!(
        html.contains(r#"<a href="B.html">B</a>"#),
        "le lien relatif doit pointer vers B.html"
    );

    let _ = std::fs::remove_dir_all(&base);
    Ok(())
}

// 3. Un wikilink vers une cible absente → texte simple, pas de lien mort.
#[test]
fn wikilink_cible_absente_pas_de_lien_mort() -> anyhow::Result<()> {
    let base = temp_dir("absente");
    let vault = base.join("vault");
    let dest = base.join("site");
    write_file(&vault.join("A.md"), "Réf [[Introuvable]] ici.\n");

    publish_site(&vault, &dest)?;

    let html = read_file(&dest.join("A.html"));
    assert!(
        !html.contains("<a"),
        "aucun lien ne doit être émis pour une cible absente"
    );
    assert!(html.contains("Introuvable"), "le texte reste affiché");

    let _ = std::fs::remove_dir_all(&base);
    Ok(())
}

// 4. Une note contenant `<script>` → tout est échappé, HTML non exécutable.
#[test]
fn script_echappe() -> anyhow::Result<()> {
    let base = temp_dir("script");
    let vault = base.join("vault");
    let dest = base.join("site");
    write_file(&vault.join("Fichier.md"), "<script>alert(1)</script>\n");

    publish_site(&vault, &dest)?;

    let html = read_file(&dest.join("Fichier.html"));
    assert!(!html.contains("<script>"), "balise raw non échappée");
    assert!(!html.contains("<script"), "aucune balise script raw");
    assert!(
        html.contains("&lt;script&gt;"),
        "le script doit être échappé"
    );

    let _ = std::fs::remove_dir_all(&base);
    Ok(())
}

// 5. Frontmatter → rendu en `<meta>`, absent du corps.
#[test]
fn frontmatter_en_meta_absent_du_corps() -> anyhow::Result<()> {
    let base = temp_dir("frontmatter");
    let vault = base.join("vault");
    let dest = base.join("site");
    write_file(
        &vault.join("Note.md"),
        "---\ntitle: Mon titre\ntags: [un, deux]\n---\nCorps de la note.\n",
    );

    publish_site(&vault, &dest)?;

    let html = read_file(&dest.join("Note.html"));
    assert!(
        html.contains(r#"<meta name="title" content="Mon titre">"#),
        "le titre doit être dans un <meta>"
    );
    assert!(
        html.contains(r#"<meta name="keywords" content="un, deux">"#),
        "les tags doivent être dans un <meta>"
    );
    assert!(html.contains("Corps de la note."), "le corps est présent");
    assert!(
        !html.contains("title: Mon titre"),
        "le YAML brut ne doit pas apparaître dans le corps"
    );
    assert!(
        !html.contains("tags:"),
        "la ligne tags ne doit pas apparaître"
    );

    let _ = std::fs::remove_dir_all(&base);
    Ok(())
}

// 6. Image jointe `![[photo.png]]` → copiée dans `assets/`, référencée en
//    relatif.
#[test]
fn image_jointe_copiee_et_referencee() -> anyhow::Result<()> {
    let base = temp_dir("image");
    let vault = base.join("vault");
    let dest = base.join("site");
    write_file(&vault.join("photo.png"), "PNG");
    write_file(&vault.join("Note.md"), "Une image : ![[photo.png]].\n");

    let report = publish_site(&vault, &dest)?;
    assert_eq!(report.assets, 1);
    assert!(dest.join("assets/photo.png").exists());

    let html = read_file(&dest.join("Note.html"));
    assert!(
        html.contains(r#"src="assets/photo.png""#),
        "la référence relative vers l'image doit être émis"
    );

    let _ = std::fs::remove_dir_all(&base);
    Ok(())
}

// 7. Un nom de note piégé est refusé par le confinement.
#[test]
fn nom_de_note_piege_refuse() {
    assert!(html_filename("../evil").is_err());
    assert!(html_filename("/evil").is_err());
    assert!(html_filename("a/../../evil").is_err());
    assert!(html_filename("").is_err());
    assert!(html_filename("evil").is_ok());
}

// 8. Une note dans un sous-dossier ne sort jamais du répertoire de destination.
#[test]
fn note_en_sous_dossier_confinnee_a_la_destination() -> anyhow::Result<()> {
    let base = temp_dir("confinement");
    let vault = base.join("vault");
    let dest = base.join("site");
    write_file(&vault.join("sous/evil.md"), "# Mal\n");

    publish_site(&vault, &dest)?;

    assert!(
        dest.join("evil.html").exists(),
        "la page est dans la destination"
    );
    assert!(
        !base.join("evil.html").exists(),
        "rien ne doit être écrit hors de la destination"
    );
    assert!(
        !dest.join("../evil.html").exists(),
        "aucune remontée hors de la destination"
    );

    let _ = std::fs::remove_dir_all(&base);
    Ok(())
}
