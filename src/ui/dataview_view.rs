//! Vue requête + tableau (style Dataview) : une barre de requête DQL, un
//! message d'erreur éventuel, puis une ligne par note résultat avec son nom,
//! ses tags, son nombre de wikilinks et la première propriété de son
//! frontmatter. Les lignes sont cliquables et ouvrent la note sélectionnée.

use iced::widget::{button, container, row, scrollable, text, text_input, Column};
use iced::{Alignment, Background, Color, Element, Length};

use crate::app::Message;
use crate::vault::note::Note;

/// Portion de largeur occupée par la colonne « Nom ».
const PORTION_NOM: u16 = 3;
/// Portion de largeur occupée par la colonne « Tags ».
const PORTION_TAGS: u16 = 2;
/// Portion de largeur occupée par la colonne « Liens ».
const PORTION_LIENS: u16 = 1;
/// Portion de largeur occupée par la colonne « Première propriété ».
const PORTION_PROPRIETE: u16 = 3;
/// Espacement horizontal entre deux colonnes.
const ESPACEMENT_COLONNES: f32 = 8.0;
/// Marge interne d'une ligne du tableau.
const MARGE_LIGNE: iced::Padding = iced::Padding {
    top: 4.0,
    right: 8.0,
    bottom: 4.0,
    left: 8.0,
};

/// Couleur du texte d'une erreur de requête.
const COULEUR_ERREUR: Color = Color::from_rgb(0.75, 0.22, 0.22);
/// Couleur de l'en-tête du tableau et des états vides.
const COULEUR_ATTENUEE: Color = Color::from_rgb(0.42, 0.46, 0.52);

/// Vue requête + tableau des notes.
///
/// `notes` est la liste complète du vault ; `indices` sélectionne et ordonne
/// les lignes à afficher (le résultat de la requête). `query` est le texte
/// courant de la barre de requête et `error` le message d'erreur de la
/// dernière exécution. Chaque ligne cliquable appelle `on_open` avec l'indice
/// dans `notes` ; les indices hors bornes sont ignorés silencieusement.
pub fn dataview_view<'a>(
    notes: &'a [Note],
    indices: &'a [usize],
    query: &'a str,
    error: Option<&'a str>,
    on_query_change: impl Fn(String) -> Message + 'a,
    on_run: Message,
    on_open: impl Fn(usize) -> Message + 'a,
) -> Element<'a, Message> {
    let input = text_input("TABLE … FROM … WHERE …", query)
        .on_input(on_query_change)
        .on_submit(on_run.clone())
        .padding(6)
        .width(Length::Fill);
    let run = button(text("Exécuter").size(13))
        .on_press(on_run)
        .padding([6, 12]);

    let mut content = Column::new()
        .spacing(6)
        .padding(12)
        .width(Length::Fill)
        .height(Length::Fill)
        .push(row![input, run].spacing(6).align_y(Alignment::Center));

    if let Some(error) = error {
        content = content.push(text(error).size(12).color(COULEUR_ERREUR));
    }

    content = content.push(header_row());

    if query.trim().is_empty() {
        content = content.push(
            text("Saisissez une requête, puis « Exécuter ».")
                .size(13)
                .color(COULEUR_ATTENUEE),
        );
        return content.into();
    }

    let mut table = Column::new().spacing(2).width(Length::Fill);
    let mut lignes = 0usize;
    for &index in indices {
        let Some(note) = notes.get(index) else {
            continue;
        };
        table = table.push(data_view_row(note, index, &on_open));
        lignes += 1;
    }

    if lignes == 0 {
        content = content.push(text("Aucun résultat.").size(13).color(COULEUR_ATTENUEE));
        return content.into();
    }

    content.push(scrollable(table).height(Length::Fill)).into()
}

