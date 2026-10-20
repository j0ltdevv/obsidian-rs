//! Panneau de recherche full-text dans le vault.
//!
//! Rendu pur : les résultats ont déjà été calculés par `app::update` (index
//! inversé), la vue ne fait que les présenter.

use iced::widget::{button, column, container, scrollable, text, text_input, Column};
use iced::{Color, Element, Length};

use crate::app::{App, Message, SearchPanelState};
use crate::vault::Note;

/// Couleur du texte discret.
const MUTED_COLOR: Color = Color::from_rgb(0.55, 0.55, 0.58);
/// Couleur du chemin d'une note dans les résultats.
const PATH_COLOR: Color = Color::from_rgb(0.45, 0.50, 0.56);
/// Taille du nom d'une note.
const NAME_SIZE: f32 = 15.0;
/// Taille du chemin et des messages discrets.
const SMALL_SIZE: f32 = 12.0;

/// Résultats affichables : ceux dont l'indice existe dans `notes`.
/// Fonction pure, testable sans iced.
pub fn result_rows(state: &SearchPanelState, notes: &[Note]) -> Vec<crate::app::SearchHit> {
    state
        .results
        .iter()
        .filter(|hit| hit.index < notes.len())
        .cloned()
        .collect()
}

/// Panneau de recherche full-text.
pub fn search_view<'a>(app: &'a App) -> Element<'a, Message> {
    let notes: &[Note] = app.notes.as_ref().map(|n| n.as_slice()).unwrap_or(&[]);

    let Some(state) = &app.search_panel else {
        return container(text("Recherche fermée").size(SMALL_SIZE).color(MUTED_COLOR))
            .padding(16)
            .into();
    };

    let input = text_input("Recherche dans le vault…", &state.query)
        .on_input(Message::SearchQuery)
        .padding(8)
        .width(Length::Fill);

    let rows = result_rows(state, notes);
    let body: Element<'a, Message> = if state.query.trim().is_empty() {
        text("Tapez une requête")
            .size(SMALL_SIZE)
            .color(MUTED_COLOR)
            .into()
    } else if rows.is_empty() {
        text("Aucun résultat")
            .size(SMALL_SIZE)
            .color(MUTED_COLOR)
            .into()
    } else {
        let mut list: Column<'a, Message> = column![].spacing(4).width(Length::Fill);
        for (row_index, hit) in rows.iter().enumerate() {
            let Some(note) = notes.get(hit.index) else {
                continue;
            };
            let label = column![
                text(note.name.clone()).size(NAME_SIZE),
                text(note.path.display().to_string())
                    .size(SMALL_SIZE)
                    .color(PATH_COLOR),
                text(hit.snippet.clone())
                    .size(SMALL_SIZE)
                    .color(Color::from_rgb(0.35, 0.38, 0.42)),
            ]
            .spacing(1);
            list = list.push(
                button(label)
                    .on_press(Message::SearchSelect(row_index))
                    .width(Length::Fill)
                    .padding(6)
                    .style(crate::ui::ghost_button_style),
            );
        }
        scrollable(list).into()
    };

    container(column![input, body].spacing(8).padding(12))
        .style(crate::ui::panel_style)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vault::NoteMetadata;
    use std::path::PathBuf;

    fn note(name: &str) -> Note {
        Note {
            path: PathBuf::from(format!("{name}.md")),
            name: name.to_string(),
            metadata: NoteMetadata {
                frontmatter: None,
                frontmatter_error: None,
                wikilinks: Vec::new(),
                tags: Vec::new(),
            },
        }
    }

    fn hit(index: usize) -> crate::app::SearchHit {
        crate::app::SearchHit {
            index,
            snippet: "…".to_string(),
        }
    }

    fn state(results: Vec<crate::app::SearchHit>) -> SearchPanelState {
        SearchPanelState {
            query: "x".to_string(),
            results,
        }
    }

    #[test]
    fn result_rows_filtre_les_indices_hors_bornes() {
        let notes = vec![note("a"), note("b")];
        let hits = result_rows(&state(vec![hit(1), hit(5), hit(0)]), &notes);
        assert_eq!(hits.iter().map(|h| h.index).collect::<Vec<_>>(), vec![1, 0]);
    }

    #[test]
    fn result_rows_conserve_l_ordre_de_score() {
        let notes = vec![note("a"), note("b"), note("c")];
        let hits = result_rows(&state(vec![hit(2), hit(0)]), &notes);
        assert_eq!(hits.iter().map(|h| h.index).collect::<Vec<_>>(), vec![2, 0]);
    }

    #[test]
    fn result_rows_vide_sans_resultat() {
        let notes = vec![note("a")];
        assert!(result_rows(&state(Vec::new()), &notes).is_empty());
    }
}
