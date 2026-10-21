//! Palettes de thèmes (données pures, sans dépendance UI).
//!
//! Deux thèmes sont fournis : `DEFAULT_DARK` et `DEFAULT_LIGHT`. Le reste du
//! code les sélectionne par nom via `by_name`, de façon insensible à la casse
//! et sans allocation.
//!
//! Un vault peut aussi fournir ses propres thèmes CSS (`.obsidian/themes/`).
//! [`theme_css_path`] et [`theme_css_source`] résolvent alors le nom du thème
//! actif vers le fichier correspondant ; l'interface-native les traduit en
//! variables (voir `crate::ui::theme_vars`).

use std::fs;
use std::path::{Path, PathBuf};

/// Palette complète d'un thème.
///
/// Les composantes sont des octets RGB (`0..=255`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Theme {
    /// Nom du thème, par exemple `"dark"` ou `"light"`.
    pub name: &'static str,
    /// Couleur d'accent (actions, liens, sélections).
    pub accent: (u8, u8, u8),
    /// Variante de l'accent lisible **en texte** sur `bg` (WCAG AA ≥ 4.5).
    pub accent_text: (u8, u8, u8),
    /// Couleur de fond principal.
    pub bg: (u8, u8, u8),
    /// Couleur du texte principal.
    pub fg: (u8, u8, u8),
    /// Couleur des surfaces (panneaux, champs, cartes).
    pub surface: (u8, u8, u8),
    /// Couleur des séparateurs et bordures.
    pub border: (u8, u8, u8),
}

/// Thème sombre par défaut.
///
/// Valeurs des tokens documentés d'Obsidian (`--color-base-*` et l'accent
/// par défaut HSL 258/88 %/66 % ≈ `#8A5CF5`).
pub const DEFAULT_DARK: Theme = Theme {
    name: "dark",
    accent: (138, 92, 245),
    accent_text: (169, 139, 255),
    bg: (28, 28, 28),
    fg: (218, 218, 218),
    surface: (40, 40, 40),
    border: (51, 51, 51),
};

/// Thème clair par défaut (mêmes tokens, mode clair).
pub const DEFAULT_LIGHT: Theme = Theme {
    name: "light",
    accent: (138, 92, 245),
    accent_text: (120, 82, 238),
    bg: (255, 255, 255),
    fg: (34, 34, 34),
    surface: (246, 246, 246),
    border: (228, 228, 228),
};

/// Liste des thèmes connus, dans l'ordre de préférence d'affichage.
const THEMES: &[&Theme] = &[&DEFAULT_DARK, &DEFAULT_LIGHT];

/// Recherche un thème par son nom, de façon insensible à la casse.
///
/// Seuls `"dark"` et `"light"` sont reconnus (et leurs variantes en casse).
/// Un nom inconnu retourne `None` plutôt qu'un thème par défaut, afin que le
/// choix de l'utilisateur ne soit pas silencieusement ignoré.
pub fn by_name(name: &str) -> Option<&'static Theme> {
    THEMES
        .iter()
        .find(|theme| theme.name.eq_ignore_ascii_case(name))
        .copied()
}

/// Résout le nom du thème actif vers un fichier CSS du vault.
///
/// Recherche, dans `.obsidian/themes/` :
/// 1. le fichier `<nom du thème>.css` ;
/// 2. un fichier dont le nom correspond sans tenir compte de la casse ;
/// 3. le premier fichier `*.css` du dossier, par ordre alphabétique.
///
/// Retourne `None` quand le vault ne fournit aucun fichier de thème : les
/// valeurs par défaut restent alors en vigueur. Un nom de thème contenant un
/// séparateur de chemin est refusé, pour ne jamais sortir du vault.
pub fn theme_css_path(vault_root: &Path, theme_name: &str) -> Option<PathBuf> {
    let name = theme_name.trim();
    if name.is_empty() || name.contains('/') || name.contains('\\') || name.contains("..") {
        return None;
    }
    let dir = vault_root.join(".obsidian").join("themes");
    let exact = dir.join(format!("{name}.css"));
    if exact.is_file() {
        return Some(exact);
    }
    let entries = fs::read_dir(&dir).ok()?;
    let mut candidates: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("css"))
        .collect();
    candidates.sort();
    let wanted = name.to_ascii_lowercase();
    candidates
        .iter()
        .find(|path| {
            path.file_stem()
                .and_then(|stem| stem.to_str())
                .map(|stem| stem.to_ascii_lowercase() == wanted)
                .unwrap_or(false)
        })
        .cloned()
        .or_else(|| candidates.into_iter().next())
}

