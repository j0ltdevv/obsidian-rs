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
use iced::font::Style as FontStyle;
use iced::font::Weight;
use iced::widget::{
    button, column, container, pane_grid, responsive, row, scrollable, stack, text, text_editor,
    text_input, Column, Row,
};
use iced::{Alignment, Background, Border, Color, Element, Font, Length};

use crate::app::{App, Message, PaneKind, SidebarTab};
use crate::vault::live_preview::{render_markdown, Segment, SegmentStyle};
use crate::vault::outline::extract_outline;
use crate::vault::{Note, Wikilink};

mod canvas_view;
mod dataview_view;
pub mod embed_view;
pub(crate) mod graph;
mod history_view;
pub mod markdown;
pub mod outline_view;
mod plugins_view;
pub mod search_view;
pub mod tasks_view;
pub mod theme;
mod themes_view;
pub mod trash_view;

use self::canvas_view::canvas_board_view;
use self::dataview_view::dataview_view;
use self::embed_view::embed_image_element;
use self::graph::graph_view;
use self::history_view::history_view;
use self::outline_view::outline_view;
use self::plugins_view::plugins_view;
use self::search_view::search_view;
use self::tasks_view::tasks_view;
use self::themes_view::themes_view;
use self::trash_view::trash_panel;

/// Couleur du texte d'erreur.
const ERROR_COLOR: Color = Color::from_rgb(0.85, 0.2, 0.2);
/// Couleur de fond de la bannière d'erreur.
const ERROR_BG: Color = Color::from_rgb(0.98, 0.9, 0.9);
/// Couleur du cadre des panneaux.
const PANEL_BORDER: Color = Color::from_rgb(0.83, 0.86, 0.89);
/// Couleur d'un lien non résolu.
const UNRESOLVED_COLOR: Color = Color::from_rgb(0.55, 0.55, 0.58);

// Géométrie mesurée de l'interface (UI-SPEC §2).
/// Largeur du ruban d'icônes gauche (UI-SPEC §2.2 : 44 px).
pub(crate) const RIBBON_WIDTH_PX: f32 = 44.0;
// Les docks latéraux sont réglés par ratios dans `default_pane_state` (le
// passage à une largeur fixe exigerait de retirer leurs panneaux de la grille,
// ce qui sort du périmètre des fichiers modifiables) : la constante 300 px est
// conservée en référence de cible dans les tests.
pub const SIDEBAR_WIDTH_PX: f32 = 300.0;
/// Hauteur du bandeau d'onglets (UI-SPEC §2.1 : 40 px).
pub(crate) const TAB_BAR_HEIGHT_PX: f32 = 40.0;
/// Hauteur de la barre d'état (UI-SPEC §2.6 : 27 px).
pub(crate) const STATUS_BAR_HEIGHT_PX: f32 = 27.0;
/// Largeur maximale de la zone de lecture (UI-SPEC §2.4.1 : 700 px).
pub(crate) const READING_LINE_WIDTH_PX: f32 = 700.0;
/// Marge intérieure de la zone de lecture (UI-SPEC §2.4.1 : 32 px).
pub(crate) const READING_MARGIN_PX: f32 = 32.0;
/// Survol mesuré : blanc à 6,7 % d'opacité (`--background-modifier-hover`,
/// UI-SPEC §3).
const HOVER_OVERLAY: Color = Color::from_rgba(1.0, 1.0, 1.0, 0.067);

/// Vue racine de l'application.
pub fn view(app: &App) -> Element<'_, Message> {
    let body = match &app.notes {
        Ok(notes) => workspace_view(app, notes),
        Err(error) => error_view(error),
    };

    let mut root = Column::new();
    if let Some(error) = &app.error {
        root = root.push(error_banner(error));
    }
    let main = row![ribbon(app), body]
        .width(Length::Fill)
        .height(Length::Fill);
    root = root.push(main).push(status_bar(app));
    let root_el: Element<'_, Message> = root.height(Length::Fill).into();
    let background = root_background(app);
    let root: Element<'_, Message> = container(root_el)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_theme| container::Style {
            background: Some(Background::Color(background)),
            ..container::Style::default()
        })
        .into();

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
    } else if app.new_note.is_some() {
        stack([root, new_note_overlay(app)])
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    } else if app.show_shortcuts {
        stack([root, shortcuts_overlay()])
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    } else if app.show_settings {
        stack([root, settings_overlay(app)])
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    } else if app.rename_dialog.is_some() {
        stack([root, rename_overlay(app)])
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    } else if app.delete_confirm.is_some() {
        stack([root, delete_confirm_overlay(app)])
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    } else if app.publish_dialog.is_some() {
        stack([root, publish_overlay(app)])
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    } else if app.trash_empty_confirm {
        stack([root, trash_empty_confirm_overlay(app)])
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    } else if app.folder_dialog {
        stack([root, folder_overlay(app)])
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    } else if app.canvas_remove_confirm.is_some() {
        stack([root, canvas_remove_confirm_overlay(app)])
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    } else {
        root
    }
}

/// Couleur de fond de l'application selon le thème choisi, surchargée par
/// le premier snippet définissant `--background-primary`.
fn root_background(app: &crate::app::App) -> Color {
    let mut rgb = (30, 32, 36); // sombre par défaut
    if let Some(theme) = crate::vault::themes::by_name(&app.current_theme) {
        rgb = theme.bg;
    }
    for (_, vars) in &app.css_snippets {
        for var in vars {
            if var.name == "--background-primary" {
                rgb = var.rgb;
                break;
            }
        }
    }
    Color::from_rgb(
        rgb.0 as f32 / 255.0,
        rgb.1 as f32 / 255.0,
        rgb.2 as f32 / 255.0,
    )
}

/// Overlay de création de note : panneau centré sur fond assombri.
fn new_note_overlay<'a>(app: &'a App) -> Element<'a, Message> {
    use iced::widget::{stack, text_input as ti};
    let Some(state) = &app.new_note else {
        return stack([]).into();
    };
    let panel = container(
        column![
            text("Nouvelle note").size(16).font(bold_font()),
            ti("Nom de la note…", &state.name)
                .on_input(Message::NewNoteName)
                .on_submit(Message::NewNoteCreate)
                .padding(8),
        ]
        .spacing(12)
        .padding(20),
    )
    .style(panel_style)
    .width(Length::FillPortion(3))
    .max_width(420);

    container(stack([panel.into()]))
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .style(overlay_style)
        .into()
}

/// Overlay de réglages : toggles vim/spellcheck et liste des raccourcis.
/// Raccourcis clavier affichés par l'aide (F1).
///
/// Source unique de vérité : la subscription de `app` doit câbler chacun de ces
/// raccourcis, et un test vérifie que la liste reste alignée sur le code.
const SHORTCUTS: &[(&str, &str)] = &[
    ("Ctrl + K", "Recherche rapide de note"),
    ("Ctrl + P", "Palette de commandes"),
    ("Ctrl + S", "Enregistrer la note"),
    ("Ctrl + F", "Rechercher dans la note"),
    ("Ctrl + Maj + F", "Recherche globale"),
    ("Ctrl + G", "Graphe"),
    ("Ctrl + Z / Y", "Annuler / rétablir"),
    ("Ctrl + W", "Fermer l'onglet"),
    ("Ctrl + Tab", "Onglet suivant"),
    ("Ctrl + Maj + Tab", "Onglet précédent"),
    ("Ctrl + B", "Sidebar gauche"),
    ("Ctrl + Maj + B", "Sidebar droite"),
    ("Ctrl + Page ↓ / ↑", "Onglet de sidebar suivant / précédent"),
    ("F1", "Cette aide"),
    ("Échap", "Fermer la fenêtre active"),
];

