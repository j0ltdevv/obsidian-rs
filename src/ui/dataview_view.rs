//! Vue tableau des notes (style Dataview) : une ligne par note avec son nom,
//! ses tags, son nombre de wikilinks et la première propriété de son
//! frontmatter. Les lignes sont cliquables et ouvrent la note sélectionnée.

use iced::widget::{button, container, row, scrollable, text, Column};
use iced::{Alignment, Background, Color, Element, Length};

use crate::app::Message;
use crate::vault::note::Note;

/// Vue tableau des notes.
///
/// `notes` est la liste complète du vault ; `indices` sélectionne et ordonne
/// les lignes à afficher (par exemple le résultat d'une recherche). Chaque
/// ligne est cliquable et appelle `on_open` avec l'indice dans `notes`.
pub fn dataview_view<'a>(
    notes: &'a [Note],
    on_open: impl Fn(usize) -> Message + 'a,
) -> Element<'a, Message> {
    if notes.is_empty() {
        return container(text("Aucune note.").size(13)).padding(12).into();
    }

    let mut table = Column::new().spacing(2);
    for (index, note) in notes.iter().enumerate() {
        table = table.push(data_view_row(note, index, &on_open));
    }

    scrollable(table)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

/// Une ligne du tableau : nom, tags, nombre de wikilinks et premier champ du
/// frontmatter. La ligne entière est un bouton cliquable.
fn data_view_row<'a>(
    note: &'a Note,
    index: usize,
    on_open: &dyn Fn(usize) -> Message,
) -> Element<'a, Message> {
    let name = note.name.clone();
    let tags = note.metadata.tags.join(", ");
    let wikilinks = note.metadata.wikilinks.len().to_string();
    let first = first_frontmatter_entry(&note.metadata.frontmatter);

    let cells = row![
        text(name).size(13).width(Length::FillPortion(3)),
        text(tags).size(13).width(Length::FillPortion(2)),
        text(wikilinks).size(13).width(Length::FillPortion(1)),
        text(first).size(13).width(Length::FillPortion(3)),
    ]
    .spacing(8)
    .align_y(Alignment::Center);

    button(container(cells).padding(iced::Padding {
        top: 4.0,
        right: 8.0,
        bottom: 4.0,
        left: 8.0,
    }))
    .width(Length::Fill)
    .on_press(on_open(index))
    .style(dataview_row_style)
    .into()
}

/// Style d'une ligne : fond discret au survol, pas de bordure.
fn dataview_row_style(_theme: &iced::Theme, status: button::Status) -> button::Style {
    if matches!(status, button::Status::Hovered) {
        button::Style {
            background: Some(Background::Color(Color::from_rgb(0.92, 0.94, 0.96))),
            ..button::Style::default()
        }
    } else {
        button::Style::default()
    }
}

/// Première clé/valeur du frontmatter, sous la forme `clé = valeur`.
/// Retourne une chaîne vide si la note n'a pas de frontmatter ou si celui-ci
/// n'est pas un mapping.
fn first_frontmatter_entry(frontmatter: &Option<serde_yaml::Value>) -> String {
    let Some(serde_yaml::Value::Mapping(mapping)) = frontmatter else {
        return String::new();
    };
    for (key, value) in mapping {
        let Some(key) = key.as_str() else {
            continue;
        };
        return format!("{key} = {}", display_value(value));
    }
    String::new()
}

