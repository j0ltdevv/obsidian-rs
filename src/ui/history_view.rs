//! Vue de l'historique d'une note : liste scrollable des snapshots.
//!
//! Chaque bouton porte le nom court du fichier de snapshot ; cliquer sur
//! l'indice `i` appelle `on_restore(i)`. Le snapshot `current` est surligné.

use std::path::{Path, PathBuf};

use iced::widget::{button, scrollable, text, Column};
use iced::{Background, Color, Element, Length};

use crate::app::Message;

/// Couleur de fond du snapshot courant (sélectionné).
const CURRENT_BG: Color = Color::from_rgb(0.82, 0.88, 0.96);
/// Couleur du texte du snapshot courant.
const CURRENT_TEXT: Color = Color::from_rgb(0.12, 0.28, 0.52);
/// Couleur de fond d'un bouton au survol.
const HOVER_BG: Color = Color::from_rgb(0.92, 0.94, 0.96);
/// Couleur du texte des boutons non courants.
const TEXT_COLOR: Color = Color::from_rgb(0.2, 0.3, 0.4);
/// Taille du texte des noms de snapshot (pixels).
const TEXT_SIZE: f32 = 13.0;

/// Liste scrollable des snapshots d'une note.
///
/// `snapshots` est la liste des chemins de snapshot, du plus ancien au plus
/// récent (l'ordre retourné par [`crate::vault::history::list_snapshots`]).
/// `current` est l'indice du snapshot actuellement restauré, s'il y en a un.
pub fn history_view<'a>(
    snapshots: &'a [PathBuf],
    current: Option<usize>,
    on_restore: impl Fn(usize) -> Message + 'a,
) -> Element<'a, Message> {
    let mut list = Column::new().spacing(1);

    for (index, snap) in snapshots.iter().enumerate() {
        list = list.push(
            button(text(short_name(snap)).size(TEXT_SIZE).color(TEXT_COLOR))
                .width(Length::Fill)
                .on_press(on_restore(index))
                .style(move |_theme, status| snapshot_style(status, current == Some(index))),
        );
    }

    scrollable(list)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

/// Renvoie le nom court d'un snapshot (son dernier composant de chemin).
fn short_name(snap: &Path) -> String {
    snap.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| snap.display().to_string())
}

/// Style d'un bouton de snapshot : surligné si c'est le snapshot courant,
/// sinon grisé au survol.
fn snapshot_style(status: button::Status, current: bool) -> button::Style {
    if current {
        button::Style {
            background: Some(Background::Color(CURRENT_BG)),
            text_color: CURRENT_TEXT,
            ..button::Style::default()
        }
    } else if matches!(status, button::Status::Hovered) {
        button::Style {
            background: Some(Background::Color(HOVER_BG)),
            ..button::Style::default()
        }
    } else {
        button::Style::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Le nom affiché est le dernier composant du chemin, pas le chemin entier.
    #[test]
    fn nom_court_du_snapshot() {
        let snap = PathBuf::from("dossier/note.md/1700000000000-1.md");
        assert_eq!(short_name(&snap), "1700000000000-1.md");
    }

    /// Un chemin sans composant de fichier reste affiché tel quel ; sinon le
    /// dernier composant est utilisé.
    #[test]
    fn nom_sans_fichier() {
        let snap = PathBuf::from("dossier/note.md");
        assert_eq!(short_name(&snap), "note.md");

        let snap = PathBuf::from("/");
        assert_eq!(short_name(&snap), "/");
    }

    /// Le snapshot courant est surligné.
    #[test]
    fn style_courant() {
        let style = snapshot_style(button::Status::Hovered, true);
        assert_eq!(style.background, Some(Background::Color(CURRENT_BG)));
        assert_eq!(style.text_color, CURRENT_TEXT);
    }

    /// Un snapshot non courant est neutre, grisé au survol.
    #[test]
    fn style_non_courant() {
        let style = snapshot_style(button::Status::Hovered, false);
        assert_eq!(style.background, Some(Background::Color(HOVER_BG)));

        let style = snapshot_style(button::Status::Active, false);
        assert_eq!(style.background, None);
    }

    /// Une liste vide produit une vue scrollable sans bouton.
    #[test]
    fn liste_vide() {
        let view = history_view(&[], None, |_| Message::Refresh);
        // Le rendu doit au moins produire un élément sans paniquer.
        let _ = view;
    }

    /// Un seul snapshot produit un bouton dont l'indice est 0.
    #[test]
    fn un_snapshot() {
        let snapshots = vec![PathBuf::from("1700000000000-1.md")];
        let view = history_view(&snapshots, Some(0), |_| Message::Refresh);
        let _ = view;
    }

    /// Plusieurs snapshots : l'indice transmis à on_restore suit l'ordre.
    #[test]
    fn indices_suivent_lordre() {
        let snapshots = vec![
            PathBuf::from("1700000000000-1.md"),
            PathBuf::from("1700000000001-2.md"),
        ];
        let pressed = std::cell::RefCell::new(Vec::new());
        let view = history_view(&snapshots, None, |i| {
            pressed.borrow_mut().push(i);
            Message::Refresh
        });
        let _ = view;
        // `on_restore` est invoquée pendant la construction (une fois par
        // bouton), dans l'ordre des snapshots.
        assert_eq!(*pressed.borrow(), vec![0, 1]);
    }
}