/// Overlay d'aide des raccourcis clavier.
fn shortcuts_overlay<'a>() -> Element<'a, Message> {
    use iced::widget::stack;
    let mut content = column![text("Raccourcis clavier").size(16).font(bold_font())].spacing(6);

    for (keys, action) in SHORTCUTS {
        content = content.push(
            row![
                text(*keys).size(13).width(Length::Fixed(190.0)),
                text(*action).size(13),
            ]
            .spacing(12),
        );
    }

    content = content.push(
        button(text("Fermer (Échap)"))
            .on_press(Message::ToggleShortcuts)
            .style(primary_button_style),
    );

    let panel = container(content)
        .style(panel_style)
        .width(Length::FillPortion(3))
        .max_width(520);

    container(stack([panel.into()]))
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .style(overlay_style)
        .into()
}

fn settings_overlay<'a>(app: &'a App) -> Element<'a, Message> {
    use iced::widget::stack;
    let vim_label = if app.settings.vim_mode {
        "vim ✓"
    } else {
        "vim"
    };
    let spell_label = if app.settings.spellcheck {
        "correction ✓"
    } else {
        "correction"
    };

    let mut content = column![
        text("Réglages").size(16).font(bold_font()),
        row![
            text("Mode vim").width(Length::FillPortion(2)),
            button(text(vim_label))
                .on_press(Message::SetVimMode(!app.settings.vim_mode))
                .style(ghost_button_style),
        ]
        .spacing(8),
        row![
            text("Correction orthographique").width(Length::FillPortion(2)),
            button(text(spell_label))
                .on_press(Message::SetSpellcheck(!app.settings.spellcheck))
                .style(ghost_button_style),
        ]
        .spacing(8),
        text(format!("Thème : {}", app.current_theme)).size(13),
        text("Raccourcis (hotkeys.json)").size(13),
        text("Vault").size(13),
        row![
            button(text("Exporter .zip"))
                .on_press(Message::VaultExportZip(
                    app.vault_root
                        .join("vault-export.zip")
                        .display()
                        .to_string(),
                ))
                .style(ghost_button_style),
            button(text("Importer .zip"))
                .on_press(Message::VaultImportZip(
                    app.vault_root
                        .join("vault-import.zip")
                        .display()
                        .to_string(),
                ))
                .style(ghost_button_style),
        ]
        .spacing(8),
    ]
    .spacing(12)
    .padding(20);

    for hotkey in &app.hotkeys {
        content = content
            .push(text(format!("{} → {}", hotkey.command, hotkey.keys.join(" + "))).size(12));
    }
    content = content.push(
        button(text("Fermer"))
            .on_press(Message::ToggleSettings)
            .style(primary_button_style),
    );

    let panel = container(content)
        .style(panel_style)
        .width(Length::FillPortion(3))
        .max_width(460);

    container(stack([panel.into()]))
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .style(overlay_style)
        .into()
}
fn canvas_remove_confirm_overlay<'a>(app: &'a App) -> Element<'a, Message> {
    use iced::widget::{button as btn, stack};
    if app.canvas_remove_confirm.is_none() {
        return stack([]).into();
    }
    let panel = container(
        column![
            text("Supprimer cette carte du canvas ?")
                .size(15)
                .font(bold_font()),
            row![
                btn(text("Supprimer"))
                    .on_press(Message::CanvasConfirmRemoveCard)
                    .style(primary_button_style),
                btn(text("Annuler"))
                    .on_press(Message::CanvasCancelRemoveCard)
                    .style(ghost_button_style),
            ]
            .spacing(8),
        ]
        .spacing(12)
        .padding(20),
    )
    .style(panel_style)
    .width(Length::FillPortion(3))
    .max_width(440);

    container(stack([panel.into()]))
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .style(overlay_style)
        .into()
}

/// Overlay de renommage d'une note.
fn rename_overlay<'a>(app: &'a App) -> Element<'a, Message> {
    use iced::widget::{button as btn, stack, text_input as ti};
    let Some((current, draft)) = &app.rename_dialog else {
        return stack([]).into();
    };
    let panel = container(
        column![
            text(format!("Renommer {}", current.display()))
                .size(15)
                .font(bold_font()),
            ti("Nouveau nom (sans .md)", draft)
                .on_input(Message::RenameDraft)
                .padding(8),
            row![
                btn(text("Valider"))
                    .on_press(Message::RenameCommit)
                    .style(primary_button_style),
                btn(text("Annuler"))
                    .on_press(Message::ToggleRename(current.clone()))
                    .style(ghost_button_style),
            ]
            .spacing(8),
        ]
        .spacing(12)
        .padding(20),
    )
    .style(panel_style)
    .width(Length::FillPortion(3))
    .max_width(440);

    container(stack([panel.into()]))
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .style(overlay_style)
        .into()
}

/// Overlay de confirmation de suppression.
fn delete_confirm_overlay<'a>(app: &'a App) -> Element<'a, Message> {
    use iced::widget::{button as btn, stack};
    let Some(path) = &app.delete_confirm else {
        return stack([]).into();
    };
    let panel = container(
        column![
            text("Déplacer vers la corbeille (.trash) ?")
                .size(15)
                .font(bold_font()),
            text(path.display().to_string()).size(13),
            row![
                btn(text("Corbeille"))
                    .on_press(Message::DeleteConfirm)
                    .style(primary_button_style),
                btn(text("Annuler"))
                    .on_press(Message::ToggleDelete(path.clone()))
                    .style(ghost_button_style),
            ]
            .spacing(8),
        ]
        .spacing(12)
        .padding(20),
    )
    .style(panel_style)
    .width(Length::FillPortion(3))
    .max_width(440);

    container(stack([panel.into()]))
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .style(overlay_style)
        .into()
}

/// Overlay d'export statique : saisie du répertoire de destination.
fn publish_overlay<'a>(app: &'a App) -> Element<'a, Message> {
    use iced::widget::{button as btn, stack, text_input as ti};
    let Some(dialog) = &app.publish_dialog else {
        return stack([]).into();
    };
    let panel = container(
        column![
            text("Exporter le vault en site HTML")
                .size(15)
                .font(bold_font()),
            text("Répertoire de destination des pages générées.").size(12),
            ti("Chemin du répertoire de destination", &dialog.dest)
                .on_input(Message::PublishDest)
                .padding(8),
            row![
                btn(text("Publier"))
                    .on_press(Message::PublishRun)
                    .style(primary_button_style),
                btn(text("Annuler"))
                    .on_press(Message::PublishClose)
                    .style(ghost_button_style),
            ]
            .spacing(8),
        ]
        .spacing(12)
        .padding(20),
    )
    .style(panel_style)
    .width(Length::FillPortion(3))
    .max_width(480);

    container(stack([panel.into()]))
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .style(overlay_style)
        .into()
}

/// Overlay de confirmation du vidage de la corbeille.
fn trash_empty_confirm_overlay<'a>(app: &'a App) -> Element<'a, Message> {
    use iced::widget::{button as btn, stack};
    if !app.trash_empty_confirm {
        return stack([]).into();
    }
    let panel = container(
        column![
            text("Vider la corbeille ?").size(15).font(bold_font()),
            text(
                "Cette action supprime définitivement tous les éléments supprimés. \
                 Elle est irréversible.",
            )
            .size(13),
            row![
                btn(text("Vider"))
                    .on_press(Message::TrashEmptyConfirm)
                    .style(primary_button_style),
                btn(text("Annuler"))
                    .on_press(Message::TrashEmptyCancel)
                    .style(ghost_button_style),
            ]
            .spacing(8),
        ]
        .spacing(12)
        .padding(20),
    )
    .style(panel_style)
    .width(Length::FillPortion(3))
    .max_width(440);

    container(stack([panel.into()]))
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .style(overlay_style)
        .into()
}

/// Overlay de création de dossier.
fn folder_overlay<'a>(app: &'a App) -> Element<'a, Message> {
    use iced::widget::{button as btn, stack, text_input as ti};
    if !app.folder_dialog {
        return stack([]).into();
    }
    let panel = container(
        column![
            text("Nouveau dossier").size(15).font(bold_font()),
            ti("Nom du dossier", &app.folder_name)
                .on_input(Message::FolderName)
                .padding(8),
            row![
                btn(text("Créer"))
                    .on_press(Message::FolderCreate(app.folder_name.clone()))
                    .style(primary_button_style),
                btn(text("Annuler"))
                    .on_press(Message::FolderDialogOpen)
                    .style(ghost_button_style),
            ]
            .spacing(8),
        ]
        .spacing(12)
        .padding(20),
    )
    .style(panel_style)
    .width(Length::FillPortion(3))
    .max_width(440);

    container(stack([panel.into()]))
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .style(overlay_style)
        .into()
}

