//! Vues iced de l'application (rendu pur, aucune I/O).
//!
//! Mise en page : barre de titre, bannière d'erreur éventuelle, puis un
//! panneau à trois colonnes — arborescence (sidebar), éditeur Markdown et
//! prévisualisation (texte brut + métadonnées parsées + backlinks).
//!
//! Phase 2 :
//! - recherche rapide (overlay centré, `App::switcher`),
//! - édition du frontmatter dans la prévisualisation (brouillons,
//!   `App::open.property_drafts`),
//! - vue graphe (`App::show_graph`) à la place du panneau central,
//! - boutons de la barre de titre pour ouvrir la recherche et basculer le
//!   graphe (les raccourcis clavier globaux sont câblés dans
//!   `app::subscription`).

use std::path::Path;

use iced::border::Radius;
use iced::font::Weight;
use iced::widget::{
    button, column, container, responsive, row, scrollable, stack, text, text_editor, text_input,
    Column,
};
use iced::{Alignment, Background, Border, Color, Element, Font, Length};

use crate::app::{App, Message};
use crate::vault::{Note, Wikilink};

mod canvas_view;
mod dataview_view;
mod graph;
mod history_view;
mod plugins_view;
mod themes_view;

use self::canvas_view::canvas_board_view;
use self::dataview_view::dataview_view;
use self::graph::graph_view;
use self::history_view::history_view;
use self::plugins_view::plugins_view;
use self::themes_view::themes_view;

/// Couleur du titre de la barre supérieure.
const TITLE_COLOR: Color = Color::from_rgb(0.25, 0.45, 0.75);
/// Couleur du texte d'erreur.
const ERROR_COLOR: Color = Color::from_rgb(0.85, 0.2, 0.2);
/// Couleur de fond de la bannière d'erreur.
const ERROR_BG: Color = Color::from_rgb(0.98, 0.9, 0.9);
/// Couleur de fond des panneaux.
const PANEL_BG: Color = Color::from_rgb(0.96, 0.97, 0.98);
/// Couleur du cadre des panneaux.
const PANEL_BORDER: Color = Color::from_rgb(0.83, 0.86, 0.89);
/// Couleur d'un lien non résolu.
const UNRESOLVED_COLOR: Color = Color::from_rgb(0.55, 0.55, 0.58);

/// Vue racine de l'application.
pub fn view(app: &App) -> Element<'_, Message> {
    let body = match &app.notes {
        Ok(notes) => workspace_view(app, notes),
        Err(error) => error_view(error),
    };

    let mut root = Column::new().push(title_bar());
    if let Some(error) = &app.error {
        root = root.push(error_banner(error));
    }
    root = root.push(body);
    let root = root.height(Length::Fill).into();

    // Overlay de la recherche rapide, par-dessus toute la fenêtre.
    if app.switcher.is_some() {
        stack([root, switcher_overlay(app)])
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    } else if app.command_palette.is_some() {
        stack([root, command_palette_overlay(app)])
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    } else {
        root
    }
}

/// Barre de titre stable de l'application.
fn title_bar() -> Element<'static, Message> {
    let title = text("obsidian-rs")
        .size(24)
        .font(bold_font())
        .color(TITLE_COLOR);
    row![
        title,
        text("").width(Length::Fill),
        button(text("Recherche"))
            .on_press(Message::OpenSwitcher)
            .style(primary_button_style),
        button(text("Graphe"))
            .on_press(Message::ToggleGraph)
            .style(primary_button_style),
        button(text("Thème"))
            .on_press(Message::ToggleThemes)
            .style(primary_button_style),
        button(text("Historique"))
            .on_press(Message::ToggleHistory)
            .style(primary_button_style),
        button(text("Dataview"))
            .on_press(Message::ToggleDataview)
            .style(primary_button_style),
        button(text("Plugins"))
            .on_press(Message::TogglePlugins)
            .style(primary_button_style),
        button(text("Canvas"))
            .on_press(Message::ToggleCanvas)
            .style(primary_button_style),
        button(text("Actualiser"))
            .on_press(Message::Refresh)
            .style(primary_button_style),
    ]
    .align_y(Alignment::Center)
    .spacing(8)
    .padding([10, 16])
    .into()
}