/// En-tête du tableau : mêmes portions de largeur que les lignes, pour que
/// les colonnes restent alignées.
fn header_row<'a>() -> Element<'a, Message> {
    let cells = row![
        header_cell("Nom", PORTION_NOM),
        header_cell("Tags", PORTION_TAGS),
        header_cell("Liens", PORTION_LIENS),
        header_cell("Première propriété", PORTION_PROPRIETE),
    ]
    .spacing(ESPACEMENT_COLONNES)
    .align_y(Alignment::Center);

    container(cells)
        .padding(MARGE_LIGNE)
        .width(Length::Fill)
        .into()
}

/// Une cellule d'en-tête : texte discret, en minuscules optiques (taille 12).
fn header_cell<'a>(label: &'a str, portion: u16) -> Element<'a, Message> {
    text(label)
        .size(12)
        .color(COULEUR_ATTENUEE)
        .width(Length::FillPortion(portion))
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
        text(name).size(13).width(Length::FillPortion(PORTION_NOM)),
        text(tags).size(13).width(Length::FillPortion(PORTION_TAGS)),
        text(wikilinks)
            .size(13)
            .width(Length::FillPortion(PORTION_LIENS)),
        text(first)
            .size(13)
            .width(Length::FillPortion(PORTION_PROPRIETE)),
    ]
    .spacing(ESPACEMENT_COLONNES)
    .align_y(Alignment::Center);

    button(container(cells).padding(MARGE_LIGNE))
        .width(Length::Fill)
        .padding(0)
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

    /// Construit la vue avec une requête non vide et sans erreur.
    fn vue<'a>(notes: &'a [Note], indices: &'a [usize]) -> Element<'a, Message> {
        dataview_view(
            notes,
            indices,
            "TABLE file.name",
            None,
            |_query| Message::Refresh,
            Message::Refresh,
            |_index| Message::Refresh,
        )
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
    fn dataview_view_sans_note_ni_resultat_se_construit() {
        let notes: Vec<Note> = vec![];
        let indices: Vec<usize> = vec![];
        let _view = vue(&notes, &indices);
    }

    #[test]
    fn dataview_view_requete_vide_se_construit() {
        let notes = vec![note("A", None)];
        let indices = vec![0];
        let _view = dataview_view(
            &notes,
            &indices,
            "",
            None,
            |_query| Message::Refresh,
            Message::Refresh,
            |_index| Message::Refresh,
        );
    }

    #[test]
    fn dataview_view_affiche_les_lignes_dans_l_ordre_des_indices() {
        let notes = vec![note("A", None), note("B", None), note("C", None)];
        let indices = vec![2, 0, 1];
        let opened = std::cell::RefCell::new(Vec::new());
        let _view = dataview_view(
            &notes,
            &indices,
            "TABLE file.name",
            None,
            |_query| Message::Refresh,
            Message::Refresh,
            |index| {
                opened.borrow_mut().push(index);
                Message::Refresh
            },
        );
        assert_eq!(*opened.borrow(), vec![2, 0, 1]);
    }

    #[test]
    fn dataview_view_ignore_les_indices_hors_bornes() {
        let notes = vec![note("A", None)];
        let indices = vec![0, 5];
        let opened = std::cell::RefCell::new(Vec::new());
        let _view = dataview_view(
            &notes,
            &indices,
            "TABLE file.name",
            None,
            |_query| Message::Refresh,
            Message::Refresh,
            |index| {
                opened.borrow_mut().push(index);
                Message::Refresh
            },
        );
        assert_eq!(*opened.borrow(), vec![0]);
    }

    #[test]
    fn dataview_view_affiche_l_erreur_se_construit() {
        let notes = vec![note("A", None)];
        let indices: Vec<usize> = vec![];
        let _view = dataview_view(
            &notes,
            &indices,
            "TABLE ??",
            Some("Requête invalide : TABLE attendu"),
            |_query| Message::Refresh,
            Message::Refresh,
            |_index| Message::Refresh,
        );
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