// État vide d'un panneau : message discret, centré, dans la couleur de texte
// atténuée. Remplace les bandeaux « Aucune note ouverte » répétés dans les
// trois panneaux, qui donnaient l'impression d'un doublon.
fn empty_hint<'a>(message: &'a str) -> Element<'a, Message> {
    container(
        text(message)
            .size(12)
            .color(Color::from_rgb8(140, 140, 140)),
    )
    .padding(12)
    .center_x(Length::Fill)
    .center_y(Length::Fill)
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

// Même chose pour un panneau qui garde son fond (panneau stylé).
fn empty_hint_panel<'a>(message: &'a str) -> Element<'a, Message> {
    container(
        text(message)
            .size(12)
            .color(Color::from_rgb8(140, 140, 140)),
    )
    .padding(12)
    .center_x(Length::Fill)
    .center_y(Length::Fill)
    .width(Length::Fill)
    .height(Length::Fill)
    .style(panel_style)
    .into()
}

// Icône SVG Lucide teintée à la couleur du thème.
//
// Les SVG de `assets/icons/` sont au format Lucide : `stroke="currentColor"`.
// En iced 0.14, le widget `svg` n'a pas d'API de couleur directe, mais son
// `Style` expose un filtre `color` qui teinte les icônes symboliques : on
// l'utilise au lieu de réécrire les SVG.
fn themed_svg<'a>(app: &'a App, bytes: &'static [u8]) -> iced::widget::svg::Svg<'a> {
    use iced::widget::svg;
    let fg = crate::vault::themes::by_name(&app.current_theme)
        .map(|t| t.fg)
        .unwrap_or((220, 220, 220));
    let color = Color::from_rgb(
        fg.0 as f32 / 255.0,
        fg.1 as f32 / 255.0,
        fg.2 as f32 / 255.0,
    );
    svg(svg::Handle::from_memory(bytes))
        .width(Length::Fixed(18.0))
        .height(Length::Fixed(18.0))
        .style(move |_, _| svg::Style { color: Some(color) })
}

// Barre d'icônes verticale gauche (façon Obsidian) : actions principales
// compactes, groupe haut et bas séparés.
fn ribbon<'a>(app: &'a App) -> Element<'a, Message> {
    use iced::widget::button as btn;
    let icon_text = |sym: &'static str, msg| {
        btn(text(sym).size(16))
            .width(Length::Fill)
            .on_press(msg)
            .style(ghost_button_style)
    };
    let icon_svg = |bytes: &'static [u8], msg| {
        btn(themed_svg(app, bytes))
            .width(Length::Fill)
            .on_press(msg)
            .style(ghost_button_style)
    };
    let haut = column![
        icon_svg(
            include_bytes!("../../assets/icons/rotate-cw.svg"),
            Message::Refresh
        ),
        icon_svg(
            include_bytes!("../../assets/icons/share-2.svg"),
            Message::ToggleGraph
        ),
        icon_svg(
            include_bytes!("../../assets/icons/command.svg"),
            Message::OpenCommandPalette
        ),
        icon_svg(
            include_bytes!("../../assets/icons/search.svg"),
            Message::OpenSwitcher
        ),
        icon_svg(
            include_bytes!("../../assets/icons/file-plus.svg"),
            Message::NewNoteOpen
        ),
    ]
    .spacing(4)
    .padding(4);
    let bas = column![
        icon_text(
            if app.show_left_sidebar { "◀" } else { "▶" },
            Message::ToggleLeftSidebar,
        ),
        icon_text(
            if app.show_right_sidebar { "◂" } else { "▸" },
            Message::ToggleRightSidebar,
        ),
        icon_svg(
            include_bytes!("../../assets/icons/settings.svg"),
            Message::ToggleSettings
        ),
    ];
    container(column![haut, text("").height(Length::Fill), bas])
        .width(RIBBON_WIDTH_PX)
        .height(Length::Fill)
        .style(panel_style)
        .into()
}

