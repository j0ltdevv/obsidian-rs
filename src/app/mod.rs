//! État applicatif et cycle de vie (Phase 1 : édition, prévisualisation).
//!
//! Règles d'architecture :
//! - La vue (`ui`) est pure : aucune I/O. Tout accès disque (scan, lecture
//!   d'une note, sauvegarde) a lieu ici, dans `boot` ou `update`.
//! - Les opérations sont synchromes dans `update` (lecture/écriture directes).
//!   C'est un choix assumé pour la Phase 1 : les fichiers sont petits et
//!   l'éditeur est simple ; une évolution vers des `Task` asynchrones
//!   (`Task::perform`) est possible sans changer la vue.
//! - L'interface ne supprime jamais de fichier.

use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use crate::plugins::api::PluginContext;
use crate::plugins::demo_stats::demo_stats_plugin;
use crate::plugins::demo_todo::demo_todo_plugin;
use crate::plugins::registry::PluginRegistry;
use crate::vault::aliases::AliasIndex;
use crate::vault::canvas_data::CanvasBoard;
use crate::vault::commands::{default_commands, search_commands, Command};
use crate::vault::graph::GraphFilters;
use crate::vault::history::{list_snapshots, restore_snapshot};
use crate::vault::index::NoteIndex;
use crate::vault::meta::{remove_frontmatter_property, set_frontmatter_property};
use crate::vault::ops::write_note;
use crate::vault::search::{build_text_index, FullTextIndex};
use crate::vault::tasks::{aggregate_tasks, Task as VaultTask};
use crate::vault::{scan_vault, Note, NoteMetadata};
use iced::widget::text_editor;
use iced::{Element, Task};

/// Nombre maximal de résultats affichés par la recherche rapide.
const SEARCH_LIMIT: usize = 10;

/// Nombre maximal de résultats affichés par le panneau de recherche.
const SEARCH_PANEL_LIMIT: usize = 50;

/// Racine du vault de démonstration, relative à la racine du dépôt.
const DEMO_VAULT: &str = "demo-vault";

/// Messages adressés à l'application.
#[derive(Debug, Clone)]
pub enum Message {
    /// Relance un scan complet et reconstruit notes + index.
    Refresh,
    /// Replie/déplie le dossier donné dans la barre latérale.
    ToggleFolder(PathBuf),
    /// Ouvre la note au chemin relatif donné.
    OpenNote(PathBuf),
    /// Action d'édition dans l'éditeur de la note ouverte.
    EditorAction(text_editor::Action),
    /// Sauvegarde la note ouverte (`vault::ops::write_note`, atomique).
    SaveNote,
    /// Résout un wikilink `[[cible]]` et ouvre la première correspondance.
    OpenWikilink(String),
    /// Ferme la bannière d'erreur.
    DismissError,
    /// Ouvre la recherche rapide (Quick Switcher).
    OpenSwitcher,
    /// Ferme la recherche rapide.
    CloseSwitcher,
    /// Change la requête de la recherche rapide (résultats recalculés).
    SwitcherQuery(String),
    /// Sélectionne le résultat d'indice donné (indice dans `App::notes`).
    SwitcherSelect(usize),
    /// Bascule l'affichage de la vue graphe.
    ToggleGraph,
    /// Bascule le sélecteur de thème.
    ToggleThemes,
    /// Bascule l'historique de versions.
    ToggleHistory,
    /// Bascule la vue Dataview-like.
    ToggleDataview,
    /// Sélectionne un thème par nom.
    SelectTheme(String),
    /// Définit/remplace une propriété du frontmatter.
    SetProperty { key: String, value: String },
    /// Supprime une propriété du frontmatter.
    RemoveProperty { key: String },
    /// Ajoute une nouvelle propriété du frontmatter.
    AddProperty { key: String, value: String },
    /// Édite la valeur brouillon d'une propriété existante.
    PropertyValueChanged { key: String, value: String },
    /// Édite la clé brouillon de la nouvelle propriété.
    NewPropertyKeyChanged(String),
    /// Édite la valeur brouillon de la nouvelle propriété.
    NewPropertyValueChanged(String),
    /// Bascule la vue Plugins (liste des plugins enregistrés).
    TogglePlugins,
    /// Restaure une version locale (snapshot) dans la note ouverte.
    RestoreSnapshot(usize),
    /// Bascule la vue Canvas.
    ToggleCanvas,
    /// Ouvre la palette de commandes.
    OpenCommandPalette,
    /// Ferme la palette de commandes.
    CloseCommandPalette,
    /// Change la requête de la palette (résultats recalculés).
    CommandPaletteQuery(String),
    /// Exécute la commande sélectionnée (par indice dans la liste filtrée).
    CommandPaletteSelect(usize),
    /// Touche Échap : ferme l'overlay courant (palette ou switcher).
    Escape,
    /// Bascule la vue Plan (plan de la note ouverte).
    ToggleOutline,
    /// Bascule le panneau de recherche full-text.
    ToggleSearch,
    /// Change la requête du panneau de recherche (résultats recalculés).
    SearchQuery(String),
    /// Sélectionne un résultat de recherche (indice dans `App::notes`).
    SearchSelect(usize),
    /// Bascule la vue Tâches (vue globale des tâches du vault).
    ToggleTasks,
    /// Bascule le filtre local du graphe (autour de la note ouverte).
    GraphToggleLocal,
    /// Efface tous les filtres du graphe.
    GraphClearFilters,
}

