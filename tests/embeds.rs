//! Tests d'intégration de la résolution des embeds (`![[...]]`) et de la
//! lecture des dimensions d'une image.

use obsy::vault::embeds::{
    image_dimensions, resolve_embed_target, EmbedType, ImageDimensions, ResolveOutcome,
};
use std::path::PathBuf;

/// Crée un répertoire temporaire unique et vide.
fn temp_dir(tag: &str) -> PathBuf {
    let mut d = std::env::temp_dir();
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|t| t.as_nanos())
        .unwrap_or(0);
    d.push(format!("obsy-embeds-{tag}-{unique}"));
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Construit les octets d'un PNG minimal portant les dimensions données.
fn png_bytes(width: u32, height: u32) -> Vec<u8> {
    let mut v = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    v.extend_from_slice(&13u32.to_be_bytes()); // longueur du bloc IHDR
    v.extend_from_slice(b"IHDR");
    v.extend_from_slice(&width.to_be_bytes());
    v.extend_from_slice(&height.to_be_bytes());
    v.extend_from_slice(&[8, 6, 0, 0, 0]); // profondeur, couleur, compression, filtre, entrelacement
    v
}

/// Construit les octets d'un JPEG minimal avec un segment SOF0 aux dimensions
/// données.
fn jpeg_bytes(width: u16, height: u16) -> Vec<u8> {
    let mut v = vec![0xFF, 0xD8, 0xFF, 0xC0]; // SOI + SOF0
    v.extend_from_slice(&10u16.to_be_bytes()); // longueur du segment SOF0
    v.push(8); // précision
    v.extend_from_slice(&height.to_be_bytes());
    v.extend_from_slice(&width.to_be_bytes());
    v.extend_from_slice(&[1, 0, 0]); // un composant
    v
}

#[test]
fn resout_une_note_sans_extension() {
    let dir = temp_dir("note");
    std::fs::write(dir.join("MaNote.md"), "# Ma note").unwrap();

    match resolve_embed_target(&dir, "MaNote") {
        ResolveOutcome::Found(e) => {
            assert_eq!(e.kind, EmbedType::Note);
            assert_eq!(e.path, dir.join("MaNote.md"));
        }
        _ => panic!("la note aurait dû être trouvée"),
    }

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn resout_une_note_avec_extension_md() {
    let dir = temp_dir("note-md");
    std::fs::write(dir.join("UneNote.md"), "# contenu").unwrap();

    match resolve_embed_target(&dir, "UneNote.md") {
        ResolveOutcome::Found(e) => {
            assert_eq!(e.kind, EmbedType::Note);
            assert_eq!(e.path, dir.join("UneNote.md"));
        }
        _ => panic!("la note .md aurait dû être trouvée"),
    }

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn resout_une_image_png_construite_a_la_volee() {
    let dir = temp_dir("png");
    std::fs::write(dir.join("photo.png"), png_bytes(10, 20)).unwrap();

    // Résolution par nom sans extension.
    match resolve_embed_target(&dir, "photo") {
        ResolveOutcome::Found(e) => {
            assert_eq!(e.kind, EmbedType::Image);
            assert_eq!(e.path, dir.join("photo.png"));
        }
        _ => panic!("l'image aurait dû être trouvée"),
    }

    // Dimensions lues depuis le fichier PNG.
    assert_eq!(
        image_dimensions(&dir.join("photo.png")).unwrap(),
        ImageDimensions {
            width: 10,
            height: 20
        }
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn resout_une_image_jpeg_avec_bonnes_dimensions() {
    let dir = temp_dir("jpeg");
    std::fs::write(dir.join("pic.jpg"), jpeg_bytes(40, 30)).unwrap();

    match resolve_embed_target(&dir, "pic.jpg") {
        ResolveOutcome::Found(e) => assert_eq!(e.kind, EmbedType::Image),
        _ => panic!("le JPEG aurait dû être trouvé"),
    }

    assert_eq!(
        image_dimensions(&dir.join("pic.jpg")).unwrap(),
        ImageDimensions {
            width: 40,
            height: 30
        }
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn resout_un_document_pdf() {
    let dir = temp_dir("pdf");
    std::fs::write(dir.join("doc.pdf"), "%PDF-1.4\n").unwrap();

    match resolve_embed_target(&dir, "doc.pdf") {
        ResolveOutcome::Found(e) => {
            assert_eq!(e.kind, EmbedType::Document);
            assert_eq!(e.path, dir.join("doc.pdf"));
        }
        _ => panic!("le PDF aurait dû être trouvé"),
    }

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn cible_absente_du_vault() {
    let dir = temp_dir("absent");
    // Aucun fichier créé : ni `Inexistant.md`, ni pièce jointe.
    assert_eq!(
        resolve_embed_target(&dir, "Inexistant"),
        ResolveOutcome::NotFound
    );
    assert_eq!(
        resolve_embed_target(&dir, "manquant.png"),
        ResolveOutcome::NotFound
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn image_corrompue_retourne_erreur_sans_panique() {
    let dir = temp_dir("corrompu");

    // Signature PNG valide mais bloc IHDR absent : erreur, pas de panique.
    std::fs::write(
        dir.join("bad.png"),
        [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A],
    )
    .unwrap();
    let err = image_dimensions(&dir.join("bad.png")).expect_err("erreur attendue");
    assert!(err.to_string().contains("corrompu"));

    // Fichier non-image : format non pris en charge.
    std::fs::write(dir.join("texte.txt"), "pas une image").unwrap();
    let err2 = image_dimensions(&dir.join("texte.txt")).expect_err("erreur attendue");
    assert!(err2.to_string().contains("non pris en charge"));

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn alias_resolu_vers_le_fichier_reel() {
    let dir = temp_dir("alias");
    std::fs::write(dir.join("fichier.png"), png_bytes(320, 200)).unwrap();

    // `[[fichier.png|alias]]` : l'alias est ignoré, la cible réelle est prise.
    match resolve_embed_target(&dir, "fichier.png|alias") {
        ResolveOutcome::Found(e) => {
            assert_eq!(e.kind, EmbedType::Image);
            assert_eq!(e.path, dir.join("fichier.png"));
        }
        _ => panic!("le fichier derrière l'alias aurait dû être trouvé"),
    }

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn section_retiree_de_la_cible() {
    let dir = temp_dir("section");
    std::fs::write(dir.join("Chapitre.md"), "# Titre").unwrap();

    // `[[Chapitre#Titre]]` référence la note, pas un fichier nommé
    // `Chapitre#Titre.md`.
    match resolve_embed_target(&dir, "Chapitre#Titre") {
        ResolveOutcome::Found(e) => {
            assert_eq!(e.kind, EmbedType::Note);
            assert_eq!(e.path, dir.join("Chapitre.md"));
        }
        _ => panic!("la note aurait dû être trouvée malgré la section"),
    }

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn cible_hors_du_vault_rejetee() {
    let dir = temp_dir("traversal");
    std::fs::write(dir.join("secret.md"), "x").unwrap();

    // Remontée de répertoire : rejetée, jamais lue.
    assert_eq!(
        resolve_embed_target(&dir, "../secret"),
        ResolveOutcome::NotFound
    );

    let _ = std::fs::remove_dir_all(&dir);
}