// Barre d'état basse : compteur de mots de la note ouverte + thème courant.
fn status_bar<'a>(app: &'a App) -> Element<'a, Message> {
    let info = match &app.open {
        Some(open) => {
            let words = open.content.text().split_whitespace().count();
            format!("{} mots · {} lignes", words, open.content.line_count())
        }
        None => "Aucune note ouverte".to_string(),
    };
    let status_label = app
        .publish_status
        .as_deref()
        .map(|status| format!(" · {status}"))
        .unwrap_or_default();
    row![
        text(info).size(12),
        text(status_label).size(12),
        text("").width(Length::Fill),
        text(format!("thème : {}", app.current_theme)).size(12),
    ]
    .align_y(Alignment::Center)
    .spacing(8)
    .height(Length::Fixed(STATUS_BAR_HEIGHT_PX))
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

    let left_el: Element<'a, Message> = if app.show_left_sidebar {
        container(sidebar(app, notes))
            .style(panel_style)
            .width(Length::Fixed(SIDEBAR_WIDTH_PX))
            .height(Length::Fill)
            .into()
    } else {
        container(
            iced::widget::Space::new()
                .width(Length::Fixed(0.0))
                .height(Length::Fill),
        )
        .width(Length::Fixed(0.0))
        .height(Length::Fill)
        .into()
    };

    if app.show_canvas {
        let report: Vec<(u64, std::path::PathBuf)> = notes
            .iter()
            .enumerate()
            .map(|(i, n)| (i as u64, n.path.clone()))
            .collect();
        let canvas = canvas_board_view(&app.canvas, &report);

        let toolbar = row![
            button(text("Auto-layout"))
                .on_press(Message::CanvasAutoLayout)
                .style(ghost_button_style),
            button(text("Importer JSON"))
                .on_press(Message::CanvasImportJson)
                .style(ghost_button_style),
            button(text("Exporter JSON"))
                .on_press(Message::CanvasExportJson)
                .style(ghost_button_style),
        ]
        .spacing(8)
        .padding([0, 8]);

        let mut stack = Column::new()
            .spacing(8)
            .width(Length::Fill)
            .height(Length::Fill);
        stack = stack.push(
            container(toolbar)
                .width(Length::Fill)
                .height(Length::Shrink),
        );
        stack = stack.push(
            container(canvas)
                .width(Length::Fill)
                .height(Length::FillPortion(5)),
        );
        if let Some(id) = app.canvas_selected {
            if let Some(card) = app.canvas.card(id) {
                let group_value = card.group().unwrap_or("").to_string();
                let panel = container(
                    column![
                        text("Carte sélectionnée").size(13).font(bold_font()),
                        text_input("Titre", &card.title)
                            .on_input(move |text| Message::CanvasEditTitle { id, text })
                            .padding(6),
                        text_input("Contenu", &card.text)
                            .on_input(move |text| Message::CanvasEditText { id, text })
                            .padding(6),
                        text_input("Groupe (vide = aucun)", &group_value)
                            .on_input(move |group| Message::CanvasSetGroup { id, group })
                            .padding(6),
                        row![button(text("Supprimer la carte"))
                            .on_press(Message::CanvasAskRemoveCard(id))
                            .style(ghost_button_style),]
                        .spacing(6),
                    ]
                    .spacing(6)
                    .padding(8),
                )
                .style(panel_style)
                .width(Length::Fill)
                .height(Length::FillPortion(2));
                stack = stack.push(panel);
            }
        }

        return row![left_el, stack]
            .width(Length::Fill)
            .height(Length::Fill)
            .spacing(8)
            .padding([8, 12])
            .into();
    }

    if app.show_themes {
        let on_select = |name: String| Message::SelectTheme(name);
        let themes = themes_view(&app.current_theme, &on_select, &app.css_snippets);
        return row![
            left_el,
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
            left_el,
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
        let plugins = plugins_view(&app.plugins.info(), |id| {
            Message::PluginToggle(id.to_string())
        });
        return row![
            left_el,
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
        let dview = dataview_view(
            notes,
            &app.dataview_indices,
            &app.dataview_query,
            app.dataview_error.as_deref(),
            Message::DataviewQueryChanged,
            Message::DataviewRun,
            |i| {
                let path = notes.get(i).map(|n| n.path.clone()).unwrap_or_default();
                Message::OpenNote(path)
            },
        );
        return row![
            left_el,
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

    if app.show_search {
        let panel = container(search_view(app))
            .width(Length::Fill)
            .height(Length::Fill);
        return row![left_el, panel]
            .width(Length::Fill)
            .height(Length::Fill)
            .spacing(8)
            .padding([8, 12])
            .into();
    }

    if app.show_tasks {
        let panel = container(tasks_view(app))
            .width(Length::Fill)
            .height(Length::Fill);
        return row![left_el, panel]
            .width(Length::Fill)
            .height(Length::Fill)
            .spacing(8)
            .padding([8, 12])
            .into();
    }

    if app.show_outline {
        let panel = container(outline_view(app))
            .width(Length::Fill)
            .height(Length::Fill);
        return row![left_el, panel]
            .width(Length::Fill)
            .height(Length::Fill)
            .spacing(8)
            .padding([8, 12])
            .into();
    }

    let panes = |_pane, &kind, _active| {
        let el: Element<'a, Message> = match kind {
            PaneKind::Editor => editor_panel(app),
            PaneKind::Preview => preview_panel(app, notes),
            // Les docks latéraux sont rendus hors grille (colonnes fixes) : la
            // grille ne contient que l'éditeur et l'aperçu.
            PaneKind::Sidebar | PaneKind::Right => container(iced::widget::Space::new()).into(),
        };
        pane_grid::Content::new(el)
    };
    let grid = pane_grid::PaneGrid::new(&app.pane_state, panes)
        .on_resize(10, Message::PaneResized)
        .spacing(8)
        .width(Length::Fill)
        .height(Length::Fill);

    let left_dock: Element<'a, Message> = if app.show_left_sidebar {
        container(sidebar(app, notes))
            .style(panel_style)
            .width(Length::Fixed(SIDEBAR_WIDTH_PX))
            .height(Length::Fill)
            .into()
    } else {
        container(iced::widget::Space::new())
            .width(Length::Fixed(0.0))
            .height(Length::Fill)
            .into()
    };
    let right_dock: Element<'a, Message> = if app.show_right_sidebar {
        container(right_sidebar(app, notes))
            .style(panel_style)
            .width(Length::Fixed(SIDEBAR_WIDTH_PX))
            .height(Length::Fill)
            .into()
    } else {
        container(iced::widget::Space::new())
            .width(Length::Fixed(0.0))
            .height(Length::Fill)
            .into()
    };

    row![left_dock, container(grid).padding([8, 12]), right_dock]
        .width(Length::Fill)
        .height(Length::Fill)
        .spacing(8)
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

// Barre latérale gauche façon Obsidian : onglets Fichiers / Recherche / Tags
// / Backlinks / Calendrier, avec le contenu de l'onglet actif en dessous.
fn sidebar<'a>(app: &'a App, notes: &'a [Note]) -> Element<'a, Message> {
    let tab_btn = |label: &'static str, active: bool, msg| {
        button(text(label).size(12)).on_press(msg).style(if active {
            primary_button_style
        } else {
            ghost_button_style
        })
    };
    // Les sept onglets ne tiennent pas dans la largeur de la sidebar : la
    // rangée déborde et le dernier libellé chevauche le panneau voisin. On la
    // rend donc défilable horizontalement (une seule ligne, aucun retour).
    let tabs = scrollable(
        row![
            tab_btn(
                "Fichiers",
                app.sidebar_tab == SidebarTab::Explorer,
                Message::SetSidebarTab(SidebarTab::Explorer),
            ),
            tab_btn(
                "Recherche",
                app.sidebar_tab == SidebarTab::Recherche,
                Message::SetSidebarTab(SidebarTab::Recherche),
            ),
            tab_btn(
                "Tags",
                app.sidebar_tab == SidebarTab::Tags,
                Message::SetSidebarTab(SidebarTab::Tags),
            ),
            tab_btn(
                "Backlinks",
                app.sidebar_tab == SidebarTab::Backlinks,
                Message::SetSidebarTab(SidebarTab::Backlinks),
            ),
            tab_btn(
                "Calendrier",
                app.sidebar_tab == SidebarTab::Calendrier,
                Message::SetSidebarTab(SidebarTab::Calendrier),
            ),
            tab_btn(
                "Pièces jointes",
                app.sidebar_tab == SidebarTab::Attachments,
                Message::SetSidebarTab(SidebarTab::Attachments),
            ),
            tab_btn(
                "Corbeille",
                app.sidebar_tab == SidebarTab::Corbeille,
                Message::SetSidebarTab(SidebarTab::Corbeille),
            ),
        ]
        .spacing(4)
        .padding([4, 8]),
    )
    .direction(iced::widget::scrollable::Direction::Horizontal(
        iced::widget::scrollable::Scrollbar::default(),
    ))
    .width(Length::Fill)
    .height(Length::Shrink);

    let content: Element<'a, Message> = match app.sidebar_tab {
        SidebarTab::Explorer => explorer_panel(app, notes),
        SidebarTab::Recherche => search_sidebar_panel(app, notes),
        SidebarTab::Tags => tags_panel(app, notes),
        SidebarTab::Backlinks => backlinks_panel(app, notes),
        SidebarTab::Calendrier => calendar_widget(app),
        SidebarTab::Attachments => attachments_panel(app),
        SidebarTab::Corbeille => trash_panel(app),
    };
    column![tabs, content]
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

// Panneau Fichiers : barre d'outils (tri, dossier) + arborescence.
fn explorer_panel<'a>(app: &'a App, notes: &'a [Note]) -> Element<'a, Message> {
    let toolbar = row![
        button(if app.sort_alpha {
            "Tri A→Z ✓"
        } else {
            "Tri A→Z"
        })
        .on_press(Message::ToggleSort)
        .style(ghost_button_style),
        button(text("+ Dossier"))
            .on_press(Message::FolderDialogOpen)
            .style(ghost_button_style),
    ]
    .spacing(6)
    .padding([2, 8]);
    let mut children: Vec<Element<'a, Message>> = vec![tree_node(app, notes, Path::new(""), 0)];
    if !app.selected_note_paths.is_empty() {
        let n = app.selected_note_paths.len();
        let bar = row![
            text(format!("{n} sélectionnée(s)")).size(12),
            text("").width(Length::Fill),
            button(text("Effacer"))
                .on_press(Message::ClearSelection)
                .style(ghost_button_style),
            button(text("Supprimer"))
                .on_press(Message::DeleteSelected)
                .style(primary_button_style),
        ]
        .spacing(6)
        .padding([2, 8]);
        children.push(bar.into());
    }
    column![toolbar, scrollable(column(children).spacing(2))]
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

// Panneau Pièces jointes : liste des fichiers non-note avec taille, bouton
// de renommage (saisie inline) et suppression vers la corbeille.
fn attachments_panel<'a>(app: &'a App) -> Element<'a, Message> {
    if app.attachments.is_empty() {
        return container(text("Aucune pièce jointe").size(12))
            .padding(8)
            .into();
    }
    let mut list = Column::new().spacing(4).width(Length::Fill);
    for att in &app.attachments {
        let size_kb = att.size as f64 / 1024.0;
        let mut row_children: Vec<Element<'a, Message>> = vec![
            text(&att.name).size(13).width(Length::Fill).into(),
            text(format!("{size_kb:.1} Ko"))
                .size(11)
                .color(UNRESOLVED_COLOR)
                .into(),
        ];
        if let Some((path, draft)) = &app.attachment_rename {
            if *path == att.path {
                row_children.push(
                    text_input("Nouveau nom", draft)
                        .on_input(Message::AttachmentRenameChange)
                        .on_submit(Message::AttachmentRenameCommit)
                        .padding(4)
                        .width(Length::Fixed(140.0))
                        .into(),
                );
                row_children.push(
                    button(text("OK"))
                        .on_press(Message::AttachmentRenameCommit)
                        .style(primary_button_style)
                        .into(),
                );
            }
        } else {
            row_children.push(
                button(text("Renommer"))
                    .on_press(Message::AttachmentRenameStart(att.path.clone()))
                    .style(ghost_button_style)
                    .into(),
            );
        }
        row_children.push(
            button(text("Supprimer"))
                .on_press(Message::AttachmentDelete(att.path.clone()))
                .style(ghost_button_style)
                .into(),
        );
        let line = container(row(row_children).spacing(6))
            .width(Length::Fill)
            .padding([2, 6]);
        list = list.push(line);
    }
    scrollable(list).into()
}