/// Overlay de la recherche rapide : panneau centré sur fond assombri.
fn switcher_overlay<'a>(app: &'a App) -> Element<'a, Message> {
    let Some(state) = &app.switcher else {
        return stack([]).into();
    };

    let mut results = Column::new().spacing(2);
    for &index in &state.results {
        let name = app
            .notes
            .as_ref()
            .ok()
            .and_then(|notes| notes.get(index))
            .map(|note| note.name.clone())
            .unwrap_or_else(|| format!("#{index}"));
        results = results.push(tree_button(
            row![text(name).width(Length::Fill)].into(),
            Message::SwitcherSelect(index),
            false,
            0,
        ));
    }
    if state.results.is_empty() {
        results = results.push(text("Aucun résultat.").size(13));
    }

    let panel = container(
        column![
            text("Recherche rapide").size(16).font(bold_font()),
            text_input("Filtrer les notes…", &state.query)
                .on_input(Message::SwitcherQuery)
                .padding([7, 8]),
            scrollable(results).height(Length::FillPortion(3)),
            row![
                text("Ctrl+K : ouvrir · Échap : fermer")
                    .size(12)
                    .color(UNRESOLVED_COLOR)
                    .width(Length::Fill),
                button(text("Fermer"))
                    .on_press(Message::CloseSwitcher)
                    .style(primary_button_style),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        ]
        .spacing(8)
        .padding(16),
    )
    .width(Length::Fixed(440.0))
    .height(Length::Shrink)
    .style(panel_style);

    container(panel)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .style(overlay_style)
        .into()
}
/// Overlay de la palette de commandes (Ctrl+P).
fn command_palette_overlay<'a>(app: &'a App) -> Element<'a, Message> {
    let Some(state) = &app.command_palette else {
        return stack([]).into();
    };

    let mut results = Column::new().spacing(2);
    for &index in &state.results {
        let command = app.commands.get(index);
        let Some(command) = command else { continue };
        let shortcut_text = command
            .shortcut
            .map(|s| format!(" · {s:?}"))
            .unwrap_or_default();
        results = results.push(tree_button(
            row![
                text(command.label).width(Length::Fill),
                text(command.category).size(12).color(UNRESOLVED_COLOR),
                text(shortcut_text).size(12).color(UNRESOLVED_COLOR),
            ]
            .spacing(8)
            .into(),
            Message::CommandPaletteSelect(index),
            false,
            0,
        ));
    }
    if state.results.is_empty() {
        results = results.push(text("Aucune commande.").size(13));
    }

    let panel = container(
        column![
            text("Palette de commandes").size(16).font(bold_font()),
            text_input("Taper une commande…", &state.query)
                .on_input(Message::CommandPaletteQuery)
                .padding([7, 8]),
            scrollable(results).height(Length::FillPortion(3)),
            row![
                text("Échap : fermer")
                    .size(12)
                    .color(UNRESOLVED_COLOR)
                    .width(Length::Fill),
                button(text("Fermer"))
                    .on_press(Message::CloseCommandPalette)
                    .style(primary_button_style),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        ]
        .spacing(8)
        .padding(16),
    )
    .width(Length::Fixed(520.0))
    .height(Length::Shrink)
    .style(panel_style);

    container(panel)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .style(overlay_style)
        .into()
}

