//! Vue « Tâches » : toutes les cases à cocher du vault.
//!
//! Rendu pur : la liste est agrégée au scan par `app::boot`/`App::refresh`.

use iced::widget::{button, column, container, scrollable, text, Column};
use iced::{Color, Element, Length};

use crate::app::{App, Message};
use crate::vault::tasks::{Task, TaskStatus};

/// Couleur du texte discret.
const MUTED_COLOR: Color = Color::from_rgb(0.55, 0.55, 0.58);
/// Couleur de la note d'origine.
const SOURCE_COLOR: Color = Color::from_rgb(0.45, 0.50, 0.56);
/// Taille du texte d'une tâche.
const TASK_SIZE: f32 = 14.0;
/// Taille du texte de provenance.
const SMALL_SIZE: f32 = 12.0;

/// Étiquette affichable d'une tâche : `[ ]`/`[x]` et son statut.
///
/// Fonction pure, extraite pour être testable sans iced.
pub fn task_label(task: &Task) -> (String, bool) {
    let done = task.status == TaskStatus::Done;
    let checkbox = if done { "[x]" } else { "[ ]" };
    (format!("{checkbox} {}", task.text), done)
}

/// Découpe le chemin d'une note en nom affichable (stem du fichier).
fn source_name(task: &Task) -> String {
    task.note_path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or_default()
        .to_string()
}

/// Lignes de la vue Tâches.
pub fn tasks_view<'a>(app: &'a App) -> Element<'a, Message> {
    if app.tasks.is_empty() {
        return container(text("Aucune tâche").size(TASK_SIZE).color(MUTED_COLOR))
            .style(crate::ui::panel_style)
            .padding(16)
            .width(Length::Fill)
            .height(Length::Fill)
            .into();
    }

    let mut list: Column<'a, Message> = column![].spacing(4).width(Length::Fill);
    for task in &app.tasks {
        let (label, done) = task_label(task);
        let color = if done {
            MUTED_COLOR
        } else {
            Color::from_rgb(0.2, 0.22, 0.26)
        };
        let body = column![
            text(label).size(TASK_SIZE).color(color),
            text(source_name(task)).size(SMALL_SIZE).color(SOURCE_COLOR),
        ]
        .spacing(1);
        list = list.push(
            button(body)
                .on_press(Message::OpenNote(task.note_path.clone()))
                .width(Length::Fill)
                .padding(6)
                .style(crate::ui::ghost_button_style),
        );
    }

    container(scrollable(list))
        .style(crate::ui::panel_style)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn task(text: &str, status: TaskStatus, path: &str) -> Task {
        Task {
            line: 0,
            text: text.to_string(),
            status,
            note_path: PathBuf::from(path),
        }
    }

    #[test]
    fn task_label_todo_est_non_coche() {
        let (label, done) = task_label(&task("Écrire", TaskStatus::Todo, "a.md"));
        assert_eq!(label, "[ ] Écrire");
        assert!(!done);
    }

    #[test]
    fn task_label_done_est_coche() {
        let (label, done) = task_label(&task("Fini", TaskStatus::Done, "a.md"));
        assert_eq!(label, "[x] Fini");
        assert!(done);
    }

    #[test]
    fn source_name_retire_dossier_et_extension() {
        let t = task("x", TaskStatus::Todo, "Dossier/Note.md");
        assert_eq!(source_name(&t), "Note");
    }
}