fn tags_panel<'a>(_app: &'a App, notes: &'a [Note]) -> Element<'a, Message> {
    use std::collections::BTreeMap;
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for note in notes {
        for tag in &note.metadata.tags {
            *counts.entry(tag.clone()).or_default() += 1;
        }
    }
    if counts.is_empty() {
        return container(text("Aucun tag").size(12)).padding(8).into();
    }
    let mut list = Column::new().spacing(2).width(Length::Fill);
    for (tag, n) in counts {
        let row = button(
            row![
                text(format!("#{tag}")).size(13),
                text("").width(Length::Fill),
                text(n.to_string()).size(12).color(UNRESOLVED_COLOR),
            ]
            .spacing(6),
        )
        .width(Length::Fill)
        .on_press(Message::GraphFilterTag(tag.clone()))
        .style(ghost_button_style);
        list = list.push(row);
    }
    scrollable(list).into()
}

// Panneau Recherche : zone de saisie + résultats (extraits) ouvrant les notes.
fn search_sidebar_panel<'a>(app: &'a App, notes: &'a [Note]) -> Element<'a, Message> {
    let query = app
        .search_panel
        .as_ref()
        .map(|state| state.query.clone())
        .unwrap_or_default();
    let input = text_input("Rechercher dans le vault…", &query)
        .on_input(Message::SearchQuery)
        .padding(6);
    let mut results = Column::new().spacing(2).width(Length::Fill);
    if let Some(state) = &app.search_panel {
        for hit in &state.results {
            if let Some(note) = notes.get(hit.index) {
                let line = button(
                    column![
                        text(&note.name).size(13).width(Length::Fill),
                        text(&hit.snippet).size(11).color(UNRESOLVED_COLOR),
                    ]
                    .spacing(2),
                )
                .width(Length::Fill)
                .on_press(Message::OpenNote(note.path.clone()))
                .style(ghost_button_style);
                results = results.push(line);
            }
        }
    }
    column![input, scrollable(results)]
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

/// Panneau « Backlinks » : toutes les notes qui pointent vers la note
/// ouverte. Chaque ligne ouvre la note référente.
fn backlinks_panel<'a>(app: &'a App, notes: &'a [Note]) -> Element<'a, Message> {
    let open_path = match &app.open {
        Some(open) => &open.path,
        None => {
            return empty_hint("Ouvrez une note pour voir ses backlinks");
        }
    };
    let Some(index) = &app.index else {
        return container(text("Index absent").size(12)).padding(8).into();
    };
    let Some(open_note) = notes.iter().find(|n| &n.path == open_path) else {
        return container(text("Note introuvable").size(12))
            .padding(8)
            .into();
    };
    let backlinks = index.backlinks(open_note);
    let mut list = Column::new().spacing(2).padding([4, 8]);
    list = list.push(
        row![
            text("Backlinks").size(13).font(bold_font()),
            text(format!("({})", backlinks.len())).size(12),
            text("").width(Length::Fill),
            button(text("Fermer"))
                .on_press(Message::ToggleBacklinks)
                .style(ghost_button_style),
        ]
        .spacing(6),
    );
    if backlinks.is_empty() {
        list = list.push(text("Aucun lien entrant").size(12));
    }
    for note in backlinks {
        list = list.push(
            button(text(note.name.clone()).size(13))
                .on_press(Message::OpenNote(note.path.clone()))
                .style(ghost_button_style)
                .width(Length::Fill),
        );
    }
    container(scrollable(list)).style(panel_style).into()
}

// Sidebar droite façon Obsidian : panneau des backlinks + plan de la note
// ouverte (les titres sont cliquables → curseur déplacé).
fn right_sidebar<'a>(app: &'a App, notes: &'a [Note]) -> Element<'a, Message> {
    let mut col = Column::new().spacing(8);
    col = col.push(text("Backlinks").size(13).font(bold_font()));
    col = col.push(backlinks_panel(app, notes));
    if let Some(open) = &app.open {
        let outline = extract_outline(&open.content.text());
        if !outline.is_empty() {
            let mut list = Column::new().spacing(2).width(Length::Fill);
            for entry in &outline {
                let indent = (entry.level.saturating_sub(1) as f32) * 12.0;
                let row = button(text(entry.title.clone()).size(13))
                    .width(Length::Fill)
                    .on_press(Message::OutlineScroll(entry.line))
                    .style(ghost_button_style)
                    .padding([2, indent as u16]);
                list = list.push(row);
            }
            col = col.push(text("Plan").size(13).font(bold_font()));
            col = col.push(scrollable(list));
        }
    }
    container(col).height(Length::Fill).into()
}

/// Mini-calendrier mensuel : cliquez sur un jour pour ouvrir/créer sa note
/// journalière (`AAAA-MM-JJ.md`).
fn calendar_widget<'a>(_app: &'a App) -> Element<'a, Message> {
    use iced::widget::stack;
    let now = std::time::SystemTime::now();
    let (year, month, today) = civil_today(now);
    let _ = today;

    // 1er du mois et nombre de jours.
    let first_weekday = weekday_of(year, month, 1);
    let days_in_month = days_in_month(year, month);
    let mut grid = Column::new().spacing(2).padding([4, 8]);
    grid = grid.push(
        row![
            text(format!("{year:04}-{month:02}"))
                .size(13)
                .font(bold_font()),
            text("").width(Length::Fill),
            button(text("Fermer"))
                .on_press(Message::ToggleCalendar)
                .style(ghost_button_style),
        ]
        .spacing(8),
    );
    let weekdays = ["L", "M", "M", "J", "V", "S", "D"];
    grid = grid.push(weekdays.iter().fold(Row::new().spacing(2), |row, d| {
        row.push(text(*d).size(11).width(Length::FillPortion(1)))
    }));
    let mut week = Row::new().spacing(2);
    let mut week_len = first_weekday;
    for _ in 0..first_weekday {
        week = week.push(text("").width(Length::FillPortion(1)));
    }
    for day in 1..=days_in_month {
        let date = format!("{year:04}-{month:02}-{day:02}");
        let mut cell = button(text(day.to_string()).size(12))
            .on_press(Message::OpenDailyDate(date))
            .style(ghost_button_style)
            .width(Length::FillPortion(1));
        if day == today {
            cell = cell.style(primary_button_style);
        }
        week = week.push(cell);
        week_len += 1;
        if week_len.is_multiple_of(7) {
            grid = grid.push(week);
            week = Row::new().spacing(2);
            week_len = 0;
        }
    }
    if week_len > 0 {
        grid = grid.push(week);
    }
    container(stack([grid.into()]))
        .style(panel_style)
        .width(Length::Fill)
        .into()
}

/// Calendrier civil : (année, mois 1..=12, jour) depuis l'epoch.
fn civil_today(now: std::time::SystemTime) -> (i64, u32, u32) {
    let secs = now
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let days = (secs / 86_400) as i64;
    let z = days + 719_468;
    let era = (if z >= 0 { z } else { z - 146_096 }) / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let mut y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    if m <= 2 {
        y += 1;
    }
    (y, m, d)
}