/// Espace de travail : sidebar + éditeur + prévisualisation, ou vue graphe.
fn workspace_view<'a>(app: &'a App, notes: &'a [Note]) -> Element<'a, Message> {
    if app.show_graph {
        // Le canevas a besoin de ses dimensions réelles pour placer les
        // nœuds : `responsive` fournit la taille disponible au layout.
        return responsive(move |size| graph_view(app, notes, size)).into();
    }

    let sidebar = container(sidebar(app, notes))
        .style(panel_style)
        .width(Length::FillPortion(3))
        .height(Length::Fill);

    if app.show_canvas {
        let report: Vec<(u64, std::path::PathBuf)> = notes
            .iter()
            .enumerate()
            .map(|(i, n)| (i as u64, n.path.clone()))
            .collect();
        let canvas = canvas_board_view(&app.canvas, &report);
        return row![
            sidebar,
            container(canvas)
                .style(panel_style)
                .width(Length::FillPortion(4))
                .height(Length::Fill)
        ]
        .width(Length::Fill)
        .height(Length::Fill)
        .spacing(8)
        .padding([8, 12])
        .into();
    }

    if app.show_themes {
        let on_select = |name: String| Message::SelectTheme(name);
        let themes = themes_view(&app.current_theme, &on_select);
        return row![
            sidebar,
            container(themes)
                .style(panel_style)
                .width(Length::FillPortion(4))
                .height(Length::Fill)
        ]
        .width(Length::Fill)
        .height(Length::Fill)
        .spacing(8)
        .padding([8, 12])
        .into();
    }

    if app.show_history && app.open.is_some() {
        let history = history_view(&app.history_snapshots, None, |index| {
            Message::RestoreSnapshot(index)
        });
        return row![
            sidebar,
            container(history)
                .style(panel_style)
                .width(Length::FillPortion(4))
                .height(Length::Fill)
        ]
        .width(Length::Fill)
        .height(Length::Fill)
        .spacing(8)
        .padding([8, 12])
        .into();
    }

    if app.show_plugins {
        let plugins = plugins_view(&app.plugins.info());
        return row![
            sidebar,
            container(plugins)
                .style(panel_style)
                .width(Length::FillPortion(4))
                .height(Length::Fill)
        ]
        .width(Length::Fill)
        .height(Length::Fill)
        .spacing(8)
        .padding([8, 12])
        .into();
    }

    if app.show_dataview {
        let dview = dataview_view(notes, |i| {
            let path = notes.get(i).map(|n| n.path.clone()).unwrap_or_default();
            Message::OpenNote(path)
        });
        return row![
            sidebar,
            container(dview)
                .style(panel_style)
                .width(Length::FillPortion(4))
                .height(Length::Fill)
        ]
        .width(Length::Fill)
        .height(Length::Fill)
        .spacing(8)
        .padding([8, 12])
        .into();
    }

    let editor = editor_panel(app);
    let preview = preview_panel(app, notes);

    row![sidebar, editor, preview]
        .width(Length::Fill)
        .height(Length::Fill)
        .spacing(8)
        .padding([8, 12])
        .into()
}

/// Message d'erreur plein écran quand le scan a échoué.
fn error_view(error: &str) -> Element<'_, Message> {
    column![text(format!("Erreur de scan : {error}")).color(ERROR_COLOR)]
        .padding([12, 16])
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

/// Bannière d'erreur repliable (dernier échec d'une opération).
fn error_banner(error: &str) -> Element<'_, Message> {
    container(
        row![
            text(format!("Erreur : {error}"))
                .color(ERROR_COLOR)
                .width(Length::Fill),
            button(text("Fermer"))
                .on_press(Message::DismissError)
                .style(primary_button_style),
        ]
        .spacing(8)
        .align_y(Alignment::Center),
    )
    .width(Length::Fill)
    .padding([8, 12])
    .style(|_theme| container::Style {
        background: Some(Background::Color(ERROR_BG)),
        border: Border {
            color: ERROR_COLOR,
            width: 1.0,
            radius: Radius::from(6.0),
        },
        ..container::Style::default()
    })
    .into()
}

/// Barre latérale : arborescence des notes, dossiers repliables.
fn sidebar<'a>(app: &'a App, notes: &'a [Note]) -> Element<'a, Message> {
    scrollable(tree_node(app, notes, Path::new(""), 0))
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

/// Rendu récursif d'un niveau d'arborescence (dossiers puis notes).
fn tree_node<'a>(
    app: &'a App,
    notes: &'a [Note],
    dir: &Path,
    depth: usize,
) -> Element<'a, Message> {
    let (subdirs, files) = children_of(notes, dir);
    let mut tree = Column::new().spacing(1);

    for subdir in subdirs {
        let expanded = app.expanded.contains(&subdir);
        let name = subdir
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| subdir.display().to_string());
        let arrow = if expanded { "▾ " } else { "▸ " };

        let header = tree_button(
            row![text(format!("{arrow}{name}")).size(14)].into(),
            Message::ToggleFolder(subdir.clone()),
            false,
            depth,
        );
        tree = tree.push(header);

        if expanded {
            tree = tree.push(tree_node(app, notes, &subdir, depth + 1));
        }
    }

    for note in files {
        let active = app.open.as_ref().is_some_and(|open| open.path == note.path);
        let label = row![text(&note.name).width(Length::Fill)].into();
        tree = tree.push(tree_button(
            label,
            Message::OpenNote(note.path.clone()),
            active,
            depth,
        ));
    }

    tree.into()
}