/// Note actuellement ouverte : chemin, métadonnées du dernier scan et tampon
/// d'édition (contenu brut chargé depuis le disque à l'ouverture).
#[derive(Debug)]
pub struct OpenNote {
    /// Chemin relatif au root du vault.
    pub path: PathBuf,
    /// Nom de la note (sans extension).
    pub name: String,
    /// Tampon d'édition (contenu brut de la note).
    pub content: text_editor::Content,
    /// Métadonnées parsées au moment du scan (frontmatter, tags, wikilinks).
    pub metadata: NoteMetadata,
    /// Brouillons des propriétés du frontmatter (saisie en cours).
    pub property_drafts: PropertyDrafts,
}
/// Recherche rapide ouverte : requête courante et résultats (indices de notes).
#[derive(Debug)]
pub struct SwitcherState {
    /// Requête courante.
    pub query: String,
    /// Indices des notes affichées, dans l'ordre de score décroissant.
    pub results: Vec<usize>,
}

/// Palette de commandes ouverte : requête et indices des commandes filtrées.
#[derive(Debug)]
pub struct CommandPaletteState {
    /// Requête courante.
    pub query: String,
    /// Indices des commandes affichées (dans `App::commands`).
    pub results: Vec<usize>,
}

/// Panneau de recherche full-text ouvert : requête courante et résultats
/// (indices de notes).
#[derive(Debug)]
pub struct SearchPanelState {
    /// Requête courante.
    pub query: String,
    /// Indices des notes affichées, dans l'ordre de score décroissant.
    pub results: Vec<usize>,
}

/// Brouillons d'édition des propriétés du frontmatter de la note ouverte.
#[derive(Debug, Default)]
pub struct PropertyDrafts {
    /// Valeurs en cours de saisie, par clé existante.
    pub values: BTreeMap<String, String>,
    /// Clé saisie pour la nouvelle propriété (ligne « + »).
    pub new_key: String,
    /// Valeur saisie pour la nouvelle propriété.
    pub new_value: String,
}

impl PropertyDrafts {
    /// Initialise les brouillons depuis le frontmatter parsé (valeur affichée).
    fn from_frontmatter(frontmatter: &Option<serde_yaml::Value>) -> Self {
        let mut values = BTreeMap::new();
        if let Some(serde_yaml::Value::Mapping(mapping)) = frontmatter {
            for (key, value) in mapping {
                if let serde_yaml::Value::String(name) = key {
                    values.insert(name.clone(), property_display(value));
                }
            }
        }
        Self {
            values,
            new_key: String::new(),
            new_value: String::new(),
        }
    }
}

