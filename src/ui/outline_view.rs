//! Vue « Plan » : table des matières de la note ouverte.
//!
//! Rendu pur : le plan est extrait du tampon d'édition (`OpenNote::content`),
//! aucune I/O n'est effectuée ici.

use iced::widget::{column, container, scrollable, text, Row};
use iced::{Color, Element, Length};

use crate::app::{App, Message};
use crate::vault::outline::{extract_outline, OutlineEntry};

/// Couleur du texte discret (aucune note, aucun titre).
const MUTED_COLOR: Color = Color::from_rgb(0.55, 0.55, 0.58);
/// Couleur d'un titre de niveau 1.
const LEVEL1_COLOR: Color = Color::from_rgb(0.25, 0.45, 0.75);
/// Couleur d'un titre de niveau 2.
const LEVEL2_COLOR: Color = Color::from_rgb(0.30, 0.55, 0.70);
/// Couleur d'un titre de niveau 3 et plus.
const LEVEL3_COLOR: Color = Color::from_rgb(0.40, 0.45, 0.55);
/// Retrait horizontal appliqué par niveau.
const INDENT_STEP: f32 = 12.0;
/// Taille du texte d'un titre.
const TITLE_SIZE: f32 = 14.0;

/// Couleur du marqueur d'un titre selon son niveau (1..=6).
fn level_color(level: u8) -> Color {
    match level {
        1 => LEVEL1_COLOR,
        2 => LEVEL2_COLOR,
        _ => LEVEL3_COLOR,
    }
}

/// Lignes affichables : niveau et titre, dans l'ordre du document.
///
/// Fonction pure, extraite pour être testable sans iced.
pub fn outline_rows(outline: &[OutlineEntry]) -> Vec<(u8, String)> {
    outline
        .iter()
        .map(|entry| (entry.level, entry.title.clone()))
        .collect()
}

/// Marqueur textuel d'un titre (`#`, `##`…), borné à 6.
fn level_marker(level: u8) -> String {
    "#".repeat(usize::from(level.clamp(1, 6)))
}

/// Vue Plan de la note ouverte.
pub fn outline_view<'a>(app: &'a App) -> Element<'a, Message> {
    let content: Element<'a, Message> = match &app.open {
        None => container(
            text("Aucune note ouverte")
                .size(TITLE_SIZE)
                .color(MUTED_COLOR),
        )
        .padding(16)
        .into(),
        Some(open) => {
            let outline = extract_outline(&open.content.text());
            if outline.is_empty() {
                container(text("Aucun titre").size(TITLE_SIZE).color(MUTED_COLOR))
                    .padding(16)
                    .into()
            } else {
                let mut entries = column![].spacing(4).width(Length::Fill);
                for (level, title) in outline_rows(&outline) {
                    let indent = f32::from(level.saturating_sub(1)) * INDENT_STEP;
                    let line: Row<'a, Message> = Row::new()
                        .push(
                            text(level_marker(level))
                                .size(TITLE_SIZE)
                                .color(level_color(level)),
                        )
                        .push(text(title).size(TITLE_SIZE))
                        .spacing(8)
                        .padding([2, indent as u16]);
                    entries = entries.push(line);
                }
                scrollable(container(entries).padding(12).width(Length::Fill)).into()
            }
        }
    };

    container(content)
        .style(crate::ui::panel_style)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(level: u8, title: &str, line: usize) -> OutlineEntry {
        OutlineEntry {
            level,
            title: title.to_string(),
            line,
        }
    }

    #[test]
    fn outline_rows_preserve_ordre_et_niveaux() {
        let outline = vec![entry(1, "Titre", 0), entry(2, "Sous-titre", 3)];
        assert_eq!(
            outline_rows(&outline),
            vec![(1, "Titre".to_string()), (2, "Sous-titre".to_string())]
        );
    }

    #[test]
    fn outline_rows_vide_donne_liste_vide() {
        assert!(outline_rows(&[]).is_empty());
    }

    #[test]
    fn level_marker_borne_a_six() {
        assert_eq!(level_marker(3), "###");
        assert_eq!(level_marker(9), "######");
        assert_eq!(level_marker(0), "#");
    }
}