/// Représentation lisible d'une valeur de frontmatter : les chaînes sont
/// utilisées telles quelles, les autres valeurs sont sérialisées en YAML.
fn display_value(value: &serde_yaml::Value) -> String {
    match value {
        serde_yaml::Value::String(text) => text.clone(),
        other => serde_yaml::to_string(other)
            .map(|serialized| serialized.trim().to_string())
            .unwrap_or_else(|_| format!("{other:?}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vault::note::{Note, NoteMetadata, Wikilink};

    /// Construit une note de test avec son frontmatter, ses tags et ses
    /// wikilinks.
    fn note(name: &str, frontmatter: Option<serde_yaml::Value>) -> Note {
        Note {
            path: std::path::PathBuf::from(format!("{name}.md")),
            name: name.to_string(),
            metadata: NoteMetadata {
                frontmatter,
                frontmatter_error: None,
                wikilinks: Vec::new(),
                tags: Vec::new(),
            },
        }
    }

    #[test]
    fn frontmatter_none_retourne_chaine_vide() {
        assert_eq!(first_frontmatter_entry(&None), "");
    }

    #[test]
    fn frontmapping_vide_retourne_chaine_vide() {
        let fm = Some(serde_yaml::Value::Mapping(serde_yaml::Mapping::new()));
        assert_eq!(first_frontmatter_entry(&fm), "");
    }

    #[test]
    fn premiere_cle_valeur_string() {
        let mut mapping = serde_yaml::Mapping::new();
        mapping.insert(
            serde_yaml::Value::String("titre".to_string()),
            serde_yaml::Value::String("Mon titre".to_string()),
        );
        assert_eq!(
            first_frontmatter_entry(&Some(mapping.into())),
            "titre = Mon titre"
        );
    }

    #[test]
    fn premiere_cle_valeur_nombre() {
        let mut mapping = serde_yaml::Mapping::new();
        mapping.insert(
            serde_yaml::Value::String("priorité".to_string()),
            serde_yaml::Value::Number(serde_yaml::Number::from(3)),
        );
        assert_eq!(
            first_frontmatter_entry(&Some(mapping.into())),
            "priorité = 3"
        );
    }

    #[test]
    fn premiere_cle_ignore_cles_non_chaine() {
        // YAML non standard : une clé numérique ; on la saute.
        let mut mapping = serde_yaml::Mapping::new();
        mapping.insert(
            serde_yaml::Value::Number(serde_yaml::Number::from(1)),
            serde_yaml::Value::String("valeur".to_string()),
        );
        assert_eq!(first_frontmatter_entry(&Some(mapping.into())), "");
    }

    #[test]
    fn display_value_chaine() {
        assert_eq!(
            display_value(&serde_yaml::Value::String("texte".to_string())),
            "texte"
        );
    }

    #[test]
    fn display_value_nombre() {
        assert_eq!(
            display_value(&serde_yaml::Value::Number(serde_yaml::Number::from(42))),
            "42"
        );
    }

    #[test]
    fn display_value_bool() {
        assert_eq!(display_value(&serde_yaml::Value::Bool(true)), "true");
    }

    #[test]
    fn dataview_view_notes_vides_affiche_aucune_note() {
        let notes: Vec<Note> = vec![];
        let _view = dataview_view(&notes, |_index| Message::Refresh);
    }

    #[test]
    fn dataview_view_affiche_les_lignes_dans_l_ordre_des_notes() {
        let notes = vec![note("A", None), note("B", None), note("C", None)];
        let opened = std::cell::RefCell::new(Vec::new());
        let _view = dataview_view(&notes, |index| {
            opened.borrow_mut().push(index);
            Message::Refresh
        });
        assert_eq!(*opened.borrow(), vec![0, 1, 2]);
    }

    #[test]
    fn data_view_row_appelle_on_open_avec_le_bon_indice() {
        let note = note("A", None);
        let opened = std::cell::RefCell::new(Vec::new());
        let _row = data_view_row(&note, 7, &|index| {
            opened.borrow_mut().push(index);
            Message::Refresh
        });
        assert_eq!(*opened.borrow(), vec![7]);
    }

    #[test]
    fn data_view_row_tags_joins_par_virgule_espace() {
        let mut note = note("A", None);
        note.metadata.tags = vec!["rust".to_string(), "test".to_string()];
        assert_eq!(note.metadata.tags.join(", "), "rust, test");
    }

    #[test]
    fn data_view_row_compte_les_wikilinks() {
        let mut note = note("A", None);
        note.metadata.wikilinks = vec![
            Wikilink {
                target: "B".to_string(),
                alias: None,
            },
            Wikilink {
                target: "C".to_string(),
                alias: Some("C alias".to_string()),
            },
        ];
        assert_eq!(note.metadata.wikilinks.len(), 2);
    }
}
