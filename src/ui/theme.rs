//! Tokens de thème centralisés (police, rayons).
//!
//! Point unique où ajuster les constantes de style communes. Les couleurs
//! viennent du thème iced (`extended_palette`), construit depuis les palettes
//! du vault (`vault::themes`).

use iced::Font;

/// Police de l'interface. Inter est embarquée dans le binaire (voir
/// `src/main.rs` → `.font(include_bytes!(...))`) et référencée par son nom.
pub fn font() -> Font {
    Font::with_name("Inter")
}

/// Rayon des bords des panneaux et cartes.
pub const PANEL_RADIUS: f32 = 6.0;