fn is_leap(year: i64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

fn days_in_month(year: i64, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if is_leap(year) {
                29
            } else {
                28
            }
        }
        _ => 30,
    }
}

/// Jour de la semaine (0 = lundi … 6 = dimanche) pour une date.
fn weekday_of(year: i64, month: u32, day: u32) -> usize {
    let (y, m) = if month <= 2 {
        (year - 1, month + 12)
    } else {
        (year, month)
    };
    // Algorithme de Zeller (résultat 0 = dimanche … 6 = samedi).
    let q = day as i64;
    let k = y % 100;
    let j = y / 100;
    let h = (q + (13 * (m as i64 + 1)) / 5 + k + k / 4 + j / 4 + 5 * j) % 7;
    ((h + 5) % 7) as usize // Zeller 0=samedi → lundi=0
}

/// Rendu récursif d'un niveau d'arborescence (dossiers puis notes).
fn tree_node<'a>(
    app: &'a App,
    notes: &'a [Note],
    dir: &Path,
    depth: usize,
) -> Element<'a, Message> {
    let (subdirs, files) = children_of(notes, dir, app.sort_alpha);
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
        let selected = app.selected_note_paths.contains(&note.path);
        let check = container(
            button(text(if selected { "☑" } else { "☐" }))
                .on_press(Message::ToggleSelectNote(note.path.clone()))
                .style(ghost_button_style),
        )
        .width(22);
        let open_button = button(
            container(text(&note.name))
                .width(Length::Fill)
                .padding([4, 4]),
        )
        .width(Length::Fill)
        .on_press(Message::OpenNote(note.path.clone()))
        .style(move |theme, status| tree_button_style(theme, status, active));
        let row = row![check, open_button]
            .spacing(2)
            .padding([0_u16, (4 + depth * 12) as u16])
            .align_y(Alignment::Center);
        tree = tree.push(row);
    }

    tree.into()
}

/// Enfants directs d'un dossier : sous-dossiers (triés) et notes (triées).
fn children_of<'a>(
    notes: &'a [Note],
    dir: &Path,
    alpha: bool,
) -> (Vec<std::path::PathBuf>, Vec<&'a Note>) {
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

    if alpha {
        files.sort_by(|a, b| a.name.cmp(&b.name));
    }
    (subdirs.into_iter().collect(), files)
}

// Bandeau d'onglets des notes ouvertes (façon Obsidian) : un onglet par
// note, clic pour activer, « × » pour fermer.
fn tab_bar<'a>(app: &'a App) -> Element<'a, Message> {
    let mut bar = Row::new().spacing(4);
    for (i, path) in app.open_tabs.iter().enumerate() {
        let name = path
            .file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
            .unwrap_or_default();
        let active = i == app.open_tabs_active;
        let label = button(text(name))
            .on_press(Message::OpenNote(path.clone()))
            .style(if active {
                tab_active_button_style
            } else {
                ghost_button_style
            });
        let close = button(text("×"))
            .on_press(Message::CloseTab(path.clone()))
            .style(ghost_button_style);
        bar = bar.push(row![label, close].spacing(2));
    }
    container(bar)
        .width(Length::Fill)
        .height(Length::Fixed(TAB_BAR_HEIGHT_PX))
        .padding([2, 8])
        .style(panel_style)
        .into()
}

/// Panneau éditeur : en-tête (nom + bouton de sauvegarde) et zone d'édition.
fn editor_panel<'a>(app: &'a App) -> Element<'a, Message> {
    let Some(open) = &app.open else {
        return empty_hint_panel("Aucune note ouverte");
    };

    let title = if app.open_dirty {
        format!("{} ●", open.name)
    } else {
        open.name.clone()
    };
    let header = row![
        text(title).size(16).font(bold_font()),
        text("").width(Length::Fill),
        button(if app.editing { "Lecture" } else { "Éditer" })
            .on_press(Message::ToggleEdit)
            .style(ghost_button_style),
        button(if app.show_backlinks {
            "Backlinks ✓"
        } else {
            "Backlinks"
        })
        .on_press(Message::ToggleBacklinks)
        .style(ghost_button_style),
        button(text("Renommer"))
            .on_press(Message::ToggleRename(open.path.clone()))
            .style(ghost_button_style),
        button(text("Supprimer"))
            .on_press(Message::ToggleDelete(open.path.clone()))
            .style(ghost_button_style),
        button(text("↶"))
            .on_press(Message::EditorUndo)
            .style(ghost_button_style),
        button(text("↷"))
            .on_press(Message::EditorRedo)
            .style(ghost_button_style),
        button(text("Sauvegarder"))
            .on_press(Message::SaveNote)
            .style(primary_button_style),
    ]
    .spacing(6)
    .padding([0, 8]);

    let editor: Element<'a, Message> = if app.editing {
        // Mode édition : édition du texte brut.
        text_editor(&open.content)
            .placeholder("Écrivez…")
            .on_action(Message::EditorAction)
            .padding([8, 10])
            .height(Length::Fill)
            .into()
    } else {
        // Mode lecture : source Markdown colorée, défilable.
        scrollable(
            container(
                container(markdown::render_source(&open.content.text()))
                    .max_width(READING_LINE_WIDTH_PX)
                    .center_x(Length::Fill),
            )
            .padding([8, READING_MARGIN_PX as u16]),
        )
        .height(Length::Fill)
        .into()
    };

    let mut children: Vec<Element<'a, Message>> = vec![tab_bar(app), header.into()];
    if let Some(find) = &app.find {
        let counter = if find.occurrences > 0 {
            format!("{}/{}", find.current + 1, find.occurrences)
        } else {
            "0/0".to_string()
        };
        let bar = row![
            text_input("Rechercher…", &find.query)
                .on_input(Message::FindQuery)
                .width(Length::FillPortion(3)),
            text_input("Remplacer par…", &find.replace)
                .on_input(Message::FindReplaceText)
                .width(Length::FillPortion(3)),
            button(text("◀"))
                .on_press(Message::FindPrev)
                .style(ghost_button_style),
            text(counter).size(12),
            button(text("▶"))
                .on_press(Message::FindNext)
                .style(ghost_button_style),
            button(text("Tout remplacer"))
                .on_press(Message::FindReplaceAll)
                .style(ghost_button_style),
            button(text("Vault entier"))
                .on_press(Message::GlobalReplaceAll)
                .style(ghost_button_style),
            button(text("Fermer"))
                .on_press(Message::FindClose)
                .style(ghost_button_style),
        ]
        .spacing(6)
        .align_y(Alignment::Center)
        .padding([0, 8]);
        children.push(bar.into());
    }
    children.push(editor);

    column(children)
        .spacing(8)
        // Contenu du pane éditeur : occupe exactement sa cellule (pas de FillPortion,
        // sinon le panneau déborde sur son voisin).
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