/// Source CSS du thème actif du vault, si celui-ci en fournit une.
///
/// Un fichier illisible est ignoré (retour `None`) : l'application conserve
/// alors les valeurs par défaut plutôt que d'échouer au démarrage.
pub fn theme_css_source(vault_root: &Path, theme_name: &str) -> Option<String> {
    let path = theme_css_path(vault_root, theme_name)?;
    fs::read_to_string(path).ok()
}

/// Luminance relative sRGB d'une couleur (WCAG 2.x).
#[cfg(test)]
fn luminance_of((r, g, b): (u8, u8, u8)) -> f64 {
    fn channel(c: u8) -> f64 {
        let v = f64::from(c) / 255.0;
        if v <= 0.03928 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    }
    0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b)
}

/// Ratio de contraste WCAG entre deux couleurs.
#[cfg(test)]
fn contrast(a: (u8, u8, u8), b: (u8, u8, u8)) -> f64 {
    let (l1, l2) = if luminance_of(a) > luminance_of(b) {
        (luminance_of(a), luminance_of(b))
    } else {
        (luminance_of(b), luminance_of(a))
    };
    (l1 + 0.05) / (l2 + 0.05)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_dark_est_present() {
        assert_eq!(DEFAULT_DARK.name, "dark");
    }

    #[test]
    fn default_light_est_present() {
        assert_eq!(DEFAULT_LIGHT.name, "light");
    }

    #[test]
    fn by_name_insensible_a_la_casse() {
        assert_eq!(by_name("dark"), Some(&DEFAULT_DARK));
        assert_eq!(by_name("DARK"), Some(&DEFAULT_DARK));
        assert_eq!(by_name("Dark"), Some(&DEFAULT_DARK));
        assert_eq!(by_name("light"), Some(&DEFAULT_LIGHT));
        assert_eq!(by_name("LIGHT"), Some(&DEFAULT_LIGHT));
        assert_eq!(by_name("LiGhT"), Some(&DEFAULT_LIGHT));
    }

    #[test]
    fn by_name_inconnu_retourne_none() {
        assert_eq!(by_name(""), None);
        assert_eq!(by_name("obsidian"), None);
        assert_eq!(by_name("darkmode"), None);
        assert_eq!(by_name("solarized"), None);
    }

    #[test]
    fn contraste_fg_bg_superieur_a_trois() {
        assert!(contrast(DEFAULT_DARK.fg, DEFAULT_DARK.bg) > 3.0);
        assert!(contrast(DEFAULT_LIGHT.fg, DEFAULT_LIGHT.bg) > 3.0);
    }

    /// WCAG AA pour texte normal exige un ratio ≥ 4.5.
    #[test]
    fn contraste_aa_texte_normal() {
        assert!(
            contrast(DEFAULT_DARK.fg, DEFAULT_DARK.bg) >= 4.5,
            "dark fg/bg doit respecter AA (≥ 4.5)"
        );
        assert!(
            contrast(DEFAULT_LIGHT.fg, DEFAULT_LIGHT.bg) >= 4.5,
            "light fg/bg doit respecter AA (≥ 4.5)"
        );
    }

    /// Le texte accentué doit rester lisible : AA exige ≥ 4.5 sur le fond.
    #[test]
    fn contraste_aa_texte_accent() {
        for theme in [DEFAULT_DARK, DEFAULT_LIGHT] {
            let ratio = contrast(theme.accent_text, theme.bg);
            assert!(
                ratio >= 4.5,
                "{} : accent_text/bg = {ratio:.2}, attendu ≥ 4.5",
                theme.name
            );
        }
    }

    /// L'accent brut sert de fond/aplat : WCAG 1.4.11 demande ≥ 3 sur le fond.
    #[test]
    fn contraste_accent_brut_sur_fond() {
        for theme in [DEFAULT_DARK, DEFAULT_LIGHT] {
            let ratio = contrast(theme.accent, theme.bg);
            assert!(
                ratio >= 3.0,
                "{} : accent/bg = {ratio:.2}, attendu ≥ 3.0",
                theme.name
            );
        }
    }

    #[test]
    fn toutes_composantes_rgb_dans_l_intervalle() {
        for theme in [DEFAULT_DARK, DEFAULT_LIGHT] {
            for &(r, g, b) in [
                &theme.accent,
                &theme.accent_text,
                &theme.bg,
                &theme.fg,
                &theme.surface,
                &theme.border,
            ] {
                assert!((0..=255).contains(&r), "r hors limite pour {}", theme.name);
                assert!((0..=255).contains(&g), "g hors limite pour {}", theme.name);
                assert!((0..=255).contains(&b), "b hors limite pour {}", theme.name);
            }
        }
    }

    #[test]
    fn by_name_retourne_des_themes_distincts() {
        assert_ne!(by_name("dark"), by_name("light"));
        assert_eq!(by_name("dark"), Some(&DEFAULT_DARK));
        assert_eq!(by_name("light"), Some(&DEFAULT_LIGHT));
    }
}
