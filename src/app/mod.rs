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

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use crate::plugins::api::PluginContext;
use crate::plugins::demo_stats::demo_stats_plugin;
use crate::plugins::demo_todo::demo_todo_plugin;
use crate::plugins::registry::PluginRegistry;
use crate::vault::aliases::AliasIndex;
use crate::vault::canvas_data::{CanvasBoard, CardId};
use crate::vault::commands::{default_commands, search_commands, Command};
use crate::vault::graph::GraphFilters;
use crate::vault::history::{list_snapshots, restore_snapshot};
use crate::vault::index::NoteIndex;
use crate::vault::meta::{remove_frontmatter_property, set_frontmatter_property};
use crate::vault::ops::{create_note, write_note};
use crate::vault::search::{build_text_index, FullTextIndex};
use crate::vault::settings::{load_app_settings, save_app_settings, AppSettings};
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
    /// Début d'un glisser-déposer de nœud du graphe.
    GraphDragStart { node: usize, origin: (f32, f32) },
    /// Mouvement de curseur pendant un glisser-déposer.
    GraphDragMove { x: f32, y: f32 },
    /// Fin du glisser-déposer.
    GraphDragEnd,
    /// Applique un décalage persistant à un nœud du graphe.
    GraphDragApply { node: usize, dx: f32, dy: f32 },
    /// Ouvre le panneau recherche/remplacement.
    FindOpen,
    /// Ferme le panneau recherche/remplacement.
    FindClose,
    /// Change la requête de recherche.
    FindQuery(String),
    /// Change le texte de remplacement.
    FindReplaceText(String),
    /// Remplace toutes les occurrences dans la note ouverte.
    FindReplaceAll,
    /// Sélectionne une carte du canvas (clic).
    CanvasSelect(CardId),
    /// Édite le titre d'une carte du canvas.
    CanvasEditTitle { id: CardId, text: String },
    /// Édite le contenu d'une carte du canvas.
    CanvasEditText { id: CardId, text: String },
    /// Ouvre le dialogue de nouvelle note.
    NewNoteOpen,
    /// Ferme le dialogue de nouvelle note.
    NewNoteClose,
    /// Change le nom de la note à créer.
    NewNoteName(String),
    /// Crée la note (ops atomique) et rafraîchit l'index.
    NewNoteCreate,
    /// Change le groupe d'une carte du canvas.
    CanvasSetGroup { id: CardId, group: String },
    /// Réorganise automatiquement les cartes du canvas.
    CanvasAutoLayout,
    /// Exporte le canvas en JSON (`.obsidian/canvas.json`).
    CanvasExportJson,
    /// Remplace toutes les occurrences dans tout le vault.
    GlobalReplaceAll,
    /// Active/désactive un plugin par id.
    PluginToggle(String),
    /// Recharge les snippets CSS depuis `.obsidian/snippets/`.
    ReloadSnippets,
    /// Bascule la vue Réglages.
    ToggleSettings,
    /// Active/désactive le mode vim (persisté).
    SetVimMode(bool),
    /// Active/désactive la correction orthographique (persistée).
    SetSpellcheck(bool),
    /// Ouvre le dialogue de renommage d'une note.
    ToggleRename(PathBuf),
    /// Valide le renommage.
    RenameCommit,
    /// Ouvre la confirmation de suppression d'une note.
    ToggleDelete(PathBuf),
    /// Confirme la suppression de la note.
    DeleteConfirm,
    /// Ouvre le dialogue de création de dossier.
    FolderDialogOpen,
    /// Crée le dossier saisi.
    FolderCreate(String),
    /// Change le nom du dossier saisi.
    FolderName(String),
    /// Change le brouillon de nouveau nom du renommage.
    RenameDraft(String),
    /// Filtre le graphe sur un tag (clic sur une pastille).
    GraphFilterTag(String),
    /// Importe le canvas depuis `.obsidian/canvas.json`.
    CanvasImportJson,
    /// Demande confirmation de suppression d'une carte du canvas.
    CanvasAskRemoveCard(CardId),
    /// Marque la note ouverte comme modifiée (non enregistrée).
    MarkDirty,
    /// Confirme la suppression de la carte.
    CanvasConfirmRemoveCard,
    /// Annule la suppression de la carte du canvas.
    CanvasCancelRemoveCard,
}
pub struct NewNoteState {
    /// Nom de la note (sans extension).
    pub name: String,
}
/// Recherche/remplacement dans la note ouverte.
#[derive(Debug)]
pub struct FindState {
    /// Requête courante.
    pub query: String,
    /// Texte de remplacement.
    pub replace: String,
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
    /// Résultats : indice de note + extrait (snippet) avec contexte.
    pub results: Vec<SearchHit>,
}