/// Panneau prévisualisation : texte brut, métadonnées, wikilinks et backlinks.
fn preview_panel<'a>(app: &'a App, notes: &'a [Note]) -> Element<'a, Message> {
    let Some(open) = &app.open else {
        return empty_hint_panel("Aucune note ouverte");
    };

    let open_note = notes.iter().find(|note| note.path == open.path);

    let mut preview = Column::new().spacing(8);
    preview = preview.push(section_title("Prévisualisation"));

    // Rendu Markdown stylisé (live preview).
    let rendered = scrollable(
        container(live_preview_column(
            app,
            &app.vault_root,
            &open.content.text(),
        ))
        .padding([8, READING_MARGIN_PX as u16]),
    )
    .height(Length::FillPortion(3));
    preview = preview.push(rendered);

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
        // Panneau prévisualisation : la largeur doit remplir exactement sa cellule de pane.
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}
/// Convertit un segment du moteur de rendu Markdown en widget iced.
fn segment_widget(root: &Path, seg: &Segment) -> Element<'static, Message> {
    match seg.style {
        SegmentStyle::Plain => text(seg.text.clone()).into(),
        SegmentStyle::H1
        | SegmentStyle::H2
        | SegmentStyle::H3
        | SegmentStyle::H4
        | SegmentStyle::H5
        | SegmentStyle::H6 => {
            let size = match seg.style {
                SegmentStyle::H1 => 26.0,
                SegmentStyle::H2 => 22.0,
                SegmentStyle::H3 => 19.0,
                SegmentStyle::H4 => 16.0,
                SegmentStyle::H5 => 14.0,
                _ => 13.0,
            };
            text(seg.text.clone()).size(size).font(bold_font()).into()
        }
        SegmentStyle::Bold => text(seg.text.clone()).font(bold_font()).into(),
        SegmentStyle::Italic => text(seg.text.clone())
            .font(Font {
                style: FontStyle::Italic,
                ..Font::default()
            })
            .into(),
        SegmentStyle::BoldItalic => text(seg.text.clone())
            .font(Font {
                weight: Weight::Bold,
                style: FontStyle::Italic,
                ..Font::default()
            })
            .into(),
        SegmentStyle::Strikethrough => {
            // iced 0.14 n'a pas de décoration de texte : barré réel via le
            // caractère combinant U+0336 (court trait superposé), rendu par
            // Inter. La couleur du texte reste normale.
            let barre: String = seg.text.chars().map(|c| format!("{c}\u{0336}")).collect();
            text(barre).into()
        }
        SegmentStyle::InlineCode => container(
            text(seg.text.clone())
                .font(Font::MONOSPACE)
                .size(12)
                .color(Color::from_rgb(0.8, 0.25, 0.25)),
        )
        .padding([1, 6])
        .style(|_theme| container::Style {
            background: Some(Background::Color(Color::from_rgb(0.94, 0.92, 0.94))),
            border: Border {
                color: Color::from_rgb(0.85, 0.8, 0.85),
                width: 1.0,
                radius: Radius::from(3.0),
            },
            ..container::Style::default()
        })
        .into(),
        SegmentStyle::CodeBlock => container(
            text(seg.text.clone())
                .font(Font::MONOSPACE)
                .size(12)
                .width(Length::Fill),
        )
        .width(Length::Fill)
        .padding(8)
        .style(|_theme| container::Style {
            background: Some(Background::Color(Color::from_rgb(0.9, 0.92, 0.95))),
            border: Border {
                color: Color::from_rgb(0.8, 0.83, 0.88),
                width: 1.0,
                radius: Radius::from(4.0),
            },
            ..container::Style::default()
        })
        .into(),
        SegmentStyle::Blockquote => container(text(seg.text.clone()))
            .width(Length::Fill)
            .padding([2, 10])
            .style(|_theme| container::Style {
                background: Some(Background::Color(Color::from_rgb(0.94, 0.95, 0.97))),
                border: Border {
                    color: Color::from_rgb(0.7, 0.75, 0.82),
                    width: 3.0,
                    radius: Radius::from(2.0),
                },
                ..container::Style::default()
            })
            .into(),
        SegmentStyle::Link | SegmentStyle::ExternalUrl => text(seg.text.clone())
            .color(Color::from_rgb(0.2, 0.4, 0.8))
            .into(),
        SegmentStyle::Wikilink => match &seg.target {
            // Wikilink du corps : cliquable, comme dans le bloc métadonnées.
            Some(target) => button(text(seg.text.clone()).color(Color::from_rgb(0.55, 0.3, 0.75)))
                .padding(0)
                .style(ghost_button_style)
                .on_press(Message::OpenWikilink(target.clone()))
                .into(),
            None => text(seg.text.clone())
                .color(Color::from_rgb(0.55, 0.3, 0.75))
                .into(),
        },
        SegmentStyle::Tag => tag_chip(&seg.text),
        SegmentStyle::TaskCheckboxTodo | SegmentStyle::TaskCheckboxDone => {
            let check = if seg.style == SegmentStyle::TaskCheckboxDone {
                "☑ "
            } else {
                "☐ "
            };
            indenter(
                seg.indent,
                row![text(check), text(seg.text.clone())].spacing(4),
            )
        }
        SegmentStyle::ListBullet => indenter(
            seg.indent,
            row![text("• "), text(seg.text.clone())].spacing(4),
        ),
        SegmentStyle::ListNumbered => indenter(
            seg.indent,
            row![text("· "), text(seg.text.clone())].spacing(4),
        ),
        SegmentStyle::HorizontalRule => container(text(""))
            .width(Length::Fill)
            .height(2)
            .style(|_theme| container::Style {
                background: Some(Background::Color(PANEL_BORDER)),
                ..container::Style::default()
            })
            .into(),
        SegmentStyle::Table => text(seg.text.clone()).font(Font::MONOSPACE).size(12).into(),
        SegmentStyle::Footnote => text(seg.text.clone())
            .size(12)
            .color(Color::from_rgb(0.35, 0.5, 0.7))
            .into(),
        SegmentStyle::Callout | SegmentStyle::CalloutTitle => {
            let bold = seg.style == SegmentStyle::CalloutTitle;
            let mut label = text(seg.text.clone()).size(13);
            if bold {
                label = label.font(bold_font());
            }
            container(label)
                .width(Length::Fill)
                .padding([2, 10])
                .style(|_theme| container::Style {
                    background: Some(Background::Color(Color::from_rgb(0.95, 0.93, 0.88))),
                    border: Border {
                        color: Color::from_rgb(0.8, 0.7, 0.45),
                        width: 3.0,
                        radius: Radius::from(2.0),
                    },
                    ..container::Style::default()
                })
                .into()
        }
        SegmentStyle::MathInline => text(seg.text.clone())
            .font(Font::MONOSPACE)
            .size(13)
            .color(Color::from_rgb(0.1, 0.45, 0.4))
            .into(),
        SegmentStyle::MathDisplay => container(
            text(seg.text.clone())
                .font(Font::MONOSPACE)
                .size(14)
                .color(Color::from_rgb(0.1, 0.45, 0.4))
                .width(Length::Fill),
        )
        .width(Length::Fill)
        .padding(8)
        .style(|_theme| container::Style {
            background: Some(Background::Color(Color::from_rgb(0.92, 0.96, 0.95))),
            border: Border {
                color: Color::from_rgb(0.6, 0.8, 0.75),
                width: 1.0,
                radius: Radius::from(4.0),
            },
            ..container::Style::default()
        })
        .into(),
        SegmentStyle::EmbedImage => embed_image_element(root, &seg.text),
        SegmentStyle::Embed | SegmentStyle::EmbedPdf => {
            let prefix = if seg.style == SegmentStyle::EmbedPdf {
                "📄 "
            } else {
                "⛓ "
            };
            let color = Color::from_rgb(0.2, 0.4, 0.8);
            row![
                text(prefix).color(color),
                text(seg.text.clone()).color(color)
            ]
            .spacing(2)
            .into()
        }
    }
}

/// Décale un élément de liste de `niveaux * 14 px` vers la droite.
fn indenter<'a>(niveaux: u8, contenu: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
    container(contenu.into())
        .padding([f32::from(niveaux) * 14.0, 0.0])
        .width(Length::Fill)
        .into()
}

/// Rend le contenu Markdown en colonne de segments stylisés.
///
/// Les segments consécutifs de tableau sont regroupés en une grille
/// cellulaire ; les groupes de callouts (titre + corps) sont repliables
/// via `app.callout_folded`.
fn live_preview_column(app: &App, root: &Path, content: &str) -> Element<'static, Message> {
    let segments = render_markdown(content);
    let mut column = Column::new().spacing(4);
    let mut index = 0usize;
    while index < segments.len() {
        let seg = &segments[index];

        // Tableau : regrouper les segments consécutifs de lignes contiguës.
        if seg.style == SegmentStyle::Table {
            let mut lignes: Vec<String> = vec![seg.text.clone()];
            let mut fin = index + 1;
            while fin < segments.len()
                && segments[fin].style == SegmentStyle::Table
                && segments[fin].line == segments[fin - 1].line + 1
            {
                lignes.push(segments[fin].text.clone());
                fin += 1;
            }
            column = column.push(table_grid_widget(&lignes));
            index = fin;
            continue;
        }

        // Callout : le groupe commence au titre (ou à la première ligne de
        // corps sans titre) et se poursuit tant que les segments `Callout`
        // se suivent ligne à ligne.
        if matches!(
            seg.style,
            SegmentStyle::Callout | SegmentStyle::CalloutTitle
        ) {
            let debut = seg.line;
            let titre = (seg.style == SegmentStyle::CalloutTitle)
                .then(|| seg.text.trim_start_matches(['>', ' ']).to_string());
            let mut fin = index + 1;
            let mut corps: Vec<String> = Vec::new();
            while fin < segments.len()
                && segments[fin].style == SegmentStyle::Callout
                && segments[fin].line == segments[fin - 1].line + 1
            {
                corps.push(segments[fin].text.clone());
                fin += 1;
            }
            column = column.push(callout_widget(app, debut, titre, corps));
            index = fin;
            continue;
        }

        column = column.push(segment_widget(root, seg));
        index += 1;
    }
    container(column)
        .max_width(READING_LINE_WIDTH_PX)
        .center_x(Length::Fill)
        .into()
}