/// Enfants directs d'un dossier : sous-dossiers (triés) et notes (triées).
fn children_of<'a>(notes: &'a [Note], dir: &Path) -> (Vec<std::path::PathBuf>, Vec<&'a Note>) {
    let mut subdirs = std::collections::BTreeSet::new();
    let mut files = Vec::new();
    let depth = dir.components().count();

    for note in notes {
        let Some(parent) = note.path.parent() else {
            continue;
        };
        if parent == dir {
            files.push(note);
        } else if parent.starts_with(dir) && parent.components().count() == depth + 1 {
            subdirs.insert(parent.to_path_buf());
        }
    }

    files.sort_by(|a, b| a.name.cmp(&b.name));
    (subdirs.into_iter().collect(), files)
}

/// Panneau éditeur : en-tête (nom + bouton de sauvegarde) et zone d'édition.
fn editor_panel<'a>(app: &'a App) -> Element<'a, Message> {
    let Some(open) = &app.open else {
        return container(text("Aucune note ouverte"))
            .width(Length::Fill)
            .height(Length::Fill)
            .style(panel_style)
            .into();
    };

    let header = row![
        text(&open.name).size(16).font(bold_font()),
        text("").width(Length::Fill),
        button(text("Sauvegarder"))
            .on_press(Message::SaveNote)
            .style(primary_button_style),
    ]
    .align_y(Alignment::Center)
    .spacing(8)
    .padding([0, 8]);

    let editor = text_editor(&open.content)
        .placeholder("Écrivez…")
        .on_action(Message::EditorAction)
        .padding([8, 10])
        .height(Length::Fill);

    column![header, editor]
        .spacing(8)
        .width(Length::FillPortion(5))
        .height(Length::Fill)
        .into()
}

/// Panneau prévisualisation : texte brut, métadonnées, wikilinks et backlinks.
fn preview_panel<'a>(app: &'a App, notes: &'a [Note]) -> Element<'a, Message> {
    let Some(open) = &app.open else {
        return container(text("Aucune note ouverte"))
            .width(Length::Fill)
            .height(Length::Fill)
            .style(panel_style)
            .into();
    };

    let open_note = notes.iter().find(|note| note.path == open.path);

    let mut preview = Column::new().spacing(8);
    preview = preview.push(section_title("Prévisualisation"));

    // Contenu brut tel quel (tampon d'édition courant).
    let raw = scrollable(text(open.content.text()).font(Font::MONOSPACE).size(13))
        .height(Length::FillPortion(3));
    preview = preview.push(raw);

    // Métadonnées parsées (snapshot du dernier scan).
    preview = preview.push(metadata_block(app, open));

    // Backlinks de la note ouverte, en bas de la prévisualisation.
    preview = preview.push(section_title("Backlinks"));
    let backlinks = match open_note {
        Some(note) => app
            .index
            .as_ref()
            .map(|index| index.backlinks(note))
            .unwrap_or_default(),
        None => Vec::new(),
    };
    if backlinks.is_empty() {
        preview = preview.push(text("Aucun backlink.").size(13));
    } else {
        let mut list = Column::new().spacing(2);
        for link in backlinks {
            let item = tree_button(
                row![text(&link.name)].into(),
                Message::OpenNote(link.path.clone()),
                false,
                0,
            );
            list = list.push(item);
        }
        preview = preview.push(list);
    }

    scrollable(preview)
        .width(Length::FillPortion(4))
        .height(Length::Fill)
        .into()
}

