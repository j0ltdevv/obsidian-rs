//! Vue de sélection du thème.
//!
//! Affiche les thèmes connus (`dark`, `light`) sous forme de boutons, chacun
//! portant son nom et une pastille de la couleur d'accent du thème. Le thème
//! actuellement sélectionné est surligné ; cliquer sur un bouton appelle
//! `on_select` avec le nom du thème.
//!
//! La lookup se fait par `crate::vault::themes::by_name`, qui est insensible à
//! la casse et retourne `None` pour un nom inconnu : un thème absent de la
//! liste est simplement ignoré, sans fallback silencieux.

use iced::border::Radius;
use iced::widget::{button, container, text, Column, Row};
use iced::{Alignment, Background, Border, Color, Element, Length};

use crate::app::Message;
use crate::vault::themes::{by_name, Theme};

/// Noms des thèmes connus, dans l'ordre d'affichage.
const THEME_NAMES: &[&str] = &["dark", "light"];

/// Rayon, en pixels, de la pastille d'accent.
const DOT_SIZE: f32 = 10.0;

/// Liste de thèmes sélectionnables.
///
/// `current` est le nom du thème actuellement appliqué (la comparaison est
/// insensible à la casse). Chaque bouton appelle `on_select(theme.name)` lorsqu'il
/// est cliqué.
pub fn themes_view<'a>(
    current: &str,
    on_select: &dyn Fn(String) -> Message,
    snippets: &[(std::path::PathBuf, Vec<crate::vault::css_snippets::CssVar>)],
) -> Element<'a, Message> {
    let mut list = Column::new().spacing(6);
    for name in THEME_NAMES {
        if let Some(theme) = by_name(name) {
            list = list.push(theme_button(theme, current, on_select));
        }
    }
    list = list.push(text("Snippets CSS (sous-ensemble couleurs)").size(13));
    if snippets.is_empty() {
        list = list.push(text("Aucun snippet dans .obsidian/snippets/").size(12));
    } else {
        for (path, vars) in snippets {
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_default();
            list = list.push(text(format!("{name} — {} variable(s)", vars.len())).size(12));
        }
    }
    list.width(Length::Fill).into()
}

/// Un bouton thème : nom + pastille d'accent, surligné si thème courant.
fn theme_button<'a>(
    theme: &'static Theme,
    current: &str,
    on_select: &dyn Fn(String) -> Message,
) -> Element<'a, Message> {
    let active = theme.name.eq_ignore_ascii_case(current);
    let accent = accent_color(theme.accent);

    let row = Row::new()
        .spacing(8)
        .align_y(Alignment::Center)
        .push(text(theme.name).size(14.0))
        .push(accent_dot(accent));

    button(row)
        .width(Length::Fill)
        .on_press(on_select(theme.name.to_string()))
        .style(move |_, status| theme_button_style(status, accent, active))
        .into()
}

/// Pastille circulaire de la couleur d'accent du thème.
fn accent_dot<'a>(accent: Color) -> Element<'a, Message> {
    container(Row::new())
        .width(DOT_SIZE)
        .height(DOT_SIZE)
        .style(move |_theme| container::Style {
            background: Some(Background::Color(accent)),
            border: Border {
                color: Color::TRANSPARENT,
                width: 0.0,
                radius: Radius::from(DOT_SIZE / 2.0),
            },
            ..container::Style::default()
        })
        .into()
}

/// Style d'un bouton thème.
fn theme_button_style(status: button::Status, accent: Color, active: bool) -> button::Style {
    if active {
        // Thème courant : fond très léger de la couleur d'accent, cadre accentué
        // et texte coloré.
        button::Style {
            background: Some(Background::Color(Color::from_rgba(
                accent.r, accent.g, accent.b, 0.12,
            ))),
            border: Border {
                color: accent,
                width: 1.0,
                radius: Radius::from(4.0),
            },
            text_color: accent,
            ..button::Style::default()
        }
    } else if matches!(status, button::Status::Hovered) {
        button::Style {
            background: Some(Background::Color(Color::from_rgba(0.0, 0.0, 0.0, 0.04))),
            ..button::Style::default()
        }
    } else {
        button::Style::default()
    }
}

/// Convertit un triplet RGB `(0..=255)` en `Color` iced.
fn accent_color((r, g, b): (u8, u8, u8)) -> Color {
    Color::from_rgb(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::vault::themes::{DEFAULT_DARK, DEFAULT_LIGHT};

    #[test]
    fn les_deux_themes_connus_sont_affiches() {
        let snippets: &[(std::path::PathBuf, Vec<crate::vault::css_snippets::CssVar>)] = &[];
        let view = themes_view("dark", &|_| Message::Refresh, snippets);
        // La vue se construit sans paniquer.
        let _ = view;
    }

    #[test]
    fn theme_courant_surligne() {
        let style_dark = button_style_pour(&DEFAULT_DARK, "dark");
        assert_eq!(style_dark.text_color, accent_color(DEFAULT_DARK.accent));
        assert!(style_dark.background.is_some());

        let style_light = button_style_pour(&DEFAULT_LIGHT, "dark");
        assert_eq!(style_light.text_color, button::Style::default().text_color);
        assert_eq!(style_light.background, None);
    }

    #[test]
    fn theme_inconnu_ne_surligne_rien() {
        let style = button_style_pour(&DEFAULT_LIGHT, "obsidian");
        assert_eq!(style.text_color, button::Style::default().text_color);
        assert_eq!(style.background, None);
    }

    #[test]
    fn hover_sans_theme_courant() {
        let style = theme_button_style(
            button::Status::Hovered,
            accent_color(DEFAULT_DARK.accent),
            false,
        );
        assert!(style.background.is_some());
    }

    #[test]
    fn accent_rgb_converti_dans_l_intervalle() {
        for (r, g, b) in [(124u8, 204, 255), (0, 0, 0), (255, 255, 255), (3, 86, 188)] {
            let color = accent_color((r, g, b));
            assert!((0.0..=1.0).contains(&color.r), "r hors limite");
            assert!((0.0..=1.0).contains(&color.g), "g hors limite");
            assert!((0.0..=1.0).contains(&color.b), "b hors limite");
            assert_eq!(color.a, 1.0);
        }
    }

    /// Rend le style d'un bouton pour un thème et un thème courant donnés.
    fn button_style_pour(theme: &Theme, current: &str) -> button::Style {
        theme_button_style(
            button::Status::Active,
            accent_color(theme.accent),
            theme.name.eq_ignore_ascii_case(current),
        )
    }
}