/// Grille d'un tableau : ligne d'en-tête en gras, bordures externes et
/// séparateurs de lignes.
fn table_grid_widget(lignes: &[String]) -> Element<'static, Message> {
    let grille = crate::ui::markdown::table_grid(lignes);
    let mut rows: Vec<Element<'static, Message>> = Vec::new();
    for (index, cells) in grille.iter().enumerate() {
        let mut row_widgets: Vec<Element<'static, Message>> = Vec::new();
        for cell in cells {
            let mut t = text(cell.clone()).size(12);
            if index == 0 {
                t = t.font(bold_font());
            }
            row_widgets.push(
                container(t)
                    .width(Length::Fill)
                    .padding([4, 8])
                    .style(move |_theme| container::Style {
                        background: (index % 2 == 1)
                            .then_some(Background::Color(Color::from_rgb(0.97, 0.97, 0.99))),
                        border: Border {
                            color: PANEL_BORDER,
                            width: 0.5,
                            radius: Radius::from(0.0),
                        },
                        ..container::Style::default()
                    })
                    .into(),
            );
        }
        rows.push(Row::with_children(row_widgets).into());
    }
    container(Column::with_children(rows))
        .width(Length::Fill)
        .style(|_theme| container::Style {
            border: Border {
                color: PANEL_BORDER,
                width: 1.0,
                radius: Radius::from(4.0),
            },
            ..container::Style::default()
        })
        .into()
}

/// Callout repliable : bandeau de titre cliquable (flèche), corps masqué
/// quand le groupe est replié (`app.callout_folded` contient la ligne de
/// début du groupe).
fn callout_widget(
    app: &App,
    debut: usize,
    titre: Option<String>,
    corps: Vec<String>,
) -> Element<'static, Message> {
    let replie = app.callout_folded.contains(&debut);
    let fleche = if replie { "▸" } else { "▾" };
    let libelle = titre.unwrap_or_else(|| "Note".to_string());
    let mut contenu = Column::new().spacing(2);
    contenu = contenu.push(
        button(
            row![
                text(fleche).size(12).color(Color::from_rgb(0.45, 0.3, 0.7)),
                text(libelle).size(13).font(bold_font())
            ]
            .spacing(6),
        )
        .padding([2, 4])
        .style(ghost_button_style)
        .on_press(Message::ToggleCallout(debut)),
    );
    if !replie {
        for ligne in corps {
            contenu = contenu.push(text(ligne).size(13).width(Length::Fill));
        }
    }
    container(contenu)
        .width(Length::Fill)
        .padding([6, 10])
        .style(|_theme| container::Style {
            background: Some(Background::Color(Color::from_rgb(0.96, 0.94, 0.99))),
            border: Border {
                color: Color::from_rgb(0.65, 0.5, 0.9),
                width: 2.0,
                radius: Radius::from(4.0),
            },
            ..container::Style::default()
        })
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
fn tag_chip(tag: &str) -> Element<'static, Message> {
    button(
        text(format!("#{tag}"))
            .size(12)
            .color(Color::from_rgb(0.2, 0.42, 0.65)),
    )
    .on_press(Message::GraphFilterTag(tag.to_string()))
    .padding([3, 8])
    .style(ghost_button_style)
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
        .style(move |theme, status| tree_button_style(theme, status, active))
        .into()
}

/// Style d'un bouton d'arborescence.
fn tree_button_style(theme: &iced::Theme, status: button::Status, active: bool) -> button::Style {
    let pal = theme.extended_palette();
    if active {
        button::Style {
            background: Some(Background::Color(pal.primary.strong.color)),
            text_color: pal.primary.strong.text,
            ..button::Style::default()
        }
    } else if matches!(status, button::Status::Hovered) {
        button::Style {
            background: Some(Background::Color(HOVER_OVERLAY)),
            ..button::Style::default()
        }
    } else {
        button::Style::default()
    }
}

// Style du bouton principal (sauvegarde, fermer).
fn primary_button_style(theme: &iced::Theme, status: button::Status) -> button::Style {
    let pal = theme.extended_palette();
    let background = if matches!(status, button::Status::Hovered) {
        pal.primary.strong.color
    } else {
        pal.primary.base.color
    };
    button::Style {
        background: Some(Background::Color(background)),
        text_color: pal.primary.strong.text,
        ..button::Style::default()
    }
}

// Style de l'onglet de note actif : fond du fond principal (#1C1C1C), comme
// mesuré (UI-SPEC §3 : « Onglet de note actif : fond #1C1C1C »).
fn tab_active_button_style(theme: &iced::Theme, _status: button::Status) -> button::Style {
    let pal = theme.extended_palette();
    button::Style {
        background: Some(Background::Color(pal.background.base.color)),
        text_color: pal.background.base.text,
        ..button::Style::default()
    }
}

// Style discret des petits boutons d'icônes (✓, ×, +).
pub(crate) fn ghost_button_style(theme: &iced::Theme, status: button::Status) -> button::Style {
    let pal = theme.extended_palette();
    let background = if matches!(status, button::Status::Hovered) {
        Some(Background::Color(pal.background.weak.color))
    } else {
        None
    };
    button::Style {
        background,
        text_color: pal.background.base.text,
        ..button::Style::default()
    }
}

// Style commun des panneaux (fond et cadre).
pub(crate) fn panel_style(theme: &iced::Theme) -> container::Style {
    let pal = theme.extended_palette();
    container::Style {
        background: Some(Background::Color(pal.background.base.color)),
        border: Border {
            color: pal.background.weak.color,
            width: 1.0,
            radius: Radius::from(theme::PANEL_RADIUS),
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
    let mut font = theme::font();
    font.weight = Weight::Bold;
    font
}

#[cfg(test)]
mod tests {
    use super::weekday_of;

    #[test]
    fn weekday_of_2026_10_21_est_mercredi() {
        // 2026-10-21 est un mercredi → index lundi=0 → 2.
        assert_eq!(weekday_of(2026, 10, 21), 2);
    }

    #[test]
    fn weekday_of_2026_10_19_est_lundi() {
        assert_eq!(weekday_of(2026, 10, 19), 0);
    }

    #[test]
    fn weekday_of_2026_10_25_est_dimanche() {
        assert_eq!(weekday_of(2026, 10, 25), 6);
    }

    #[test]
    fn geometrie_principale_correspond_aux_mesures() {
        // Mesures UI-SPEC §2 : ruban 44 px, docks 300 px, bandeau d'onglets
        // 40 px, barre d'état 27 px, lecture 700 px avec marges 32 px.
        assert_eq!(super::RIBBON_WIDTH_PX, 44.0);
        assert_eq!(super::SIDEBAR_WIDTH_PX, 300.0);
        assert_eq!(super::TAB_BAR_HEIGHT_PX, 40.0);
        assert_eq!(super::STATUS_BAR_HEIGHT_PX, 27.0);
        assert_eq!(super::READING_LINE_WIDTH_PX, 700.0);
        assert_eq!(super::READING_MARGIN_PX, 32.0);
    }
}