/// État global de l'application.
pub struct App {
    /// Résultat du dernier scan : liste des notes ou message d'erreur.
    pub notes: Result<Vec<Note>, String>,
    /// Index des wikilinks/backlinks, construit depuis `notes`.
    pub index: Option<NoteIndex>,
    /// Index des alias de notes, construit depuis `notes`.
    pub aliases: Option<AliasIndex>,
    /// Racine du vault scanné, réutilisée pour rafraîchir/sauvegarder.
    pub vault_root: PathBuf,
    /// Note ouverte dans l'éditeur, si présente.
    pub open: Option<OpenNote>,
    /// Dossiers dépliés dans la barre latérale (chemins relatifs).
    pub expanded: HashSet<PathBuf>,
    /// Message d'erreur du dernier échec, affiché en bannière.
    pub error: Option<String>,
    /// Recherche rapide ouverte (overlay), si présente.
    pub switcher: Option<SwitcherState>,
    /// Affiche la vue graphe à la place de l'éditeur/prévisualisation.
    pub show_graph: bool,
    /// Affiche le sélecteur de thème à la place de la prévisualisation.
    pub show_themes: bool,
    /// Affiche l'historique de versions à la place de la prévisualisation.
    pub show_history: bool,
    /// Affiche la vue Dataview-like à la place de la prévisualisation.
    pub show_dataview: bool,
    /// Thème actuellement appliqué (nom, ex. "dark").
    pub current_theme: String,
    /// Snapshots de la note ouverte (chargés pour la vue historique).
    pub history_snapshots: Vec<PathBuf>,
    /// Affiche la vue Canvas à la place de l'éditeur/prévisualisation.
    pub show_canvas: bool,
    /// Planche Canvas (une carte par note, arêtes = wikilinks résolus).
    pub canvas: CanvasBoard,
    /// Registre des plugins chargés (démo enregistrées au boot).
    pub plugins: PluginRegistry,
    /// Affiche la vue Plugins à la place de l'éditeur/prévisualisation.
    pub show_plugins: bool,
    /// Commandes enregistrées (palette). Fet au boot depuis `default_commands`.
    pub commands: Vec<Command>,
    /// Palette de commandes ouverte (overlay), si présente.
    pub command_palette: Option<CommandPaletteState>,
    /// Affiche la vue Plan à la place de l'éditeur/prévisualisation.
    pub show_outline: bool,
    /// Affiche le panneau de recherche full-text à la place de l'éditeur.
    pub show_search: bool,
    /// Affiche la vue Tâches à la place de l'éditeur/prévisualisation.
    pub show_tasks: bool,
    /// Panneau de recherche full-text ouvert, si présent.
    pub search_panel: Option<SearchPanelState>,
    /// Index full-text des notes (construit au scan, via `build_text_index`).
    pub fulltext: Option<FullTextIndex>,
    /// Tâches agrégées de toutes les notes (construites au scan).
    pub tasks: Vec<VaultTask>,
    /// Filtres du graphe (local / tag / dossier).
    pub graph_filters: GraphFilters,
}

impl App {
    /// Scanne le vault donné et reconstitue notes + index.
    fn rescan(vault_root: &Path) -> (Result<Vec<Note>, String>, Option<NoteIndex>) {
        let notes = scan_vault(vault_root).map_err(|error| error.to_string());
        let index = notes
            .as_ref()
            .ok()
            .map(|notes| NoteIndex::build_index(notes));
        (notes, index)
    }

    /// Construit la planche Canvas depuis les notes et l'index.
    fn build_canvas(notes: &[Note], index: &Option<NoteIndex>) -> CanvasBoard {
        let mut board = CanvasBoard::new();
        let mut by_name = std::collections::HashMap::new();
        for note in notes {
            let id = board.add_card(&note.name);
            by_name.insert(note.name.clone(), id);
        }
        if let Some(index) = index {
            for note in notes {
                let Some(&from) = by_name.get(&note.name) else {
                    continue;
                };
                for link in &note.metadata.wikilinks {
                    let resolved = index.resolve(&link.target);
                    if let Some(target) = resolved.first() {
                        if let Some(&to) = by_name.get(&target.name) {
                            let _ = board.connect(from, to);
                        }
                    }
                }
            }
        }
        board
    }
}

/// Point d'entrée iced : construit l'état initial en scannant le vault.
pub fn boot() -> (App, Task<Message>) {
    let (notes, index) = App::rescan(Path::new(DEMO_VAULT));
    let canvas = App::build_canvas(
        notes.as_ref().map(|n| n.as_slice()).unwrap_or_default(),
        &index,
    );
    // Registre de plugins : enregistre les démo, puis les charge. Une erreur
    // de chargement n'empêche pas l'app de démarrer (elle est affichée).
    let note_count = notes.as_ref().map(|n| n.len()).unwrap_or(0);
    let mut plugins = PluginRegistry::new();
    let _ = plugins.register(demo_stats_plugin());
    let _ = plugins.register(demo_todo_plugin());
    let ctx = PluginContext {
        vault_name: DEMO_VAULT,
        note_count,
    };
    let plugin_error = plugins.load_all(&ctx).err();
    let (fulltext, tasks) = build_search_data(Path::new(DEMO_VAULT), &notes);
    let aliases = notes.as_ref().ok().map(|notes| AliasIndex::build(notes));
    let app = App {
        notes,
        index,
        aliases,
        vault_root: PathBuf::from(DEMO_VAULT),
        open: None,
        expanded: HashSet::new(),
        error: plugin_error,
        switcher: None,
        show_graph: false,
        show_themes: false,
        show_history: false,
        show_dataview: false,
        current_theme: "dark".to_string(),
        history_snapshots: Vec::new(),
        show_canvas: false,
        canvas,
        plugins,
        show_plugins: false,
        commands: default_commands(),
        command_palette: None,
        show_outline: false,
        show_search: false,
        show_tasks: false,
        search_panel: None,
        fulltext,
        tasks,
        graph_filters: GraphFilters::default(),
    };
    (app, Task::none())
}

