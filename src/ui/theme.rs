//! Tokens de thème centralisés (police, couleurs, rayons).
//!
//! Point unique où ajuster les constantes de style communes. Les couleurs
//! proviennent des variables CSS du thème de l'utilisateur (voir
//! [`theme_vars`]), traduites en palette iced et en couleurs d'interface.
//!
//! L'ordre de chargement des variables est documenté dans
//! [`theme_vars`] : valeurs par défaut, puis fichier CSS du thème actif du
//! vault, puis snippets de l'utilisateur.

use iced::theme::Palette;
use iced::{Color, Font, Theme};

#[path = "theme_vars.rs"]
pub mod theme_vars;

pub use self::theme_vars::{load_theme_vars, ThemeMode, ThemeVars};

/// Police de l'interface. Inter est embarquée dans le binaire (voir
/// `src/main.rs` → `.font(include_bytes!(...))`) et référencée par son nom.
pub fn font() -> Font {
    Font::with_name("Inter")
}

/// Rayon des bords des panneaux et cartes.
pub const PANEL_RADIUS: f32 = 6.0;

/// Couleurs de l'interface dérivées des variables de thème.
///
/// La `Palette` iced ne couvre que six teintes : ce regroupement expose en plus
/// les surfaces, les bordures et les tons de texte atténués utilisés par les
/// vues.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ThemeColors {
    /// Fond principal de la fenêtre.
    pub background: Color,
    /// Fond des panneaux, champs et cartes.
    pub surface: Color,
    /// Couleur des bordures et séparateurs.
    pub border: Color,
    /// Fond de survol.
    pub hover: Color,
    /// Couleur du texte principal.
    pub text: Color,
    /// Couleur du texte secondaire.
    pub muted: Color,
    /// Couleur du texte très discret.
    pub faint: Color,
    /// Couleur d'accent.
    pub accent: Color,
    /// Couleur d'accent au survol.
    pub accent_hover: Color,
    /// Accent interactif (sélections, éléments actifs).
    pub interactive_accent: Color,
    /// Couleur du texte posé sur un aplat d'accent.
    pub text_on_accent: Color,
}

/// Teinte d'état « succès » sur fond sombre.
const SUCCESS_DARK: Color = Color::from_rgb8(0x3f, 0xb9, 0x50);
/// Teinte d'état « succès » sur fond clair.
const SUCCESS_LIGHT: Color = Color::from_rgb8(0x1a, 0x7f, 0x37);
/// Teinte d'état « avertissement » sur fond sombre.
const WARNING_DARK: Color = Color::from_rgb8(0xe3, 0xb3, 0x41);
/// Teinte d'état « avertissement » sur fond clair.
const WARNING_LIGHT: Color = Color::from_rgb8(0x9a, 0x67, 0x00);
/// Teinte d'état « danger » sur fond sombre.
const DANGER_DARK: Color = Color::from_rgb8(0xf8, 0x51, 0x49);
/// Teinte d'état « danger » sur fond clair.
const DANGER_LIGHT: Color = Color::from_rgb8(0xcf, 0x22, 0x2e);

/// Construit les couleurs de l'interface à partir des variables de thème.
pub fn colors_from_vars(vars: &ThemeVars) -> ThemeColors {
    ThemeColors {
        background: vars.background_primary,
        surface: vars.background_secondary,
        border: vars.background_modifier_border,
        hover: vars.background_modifier_hover,
        text: vars.text_normal,
        muted: vars.text_muted,
        faint: vars.text_faint,
        accent: vars.accent,
        accent_hover: vars.accent_hover,
        interactive_accent: vars.interactive_accent,
        text_on_accent: vars.text_on_accent,
    }
}

/// Construit une palette iced à partir des variables de thème.
///
/// Les teintes d'état (succès, avertissement, danger) n'existent pas chez
/// Obsidian : elles sont déduites de la luminosité du fond pour rester
/// lisibles dans les deux modes.
pub fn palette_from_vars(vars: &ThemeVars) -> Palette {
    let colors = colors_from_vars(vars);
    let dark = relative_luminance(colors.background) < 0.5;
    Palette {
        background: colors.background,
        text: colors.text,
        primary: colors.interactive_accent,
        success: if dark { SUCCESS_DARK } else { SUCCESS_LIGHT },
        warning: if dark { WARNING_DARK } else { WARNING_LIGHT },
        danger: if dark { DANGER_DARK } else { DANGER_LIGHT },
    }
}

/// Construit un thème iced à partir des variables de thème.
///
/// Le thème returned est un thème « custom » : l'application peut ainsi
/// appliquer la palette du vault sans passer par les thèmes intégrés d'iced.
pub fn iced_theme(vars: &ThemeVars) -> Theme {
    Theme::custom("obsidian", palette_from_vars(vars))
}

/// Police de l'interface déduite des variables de thème.
///
/// Seules les polices embarquées dans le binaire sont utilisables : une famille
/// demandée mais absente du binaire retombe sur la police par défaut.
pub fn font_from_vars(vars: &ThemeVars) -> Font {
    embedded_font(&first_family(&vars.font_interface)).unwrap_or_else(font)
}

/// Familles de polices embarquées dans le binaire (voir `src/main.rs`).
const EMBEDDED_FAMILIES: &[&str] = &["inter"];

/// Police embarquée correspondant à une famille demandée par le thème.
fn embedded_font(family: &str) -> Option<Font> {
    let family = family
        .trim()
        .trim_matches('"')
        .trim_matches('\'')
        .to_ascii_lowercase();
    if EMBEDDED_FAMILIES.contains(&family.as_str()) {
        Some(font())
    } else {
        None
    }
}

/// Première famille d'une liste de polices CSS (`"Inter", sans-serif`).
fn first_family(value: &str) -> String {
    value
        .split(',')
        .next()
        .unwrap_or_default()
        .trim()
        .trim_matches('"')
        .trim_matches('\'')
        .to_string()
}

/// Luminance relative d'une couleur (WCAG 2.x), entre `0.0` et `1.0`.
fn relative_luminance(color: Color) -> f32 {
    fn channel(value: f32) -> f32 {
        if value <= 0.03928 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    }
    0.2126 * channel(color.r) + 0.7152 * channel(color.g) + 0.0722 * channel(color.b)
}