/// Bloc métadonnées : frontmatter éditable, tags et wikilinks navigables.
fn metadata_block<'a>(app: &'a App, open: &'a crate::app::OpenNote) -> Element<'a, Message> {
    let mut block = Column::new().spacing(4);

    match (&open.metadata.frontmatter, &open.metadata.frontmatter_error) {
        (Some(_), _) => {
            block = block.push(section_title("Frontmatter"));
            block = block.push(frontmatter_editor(open));
        }
        (None, Some(error)) => {
            block = block.push(
                text(format!("Frontmatter invalide : {error}"))
                    .color(ERROR_COLOR)
                    .size(13),
            );
        }
        (None, None) => {}
    }

    if !open.metadata.tags.is_empty() {
        block = block.push(section_title("Tags"));
        let mut chips = row![].spacing(4);
        for tag in &open.metadata.tags {
            chips = chips.push(tag_chip(tag));
        }
        block = block.push(chips);
    }

    if !open.metadata.wikilinks.is_empty() {
        block = block.push(section_title("Wikilinks"));
        let mut links = Column::new().spacing(2);
        for link in &open.metadata.wikilinks {
            links = links.push(wikilink_button(app, link));
        }
        block = block.push(links);
    }

    block.into()
}

/// Éditeur ligne par ligne du frontmatter : une ligne par propriété existante
/// (valeur éditée via brouillon, validation ✓ / suppression ×), plus une ligne
/// d'ajout (clé + valeur + bouton).
fn frontmatter_editor<'a>(open: &'a crate::app::OpenNote) -> Element<'a, Message> {
    let drafts = &open.property_drafts;

    let mut rows = Column::new().spacing(4);
    for (key, value) in &drafts.values {
        rows = rows.push(property_row(key, value));
    }

    rows = rows.push(
        row![
            text_input("Nouvelle clé", &drafts.new_key)
                .on_input(Message::NewPropertyKeyChanged)
                .width(Length::FillPortion(2)),
            text_input("Valeur", &drafts.new_value)
                .on_input(Message::NewPropertyValueChanged)
                .width(Length::FillPortion(3)),
            button(text("+"))
                .on_press(Message::AddProperty {
                    key: drafts.new_key.clone(),
                    value: drafts.new_value.clone(),
                })
                .style(ghost_button_style),
        ]
        .spacing(6)
        .align_y(Alignment::Center),
    );

    container(rows)
        .width(Length::Fill)
        .padding(6)
        .style(panel_style)
        .into()
}

/// Une ligne de propriété : clé, champ valeur, valider (✓) et supprimer (×).
fn property_row<'a>(key: &'a str, value: &'a str) -> Element<'a, Message> {
    row![
        text(key).size(13).width(Length::FillPortion(2)),
        text_input("Valeur", value)
            .on_input(move |new_value| Message::PropertyValueChanged {
                key: key.to_string(),
                value: new_value,
            })
            .width(Length::FillPortion(3)),
        button(text("✓"))
            .on_press(Message::SetProperty {
                key: key.to_string(),
                value: value.to_string(),
            })
            .style(ghost_button_style),
        button(text("×"))
            .on_press(Message::RemoveProperty {
                key: key.to_string()
            })
            .style(ghost_button_style),
    ]
    .spacing(6)
    .align_y(Alignment::Center)
    .into()
}

/// Bouton de wikilink : cliquable s'il est résolu via l'index, sinon grisé.
fn wikilink_button<'a>(app: &'a App, link: &'a Wikilink) -> Element<'a, Message> {
    let label = link.alias.clone().unwrap_or_else(|| link.target.clone());
    let resolved = app
        .index
        .as_ref()
        .is_some_and(|index| !index.resolve(&link.target).is_empty());

    if resolved {
        tree_button(
            row![text(format!("[[{label}]]")).size(13)].into(),
            Message::OpenWikilink(link.target.clone()),
            false,
            0,
        )
    } else {
        container(
            text(format!("[[{label}]]"))
                .size(13)
                .color(UNRESOLVED_COLOR),
        )
        .width(Length::Fill)
        .padding([4, 8])
        .style(unresolved_link_style)
        .into()
    }
}

