//! Palettes de thèmes (données pures, sans dépendance UI).
//!
//! Deux thèmes sont fournis : `DEFAULT_DARK` et `DEFAULT_LIGHT`. Le reste du
//! code les sélectionne par nom via `by_name`, de façon insensible à la casse
//! et sans allocation.

/// Palette complète d'un thème.
///
/// Les composantes sont des octets RGB (`0..=255`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Theme {
    /// Nom du thème, par exemple `"dark"` ou `"light"`.
    pub name: &'static str,
    /// Couleur d'accent (actions, liens, sélections).
    pub accent: (u8, u8, u8),
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
pub const DEFAULT_DARK: Theme = Theme {
    name: "dark",
    accent: (124, 204, 255),
    bg: (20, 20, 25),
    fg: (220, 220, 225),
    surface: (30, 30, 38),
    border: (45, 45, 55),
};

/// Thème clair par défaut.
pub const DEFAULT_LIGHT: Theme = Theme {
    name: "light",
    accent: (3, 86, 188),
    bg: (255, 255, 255),
    fg: (30, 30, 35),
    surface: (245, 245, 247),
    border: (210, 210, 215),
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

    #[test]
    fn toutes_composantes_rgb_dans_l_intervalle() {
        for theme in [DEFAULT_DARK, DEFAULT_LIGHT] {
            for &(r, g, b) in [
                &theme.accent,
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
