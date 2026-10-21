//! Tests de la logique **pure** d'affichage des embeds d'image (`embed_view`),
//! et de son intégration avec la lecture des dimensions (`image_dimensions`).
//! On teste les fonctions pures, pas l'affichage iced lui-même.

use obsidian_rs::ui::embed_view::{
    decide_display, display_size, within_size_limit, DisplayKind, MAX_IMAGE_BYTES, MAX_IMAGE_WIDTH,
};
use obsidian_rs::vault::embeds::{image_dimensions, EmbedType, ImageDimensions};
use std::path::PathBuf;

/// Crée un répertoire temporaire unique et vide.
fn temp_dir(tag: &str) -> PathBuf {
    let mut d = std::env::temp_dir();
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|t| t.as_nanos())
        .unwrap_or(0);
    d.push(format!("obsidian-rs-embed-view-{tag}-{unique}"));
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Construit les octets d'un PNG minimal portant les dimensions données
/// (signature + bloc IHDR : suffisant pour `image_dimensions`).
fn png_bytes(width: u32, height: u32) -> Vec<u8> {
    let mut v = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    v.extend_from_slice(&13u32.to_be_bytes()); // longueur du bloc IHDR
    v.extend_from_slice(b"IHDR");
    v.extend_from_slice(&width.to_be_bytes());
    v.extend_from_slice(&height.to_be_bytes());
    v.extend_from_slice(&[8, 6, 0, 0, 0]); // profondeur, couleur, compression, filtre, entrelacement
    v
}

#[test]
fn taille_daffichage_reduit_sans_deformer() {
    // 800×400 bornée à 400 : le rapport d'aspect (2:1) est conservé.
    let (w, h) = display_size(
        ImageDimensions {
            width: 800,
            height: 400,
        },
        MAX_IMAGE_WIDTH,
    );
    assert_eq!((w, h), (400.0, 200.0));
}

#[test]
fn taille_naturelle_gardee_sous_le_plafond() {
    // Plus petite que la largeur max : on affiche à taille naturelle, sans agrandir.
    let (w, h) = display_size(
        ImageDimensions {
            width: 100,
            height: 50,
        },
        MAX_IMAGE_WIDTH,
    );
    assert_eq!((w, h), (100.0, 50.0));
}

#[test]
fn largeur_nulle_aucune_division_par_zero() {
    let (w, h) = display_size(
        ImageDimensions {
            width: 0,
            height: 500,
        },
        MAX_IMAGE_WIDTH,
    );
    assert_eq!((w, h), (0.0, 0.0));
}

#[test]
fn plafond_de_taille_respecte_la_limite() {
    assert!(within_size_limit(MAX_IMAGE_BYTES, MAX_IMAGE_BYTES));
    assert!(!within_size_limit(MAX_IMAGE_BYTES + 1, MAX_IMAGE_BYTES));
}

#[test]
fn decision_image_lisible_affichable() {
    let dims = ImageDimensions {
        width: 10,
        height: 10,
    };
    assert_eq!(
        decide_display(EmbedType::Image, 100, Some(dims)),
        DisplayKind::Image(dims)
    );
}

#[test]
fn decision_non_image_replie_placeholder() {
    for kind in [
        EmbedType::Document,
        EmbedType::Audio,
        EmbedType::Video,
        EmbedType::Note,
        EmbedType::NotFound,
    ] {
        assert_eq!(
            decide_display(
                kind,
                100,
                Some(ImageDimensions {
                    width: 1,
                    height: 1
                })
            ),
            DisplayKind::Placeholder
        );
    }
}

#[test]
fn decision_image_trop_volumineuse() {
    let dims = ImageDimensions {
        width: 10,
        height: 10,
    };
    assert_eq!(
        decide_display(EmbedType::Image, MAX_IMAGE_BYTES + 1, Some(dims)),
        DisplayKind::TooLarge
    );
}

#[test]
fn decision_image_illisible_replie_placeholder() {
    assert_eq!(
        decide_display(EmbedType::Image, 100, None),
        DisplayKind::Placeholder
    );
}

#[test]
fn png_dimensions_et_taille_daffichage() {
    // Un PNG construit à la volée doit donner les bonnes dimensions, puis la
    // bonne taille d'affichage (rapport conservé, largeur bornée).
    let dir = temp_dir("png");
    let path = dir.join("photo.png");
    std::fs::write(&path, png_bytes(320, 160)).unwrap();

    let dims = image_dimensions(&path).expect("le PNG doit être lisible");
    assert_eq!(
        dims,
        ImageDimensions {
            width: 320,
            height: 160
        }
    );

    // Bornée à 200 de large : 200×100 (même rapport 2:1).
    let (w, h) = display_size(dims, 200.0);
    assert_eq!((w, h), (200.0, 100.0));

    // Au-delà de la largeur max, la taille naturelle est conservée.
    let (w, h) = display_size(dims, MAX_IMAGE_WIDTH);
    assert_eq!((w, h), (320.0, 160.0));

    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_dir(&dir);
}