/// Un résultat de recherche : indice de note et extrait autour de la
/// première correspondance.
#[derive(Debug, Clone)]
pub struct SearchHit {
    pub index: usize,
    /// Extrait (contexte autour de la correspondance).
    pub snippet: String,
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
    /// Nombre de lignes par note (indice → lignes), pour les requêtes
    /// avec opérateur `line:`.
    pub search_line_counts: HashMap<usize, usize>,
    /// Tâches agrégées de toutes les notes (construites au scan).
    pub tasks: Vec<VaultTask>,
    /// Dialogue de nouvelle note ouvert, si présent.
    pub new_note: Option<NewNoteState>,
    /// Snippets CSS chargés (chemin, variables extraites).
    pub css_snippets: Vec<(std::path::PathBuf, Vec<crate::vault::css_snippets::CssVar>)>,
    /// Filtres du graphe (local / tag / dossier).
    pub graph_filters: GraphFilters,
    /// Décalages persistants de nœuds du graphe (nœud → dx, dy).
    pub graph_drags: HashMap<usize, (f32, f32)>,
    /// Nœud de graphe en cours de glisser-déposer.
    pub graph_drag_active: Option<usize>,
    /// Origine (curseur) du glisser-déposer en cours.
    pub graph_drag_origin: Option<(f32, f32)>,
    /// Programme de graphe persistant (conserve l'état de glisser-déposer).
    pub graph_program: Option<crate::ui::graph::GraphProgram>,
    /// Panneau recherche/remplacement ouvert, si présent.
    pub find: Option<FindState>,
    /// Carte du canvas actuellement sélectionnée.
    pub canvas_selected: Option<CardId>,
    /// Paramètres persistés (app.json).
    pub settings: AppSettings,
    /// Affiche la vue Réglages.
    pub show_settings: bool,
    /// Dialogue de renommage ouvert : (chemin courant, nouveau nom).
    pub rename_dialog: Option<(PathBuf, String)>,
    /// Carte de note en attente de confirmation de suppression.
    pub delete_confirm: Option<PathBuf>,
    /// Dialogue de création de dossier ouvert.
    pub folder_dialog: bool,
    /// Nom du dossier en cours de saisie.
    pub folder_name: String,
    /// Raccourcis persistés (hotkeys.json), chargés au boot.
    pub hotkeys: Vec<crate::vault::settings::Hotkey>,
    /// Dernières notes ouvertes (chemins), plus récente en tête.
    pub recent: Vec<PathBuf>,
    /// La note ouverte contient des modifications non enregistrées.
    pub open_dirty: bool,
    /// Carte du canvas en attente de confirmation de suppression.
    pub canvas_remove_confirm: Option<CardId>,
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
    let settings = load_app_settings(Path::new(DEMO_VAULT)).unwrap_or_default();
    let (fulltext, tasks, search_line_counts) = build_search_data(Path::new(DEMO_VAULT), &notes);
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
        current_theme: settings.theme.clone(),
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
        search_line_counts,
        graph_filters: GraphFilters::default(),
        graph_drags: HashMap::new(),
        graph_drag_active: None,
        graph_drag_origin: None,
        graph_program: None,
        find: None,
        canvas_selected: None,
        new_note: None,
        css_snippets: crate::vault::css_snippets::load_snippets(Path::new(DEMO_VAULT)),
        settings,
        show_settings: false,
        rename_dialog: None,
        delete_confirm: None,
        folder_dialog: false,
        folder_name: String::new(),
        hotkeys: crate::vault::settings::load_hotkeys(Path::new(DEMO_VAULT)).unwrap_or_default(),
        recent: Vec::new(),
        open_dirty: false,
        canvas_remove_confirm: None,
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
            let (fulltext, tasks, search_line_counts) = build_search_data(&app.vault_root, &notes);
            app.fulltext = fulltext;
            app.tasks = tasks;
            app.search_line_counts = search_line_counts;
            app.notes = notes;
            app.index = index;
            app.aliases = app
                .notes
                .as_ref()
                .ok()
                .map(|notes| AliasIndex::build(notes));
            app.canvas = canvas;
            app.error = None;
            if let Some(query) = app.search_panel.as_ref().map(|s| s.query.clone()) {
                let hits = search_results(app, &query);
                if let Some(state) = &mut app.search_panel {
                    state.results = hits;
                }
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
                app.open_dirty = true;
            }
        }
        Message::SaveNote => {
            if let Some(open) = &app.open {
                match write_note(&app.vault_root, &open.path, &open.content.text()) {
                    Ok(()) => {
                        app.error = None;
                        app.open_dirty = false;
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
            } else if app.switcher.is_some() {
                app.switcher = None;
            } else if app.new_note.is_some() {
                app.new_note = None;
            } else if app.rename_dialog.is_some() {
                app.rename_dialog = None;
            } else if app.delete_confirm.is_some() {
                app.delete_confirm = None;
            } else if app.folder_dialog {
                app.folder_dialog = false;
            } else if app.show_settings {
                app.show_settings = false;
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
                            "new-note" => {
                                return update(app, Message::NewNoteOpen);
                            }
                            "create-daily-note" => {
                                return create_daily_note_from_command(app);
                            }
                            "insert-template" => {
                                return insert_template_in_open_note(app);
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
                    let hits = search_results(app, "");
                    app.search_panel = Some(SearchPanelState {
                        query: String::new(),
                        results: hits,
                    });
                }
            }
        }
        Message::SearchQuery(query) => {
            let hits = search_results(app, &query);
            if let Some(state) = &mut app.search_panel {
                state.query = query;
                state.results = hits;
            }
        }
        Message::SearchSelect(index) => {
            let hit_index = app
                .search_panel
                .as_ref()
                .and_then(|s| s.results.get(index))
                .map(|hit| hit.index);
            if let Some(note_index) = hit_index {
                if let Some(note) = app
                    .notes
                    .as_ref()
                    .ok()
                    .and_then(|notes| notes.get(note_index))
                {
                    let path = note.path.clone();
                    app.search_panel = None;
                    app.show_search = false;
                    open_note(app, &path);
                }
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
        Message::GraphDragStart { node, origin } => {
            app.graph_drag_active = Some(node);
            app.graph_drag_origin = Some(origin);
        }
        Message::GraphDragMove { x, y } => {
            if let (Some(node), Some((ox, oy))) = (app.graph_drag_active, app.graph_drag_origin) {
                let (px, py) = app.graph_drags.get(&node).copied().unwrap_or((0.0, 0.0));
                app.graph_drags.insert(node, (px + (x - ox), py + (y - oy)));
            }
        }
        Message::GraphDragEnd => {
            app.graph_drag_active = None;
            app.graph_drag_origin = None;
        }
        Message::GraphDragApply { node, dx, dy } => {
            let (px, py) = app.graph_drags.get(&node).copied().unwrap_or((0.0, 0.0));
            app.graph_drags.insert(node, (px + dx, py + dy));
        }
        Message::FindOpen => {
            app.find = Some(FindState {
                query: String::new(),
                replace: String::new(),
            });
        }
        Message::FindClose => {
            app.find = None;
        }
        Message::FindQuery(query) => {
            if let Some(find) = &mut app.find {
                find.query = query;
            }
        }
        Message::FindReplaceText(text) => {
            if let Some(find) = &mut app.find {
                find.replace = text;
            }
        }
        Message::FindReplaceAll => {
            if let Some(open) = &mut app.open {
                let current = open.content.text().to_string();
                let Some(find) = &app.find else {
                    return Task::none();
                };
                if !find.query.is_empty() {
                    let replaced = current.replace(&find.query, &find.replace);
                    open.content = text_editor::Content::with_text(&replaced);
                }
            }
        }
        Message::CanvasSelect(id) => {
            app.canvas_selected = Some(id);
        }
        Message::CanvasEditTitle { id, text } => {
            if let Some(card) = app.canvas.card_mut(id) {
                card.set_title(&text);
            }
        }
        Message::CanvasEditText { id, text } => {
            if let Some(card) = app.canvas.card_mut(id) {
                card.set_text(&text);
            }
        }
        Message::NewNoteOpen => {
            app.new_note = Some(NewNoteState {
                name: String::new(),
            });
        }
        Message::NewNoteClose => {
            app.new_note = None;
        }
        Message::NewNoteName(name) => {
            if let Some(state) = &mut app.new_note {
                state.name = name;
            }
        }
        Message::NewNoteCreate => {
            let Some(state) = &app.new_note else {
                return Task::none();
            };
            let name = state.name.trim().to_string();
            if name.is_empty() {
                app.error = Some("Nom de note vide.".to_string());
                return Task::none();
            }
            let rel = PathBuf::from(format!("{name}.md"));
            match create_note(&app.vault_root, &rel, "") {
                Ok(_note) => {
                    app.new_note = None;
                    return update(app, Message::Refresh);
                }
                Err(error) => {
                    app.error = Some(format!("Création impossible : {error}"));
                }
            }
        }
        Message::CanvasSetGroup { id, group } => {
            if let Some(card) = app.canvas.card_mut(id) {
                let group = group.trim();
                card.set_group(if group.is_empty() {
                    None
                } else {
                    Some(group.to_string())
                });
            }
        }
        Message::CanvasAutoLayout => {
            app.canvas.auto_layout();
        }
        Message::CanvasExportJson => {
            let json = match app.canvas.to_json() {
                Ok(json) => json,
                Err(error) => {
                    app.error = Some(format!("Export canvas impossible : {error}"));
                    return Task::none();
                }
            };
            let dir = app.vault_root.join(".obsidian");
            if let Err(error) = fs::create_dir_all(&dir) {
                app.error = Some(format!("Export canvas impossible : {error}"));
                return Task::none();
            }
            let target = dir.join("canvas.json");
            let tmp = dir.join("canvas.json.tmp");
            match fs::write(&tmp, &json).and_then(|_| fs::rename(&tmp, &target)) {
                Ok(()) => app.error = None,
                Err(error) => {
                    app.error = Some(format!("Export canvas impossible : {error}"));
                }
            }
        }
        Message::CanvasImportJson => {
            let path = app.vault_root.join(".obsidian").join("canvas.json");
            match fs::read_to_string(&path) {
                Ok(json) => match crate::vault::canvas_data::CanvasBoard::from_json(&json) {
                    Ok(board) => {
                        app.canvas = board;
                        app.error = None;
                    }
                    Err(error) => {
                        app.error = Some(format!("Import canvas impossible : {error}"));
                    }
                },
                Err(error) => {
                    app.error = Some(format!("Import canvas impossible : {error}"));
                }
            }
        }
        Message::CanvasAskRemoveCard(id) => {
            app.canvas_remove_confirm = Some(id);
        }
        Message::CanvasConfirmRemoveCard => {
            let Some(id) = app.canvas_remove_confirm.take() else {
                return Task::none();
            };
            app.canvas.remove_card(id);
            if app.canvas_selected == Some(id) {
                app.canvas_selected = None;
            }
        }
        Message::CanvasCancelRemoveCard => {
            app.canvas_remove_confirm = None;
        }
        Message::GlobalReplaceAll => {
            let Some(find) = &app.find else {
                return Task::none();
            };
            if find.query.is_empty() {
                app.error = Some("Requête de remplacement vide.".to_string());
                return Task::none();
            }
            let Ok(notes) = &app.notes else {
                return Task::none();
            };
            for note in notes {
                let Ok(source) = fs::read_to_string(app.vault_root.join(&note.path)) else {
                    continue;
                };
                if !source.contains(&find.query) {
                    continue;
                }
                let replaced = source.replace(&find.query, &find.replace);
                if let Err(error) = write_note(&app.vault_root, &note.path, &replaced) {
                    app.error = Some(format!("Remplacement global impossible : {error}"));
                    return Task::none();
                }
            }
            return update(app, Message::Refresh);
        }
        Message::PluginToggle(id) => {
            let enabled = app
                .plugins
                .info()
                .into_iter()
                .find(|info| info.id == id)
                .map(|info| !info.enabled)
                .unwrap_or(false);
            if let Err(error) = app.plugins.set_enabled(&id, enabled) {
                app.error = Some(format!("Plugin « {id} » : {error}"));
            }
        }
        Message::ReloadSnippets => {
            app.css_snippets = crate::vault::css_snippets::load_snippets(&app.vault_root);
        }
        Message::ToggleSettings => {
            app.show_settings = !app.show_settings;
            app.hotkeys = crate::vault::settings::load_hotkeys(&app.vault_root).unwrap_or_default();
        }
        Message::SetVimMode(enabled) => {
            app.settings.vim_mode = enabled;
            if let Err(error) = save_app_settings(&app.vault_root, &app.settings) {
                app.error = Some(format!("Sauvegarde des paramètres impossible : {error}"));
            }
        }
        Message::SetSpellcheck(enabled) => {
            app.settings.spellcheck = enabled;
            if let Err(error) = save_app_settings(&app.vault_root, &app.settings) {
                app.error = Some(format!("Sauvegarde des paramètres impossible : {error}"));
            }
        }
        Message::ToggleRename(path) => {
            let name = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or_default()
                .to_string();
            app.rename_dialog = Some((path, name));
        }
        Message::RenameCommit => {
            let Some((from, new_name)) = app.rename_dialog.take() else {
                return Task::none();
            };
            let new_name = new_name.trim().to_string();
            if new_name.is_empty() {
                app.error = Some("Nom de note vide.".to_string());
                return Task::none();
            }
            let to = match from.parent() {
                Some(parent) if !parent.as_os_str().is_empty() => {
                    parent.join(format!("{new_name}.md"))
                }
                _ => PathBuf::from(format!("{new_name}.md")),
            };
            match crate::vault::ops::move_note(&app.vault_root, &from, &to) {
                Ok(()) => {
                    if app.open.as_ref().is_some_and(|o| o.path == from) {
                        app.open = None;
                    }
                    return update(app, Message::Refresh);
                }
                Err(error) => {
                    app.error = Some(format!("Renommage impossible : {error}"));
                }
            }
        }
        Message::ToggleDelete(path) => {
            app.delete_confirm = Some(path);
        }
        Message::DeleteConfirm => {
            let Some(path) = app.delete_confirm.take() else {
                return Task::none();
            };
            let target = app.vault_root.join(&path);
            match fs::remove_file(&target) {
                Ok(()) => {
                    if app.open.as_ref().is_some_and(|o| o.path == path) {
                        app.open = None;
                    }
                    return update(app, Message::Refresh);
                }
                Err(error) => {
                    app.error = Some(format!("Suppression impossible : {error}"));
                }
            }
        }
        Message::FolderDialogOpen => {
            app.folder_dialog = true;
        }
        Message::FolderCreate(name) => {
            app.folder_dialog = false;
            let name = name.trim();
            if name.is_empty() {
                app.error = Some("Nom de dossier vide.".to_string());
                return Task::none();
            }
            // Confinement : refuse les chemins absolus et les traversées.
            let path = Path::new(name);
            if path.is_absolute()
                || path.components().any(|c| {
                    matches!(
                        c,
                        std::path::Component::ParentDir | std::path::Component::RootDir
                    )
                })
            {
                app.error = Some("Nom de dossier invalide (absolu ou « .. »).".to_string());
                return Task::none();
            }
            let dir = app.vault_root.join(path);
            if let Err(error) = fs::create_dir_all(&dir) {
                app.error = Some(format!("Création du dossier impossible : {error}"));
            }
        }
        Message::FolderName(name) => {
            app.folder_name = name;
        }
        Message::RenameDraft(draft) => {
            if let Some((_, name)) = &mut app.rename_dialog {
                *name = draft;
            }
        }
        Message::ToggleGraph => {
            app.show_graph = !app.show_graph;
            if app.show_graph {
                app.switcher = None;
            }
        }
        Message::GraphFilterTag(tag) => {
            app.graph_filters.tag = Some(tag);
            app.show_graph = true;
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
            app.current_theme = name.clone();
            app.settings.theme = name;
            if let Err(error) = save_app_settings(&app.vault_root, &app.settings) {
                app.error = Some(format!("Sauvegarde des paramètres impossible : {error}"));
            }
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
        Message::MarkDirty => {
            app.open_dirty = true;
        }
    }
    Task::none()
}

/// Ouvre la note au chemin relatif donné : lecture du contenu depuis le disque
/// (I/O dans `update`), mémorisation des métadonnées du dernier scan.
fn open_note(app: &mut App, path: &Path) {
    app.recent.retain(|p| p != path);
    app.recent.insert(0, path.to_path_buf());
    app.recent.truncate(20);
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
            app.open_dirty = false;
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
                Key::Character(character) if character.eq_ignore_ascii_case("f") => {
                    if modifiers.shift() {
                        return Some(Message::ToggleSearch);
                    }
                    return Some(Message::FindOpen);
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
    if query.trim().is_empty() {
        return (0..notes.len()).collect();
    }
    index
        .search(query, notes, SEARCH_LIMIT)
        .into_iter()
        .filter_map(|hit| notes.iter().position(|note| note == hit))
        .collect()
}

/// Recherche full-text : indices des notes correspondant à `query`, triés
/// Recherche avancée : parse la requête (opérateurs `path:`, `tag:`,
/// `file:`, `line:N`, `-exclu`) puis filtre les notes. Chaque résultat
/// porte un extrait autour de la première correspondance.
fn search_results(app: &App, query: &str) -> Vec<SearchHit> {
    let (Ok(notes), Some(fulltext)) = (&app.notes, &app.fulltext) else {
        return Vec::new();
    };
    if query.trim().is_empty() {
        return Vec::new();
    }
    let parsed = crate::vault::search_ops::parse_query(query);
    let keyword = first_keyword(query);
    crate::vault::search_ops::filter_notes(notes, Some(fulltext), &app.search_line_counts, &parsed)
        .into_iter()
        .take(SEARCH_PANEL_LIMIT)
        .map(|index| {
            let snippet = notes
                .get(index)
                .and_then(|note| fs::read_to_string(app.vault_root.join(&note.path)).ok())
                .map(|text| make_snippet(&text, keyword))
                .unwrap_or_default();
            SearchHit { index, snippet }
        })
        .collect()
}

/// Premier terme de mot-clé d'une requête (ignore les opérateurs `x:` et
/// les exclusions `-…`), pour construire l'extrait.
fn first_keyword(query: &str) -> &str {
    query
        .split_whitespace()
        .find(|token| !token.starts_with('-') && !token.contains(':'))
        .unwrap_or(query)
}

/// Extrait un contexte autour de la première occurrence de `query`.
fn make_snippet(content: &str, query: &str) -> String {
    let lower = content.to_lowercase();
    let q = query.to_lowercase();
    if let Some(pos) = lower.find(&q) {
        let start = char_boundary(content, pos.saturating_sub(40));
        let end = char_boundary(content, (pos + q.len() + 40).min(content.len()));
        let mut out = String::from("…");
        out.push_str(&content[start..end]);
        out.push('…');
        out
    } else {
        let end = char_boundary(content, 80.min(content.len()));
        content[..end].to_string()
    }
}

/// Crée la note journalière du jour (commande palette) puis l'ouvre.
fn create_daily_note_from_command(app: &mut App) -> Task<Message> {
    let date = today_iso();
    match crate::vault::daily_notes::create_daily_note(&app.vault_root, &date, None) {
        Ok(rel) => {
            app.error = None;
            open_note(app, &rel);
        }
        Err(error) => {
            app.error = Some(format!("Note du jour impossible : {error}"));
        }
    }
    Task::none()
}

/// Insère le premier gabarit trouvé dans `Templates` ou `templates` dans la
/// note ouverte (contenu ajouté, non sauvegardé).
fn insert_template_in_open_note(app: &mut App) -> Task<Message> {
    let dirs = [
        app.vault_root.join("Templates"),
        app.vault_root.join("templates"),
    ];
    let mut template: Option<String> = None;
    for dir in &dirs {
        let Ok(entries) = fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("md") {
                template = fs::read_to_string(&path).ok();
                if template.is_some() {
                    break;
                }
            }
        }
        if template.is_some() {
            break;
        }
    }
    let Some(template) = template else {
        app.error = Some("Aucun gabarit .md dans Templates/ ou templates/.".to_string());
        return Task::none();
    };
    if let Some(open) = &mut app.open {
        let mut combined = open.content.text().to_string();
        if !combined.ends_with('\n') {
            combined.push('\n');
        }
        combined.push_str(&template);
        open.content = text_editor::Content::with_text(&combined);
    } else {
        app.error = Some("Ouvrez d'abord une note pour insérer un modèle.".to_string());
    }
    Task::none()
}

/// Date du jour au format `AAAA-MM-JJ` (calcul civil depuis l'epoch).
fn today_iso() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let days = (secs / 86_400) as i64;
    // Algorithme civil_from_days (Howard Hinnant).
    let z = days + 719_468;
    let era = (if z >= 0 { z } else { z - 146_096 }) / 146_097;
    let doe = (z - era * 146_097) as u64; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365; // [0, 399]
    let mut y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = doy - (153 * mp + 2) / 5 + 1; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 }; // [1, 12]
    if m <= 2 {
        y += 1;
    }
    format!("{y:04}-{m:02}-{d:02}")
}

/// Ajuste `i` à la frontière de caractère UTF-8 la plus proche.
fn char_boundary(s: &str, mut i: usize) -> usize {
    while i < s.len() && !s.is_char_boundary(i) {
        i += 1;
    }
    i.min(s.len())
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
) -> (Option<FullTextIndex>, Vec<VaultTask>, HashMap<usize, usize>) {
    let Ok(notes) = notes else {
        return (None, Vec::new(), HashMap::new());
    };
    let mut items = Vec::with_capacity(notes.len());
    let mut contents = std::collections::HashMap::new();
    let mut line_counts = HashMap::new();
    for (i, note) in notes.iter().enumerate() {
        match fs::read_to_string(vault_root.join(&note.path)) {
            Ok(text) => {
                items.push((note.name.clone(), text.clone()));
                line_counts.insert(i, text.lines().count());
                contents.insert(note.path.clone(), text);
            }
            Err(_) => items.push((note.name.clone(), String::new())),
        }
    }
    let fulltext = Some(build_text_index(&items));
    let tasks = aggregate_tasks(notes, &contents);
    (fulltext, tasks, line_counts)
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

    #[test]
    fn today_iso_est_au_format_aaaa_mm_jj() {
        let date = today_iso();
        assert_eq!(date.len(), 10);
        assert_eq!(&date[4..5], "-");
        assert_eq!(&date[7..8], "-");
        assert!(date[..4].bytes().all(|b| b.is_ascii_digit()));
        assert!(date[5..7].bytes().all(|b| b.is_ascii_digit()));
        assert!(date[8..].bytes().all(|b| b.is_ascii_digit()));
    }

    #[test]
    fn select_theme_persiste_l_app_json() {
        use crate::vault::settings::load_app_settings;
        let mut dir = std::env::temp_dir();
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        dir.push(format!("obsidian-rs-settings-{unique}"));
        std::fs::create_dir_all(&dir).unwrap();

        let (mut app, _task) = boot();
        app.vault_root = dir.clone();
        app.settings.theme = "dark".to_string();
        let _ = update(&mut app, Message::SelectTheme("light".to_string()));
        let loaded = load_app_settings(&dir).unwrap();
        assert_eq!(loaded.theme, "light");
        assert_eq!(app.current_theme, "light");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