/// Traite un message : seule source de mutation de l'état. Toute I/O a lieu
/// ici, jamais dans la vue.
pub fn update(app: &mut App, message: Message) -> Task<Message> {
    match message {
        Message::Refresh => {
            let (notes, index) = App::rescan(&app.vault_root);
            // Conserve la note ouverte si son fichier existe toujours ; on ne
            // recharge pas son contenu (le tampon d'édition est préservé).
            let keep_open = match (&notes, &app.open) {
                (Ok(notes), Some(open)) => notes.iter().any(|note| note.path == open.path),
                _ => false,
            };
            if !keep_open {
                app.open = None;
            }
            let canvas = App::build_canvas(
                notes.as_ref().map(|n| n.as_slice()).unwrap_or_default(),
                &index,
            );
            let (fulltext, tasks) = build_search_data(&app.vault_root, &notes);
            app.fulltext = fulltext;
            app.tasks = tasks;
            app.notes = notes;
            app.index = index;
            app.aliases = app
                .notes
                .as_ref()
                .ok()
                .map(|notes| AliasIndex::build(notes));
            app.canvas = canvas;
            app.error = None;
            if let Some(state) = &mut app.search_panel {
                state.results = search_results(&app.fulltext, &app.notes, &state.query);
            }
            if let Some(state) = &mut app.switcher {
                state.results = switcher_results(&app.notes, &app.index, &state.query);
            }
        }
        Message::ToggleCanvas => {
            app.show_canvas = !app.show_canvas;
            if app.show_canvas {
                app.show_themes = false;
                app.show_history = false;
                app.show_dataview = false;
                app.show_graph = false;
            }
        }
        Message::TogglePlugins => {
            app.show_plugins = !app.show_plugins;
            if app.show_plugins {
                app.show_themes = false;
                app.show_history = false;
                app.show_dataview = false;
                app.show_graph = false;
                app.show_canvas = false;
            }
        }
        Message::ToggleFolder(dir) => {
            if !app.expanded.remove(&dir) {
                app.expanded.insert(dir);
            }
        }
        Message::OpenNote(path) => open_note(app, &path),
        Message::EditorAction(action) => {
            if let Some(open) = &mut app.open {
                open.content.perform(action);
            }
        }
        Message::SaveNote => {
            if let Some(open) = &app.open {
                match write_note(&app.vault_root, &open.path, &open.content.text()) {
                    Ok(()) => {
                        app.error = None;
                        // Notifie les plugins de la mutation (id = chemin).
                        let note_count = app.notes.as_ref().map(|n| n.len()).unwrap_or(0);
                        let ctx = PluginContext {
                            vault_name: DEMO_VAULT,
                            note_count,
                        };
                        let id = open.path.to_string_lossy().into_owned();
                        if let Err(error) = app.plugins.notify_change(&ctx, &[id]) {
                            app.error = Some(format!("Erreur plugin : {error}"));
                        }
                    }
                    Err(error) => {
                        app.error = Some(format!("Sauvegarde impossible : {error}"));
                    }
                }
            }
        }
        Message::OpenWikilink(target) => {
            // Résolution combinée : index des wikilinks puis alias de notes.
            let resolved = resolve_link(&app.index, &app.aliases, &target)
                .first()
                .map(|note| note.path.clone());
            match resolved {
                Some(path) => open_note(app, &path),
                None => {
                    app.error = Some(format!("Wikilink non résolu : [[{target}]]"));
                }
            }
        }
        Message::OpenSwitcher => {
            let results = switcher_results(&app.notes, &app.index, "");
            app.switcher = Some(SwitcherState {
                query: String::new(),
                results,
            });
        }
        Message::CloseSwitcher => app.switcher = None,
        Message::SwitcherQuery(query) => {
            if let Some(state) = &mut app.switcher {
                state.results = switcher_results(&app.notes, &app.index, &query);
                state.query = query;
            }
        }
        Message::SwitcherSelect(index) => {
            if let Some(note) = app.notes.as_ref().ok().and_then(|notes| notes.get(index)) {
                let path = note.path.clone();
                app.switcher = None;
                open_note(app, &path);
            }
        }
        Message::OpenCommandPalette => {
            let results = search_commands(&app.commands, "");
            app.command_palette = Some(CommandPaletteState {
                query: String::new(),
                results,
            });
        }
        Message::CloseCommandPalette => app.command_palette = None,
        Message::Escape => {
            if app.command_palette.is_some() {
                app.command_palette = None;
            } else {
                app.switcher = None;
            }
        }
        Message::CommandPaletteQuery(query) => {
            if let Some(state) = &mut app.command_palette {
                state.results = search_commands(&app.commands, &query);
                state.query = query;
            }
        }
        Message::CommandPaletteSelect(index) => {
            if let Some(state) = &app.command_palette {
                if let Some(&command_index) = state.results.get(index) {
                    if let Some(command) = app.commands.get(command_index) {
                        app.command_palette = None;
                        match command.id {
                            "open-switcher" => {
                                let results = switcher_results(&app.notes, &app.index, "");
                                app.switcher = Some(SwitcherState {
                                    query: String::new(),
                                    results,
                                });
                            }
                            "save-note" => {
                                return update(app, Message::SaveNote);
                            }
                            "toggle-graph" => {
                                return update(app, Message::ToggleGraph);
                            }
                            "toggle-themes" => {
                                return update(app, Message::ToggleThemes);
                            }
                            "toggle-history" => {
                                return update(app, Message::ToggleHistory);
                            }
                            "toggle-dataview" => {
                                return update(app, Message::ToggleDataview);
                            }
                            "toggle-canvas" => {
                                return update(app, Message::ToggleCanvas);
                            }
                            "toggle-plugins" => {
                                return update(app, Message::TogglePlugins);
                            }
                            "toggle-outline" => {
                                return update(app, Message::ToggleOutline);
                            }
                            "toggle-search" => {
                                return update(app, Message::ToggleSearch);
                            }
                            "toggle-tasks" => {
                                return update(app, Message::ToggleTasks);
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
        Message::ToggleOutline => {
            app.show_outline = !app.show_outline;
            if app.show_outline {
                app.show_themes = false;
                app.show_history = false;
                app.show_dataview = false;
                app.show_graph = false;
                app.show_canvas = false;
                app.show_plugins = false;
                app.show_search = false;
                app.show_tasks = false;
            }
        }
        Message::ToggleSearch => {
            app.show_search = !app.show_search;
            if app.show_search {
                app.show_themes = false;
                app.show_history = false;
                app.show_dataview = false;
                app.show_graph = false;
                app.show_canvas = false;
                app.show_plugins = false;
                app.show_outline = false;
                app.show_tasks = false;
                if app.search_panel.is_none() {
                    app.search_panel = Some(SearchPanelState {
                        query: String::new(),
                        results: search_results(&app.fulltext, &app.notes, ""),
                    });
                }
            }
        }
        Message::SearchQuery(query) => {
            if let Some(state) = &mut app.search_panel {
                state.query = query.clone();
                state.results = search_results(&app.fulltext, &app.notes, &query);
            }
        }
        Message::SearchSelect(index) => {
            if let Some(note) = app.notes.as_ref().ok().and_then(|notes| notes.get(index)) {
                let path = note.path.clone();
                app.search_panel = None;
                app.show_search = false;
                open_note(app, &path);
            }
        }
        Message::ToggleTasks => {
            app.show_tasks = !app.show_tasks;
            if app.show_tasks {
                app.show_themes = false;
                app.show_history = false;
                app.show_dataview = false;
                app.show_graph = false;
                app.show_canvas = false;
                app.show_plugins = false;
                app.show_outline = false;
                app.show_search = false;
            }
        }
        Message::GraphToggleLocal => {
            if let Some(path) = app.open.as_ref().map(|o| o.path.clone()) {
                if app.graph_filters.local_note.as_ref() == Some(&path) {
                    app.graph_filters.local_note = None;
                } else {
                    app.graph_filters.local_note = Some(path);
                }
            }
        }
        Message::GraphClearFilters => {
            app.graph_filters = GraphFilters::default();
        }
        Message::ToggleGraph => {
            app.show_graph = !app.show_graph;
            if app.show_graph {
                app.switcher = None;
            }
        }
        Message::ToggleThemes => {
            app.show_themes = !app.show_themes;
            if app.show_themes {
                app.show_history = false;
                app.show_dataview = false;
                app.show_graph = false;
            }
        }
        Message::ToggleHistory => {
            app.show_history = !app.show_history;
            if app.show_history {
                app.show_themes = false;
                app.show_dataview = false;
                app.show_graph = false;
                app.history_snapshots = match (
                    &app.open,
                    list_snapshots(
                        &app.vault_root,
                        &app.open
                            .as_ref()
                            .map(|o| o.path.clone())
                            .unwrap_or_default(),
                    ),
                ) {
                    (Some(_), Ok(list)) => list,
                    _ => Vec::new(),
                };
            }
        }
        Message::ToggleDataview => {
            app.show_dataview = !app.show_dataview;
            if app.show_dataview {
                app.show_themes = false;
                app.show_history = false;
                app.show_graph = false;
            }
        }
        Message::SelectTheme(name) => {
            app.current_theme = name;
        }
        Message::RestoreSnapshot(index) => {
            if let Some(open) = &app.open {
                if let Some(snap) = app.history_snapshots.get(index) {
                    match restore_snapshot(&app.vault_root, &open.path, snap) {
                        Ok(source) => {
                            if let Some(open) = &mut app.open {
                                open.content = text_editor::Content::with_text(&source);
                                open.property_drafts =
                                    PropertyDrafts::from_frontmatter(&open.metadata.frontmatter);
                            }
                            app.error = None;
                        }
                        Err(error) => {
                            app.error = Some(format!("Restauration impossible : {error}"));
                        }
                    }
                }
            }
        }
        Message::SetProperty { key, value } => set_property(app, &key, &value),
        Message::RemoveProperty { key } => remove_property(app, &key),
        Message::AddProperty { key, value } => {
            if key.trim().is_empty() {
                app.error = Some("Nom de propriété vide.".to_string());
            } else {
                set_property(app, &key, &value);
                if app.error.is_none() {
                    if let Some(open) = &mut app.open {
                        open.property_drafts.new_key.clear();
                        open.property_drafts.new_value.clear();
                    }
                }
            }
        }
        Message::PropertyValueChanged { key, value } => {
            if let Some(open) = &mut app.open {
                open.property_drafts.values.insert(key, value);
            }
        }
        Message::NewPropertyKeyChanged(key) => {
            if let Some(open) = &mut app.open {
                open.property_drafts.new_key = key;
            }
        }
        Message::NewPropertyValueChanged(value) => {
            if let Some(open) = &mut app.open {
                open.property_drafts.new_value = value;
            }
        }
        Message::DismissError => app.error = None,
    }
    Task::none()
}

/// Ouvre la note au chemin relatif donné : lecture du contenu depuis le disque
/// (I/O dans `update`), mémorisation des métadonnées du dernier scan.
fn open_note(app: &mut App, path: &Path) {
    let snapshot = app
        .notes
        .as_ref()
        .ok()
        .and_then(|notes| notes.iter().find(|note| note.path == path).cloned());

    let (name, metadata) = match snapshot {
        Some(note) => (note.name, note.metadata),
        None => {
            let name = path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or_default()
                .to_string();
            let metadata = NoteMetadata {
                frontmatter: None,
                frontmatter_error: None,
                wikilinks: Vec::new(),
                tags: Vec::new(),
            };
            (name, metadata)
        }
    };

    let property_drafts = PropertyDrafts::from_frontmatter(&metadata.frontmatter);

    match fs::read_to_string(app.vault_root.join(path)) {
        Ok(source) => {
            app.open = Some(OpenNote {
                path: path.to_path_buf(),
                name,
                content: text_editor::Content::with_text(&source),
                metadata,
                property_drafts,
            });
            app.error = None;
        }
        Err(error) => {
            app.error = Some(format!(
                "Impossible d'ouvrir « {} » : {}",
                path.display(),
                error
            ));
        }
    }
}

/// Vue racine : délègue le rendu au module `ui`.
pub fn view(app: &App) -> Element<'_, Message> {
    crate::ui::view(app)
}
/// Raccourcis clavier globaux : Ctrl+K (recherche rapide), Ctrl+S
/// (sauvegarde), Ctrl+G (graphe) et Échap (fermer la recherche rapide).
///
/// La vue `ui` expose des boutons équivalents ; cette subscription est
/// branchée par `main` via `.subscription(app::subscription)`.
pub fn subscription(_app: &App) -> iced::Subscription<Message> {
    use iced::event::{self, Event};
    use iced::keyboard::{key, Key};

    event::listen_raw(|event, _status, _window| {
        let Event::Keyboard(iced::keyboard::Event::KeyPressed { key, modifiers, .. }) = event
        else {
            return None;
        };

        if modifiers.control() {
            match key.as_ref() {
                Key::Character(character) if character.eq_ignore_ascii_case("k") => {
                    return Some(Message::OpenSwitcher);
                }
                Key::Character(character) if character.eq_ignore_ascii_case("s") => {
                    return Some(Message::SaveNote);
                }
                Key::Character(character) if character.eq_ignore_ascii_case("g") => {
                    return Some(Message::ToggleGraph);
                }
                Key::Character(character) if character.eq_ignore_ascii_case("p") => {
                    return Some(Message::OpenCommandPalette);
                }
                _ => {}
            }
        }

        if key.as_ref() == Key::Named(key::Named::Escape) {
            return Some(Message::Escape);
        }

        None
    })
}

/// Calcule les indices des notes correspondant à `query` (recherche floue
/// via l'index), dans l'ordre de score décroissant.
fn switcher_results(
    notes: &Result<Vec<Note>, String>,
    index: &Option<NoteIndex>,
    query: &str,
) -> Vec<usize> {
    let (Some(notes), Some(index)) = (notes.as_ref().ok(), index.as_ref()) else {
        return Vec::new();
    };
    index
        .search(query, notes, SEARCH_LIMIT)
        .into_iter()
        .filter_map(|hit| notes.iter().position(|note| note == hit))
        .collect()
}

/// Recherche full-text : indices des notes correspondant à `query`, triés
/// par score décroissant (via `FullTextIndex`). Une requête vide retourne
/// une liste vide.
fn search_results(
    fulltext: &Option<FullTextIndex>,
    notes: &Result<Vec<Note>, String>,
    query: &str,
) -> Vec<usize> {
    let (Some(fulltext), Ok(notes)) = (fulltext, notes) else {
        return Vec::new();
    };
    fulltext
        .search_text(query, SEARCH_PANEL_LIMIT)
        .into_iter()
        .filter(|&i| i < notes.len())
        .collect()
}

/// Résout une cible de lien (`[[cible]]` ou alias) vers les notes
/// correspondantes. Combine l'index des wikilinks puis l'index des alias,
/// en dédupliquant par chemin (l'ordre de l'index prime).
fn resolve_link<'a>(
    index: &'a Option<NoteIndex>,
    aliases: &'a Option<AliasIndex>,
    target: &str,
) -> Vec<&'a Note> {
    let mut hits: Vec<&'a Note> = Vec::new();
    if let Some(index) = index {
        hits.extend(index.resolve(target));
    }
    if let Some(aliases) = aliases {
        hits.extend(aliases.resolve(target));
    }
    let mut seen = HashSet::new();
    hits.into_iter()
        .filter(|note| seen.insert(note.path.clone()))
        .collect()
}

/// Construit l'index full-text et les tâches agrégées du vault. Les lectures
/// de fichiers sont best-effort : un fichier illisible contribue avec un
/// texte vide et n'est pas inclus dans l'agrégat de tâches.
fn build_search_data(
    vault_root: &Path,
    notes: &Result<Vec<Note>, String>,
) -> (Option<FullTextIndex>, Vec<VaultTask>) {
    let Ok(notes) = notes else {
        return (None, Vec::new());
    };
    let mut items = Vec::with_capacity(notes.len());
    let mut contents = std::collections::HashMap::new();
    for note in notes {
        match fs::read_to_string(vault_root.join(&note.path)) {
            Ok(text) => {
                items.push((note.name.clone(), text.clone()));
                contents.insert(note.path.clone(), text);
            }
            Err(_) => items.push((note.name.clone(), String::new())),
        }
    }
    let fulltext = Some(build_text_index(&items));
    let tasks = aggregate_tasks(notes, &contents);
    (fulltext, tasks)
}

/// Définit/remplace une propriété du frontmatter de la note ouverte, puis
/// re-parse localement la note (métadonnées + index).
fn set_property(app: &mut App, key: &str, value: &str) {
    let Some(open) = &app.open else {
        return;
    };
    let rel_path = open.path.clone();
    let yaml_value = parse_property_value(value);
    match set_frontmatter_property(&app.vault_root, &rel_path, key, &yaml_value) {
        Ok(note) => {
            apply_reparsed_note(app, note);
            if let Some(open) = &mut app.open {
                // On conserve la saisie texte telle quelle dans le brouillon.
                open.property_drafts
                    .values
                    .insert(key.to_string(), value.to_string());
            }
            app.error = None;
        }
        Err(error) => {
            app.error = Some(format!("Propriété « {key} » impossible : {error}"));
        }
    }
}

/// Supprime une propriété du frontmatter de la note ouverte, puis re-parse
/// localement la note (métadonnées + index).
fn remove_property(app: &mut App, key: &str) {
    let Some(open) = &app.open else {
        return;
    };
    let rel_path = open.path.clone();
    match remove_frontmatter_property(&app.vault_root, &rel_path, key) {
        Ok(note) => {
            apply_reparsed_note(app, note);
            if let Some(open) = &mut app.open {
                open.property_drafts.values.remove(key);
            }
            app.error = None;
        }
        Err(error) => {
            app.error = Some(format!("Suppression de « {key} » impossible : {error}"));
        }
    }
}

/// Applique une note re-parsée après écriture : remplace l'entrée dans
/// `notes`, reconstruit l'index et rafraîchit les métadonnées de la note
/// ouverte si c'est la même.
fn apply_reparsed_note(app: &mut App, note: Note) {
    if let Ok(notes) = &mut app.notes {
        if let Some(slot) = notes
            .iter_mut()
            .find(|candidate| candidate.path == note.path)
        {
            *slot = note.clone();
        } else {
            notes.push(note.clone());
        }
        app.index = Some(NoteIndex::build_index(notes));
        app.aliases = Some(AliasIndex::build(notes));
    }
    if let Some(open) = &mut app.open {
        if open.path == note.path {
            open.metadata = note.metadata;
        }
    }
}

/// Convertit une saisie texte en valeur YAML : le texte est d'abord tenté
/// comme YAML (nombres, booléens, listes…), sinon utilisé comme chaîne.
/// Une saisie vide devient une chaîne vide.
fn parse_property_value(text: &str) -> serde_yaml::Value {
    if text.is_empty() {
        return serde_yaml::Value::String(String::new());
    }
    serde_yaml::from_str(text).unwrap_or_else(|_| serde_yaml::Value::String(text.to_string()))
}

/// Représentation éditable d'une valeur de frontmatter : les chaînes sont
/// utilisées telles quelles, les autres valeurs sont sérialisées en YAML.
fn property_display(value: &serde_yaml::Value) -> String {
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

    #[test]
    fn boot_charge_les_plugins_demo_actifs() {
        let (app, _task) = boot();
        let info = app.plugins.info();
        assert_eq!(info.len(), 2);
        assert!(info.iter().all(|p| p.enabled));
        assert!(info.iter().any(|p| p.id == "demo-stats"));
        assert!(info.iter().any(|p| p.id == "demo-todo"));
    }

    #[test]
    fn toggle_plugins_bascule_la_vue() {
        let (mut app, _task) = boot();
        assert!(!app.show_plugins);
        let _ = update(&mut app, Message::TogglePlugins);
        assert!(app.show_plugins);
        let _ = update(&mut app, Message::TogglePlugins);
        assert!(!app.show_plugins);
    }

    #[test]
    fn resolve_link_tombe_sur_l_alias_quand_le_nom_est_inconnu() {
        use crate::vault::aliases::extract_aliases;
        use crate::vault::note::NoteMetadata;
        use std::path::PathBuf;

        fn note_with_alias(name: &str, alias: &str) -> Note {
            let fm = Some(serde_yaml::Value::Mapping(
                [(
                    serde_yaml::Value::String("aliases".to_string()),
                    serde_yaml::Value::String(alias.to_string()),
                )]
                .into_iter()
                .collect(),
            ));
            Note {
                path: PathBuf::from(format!("{name}.md")),
                name: name.to_string(),
                metadata: NoteMetadata {
                    frontmatter: fm,
                    frontmatter_error: None,
                    wikilinks: Vec::new(),
                    tags: Vec::new(),
                },
            }
        }

        let _ = extract_aliases;
        let notes = vec![
            note_with_alias("Cible", "surnom"),
            Note {
                path: PathBuf::from("Autre.md"),
                name: "Autre".to_string(),
                metadata: NoteMetadata {
                    frontmatter: None,
                    frontmatter_error: None,
                    wikilinks: Vec::new(),
                    tags: Vec::new(),
                },
            },
        ];
        let index = Some(NoteIndex::build_index(&notes));
        let aliases = Some(AliasIndex::build(&notes));
        let hits = resolve_link(&index, &aliases, "surnom");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].name, "Cible");
    }
}
