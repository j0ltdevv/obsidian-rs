//! Panneau « Corbeille » de la sidebar gauche : liste des éléments supprimés
//! (chemin d'origine, date, taille) avec restauration et vidage.
//!
//! Le rendu est pur (aucune I/O) : les entrées et les décisions d'action
//! viennent de `App`, et les opérations sont déclenchées par des `Message`
//! traités dans `app::update`. La mise en forme des lignes (`TrashRow`) et
//! les conversions date/taille sont isolées dans des fonctions pures testables.

use iced::border::Radius;
use iced::widget::{button, column, container, row, scrollable, text, Column};
use iced::{Alignment, Background, Border, Color, Element, Length};

use crate::app::{format_deleted_at, human_size, App, Message};
use crate::vault::trash::TrashEntry;

/// Une ligne affichable de la corbeille (données purement formatées).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrashRow {
    /// Chemin d'origine de l'élément, relatif au vault.
    pub path: String,
    /// Date de suppression (format `AAAA-MM-JJ HH:MM`).
    pub date: String,
    /// Taille humaine lisible.
    pub size: String,
}

/// Construit les lignes affichables d'une liste d'entrées de la corbeille.
pub fn trash_rows(entries: &[TrashEntry]) -> Vec<TrashRow> {
    entries
        .iter()
        .map(|entry| TrashRow {
            path: entry.original_path.display().to_string(),
            date: format_deleted_at(entry.deleted_at),
            size: human_size(entry.size),
        })
        .collect()
}

/// Panneau « Corbeille » de la sidebar.
pub fn trash_panel<'a>(app: &'a App) -> Element<'a, Message> {
    let header = row![
        text("Corbeille").size(14).font(super::bold_font()),
        text("").width(Length::Fill),
        button(text("Vider la corbeille"))
            .on_press(Message::TrashEmptyRequest)
            .style(super::primary_button_style),
    ]
    .spacing(6)
    .padding([2, 8]);

    let mut body = Column::new().spacing(4);

    if let Some(err) = &app.trash_error {
        body = body.push(
            container(text(format!("Erreur : {err}")).size(12).color(ERROR_COLOR))
                .width(Length::Fill)
                .padding([4, 8])
                .style(|_theme| container::Style {
                    background: Some(Background::Color(ERROR_BG)),
                    border: Border {
                        color: ERROR_COLOR,
                        width: 1.0,
                        radius: Radius::from(4.0),
                    },
                    ..container::Style::default()
                }),
        );
    }

    let rows = trash_rows(&app.trash_entries);
    if rows.is_empty() {
        body = body.push(
            text("La corbeille est vide.")
                .size(13)
                .color(Color::from_rgb(0.55, 0.55, 0.58)),
        );
    } else {
        for (entry, row_data) in app.trash_entries.iter().zip(&rows) {
            let item = row![
                text(row_data.path.clone()).size(12).width(Length::Fill),
                text(row_data.date.clone())
                    .size(11)
                    .color(Color::from_rgb(0.5, 0.5, 0.5)),
                text(row_data.size.clone())
                    .size(11)
                    .color(Color::from_rgb(0.5, 0.5, 0.5)),
                button(text("Restaurer"))
                    .on_press(Message::TrashRestore(entry.clone()))
                    .style(super::ghost_button_style),
            ]
            .spacing(6)
            .align_y(Alignment::Center)
            .padding([2, 4]);
            body = body.push(item);
        }
    }

    column![header, scrollable(body)]
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

/// Couleur du texte d'erreur (partagée avec le module parent).
const ERROR_COLOR: Color = Color::from_rgb(0.85, 0.2, 0.2);
/// Couleur de fond de la bannière d'erreur.
const ERROR_BG: Color = Color::from_rgb(0.98, 0.9, 0.9);