/// Pastille de tag.
fn tag_chip<'a>(tag: &'a str) -> Element<'a, Message> {
    container(
        text(format!("#{tag}"))
            .size(12)
            .color(Color::from_rgb(0.2, 0.42, 0.65)),
    )
    .padding([3, 8])
    .style(|_theme| container::Style {
        background: Some(Background::Color(Color::from_rgb(0.87, 0.92, 0.97))),
        border: Border {
            color: Color::from_rgb(0.7, 0.8, 0.9),
            width: 1.0,
            radius: Radius::from(12.0),
        },
        ..container::Style::default()
    })
    .into()
}

/// Ligne d'arborescence (dossier ou note) : bouton pleine largeur, texte à
/// gauche, indentation selon la profondeur, surlignage si note active.
fn tree_button<'a>(
    label: Element<'a, Message>,
    message: Message,
    active: bool,
    depth: usize,
) -> Element<'a, Message> {
    let left = (8 + depth * 12) as f32;
    let inner = container(label).width(Length::Fill).padding(iced::Padding {
        top: 4.0,
        right: 8.0,
        bottom: 4.0,
        left,
    });
    button(inner)
        .width(Length::Fill)
        .on_press(message)
        .style(move |_theme, status| tree_button_style(status, active))
        .into()
}

/// Style d'un bouton d'arborescence.
fn tree_button_style(status: button::Status, active: bool) -> button::Style {
    if active {
        button::Style {
            background: Some(Background::Color(Color::from_rgb(0.82, 0.88, 0.96))),
            text_color: Color::from_rgb(0.12, 0.28, 0.52),
            ..button::Style::default()
        }
    } else if matches!(status, button::Status::Hovered) {
        button::Style {
            background: Some(Background::Color(Color::from_rgb(0.92, 0.94, 0.96))),
            ..button::Style::default()
        }
    } else {
        button::Style::default()
    }
}

/// Style du bouton principal (sauvegarde, fermer).
fn primary_button_style(_theme: &iced::Theme, status: button::Status) -> button::Style {
    let background = if matches!(status, button::Status::Hovered) {
        Color::from_rgb(0.3, 0.52, 0.85)
    } else {
        Color::from_rgb(0.25, 0.45, 0.75)
    };
    button::Style {
        background: Some(Background::Color(background)),
        text_color: Color::WHITE,
        ..button::Style::default()
    }
}

/// Style discret des petits boutons d'icônes (✓, ×, +).
fn ghost_button_style(_theme: &iced::Theme, status: button::Status) -> button::Style {
    let background = if matches!(status, button::Status::Hovered) {
        Some(Background::Color(Color::from_rgb(0.88, 0.91, 0.94)))
    } else {
        None
    };
    button::Style {
        background,
        text_color: Color::from_rgb(0.2, 0.3, 0.4),
        ..button::Style::default()
    }
}

/// Style commun des panneaux (fond et cadre).
pub(crate) fn panel_style(_theme: &iced::Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(PANEL_BG)),
        border: Border {
            color: PANEL_BORDER,
            width: 1.0,
            radius: Radius::from(6.0),
        },
        ..container::Style::default()
    }
}

/// Style du fond assombri sous l'overlay de la recherche rapide.
fn overlay_style(_theme: &iced::Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(Color::from_rgba(0.0, 0.0, 0.0, 0.35))),
        ..container::Style::default()
    }
}

/// Style d'un lien non résolu (fond légèrement grisé).
fn unresolved_link_style(_theme: &iced::Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(Color::from_rgb(0.95, 0.95, 0.96))),
        border: Border {
            color: Color::from_rgb(0.87, 0.87, 0.89),
            width: 1.0,
            radius: Radius::from(4.0),
        },
        ..container::Style::default()
    }
}

/// Titre de section.
fn section_title<'a>(title: &'a str) -> Element<'a, Message> {
    text(title).size(14).font(bold_font()).into()
}

/// Police en gras.
fn bold_font() -> Font {
    Font {
        weight: Weight::Bold,
        ..Font::default()
    }
}
