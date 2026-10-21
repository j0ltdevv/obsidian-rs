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
use crate::vault::keymap::{self as keymap, KeyAction, KeyMap};
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

/// Racine du vault de démonstration (publique, pour le binaire).
pub const DEMO_VAULT_DIR: &str = DEMO_VAULT;

/// Chemin du fichier de taille de fenêtre persistée, sous `.obsidian/`.
pub fn window_size_path(root: &Path) -> PathBuf {
    root.join(".obsidian").join("window.json")
}

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
    /// Enregistre la note ouverte sous un autre nom (copie), puis l'ouvre.
    SaveNoteAs,
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
    /// Bascule la sélection d'une note dans l'explorateur.
    ToggleSelectNote(PathBuf),
    /// Vide la sélection multiple de notes.
    ClearSelection,
    /// Déplace les notes sélectionnées vers la corbeille.
    DeleteSelected,
    /// Prend un snapshot global de tout le vault (copie de chaque note).
    TakeVaultSnapshot,
    /// Déplace le curseur de l'éditeur sur la ligne du titre cliqué (Plan).
    OutlineScroll(usize),
    /// Bascule l'affichage de la sidebar gauche.
    ToggleLeftSidebar,
    /// Change l'onglet actif de la sidebar gauche.
    SetSidebarTab(SidebarTab),
    /// Onglet de note suivant (Ctrl+Tab).
    NextTab,
    /// Onglet de note précédent (Ctrl+Shift+Tab).
    PrevTab,
    /// Ferme l'onglet de note actif (Ctrl+W).
    CloseActiveTab,
    /// Bascule entre lecture et édition (Ctrl+E) : masque ou réaffiche le
    /// panneau de prévisualisation.
    ToggleEditorMode,
    /// Bascule le mode d'édition de la note ouverte (lecture seule ↔ édition).
    ToggleEdit,
    /// Revient en arrière dans l'historique de navigation (Ctrl+Alt+←).
    NavigateBack,
    /// Avance dans l'historique de navigation (Ctrl+Alt+→).
    NavigateForward,
    /// Onglet de sidebar suivant (Ctrl+PageDown).
    NextSidebarTab,
    /// Onglet de sidebar précédent (Ctrl+PageUp).
    PrevSidebarTab,
    /// Affiche/masque l'aide des raccourcis clavier.
    ToggleShortcuts,
    /// Ouvre la palette de commandes.
    OpenCommandPalette,
    /// Ferme l'onglet d'une note ouverte (active le voisin si besoin).
    CloseTab(PathBuf),
    /// Bascule l'affichage de la sidebar droite.
    ToggleRightSidebar,
    /// Redimensionne un panneau (pane_grid).
    PaneResized(iced::widget::pane_grid::ResizeEvent),
    /// Démarre le renommage d'une pièce jointe.
    AttachmentRenameStart(PathBuf),
    /// Édite le nom (sans extension) du renommage courant.
    AttachmentRenameChange(String),
    /// Valide le renommage de la pièce jointe.
    AttachmentRenameCommit,
    /// Met une pièce jointe à la corbeille.
    AttachmentDelete(PathBuf),
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
    /// Va à l'occurrence suivante de la recherche in-note.
    FindNext,
    /// Va à l'occurrence précédente de la recherche in-note.
    FindPrev,
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
    /// Annule la dernière édition de la note ouverte.
    EditorUndo,
    /// Rétablit l'édition annulée.
    EditorRedo,
    /// Sauvegarde automatique périodique (note sale uniquement).
    Autosave,
    /// Bascule le tri de l'Explorer (alphabétique / ordre du vault).
    ToggleSort,
    /// Exporte le vault en archive ZIP.
    VaultExportZip(String),
    /// Importe une archive ZIP dans le vault.
    VaultImportZip(String),
    /// Fait cycler la profondeur du graphe local (1→2→3→1).
    GraphCycleDepth,
    /// Fait cycler le filtre « modifiées depuis » (aucun→7j→30j→90j→aucun).
    GraphCyclePeriod,
    /// Zoom du graphe (delta de molette).
    GraphZoom(f32),
    /// Pan du graphe.
    GraphPan { dx: f32, dy: f32 },
    /// Bascule l'affichage des noms de nœuds.
    GraphToggleLabels,
    /// Bascule le quadrillage.
    GraphToggleGrid,
    /// Fenêtre redimensionnée (largeur, hauteur).
    WindowResized(u32, u32),
    /// Bascule le mini-calendrier dans la sidebar.
    ToggleCalendar,
    /// Ouvre la note journalière de la date ISO `AAAA-MM-JJ` (créée si absente).
    OpenDailyDate(String),
    /// Bascule le panneau Backlinks de la note ouverte.
    ToggleBacklinks,
}
pub struct NewNoteState {
    /// Nom de la note (sans extension).
    pub name: String,
    /// Note source à copier (« enregistrer sous ») : `None` pour une note
    /// vide.
    pub copy_from: Option<PathBuf>,
}
/// Recherche/remplacement dans la note ouverte.
#[derive(Debug)]
pub struct FindState {
    /// Requête courante.
    pub query: String,
    /// Texte de remplacement.
    pub replace: String,
    /// Nombre d'occurrences de `query` dans la note ouverte.
    pub occurrences: usize,
    /// Indice (0-based) de l'occurrence courante.
    pub current: usize,
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

/// Onglet actif de la sidebar gauche (façon Obsidian).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidebarTab {
    /// Explorateur de fichiers.
    Explorer,
    /// Recherche full-text.
    Recherche,
    /// Liste des tags du vault.
    Tags,
    /// Backlinks de la note ouverte.
    Backlinks,
    /// Mini-calendrier (notes journalières).
    Calendrier,
    /// Pièces jointes du vault.
    Attachments,
}

impl SidebarTab {
    /// Ordre de navigation clavier (Ctrl+Tab fait le tour).
    pub const ORDER: [SidebarTab; 6] = [
        SidebarTab::Explorer,
        SidebarTab::Recherche,
        SidebarTab::Tags,
        SidebarTab::Backlinks,
        SidebarTab::Calendrier,
        SidebarTab::Attachments,
    ];

    /// Onglet suivant dans l'ordre (cyclique).
    pub fn next(self) -> Self {
        let i = Self::ORDER.iter().position(|t| *t == self).unwrap_or(0);
        Self::ORDER[(i + 1) % Self::ORDER.len()]
    }

    /// Onglet précédent dans l'ordre (cyclique).
    pub fn prev(self) -> Self {
        let i = Self::ORDER.iter().position(|t| *t == self).unwrap_or(0);
        Self::ORDER[(i + Self::ORDER.len() - 1) % Self::ORDER.len()]
    }
}

/// Contenu d'un panneau redimensionnable de l'espace de travail.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaneKind {
    /// Sidebar gauche (Fichiers/Recherche/Tags/Backlinks/Calendrier).
    Sidebar,
    /// Éditeur de la note active.
    Editor,
    /// Prévisualisation Markdown.
    Preview,
    /// Sidebar droite (Backlinks + Plan).
    Right,
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
    /// Onglets de notes ouvertes (chemins relatifs), dans l'ordre d'ouverture.
    pub open_tabs: Vec<PathBuf>,
    /// Indice de l'onglet actif dans `open_tabs`.
    pub open_tabs_active: usize,
    /// Tampons bruts non sauvegardés, indexés par chemin (préservés au
    /// changement d'onglet).
    pub open_buffers: HashMap<PathBuf, String>,
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
    /// Affiche l'aide des raccourcis clavier (overlay).
    pub show_shortcuts: bool,
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
    /// Table de raccourcis résolue : défauts du catalogue + personnalisations
    /// du vault (`hotkeys.json`).
    pub keymap: KeyMap,
    /// Historique de navigation (chemins relatifs), du plus ancien au plus
    /// récent.
    pub nav_history: Vec<PathBuf>,
    /// Position de la note courante dans `nav_history`.
    pub nav_index: usize,
    /// Dernières notes ouvertes (chemins), plus récente en tête.
    pub recent: Vec<PathBuf>,
    /// La note ouverte contient des modifications non enregistrées.
    pub open_dirty: bool,
    /// Mode d'édition actif pour la note ouverte (false = lecture seule).
    pub editing: bool,
    /// Carte du canvas en attente de confirmation de suppression.
    pub canvas_remove_confirm: Option<CardId>,
    /// Pile d'annulation (contenus précédents), bornée.
    pub editor_undo: Vec<String>,
    /// Pile de rétablissement.
    pub editor_redo: Vec<String>,
    /// Nombre d'éditions depuis la dernière sauvegarde (autosave).
    pub edits_since_save: u32,
    /// Cache texte par chemin pour l'index incrémental (mtime).
    pub text_cache: std::collections::HashMap<PathBuf, String>,
    /// Dernière modification observée par chemin (mtime).
    pub file_mtimes: std::collections::HashMap<PathBuf, std::time::SystemTime>,
    /// Tri alphabétique de l'Explorer (false = ordre du vault).
    pub sort_alpha: bool,
    /// Affiche la sidebar gauche (Explorer) à la façon Obsidian.
    pub show_left_sidebar: bool,
    /// Affiche la sidebar droite (Backlinks/Plan).
    pub show_right_sidebar: bool,
    /// Affiche le mini-calendrier dans la sidebar.
    pub show_calendar: bool,
    /// Affiche le panneau Backlinks de la note ouverte.
    pub show_backlinks: bool,
    /// Notes sélectionnées dans l'explorateur (sélection multiple).
    pub selected_note_paths: HashSet<PathBuf>,
    /// Renommage d'une pièce jointe : (chemin, nouveau nom sans extension).
    pub attachment_rename: Option<(PathBuf, String)>,
    /// Facteur de zoom du graphe (1.0 = 100 %).
    pub graph_zoom: f32,
    /// Décalage de pan du graphe (px).
    pub graph_pan: (f32, f32),
    /// Affiche les noms des nœuds.
    pub graph_show_labels: bool,
    /// Affiche un quadrillage.
    pub graph_show_grid: bool,
    /// Taille de fenêtre persistée (largeur, hauteur).
    pub window_size: (u32, u32),
    /// Onglet actif de la sidebar gauche.
    pub sidebar_tab: SidebarTab,
    /// État des panneaux redimensionnables (pane_grid).
    pub pane_state: iced::widget::pane_grid::State<PaneKind>,
    /// Inventaire des pièces jointes du vault.
    pub attachments: Vec<crate::vault::attachments::Attachment>,
}
/// `true` si la note doit être ignorée par l'Explorer : à l'intérieur du
/// dossier de gabarits configuré ou dans un dossier `.archive`.
fn is_ignored_note(note: &Note, templates_dir: &str) -> bool {
    let first = note
        .path
        .components()
        .next()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .unwrap_or_default();
    if !templates_dir.is_empty() && first == templates_dir {
        return true;
    }
    first == ".archive"
}

impl App {
    /// Scanne le vault donné et reconstitue notes + index, en excluant les
    /// notes ignorées (gabarits, .archive).
    fn rescan(
        vault_root: &Path,
        templates_dir: &str,
    ) -> (Result<Vec<Note>, String>, Option<NoteIndex>) {
        let mut notes = scan_vault(vault_root).map_err(|error| error.to_string());
        if let Ok(list) = &mut notes {
            list.retain(|note| !is_ignored_note(note, templates_dir));
        }
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

// Construit l'état initial des panneaux : 4 colonnes redimensionnables
// (sidebar | éditeur | prévisualisation | droite).
fn default_pane_state() -> (
    iced::widget::pane_grid::State<PaneKind>,
    Vec<iced::widget::pane_grid::Split>,
) {
    use iced::widget::pane_grid::{Axis, State};
    let (mut state, sidebar) = State::new(PaneKind::Sidebar);
    let (editor, s1) = state
        .split(Axis::Vertical, sidebar, PaneKind::Editor)
        .unwrap();
    let (preview, s2) = state
        .split(Axis::Vertical, editor, PaneKind::Preview)
        .unwrap();
    let (_right, s3) = state
        .split(Axis::Vertical, preview, PaneKind::Right)
        .unwrap();
    // Ratios raisonnables : sidebar 22 %, éditeur 62 % du reste, aperçu/droite
    // partagés à 50 %.
    state.resize(s1, 0.22);
    state.resize(s2, 0.62);
    state.resize(s3, 0.5);
    (state, vec![s1, s2, s3])
}

// Extrait les ratios des splits d'un état pane_grid, dans l'ordre du layout.
fn pane_ratios(state: &iced::widget::pane_grid::State<PaneKind>) -> Vec<f32> {
    use iced::widget::pane_grid::Node;
    fn walk(node: &Node, out: &mut Vec<f32>) {
        match node {
            Node::Split { ratio, a, b, .. } => {
                out.push(*ratio);
                walk(a, out);
                walk(b, out);
            }
            Node::Pane(_) => {}
        }
    }
    let mut out = Vec::new();
    walk(state.layout(), &mut out);
    out
}

// Chemin du fichier de disposition des panneaux.
fn pane_layout_path(root: &Path) -> PathBuf {
    root.join(".obsidian").join("pane_layout.json")
}

// Charge les ratios persistés (None si absents/invalides).
fn load_pane_ratios(root: &Path) -> Option<Vec<f32>> {
    std::fs::read_to_string(pane_layout_path(root))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
}

// Sauvegarde les ratios (écriture silencieuse en cas d'échec).
fn save_pane_ratios(root: &Path, ratios: &[f32]) {
    let path = pane_layout_path(root);
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string(ratios) {
        let _ = fs::write(path, json);
    }
}

/// Chemin du fichier d'état du vault (note ouverte persistée). Suit la même
/// convention que `window_size_path` / `pane_layout_path` : sous `.obsidian/`.
fn open_note_state_path(root: &Path) -> PathBuf {
    root.join(".obsidian").join("obsidian-rs-state.json")
}

/// État persisté du vault : chemin relatif de la note ouverte.
#[derive(serde::Serialize, serde::Deserialize)]
struct VaultState {
    open_note: Option<String>,
}

/// Charge le chemin de la note ouverte persistée (`None` si absent/invalide).
/// Une erreur de lecture ne fait jamais paniquer.
fn load_open_note(root: &Path) -> Option<PathBuf> {
    let raw = fs::read_to_string(open_note_state_path(root)).ok()?;
    serde_json::from_str::<VaultState>(&raw)
        .ok()
        .and_then(|s| s.open_note)
        .map(PathBuf::from)
}

/// Sauvegarde atomiquement (fichier temporaire + `rename`) le chemin de la
/// note ouverte. Une erreur d'écriture est ignorée silencieusement.
fn save_open_note(root: &Path, note: Option<&Path>) {
    let path = open_note_state_path(root);
    let parent = match path.parent() {
        Some(parent) => parent,
        None => return,
    };
    if fs::create_dir_all(parent).is_err() {
        return;
    }
    let state = VaultState {
        open_note: note.map(|p| p.to_string_lossy().into_owned()),
    };
    let json = match serde_json::to_string(&state) {
        Ok(json) => json,
        Err(_) => return,
    };
    let tmp = parent.join(format!("obsidian-rs-state.{}.tmp", std::process::id()));
    if fs::write(&tmp, json).is_err() {
        let _ = fs::remove_file(&tmp);
        return;
    }
    if fs::rename(&tmp, &path).is_err() {
        let _ = fs::remove_file(&tmp);
    }
}

/// Ouvre une note à la fin d'un scan si aucune n'est ouverte : la note
/// enregistrée dans l'état si elle figure toujours dans la liste, sinon la
/// première note de la liste. Ne fait rien si le vault est vide.
fn ensure_open_note(app: &mut App) {
    if app.open.is_some() {
        return;
    }
    let notes = match app.notes.as_ref() {
        Ok(notes) => notes,
        Err(_) => return,
    };
    if notes.is_empty() {
        return;
    }
    let target = load_open_note(&app.vault_root)
        .filter(|p| notes.iter().any(|n| n.path == *p))
        .unwrap_or_else(|| notes[0].path.clone());
    open_note_silent(app, &target);
}

// Point d'entrée iced : construit l'état initial en scannant le vault.
pub fn boot() -> (App, Task<Message>) {
    let settings = load_app_settings(Path::new(DEMO_VAULT)).unwrap_or_default();
    let (notes, index) = App::rescan(Path::new(DEMO_VAULT), &settings.templates_dir);
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
    let mut text_cache = std::collections::HashMap::new();
    let mut file_mtimes = std::collections::HashMap::new();
    let (fulltext, tasks, search_line_counts) = build_search_data(
        Path::new(DEMO_VAULT),
        &notes,
        &mut text_cache,
        &mut file_mtimes,
    );
    let aliases = notes.as_ref().ok().map(|notes| AliasIndex::build(notes));
    // Raccourcis : les personnalisations du vault remplacent les liaisons
    // par défaut. Un fichier illisible est ignoré (table par défaut).
    let hotkeys = crate::vault::settings::load_hotkeys(Path::new(DEMO_VAULT)).unwrap_or_default();
    let keymap = KeyMap::from_hotkeys(&hotkeys);
    let (mut pane_state, pane_splits) = default_pane_state();
    if let Some(ratios) = load_pane_ratios(Path::new(DEMO_VAULT)) {
        for (i, ratio) in ratios.into_iter().enumerate() {
            if let Some(split) = pane_splits.get(i) {
                pane_state.resize(*split, ratio.clamp(0.05, 0.95));
            }
        }
    }
    let mut app = App {
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
        commands: palette_commands(&keymap),
        command_palette: None,
        show_outline: false,
        show_shortcuts: false,
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
        hotkeys,
        keymap,
        nav_history: Vec::new(),
        nav_index: 0,
        recent: Vec::new(),
        open_dirty: false,
        editing: false,
        canvas_remove_confirm: None,
        editor_undo: Vec::new(),
        editor_redo: Vec::new(),
        edits_since_save: 0,
        text_cache,
        file_mtimes,
        sort_alpha: false,
        show_calendar: false,
        show_backlinks: false,
        selected_note_paths: HashSet::new(),
        show_left_sidebar: true,
        sidebar_tab: SidebarTab::Explorer,
        pane_state,
        attachments: Vec::new(),
        attachment_rename: None,
        graph_zoom: 1.0,
        graph_pan: (0.0, 0.0),
        graph_show_labels: true,
        graph_show_grid: false,
        window_size: (720, 480),
        open_tabs: Vec::new(),
        open_tabs_active: 0,
        open_buffers: HashMap::new(),
        show_right_sidebar: false,
    };
    ensure_open_note(&mut app);
    (app, Task::none())
}

/// Traite un message : seule source de mutation de l'état. Toute I/O a lieu
/// `true` si le verrou est périmé (PID mort sur Linux, ou âgé de plus de
/// 10 minutes).
fn lock_is_stale(lock: &Path) -> bool {
    if let Ok(content) = std::fs::read_to_string(lock) {
        if let Ok(pid) = content.trim().parse::<u32>() {
            let alive = std::path::Path::new(&format!("/proc/{pid}")).exists();
            if !alive {
                return true;
            }
        }
    }
    let stale_after = std::time::Duration::from_secs(600);
    std::fs::metadata(lock)
        .and_then(|m| m.modified())
        .map(|t| t.elapsed().map(|e| e > stale_after).unwrap_or(false))
        .unwrap_or(false)
}

/// Acquiert un verrou d'écriture (`.obsidian/lock`, PID écrit). Retourne
/// `true` si détenu, `false` si une autre instance le détient.
fn acquire_write_lock(vault_root: &Path) -> bool {
    let lock = vault_root.join(".obsidian").join("lock");
    if let Some(parent) = lock.parent() {
        if std::fs::create_dir_all(parent).is_err() {
            return false;
        }
    }
    if lock.exists() && lock_is_stale(&lock) {
        let _ = std::fs::remove_file(&lock);
    }
    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&lock)
    {
        Ok(mut file) => {
            use std::io::Write;
            let _ = writeln!(file, "{}", std::process::id());
            true
        }
        Err(_) => false,
    }
}

/// Libère le verrou d'écriture s'il était détenu.
fn release_write_lock(vault_root: &Path, held: bool) {
    if held {
        let _ = std::fs::remove_file(vault_root.join(".obsidian").join("lock"));
    }
}
// ici, jamais dans la vue.
pub fn update(app: &mut App, message: Message) -> Task<Message> {
    match message {
        Message::Refresh => {
            let (notes, index) = App::rescan(&app.vault_root, &app.settings.templates_dir);
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
            let (fulltext, tasks, search_line_counts) = build_search_data(
                &app.vault_root,
                &notes,
                &mut app.text_cache,
                &mut app.file_mtimes,
            );
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
            app.attachments =
                crate::vault::attachments::scan_attachments(&app.vault_root).unwrap_or_default();
            if let Ok(notes) = &app.notes {
                let present: HashSet<PathBuf> = notes.iter().map(|n| n.path.clone()).collect();
                app.selected_note_paths.retain(|p| present.contains(p));
                app.open_tabs.retain(|p| present.contains(p));
                app.open_buffers.retain(|p, _| present.contains(p));
                let n = app.open_tabs.len();
                app.open_tabs_active = if n == 0 {
                    0
                } else {
                    app.open_tabs_active.min(n - 1)
                };
            }
            ensure_open_note(app);
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
        Message::OpenNote(path) => {
            push_nav_history(app, &path);
            open_note(app, &path);
        }
        Message::ToggleEdit => {
            app.editing = !app.editing;
        }
        Message::EditorAction(action) => {
            if let Some(open) = &mut app.open {
                if action.is_edit() {
                    let before = open.content.text();
                    if app.editor_undo.last().map(|s| s != &before).unwrap_or(true) {
                        app.editor_undo.push(before);
                        if app.editor_undo.len() > 100 {
                            app.editor_undo.remove(0);
                        }
                    }
                    app.editor_redo.clear();
                    app.edits_since_save += 1;
                    app.open_dirty = true;
                }
                open.content.perform(action);
            }
            if app.edits_since_save >= 50 {
                return update(app, Message::Autosave);
            }
        }
        Message::EditorUndo => {
            if let Some(open) = &mut app.open {
                if let Some(prev) = app.editor_undo.pop() {
                    app.editor_redo.push(open.content.text());
                    open.content = text_editor::Content::with_text(&prev);
                    app.open_dirty = true;
                }
            }
        }
        Message::EditorRedo => {
            if let Some(open) = &mut app.open {
                if let Some(next) = app.editor_redo.pop() {
                    app.editor_undo.push(open.content.text());
                    open.content = text_editor::Content::with_text(&next);
                    app.open_dirty = true;
                }
            }
        }
        Message::Autosave => {
            if app.open_dirty && app.open.is_some() {
                return update(app, Message::SaveNote);
            }
        }
        Message::ToggleSort => {
            app.sort_alpha = !app.sort_alpha;
        }
        Message::VaultExportZip(dest) => {
            let dest = PathBuf::from(dest);
            match crate::vault::vault_archive::export_vault_zip(&app.vault_root, &dest) {
                Ok(()) => app.error = None,
                Err(error) => {
                    app.error = Some(format!("Export zip impossible : {error}"));
                }
            }
        }
        Message::VaultImportZip(source) => {
            let source = PathBuf::from(source);
            match crate::vault::vault_archive::import_vault_zip(&source, &app.vault_root) {
                Ok(count) => {
                    app.error = None;
                    if count > 0 {
                        return update(app, Message::Refresh);
                    }
                }
                Err(error) => {
                    app.error = Some(format!("Import zip impossible : {error}"));
                }
            }
        }
        Message::GraphCycleDepth => {
            app.graph_filters.local_depth = match app.graph_filters.local_depth {
                1 => 2,
                2 => 3,
                _ => 1,
            };
        }
        Message::GraphCyclePeriod => {
            let now = std::time::SystemTime::now();
            const DAY: u64 = 86_400;
            app.graph_filters.modified_after = match app.graph_filters.modified_after {
                None => now.checked_sub(std::time::Duration::from_secs(7 * DAY)),
                Some(since)
                    if since
                        <= now
                            .checked_sub(std::time::Duration::from_secs(90 * DAY))
                            .unwrap_or(std::time::UNIX_EPOCH) =>
                {
                    None
                }
                Some(since)
                    if since
                        <= now
                            .checked_sub(std::time::Duration::from_secs(30 * DAY))
                            .unwrap_or(std::time::UNIX_EPOCH) =>
                {
                    now.checked_sub(std::time::Duration::from_secs(90 * DAY))
                }
                Some(_) => now.checked_sub(std::time::Duration::from_secs(30 * DAY)),
            };
        }
        Message::GraphZoom(delta) => {
            let factor = if delta > 0.0 { 1.1 } else { 1.0 / 1.1 };
            app.graph_zoom = (app.graph_zoom * factor).clamp(0.25, 4.0);
        }
        Message::GraphPan { dx, dy } => {
            app.graph_pan.0 += dx;
            app.graph_pan.1 += dy;
        }
        Message::GraphToggleLabels => {
            app.graph_show_labels = !app.graph_show_labels;
        }
        Message::GraphToggleGrid => {
            app.graph_show_grid = !app.graph_show_grid;
        }
        Message::WindowResized(w, h) => {
            if w > 0 && h > 0 {
                app.window_size = (w, h);
                if let Ok(json) = serde_json::to_string(&app.window_size) {
                    let path = window_size_path(&app.vault_root);
                    if let Some(parent) = path.parent() {
                        let _ = fs::create_dir_all(parent);
                    }
                    // Écriture atomique : temp puis rename.
                    let tmp = path.with_extension("json.tmp");
                    if fs::write(&tmp, json).is_ok() {
                        let _ = fs::rename(&tmp, &path);
                    }
                }
            }
        }
        Message::ToggleCalendar => {
            app.show_calendar = !app.show_calendar;
        }
        Message::OpenDailyDate(date) => {
            match crate::vault::daily_notes::create_daily_note(&app.vault_root, &date, None) {
                Ok(rel) => {
                    app.error = None;
                    open_note(app, &rel);
                }
                Err(crate::vault::daily_notes::DailyNotesError::Exists(_)) => {
                    let rel = PathBuf::from(format!("{date}.md"));
                    open_note(app, &rel);
                }
                Err(error) => {
                    app.error = Some(format!("Note du jour impossible : {error}"));
                }
            }
        }
        Message::ToggleBacklinks => {
            app.show_backlinks = !app.show_backlinks;
        }
        Message::SaveNote => {
            if !acquire_write_lock(&app.vault_root) {
                app.error = Some(
                    "Vault verrouillé par une autre instance (fichier .obsidian/lock).".to_string(),
                );
                return Task::none();
            }
            if let Some(open) = &app.open {
                match write_note(&app.vault_root, &open.path, &open.content.text()) {
                    Ok(()) => {
                        app.error = None;
                        app.open_dirty = false;
                        app.edits_since_save = 0;
                        app.open_buffers.remove(&open.path);
                        let note_count = app.notes.as_ref().map(|n| n.len()).unwrap_or(0);
                        let ctx = PluginContext {
                            vault_name: DEMO_VAULT,
                            note_count,
                        };
                        let id = open.path.to_string_lossy().into_owned();
                        release_write_lock(&app.vault_root, true);
                        if let Err(error) = app.plugins.notify_change(&ctx, &[id]) {
                            app.error = Some(format!("Erreur plugin : {error}"));
                        }
                    }
                    Err(error) => {
                        release_write_lock(&app.vault_root, true);
                        app.error = Some(format!("Sauvegarde impossible : {error}"));
                    }
                }
            } else {
                release_write_lock(&app.vault_root, true);
            }
        }
        Message::SaveNoteAs => {
            let Some(open) = &app.open else {
                app.error = Some("Aucune note ouverte à enregistrer.".to_string());
                return Task::none();
            };
            let path = open.path.clone();
            let stem = path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .filter(|stem| !stem.is_empty())
                .unwrap_or("note")
                .to_string();
            // « Enregistrer sous » : le dialogue de nouvelle note est ouvert,
            // pré-rempli, et la note courante sera copiée à la validation.
            app.new_note = Some(NewNoteState {
                name: format!("{stem}-copie"),
                copy_from: Some(path),
            });
            app.error = None;
        }
        Message::OpenWikilink(target) => {
            // Résolution combinée : index des wikilinks puis alias de notes.
            let resolved = resolve_link(&app.index, &app.aliases, &target)
                .first()
                .map(|note| note.path.clone());
            match resolved {
                Some(path) => {
                    push_nav_history(app, &path);
                    open_note(app, &path);
                }
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
                push_nav_history(app, &path);
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
            } else if app.show_shortcuts {
                app.show_shortcuts = false;
            }
        }
        Message::ToggleShortcuts => {
            app.show_shortcuts = !app.show_shortcuts;
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
                        let id = command.id;
                        app.command_palette = None;
                        // Commandes du catalogue : l'action du raccourci est
                        // exécutée (la palette liste les mêmes actions que
                        // les raccourcis).
                        if let Some(action) = keymap::action_by_id(id) {
                            return update(app, action_message(action));
                        }
                        match id {
                            "toggle-theme" => {
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
                            "toggle-tasks" => {
                                return update(app, Message::ToggleTasks);
                            }
                            "create-daily-note" => {
                                return create_daily_note_from_command(app);
                            }
                            "insert-template" => {
                                return insert_template_in_open_note(app);
                            }
                            "snapshot-vault" => {
                                return update(app, Message::TakeVaultSnapshot);
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
        Message::OutlineScroll(target_line) => {
            app.show_outline = false;
            if let Some(open) = &mut app.open {
                let line_count = open.content.line_count();
                let target_line = target_line.min(line_count.saturating_sub(1));
                let current_line = open.content.cursor().position.line;
                let delta = (target_line as i32) - (current_line as i32);
                open.content
                    .perform(text_editor::Action::Scroll { lines: delta });
                open.content.move_to(text_editor::Cursor {
                    position: text_editor::Position {
                        line: target_line,
                        column: 0,
                    },
                    selection: None,
                });
            }
        }
        Message::ToggleLeftSidebar => {
            app.show_left_sidebar = !app.show_left_sidebar;
        }
        Message::ToggleRightSidebar => {
            app.show_right_sidebar = !app.show_right_sidebar;
        }
        Message::ToggleEditorMode => {
            use iced::widget::pane_grid::Axis;
            if let Some(pane) = find_pane(&app.pane_state, PaneKind::Preview) {
                // Mode lecture : le panneau de prévisualisation est retiré
                // de la grille, l'éditeur occupe toute la place.
                let _ = app.pane_state.close(pane);
            } else if let Some(editor) = find_pane(&app.pane_state, PaneKind::Editor) {
                // Mode édition : la prévisualisation revient à côté de
                // l'éditeur.
                if let Some((_pane, split)) =
                    app.pane_state
                        .split(Axis::Vertical, editor, PaneKind::Preview)
                {
                    app.pane_state.resize(split, 0.5);
                }
            }
        }
        Message::NavigateBack => {
            if app.nav_index > 0 {
                app.nav_index -= 1;
                let path = app.nav_history[app.nav_index].clone();
                open_note(app, &path);
            }
        }
        Message::NavigateForward => {
            if app.nav_index + 1 < app.nav_history.len() {
                app.nav_index += 1;
                let path = app.nav_history[app.nav_index].clone();
                open_note(app, &path);
            }
        }
        Message::PaneResized(event) => {
            app.pane_state.resize(event.split, event.ratio);
            let ratios = pane_ratios(&app.pane_state);
            save_pane_ratios(&app.vault_root, &ratios);
        }
        Message::AttachmentRenameStart(path) => {
            let base = path
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            app.attachment_rename = Some((path, base));
        }
        Message::AttachmentRenameChange(name) => {
            if let Some((_, current)) = &mut app.attachment_rename {
                *current = name;
            }
        }
        Message::AttachmentRenameCommit => {
            let Some((path, new_name)) = app.attachment_rename.take() else {
                return Task::none();
            };
            let new_name = new_name.trim().to_string();
            if new_name.is_empty() {
                app.error = Some("Nom de pièce jointe vide.".to_string());
                return Task::none();
            }
            let candidate = Path::new(&new_name);
            if candidate.is_absolute() || candidate.components().count() > 1 {
                app.error = Some("Nom invalide (chemins interdits).".to_string());
                return Task::none();
            }
            let ext = path.extension().map(|e| e.to_string_lossy().into_owned());
            let mut to = path
                .parent()
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| PathBuf::from(""));
            to.push(&new_name);
            if let Some(ext) = ext {
                to.set_extension(ext);
            }
            if !acquire_write_lock(&app.vault_root) {
                app.error = Some(
                    "Vault verrouillé par une autre instance (fichier .obsidian/lock).".to_string(),
                );
                return Task::none();
            }
            let result = fs::rename(app.vault_root.join(&path), app.vault_root.join(&to));
            release_write_lock(&app.vault_root, true);
            match result {
                Ok(()) => {
                    app.error = None;
                    return update(app, Message::Refresh);
                }
                Err(error) => {
                    app.error = Some(format!("Renommage impossible : {error}"));
                }
            }
        }
        Message::AttachmentDelete(path) => {
            if !acquire_write_lock(&app.vault_root) {
                app.error = Some(
                    "Vault verrouillé par une autre instance (fichier .obsidian/lock).".to_string(),
                );
                return Task::none();
            }
            let stamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0)
                .to_string();
            let target = app.vault_root.join(&path);
            let trash = app.vault_root.join(".trash").join(&stamp).join(&path);
            if let Some(parent) = trash.parent() {
                let _ = fs::create_dir_all(parent);
            }
            let result = fs::rename(&target, &trash);
            release_write_lock(&app.vault_root, true);
            if result.is_err() {
                app.error = Some("Suppression de la pièce jointe impossible.".to_string());
            } else {
                app.error = None;
                return update(app, Message::Refresh);
            }
        }
        Message::CloseTab(path) => {
            if let Some(i) = app.open_tabs.iter().position(|p| *p == path) {
                let was_active = app.open.as_ref().is_some_and(|o| o.path == path);
                app.open_tabs.remove(i);
                // On conserve un éventuel tampon non sauvegardé (open_buffers) :
                // rouvrir la note restaurera ces éditions plutôt que de les perdre.
                if i < app.open_tabs_active {
                    app.open_tabs_active = app.open_tabs_active.saturating_sub(1);
                }
                if was_active {
                    if app.open_tabs.is_empty() {
                        if app.open_dirty {
                            if let Some(open) = &app.open {
                                app.open_buffers
                                    .insert(open.path.clone(), open.content.text());
                            }
                        }
                        app.open = None;
                        app.editing = false;
                        save_open_note(&app.vault_root, None);
                        app.open_dirty = false;
                        app.editor_undo.clear();
                        app.editor_redo.clear();
                    } else {
                        let next = app.open_tabs[app.open_tabs_active.min(app.open_tabs.len() - 1)]
                            .clone();
                        open_note(app, &next);
                    }
                }
            }
        }
        Message::SetSidebarTab(tab) => {
            app.sidebar_tab = tab;
            if tab == SidebarTab::Recherche && app.search_panel.is_none() {
                let hits = search_results(app, "");
                app.search_panel = Some(SearchPanelState {
                    query: String::new(),
                    results: hits,
                });
            }
        }
        Message::NextTab => {
            if !app.open_tabs.is_empty() {
                let i = (app.open_tabs_active + 1) % app.open_tabs.len();
                let path = app.open_tabs[i].clone();
                open_note(app, &path);
            }
        }
        Message::PrevTab => {
            if !app.open_tabs.is_empty() {
                let i = (app.open_tabs_active + app.open_tabs.len() - 1) % app.open_tabs.len();
                let path = app.open_tabs[i].clone();
                open_note(app, &path);
            }
        }
        Message::CloseActiveTab => {
            if let Some(path) = app.open.as_ref().map(|o| o.path.clone()) {
                return update(app, Message::CloseTab(path));
            }
        }
        Message::NextSidebarTab => {
            let next = app.sidebar_tab.next();
            return update(app, Message::SetSidebarTab(next));
        }
        Message::PrevSidebarTab => {
            let prev = app.sidebar_tab.prev();
            return update(app, Message::SetSidebarTab(prev));
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
            let occurrences = app
                .open
                .as_ref()
                .map(|open| count_occurrences(&open.content.text(), ""))
                .unwrap_or(0);
            app.find = Some(FindState {
                query: String::new(),
                replace: String::new(),
                occurrences,
                current: 0,
            });
        }
        Message::FindClose => {
            app.find = None;
        }
        Message::FindQuery(query) => {
            if let Some(find) = &mut app.find {
                find.query = query;
                find.occurrences = app
                    .open
                    .as_ref()
                    .map(|open| count_occurrences(&open.content.text(), &find.query))
                    .unwrap_or(0);
                find.current = 0;
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
                    if replaced != current {
                        app.editor_undo.push(current);
                        if app.editor_undo.len() > 100 {
                            app.editor_undo.remove(0);
                        }
                        app.editor_redo.clear();
                        open.content = text_editor::Content::with_text(&replaced);
                        app.open_dirty = true;
                        app.edits_since_save = app.edits_since_save.saturating_add(1);
                    }
                }
            }
            if let Some(find) = &mut app.find {
                find.occurrences = app
                    .open
                    .as_ref()
                    .map(|open| count_occurrences(&open.content.text(), &find.query))
                    .unwrap_or(0);
                find.current = 0;
            }
        }
        Message::FindNext => {
            if let Some(find) = &mut app.find {
                if find.occurrences > 0 {
                    find.current = (find.current + 1) % find.occurrences;
                }
            }
            focus_current_occurrence(app);
        }
        Message::FindPrev => {
            if let Some(find) = &mut app.find {
                if find.occurrences > 0 {
                    find.current = (find.current + find.occurrences - 1) % find.occurrences;
                }
            }
            focus_current_occurrence(app);
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
                copy_from: None,
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
            let copy_from = state.copy_from.clone();
            let rel = PathBuf::from(format!("{name}.md"));
            // « Enregistrer sous » : le contenu de la note source est
            // recopié dans la nouvelle note.
            let content = match &copy_from {
                Some(source) => {
                    let source_content = app
                        .open
                        .as_ref()
                        .filter(|open| open.path == *source)
                        .map(|open| open.content.text())
                        .or_else(|| app.open_buffers.get(source).cloned())
                        .or_else(|| fs::read_to_string(app.vault_root.join(source)).ok());
                    match source_content {
                        Some(content) => content,
                        None => {
                            app.error = Some("Note source introuvable.".to_string());
                            return Task::none();
                        }
                    }
                }
                None => String::new(),
            };
            match create_note(&app.vault_root, &rel, &content) {
                Ok(_note) => {
                    app.new_note = None;
                    if copy_from.is_some() {
                        open_note(app, &rel);
                    }
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
            if !acquire_write_lock(&app.vault_root) {
                app.error = Some(
                    "Vault verrouillé par une autre instance (fichier .obsidian/lock).".to_string(),
                );
                return Task::none();
            }
            // Un remplacement global ne doit pas écraser des éditions non
            // sauvegardées de la note ouverte.
            if app.open_dirty {
                release_write_lock(&app.vault_root, true);
                app.error =
                    Some("Sauvegardez la note ouverte avant un remplacement global.".to_string());
                return Task::none();
            }
            // Sauvegarde de sécurité avant toute écriture (snapshots), en
            // préservant l'arborescence pour éviter les collisions de noms.
            let backup_dir = app
                .vault_root
                .join(".obsidian")
                .join("snapshots")
                .join("backup-global");
            let stamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0)
                .to_string();
            let mut open_rewritten = false;
            for note in notes {
                let Ok(source) = fs::read_to_string(app.vault_root.join(&note.path)) else {
                    continue;
                };
                if !source.contains(&find.query) {
                    continue;
                }
                let backup_path = backup_dir.join(&stamp).join(&note.path);
                if let Some(parent) = backup_path.parent() {
                    if fs::create_dir_all(parent).is_err() {
                        release_write_lock(&app.vault_root, true);
                        app.error = Some("Backup global impossible.".to_string());
                        return Task::none();
                    }
                }
                if fs::copy(app.vault_root.join(&note.path), &backup_path).is_err() {
                    release_write_lock(&app.vault_root, true);
                    app.error = Some(format!("Backup impossible pour {}", note.path.display()));
                    return Task::none();
                }
                let replaced = source.replace(&find.query, &find.replace);
                if let Err(error) = write_note(&app.vault_root, &note.path, &replaced) {
                    release_write_lock(&app.vault_root, true);
                    app.error = Some(format!("Remplacement global impossible : {error}"));
                    return Task::none();
                }
                if app.open.as_ref().is_some_and(|o| o.path == note.path) {
                    open_rewritten = true;
                }
            }
            // Recharge la note ouverte si elle a été réécrite.
            if open_rewritten {
                if let Some(open) = &mut app.open {
                    if let Ok(new_content) = fs::read_to_string(app.vault_root.join(&open.path)) {
                        open.content = text_editor::Content::with_text(&new_content);
                        open.property_drafts =
                            PropertyDrafts::from_frontmatter(&open.metadata.frontmatter);
                    }
                }
            }
            release_write_lock(&app.vault_root, true);
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
            // Les raccourcis du vault sont relus : une personnalisation
            // devient effective sans redémarrer l'application.
            app.hotkeys = crate::vault::settings::load_hotkeys(&app.vault_root).unwrap_or_default();
            app.keymap = KeyMap::from_hotkeys(&app.hotkeys);
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
            let old_stem = from
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or_default()
                .to_string();
            let new_stem = new_name.clone();
            if !acquire_write_lock(&app.vault_root) {
                app.error = Some(
                    "Vault verrouillé par une autre instance (fichier .obsidian/lock).".to_string(),
                );
                return Task::none();
            }
            // Préserve les éditions non sauvegardées de la note ouverte avant
            // de la déplacer et de fermer le tampon.
            if app.open_dirty {
                if let Some(open) = &app.open {
                    if open.path == from {
                        app.open_buffers.insert(from.clone(), open.content.text());
                    }
                }
            }
            match crate::vault::ops::move_note(&app.vault_root, &from, &to) {
                Ok(()) => {
                    if app.open.as_ref().is_some_and(|o| o.path == from) {
                        app.open = None;
                    }
                    // L'onglet et le tampon caché suivent le renommage.
                    if let Some(i) = app.open_tabs.iter().position(|p| *p == from) {
                        app.open_tabs[i] = to.clone();
                    }
                    if let Some(content) = app.open_buffers.remove(&from) {
                        app.open_buffers.insert(to.clone(), content);
                    }
                    // Met à jour les wikilinks référents (formes `[[x]]`,
                    // `[[x|alias]]`, `[[x#titre]]`, `![[x]]`).
                    if let Ok(notes) = &app.notes {
                        for note in notes {
                            if note.path == from {
                                continue;
                            }
                            let Ok(source) = fs::read_to_string(app.vault_root.join(&note.path))
                            else {
                                continue;
                            };
                            let replaced = rewrite_wikilink_target(&source, &old_stem, &new_stem);
                            if replaced != source {
                                if let Err(error) =
                                    write_note(&app.vault_root, &note.path, &replaced)
                                {
                                    release_write_lock(&app.vault_root, true);
                                    app.error =
                                        Some(format!("Mise à jour des liens impossible : {error}"));
                                    return Task::none();
                                }
                            }
                        }
                    }
                    release_write_lock(&app.vault_root, true);
                    return update(app, Message::Refresh);
                }
                Err(error) => {
                    release_write_lock(&app.vault_root, true);
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
            if !acquire_write_lock(&app.vault_root) {
                app.error = Some(
                    "Vault verrouillé par une autre instance (fichier .obsidian/lock).".to_string(),
                );
                return Task::none();
            }
            let target = app.vault_root.join(&path);
            // Corbeille du vault : `.trash/<horodatage>/<chemin>` — préserve
            // le chemin relatif et évite l'écrasement de versions précédentes.
            let stamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0)
                .to_string();
            let trash = app.vault_root.join(".trash").join(&stamp).join(&path);
            if let Some(parent) = trash.parent() {
                if fs::create_dir_all(parent).is_err() {
                    release_write_lock(&app.vault_root, true);
                    app.error = Some("Création de la corbeille impossible.".to_string());
                    return Task::none();
                }
            }
            match fs::rename(&target, &trash) {
                Ok(()) => {
                    release_write_lock(&app.vault_root, true);
                    if app.open.as_ref().is_some_and(|o| o.path == path) {
                        app.open = None;
                    }
                    app.open_tabs.retain(|p| p != &path);
                    app.open_buffers.remove(&path);
                    let n = app.open_tabs.len();
                    app.open_tabs_active = if n == 0 {
                        0
                    } else {
                        app.open_tabs_active.min(n - 1)
                    };
                    return update(app, Message::Refresh);
                }
                Err(error) => {
                    release_write_lock(&app.vault_root, true);
                    app.error = Some(format!("Mise à la corbeille impossible : {error}"));
                }
            }
        }
        Message::ToggleSelectNote(path) => {
            if !app.selected_note_paths.remove(&path) {
                app.selected_note_paths.insert(path);
            }
        }
        Message::ClearSelection => {
            app.selected_note_paths.clear();
        }
        Message::DeleteSelected => {
            if app.selected_note_paths.is_empty() {
                return Task::none();
            }
            if !acquire_write_lock(&app.vault_root) {
                app.error = Some(
                    "Vault verrouillé par une autre instance (fichier .obsidian/lock).".to_string(),
                );
                return Task::none();
            }
            let stamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0)
                .to_string();
            let mut errored = false;
            let mut remaining = app.selected_note_paths.clone();
            for path in &app.selected_note_paths {
                let target = app.vault_root.join(path);
                let trash = app.vault_root.join(".trash").join(&stamp).join(path);
                if let Some(parent) = trash.parent() {
                    if fs::create_dir_all(parent).is_err() {
                        errored = true;
                        break;
                    }
                }
                match fs::rename(&target, &trash) {
                    Ok(()) => {
                        if app.open.as_ref().is_some_and(|o| o.path == *path) {
                            app.open = None;
                        }
                        app.open_tabs.retain(|p| p != path);
                        app.open_buffers.remove(path);
                        remaining.remove(path);
                    }
                    Err(error) => {
                        app.error = Some(format!("Mise à la corbeille impossible : {error}"));
                        errored = true;
                    }
                }
            }
            app.selected_note_paths = remaining;
            let n = app.open_tabs.len();
            app.open_tabs_active = if n == 0 {
                0
            } else {
                app.open_tabs_active.min(n - 1)
            };
            release_write_lock(&app.vault_root, true);
            if errored {
                return Task::none();
            }
            return update(app, Message::Refresh);
        }
        Message::TakeVaultSnapshot => {
            if !acquire_write_lock(&app.vault_root) {
                app.error = Some(
                    "Vault verrouillé par une autre instance (fichier .obsidian/lock).".to_string(),
                );
                return Task::none();
            }
            let stamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0)
                .to_string();
            let snap_dir = app
                .vault_root
                .join(".obsidian")
                .join("snapshots")
                .join("vault")
                .join(&stamp);
            let tmp_dir = snap_dir.with_extension(format!("{stamp}.tmp"));
            let mut errored = false;
            if let Ok(notes) = &app.notes {
                for note in notes {
                    let source = app.vault_root.join(&note.path);
                    let target = tmp_dir.join(&note.path);
                    if let Some(parent) = target.parent() {
                        if fs::create_dir_all(parent).is_err() {
                            errored = true;
                            break;
                        }
                    }
                    if fs::copy(&source, &target).is_err() {
                        errored = true;
                        break;
                    }
                }
            }
            if !errored && fs::rename(&tmp_dir, &snap_dir).is_err() {
                errored = true;
            }
            if errored {
                let _ = fs::remove_dir_all(&tmp_dir);
            }
            release_write_lock(&app.vault_root, true);
            if errored {
                app.error = Some("Prendre le snapshot global a échoué.".to_string());
            } else {
                app.error = Some(format!("Snapshot global enregistré ({stamp})."));
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
            app.editing = false;
            if let Some(open) = &app.open {
                if let Some(snap) = app.history_snapshots.get(index) {
                    match restore_snapshot(&app.vault_root, &open.path, snap) {
                        Ok(source) => {
                            if let Some(open) = &mut app.open {
                                open.content = text_editor::Content::with_text(&source);
                                open.property_drafts =
                                    PropertyDrafts::from_frontmatter(&open.metadata.frontmatter);
                                app.open_dirty = true;
                                app.edits_since_save = app.edits_since_save.saturating_add(1);
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

/// Enregistre une note dans l'historique de navigation.
///
/// L'historique est borné (50 entrées) ; naviguer depuis une position
/// intermédiaire abandonne la suite de l'historique.
fn push_nav_history(app: &mut App, path: &Path) {
    app.nav_history.truncate(app.nav_index + 1);
    if app.nav_history.last().map(|last| last.as_path()) != Some(path) {
        app.nav_history.push(path.to_path_buf());
    }
    if app.nav_history.len() > 50 {
        app.nav_history.remove(0);
    }
    app.nav_index = app.nav_history.len().saturating_sub(1);
}

/// Panneau de la grille correspondant au contenu donné.
fn find_pane(
    state: &iced::widget::pane_grid::State<PaneKind>,
    kind: PaneKind,
) -> Option<iced::widget::pane_grid::Pane> {
    state
        .iter()
        .find(|(_, pane_kind)| **pane_kind == kind)
        .map(|(pane, _)| *pane)
}

/// Ouvre la note au chemin relatif donné : lecture du contenu depuis le disque
/// (I/O dans `update`), mémorisation des métadonnées du dernier scan.
fn open_note(app: &mut App, path: &Path) {
    open_note_impl(app, path, true);
}

/// Variante sans persistance de l'état (utilisée lors de l'ouverture
/// automatique au démarrage : l'état n'enregistre que les actions utilisateur).
fn open_note_silent(app: &mut App, path: &Path) {
    open_note_impl(app, path, false);
}

/// Implémentation commune de l'ouverture d'une note. `persist` contrôle
/// l'écriture de l'état (note ouverte) dans le fichier d'état du vault.
/// Ouvrir une note quitte le mode édition.
fn open_note_impl(app: &mut App, path: &Path, persist: bool) {
    app.editing = false;
    // Rouvrir la note déjà active : ne rien écraser (ré-éditions en cours).
    if app.open.as_ref().is_some_and(|open| open.path == *path) {
        app.open_tabs_active = app
            .open_tabs
            .iter()
            .position(|p| p == path)
            .unwrap_or(app.open_tabs_active);
        return;
    }
    app.recent.retain(|p| p != path);
    app.recent.insert(0, path.to_path_buf());
    app.recent.truncate(20);

    // Met de côté le tampon non sauvegardé de la note courante avant de
    // basculer vers une autre.
    if let Some(open) = &app.open {
        if open.path != *path && app.open_dirty {
            app.open_buffers
                .insert(open.path.clone(), open.content.text());
        }
    }

    // Enregistre l'onglet et le marque actif.
    if !app.open_tabs.iter().any(|p| p.as_path() == path) {
        app.open_tabs.push(path.to_path_buf());
    }
    app.open_tabs_active = app.open_tabs.iter().position(|p| p == path).unwrap_or(0);

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

    // Restaure le tampon en cache (éditions non sauvegardées) s'il existe,
    // sinon relit le contenu depuis le disque.
    if let Some(cached) = app.open_buffers.remove(path) {
        app.open = Some(OpenNote {
            path: path.to_path_buf(),
            name,
            content: text_editor::Content::with_text(&cached),
            metadata,
            property_drafts,
        });
        app.error = None;
        app.open_dirty = true;
        app.editor_undo.clear();
        app.editor_redo.clear();
        if persist {
            save_open_note(&app.vault_root, Some(path));
        }
        return;
    }

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
            app.editor_undo.clear();
            app.editor_redo.clear();
            if persist {
                save_open_note(&app.vault_root, Some(path));
            }
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
/// Raccourcis clavier globaux.
///
/// Les combinaisons configurables sont résolues via la table
/// `App::keymap`, construite à partir des valeurs par défaut du catalogue
/// et des personnalisations du vault (`.obsidian/hotkeys.json`). Les
/// quelques raccourcis propres à l'éditeur (annulation, recherche dans la
/// note, onglets de sidebar) restent fixes : ils ne font pas partie du
/// catalogue affiché dans la palette.
///
/// La vue `ui` expose des boutons équivalents ; cette subscription est
/// branchée par `main` via `.subscription(app::subscription)`.
pub fn subscription(app: &App) -> iced::Subscription<Message> {
    use iced::event::{self, Event};

    // La subscription n'accepte qu'un pointeur de fonction : la table
    // résolue est donc publiée dans le magasin partagé.
    publish_keymap(&app.keymap);

    let keys = event::listen_raw(|event, _status, _window| {
        let Event::Keyboard(iced::keyboard::Event::KeyPressed { key, modifiers, .. }) = event
        else {
            return None;
        };

        // Combinaisons du catalogue : la table résolue fait foi.
        let key_ref = key.as_ref();
        if let Some(key_name) = canonical_key_name(&key_ref) {
            let modifier_names = canonical_modifiers(modifiers);
            if let Some(action) = action_for_key(&modifier_names, &key_name) {
                return Some(action_message(action));
            }
        }

        // Raccourcis propres à l'éditeur (hors catalogue).
        editor_key_message(&key_ref, modifiers)
    });

    let windows = event::listen_raw(|event, _status, _window| {
        let Event::Window(iced::window::Event::Resized(size)) = event else {
            return None;
        };
        Some(Message::WindowResized(
            size.width as u32,
            size.height as u32,
        ))
    });

    iced::Subscription::batch([keys, windows])
}

/// Table de raccourcis partagée avec la subscription clavier.
///
/// `iced::event::listen_raw` n'accepte qu'un pointeur de fonction (aucune
/// capture) : la table résolue est donc publiée dans un magasin partagé,
/// resynchronisé à chaque appel de [`subscription`].
static KEYMAP: std::sync::LazyLock<std::sync::RwLock<KeyMap>> =
    std::sync::LazyLock::new(|| std::sync::RwLock::new(KeyMap::defaults()));

/// Accès au magasin de raccourcis partagé.
fn keymap_store() -> &'static std::sync::RwLock<KeyMap> {
    &KEYMAP
}

/// Publie la table de raccourcis résolue auprès de la subscription clavier.
fn publish_keymap(resolved: &KeyMap) {
    if let Ok(mut guard) = keymap_store().write() {
        if *guard != *resolved {
            *guard = resolved.clone();
        }
    }
}

/// Action déclenchée par la combinaison pressée, selon la table publiée.
fn action_for_key(modifiers: &[String], key: &str) -> Option<KeyAction> {
    let guard = keymap_store().read().ok()?;
    guard.action_for(modifiers, key)
}

/// Nom canonique d'une touche (`"P"`, `"Tab"`, `"Escape"`, …).
///
/// Renvoie `None` pour les touches que le catalogue ne connaît pas.
fn canonical_key_name(key: &iced::keyboard::Key<&str>) -> Option<String> {
    match key {
        iced::keyboard::Key::Character(character) => {
            let mut chars = character.chars();
            let first = chars.next()?;
            if chars.next().is_some() {
                return None;
            }
            Some(match first {
                ',' => "Comma".to_string(),
                '.' => "Period".to_string(),
                '/' => "Slash".to_string(),
                other => other.to_ascii_uppercase().to_string(),
            })
        }
        iced::keyboard::Key::Named(named) => {
            let canonical = match named {
                iced::keyboard::key::Named::Escape => "Escape".to_string(),
                iced::keyboard::key::Named::Tab => "Tab".to_string(),
                iced::keyboard::key::Named::Enter => "Enter".to_string(),
                iced::keyboard::key::Named::Space => "Space".to_string(),
                iced::keyboard::key::Named::Backspace => "Backspace".to_string(),
                iced::keyboard::key::Named::Delete => "Delete".to_string(),
                iced::keyboard::key::Named::ArrowLeft => "Left".to_string(),
                iced::keyboard::key::Named::ArrowRight => "Right".to_string(),
                iced::keyboard::key::Named::ArrowUp => "Up".to_string(),
                iced::keyboard::key::Named::ArrowDown => "Down".to_string(),
                iced::keyboard::key::Named::Home => "Home".to_string(),
                iced::keyboard::key::Named::End => "End".to_string(),
                iced::keyboard::key::Named::PageUp => "PageUp".to_string(),
                iced::keyboard::key::Named::PageDown => "PageDown".to_string(),
                // Touches de fonction F1..F12 : le nom `Debug` est déjà
                // canonique.
                other => {
                    let name = format!("{other:?}");
                    if is_function_key(&name) {
                        name
                    } else {
                        return None;
                    }
                }
            };
            Some(canonical)
        }
        iced::keyboard::Key::Unidentified => None,
    }
}

/// Vrai si le nom désigne une touche de fonction (F1..F12).
fn is_function_key(name: &str) -> bool {
    let Some(rest) = name.strip_prefix('F') else {
        return false;
    };
    rest.parse::<u8>()
        .is_ok_and(|number| (1..=12).contains(&number))
}

/// Modificateurs pressés, sous forme canonique (triée).
fn canonical_modifiers(modifiers: iced::keyboard::Modifiers) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    if modifiers.control() {
        out.push("Ctrl".to_string());
    }
    if modifiers.alt() {
        out.push("Alt".to_string());
    }
    if modifiers.shift() {
        out.push("Shift".to_string());
    }
    if modifiers.logo() {
        out.push("Super".to_string());
    }
    out.sort();
    out
}

/// Message déclenché par une action du catalogue.
fn action_message(action: KeyAction) -> Message {
    match action {
        KeyAction::OpenCommandPalette => Message::OpenCommandPalette,
        KeyAction::OpenSwitcher => Message::OpenSwitcher,
        KeyAction::NewNote => Message::NewNoteOpen,
        KeyAction::SaveNote => Message::SaveNote,
        KeyAction::SaveAs => Message::SaveNoteAs,
        KeyAction::SearchInAllFiles => Message::ToggleSearch,
        KeyAction::ToggleEditPreview => Message::ToggleEditorMode,
        KeyAction::NextTab => Message::NextTab,
        KeyAction::PrevTab => Message::PrevTab,
        KeyAction::CloseTab => Message::CloseActiveTab,
        KeyAction::OpenSettings => Message::ToggleSettings,
        KeyAction::NavigateBack => Message::NavigateBack,
        KeyAction::NavigateForward => Message::NavigateForward,
        KeyAction::ToggleLeftSidebar | KeyAction::ToggleSidebar => Message::ToggleLeftSidebar,
        KeyAction::ToggleRightSidebar => Message::ToggleRightSidebar,
        KeyAction::ShortcutHelp => Message::ToggleShortcuts,
        KeyAction::Escape => Message::Escape,
        KeyAction::ToggleGraph => Message::ToggleGraph,
    }
}

/// Raccourcis propres à l'éditeur, hors catalogue : annulation, rétablissement,
/// recherche dans la note et onglets de la barre latérale.
fn editor_key_message(
    key: &iced::keyboard::Key<&str>,
    modifiers: iced::keyboard::Modifiers,
) -> Option<Message> {
    if !modifiers.control() {
        return None;
    }
    match key {
        iced::keyboard::Key::Character(character) if character.eq_ignore_ascii_case("f") => {
            Some(Message::FindOpen)
        }
        iced::keyboard::Key::Character(character) if character.eq_ignore_ascii_case("z") => {
            if modifiers.shift() {
                Some(Message::EditorRedo)
            } else {
                Some(Message::EditorUndo)
            }
        }
        iced::keyboard::Key::Character(character) if character.eq_ignore_ascii_case("y") => {
            Some(Message::EditorRedo)
        }
        iced::keyboard::Key::Named(iced::keyboard::key::Named::PageDown) => {
            Some(Message::NextSidebarTab)
        }
        iced::keyboard::Key::Named(iced::keyboard::key::Named::PageUp) => {
            Some(Message::PrevSidebarTab)
        }
        _ => None,
    }
}

/// Commandes de la palette : le registre applicatif complété par les actions
/// du catalogue de raccourcis.
///
/// Les commandes dont l'identifiant est repris par le catalogue ne sont pas
/// dupliquées : chaque action apparaît une seule fois, avec son raccourci.
pub fn palette_commands(resolved: &KeyMap) -> Vec<Command> {
    let mut commands: Vec<Command> = default_commands()
        .into_iter()
        .filter(|command| keymap::action_by_id(command.id).is_none())
        .collect();
    for action in keymap::all_actions() {
        // Une action sans liaison résolue n'a pas de raccourci à afficher.
        if resolved.shortcut_for(*action).is_none() {
            continue;
        }
        commands.push(Command {
            id: keymap::action_id(*action),
            label: keymap::action_label(*action),
            shortcut: Some(*action),
            category: keymap::action_category(*action),
        });
    }
    commands
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

/// Réécrit les cibles de wikilinks `[[ancien…]]` / `![[ancien…]]` vers
/// `[[nouveau…]]` quand le jeton s'arrête avant `]]`, `|` ou `#`.
fn rewrite_wikilink_target(source: &str, old: &str, new: &str) -> String {
    let mut out = String::with_capacity(source.len());
    let mut rest = source;
    while let Some(lt) = rest.find('[') {
        out.push_str(&rest[..lt]);
        let after = &rest[lt..];
        let prefix = if after.starts_with("![[") {
            3
        } else if after.starts_with("[[") {
            2
        } else {
            out.push('[');
            rest = &after[1..];
            continue;
        };
        let link_tail = &after[prefix..];
        if let Some(after_old) = link_tail.strip_prefix(old) {
            if matches!(after_old.chars().next(), Some(']' | '|' | '#')) {
                out.push_str(&after[..prefix]);
                out.push_str(new);
                rest = after_old;
                continue;
            }
        }
        out.push_str(&after[..prefix]);
        rest = link_tail;
    }
    out.push_str(rest);
    out
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
    let configured = app.settings.templates_dir.trim().to_string();
    let dirs = if configured.is_empty() {
        vec![
            app.vault_root.join("Templates"),
            app.vault_root.join("templates"),
        ]
    } else {
        vec![app.vault_root.join(&configured)]
    };
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
        app.open_dirty = true;
        app.edits_since_save = app.edits_since_save.saturating_add(1);
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

/// Compte les occurrences (non chevauchantes) de `query` dans `content`.
/// Une requête vide retourne 0.
fn count_occurrences(content: &str, query: &str) -> usize {
    if query.is_empty() {
        return 0;
    }
    content.match_indices(query).count()
}

/// Position (ligne, colonne, 0-based) de la `n`-ième occurrence de `query`
/// dans `content`. Retourne `None` si `query` est vide ou si l'occurrence
/// n'existe pas.
fn occurrence_line_col(content: &str, query: &str, nth: usize) -> Option<(usize, usize)> {
    if query.is_empty() {
        return None;
    }
    let byte = content.match_indices(query).nth(nth).map(|(i, _)| i)?;
    let prefix = &content[..byte];
    let line = prefix.matches('\n').count();
    let column = prefix.len() - prefix.rfind('\n').map(|i| i + 1).unwrap_or(0);
    Some((line, column))
}

// Déplace le curseur de l'éditeur sur l'occurrence courante de la recherche
// in-note (utilisé par Suivant/Précédent).
fn focus_current_occurrence(app: &mut App) {
    let Some(find) = &app.find else { return };
    let Some(open) = &mut app.open else { return };
    if let Some((line, column)) =
        occurrence_line_col(&open.content.text(), &find.query, find.current)
    {
        open.content.move_to(text_editor::Cursor {
            position: text_editor::Position { line, column },
            selection: None,
        });
    }
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
    text_cache: &mut std::collections::HashMap<PathBuf, String>,
    file_mtimes: &mut std::collections::HashMap<PathBuf, std::time::SystemTime>,
) -> (Option<FullTextIndex>, Vec<VaultTask>, HashMap<usize, usize>) {
    let Ok(notes) = notes else {
        return (None, Vec::new(), HashMap::new());
    };
    let mut items = Vec::with_capacity(notes.len());
    let mut contents = std::collections::HashMap::new();
    let mut line_counts = HashMap::new();
    for (i, note) in notes.iter().enumerate() {
        let full = vault_root.join(&note.path);
        let mtime = fs::metadata(&full).and_then(|m| m.modified()).ok();
        let changed = match (file_mtimes.get(&note.path), mtime) {
            (Some(old), Some(new)) => old != &new,
            _ => true,
        };
        if changed {
            match fs::read_to_string(&full) {
                Ok(text) => {
                    text_cache.insert(note.path.clone(), text.clone());
                    if let Some(m) = mtime {
                        file_mtimes.insert(note.path.clone(), m);
                    }
                    items.push((note.name.clone(), text.clone()));
                    line_counts.insert(i, text.lines().count());
                    contents.insert(note.path.clone(), text);
                }
                Err(_) => {
                    items.push((note.name.clone(), String::new()));
                    line_counts.insert(i, 0);
                }
            }
        } else if let Some(text) = text_cache.get(&note.path) {
            items.push((note.name.clone(), text.clone()));
            line_counts.insert(i, text.lines().count());
            contents.insert(note.path.clone(), text.clone());
        } else {
            items.push((note.name.clone(), String::new()));
            line_counts.insert(i, 0);
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
    if !acquire_write_lock(&app.vault_root) {
        app.error =
            Some("Vault verrouillé par une autre instance (fichier .obsidian/lock).".to_string());
        return;
    }
    let yaml_value = parse_property_value(value);
    let result = set_frontmatter_property(&app.vault_root, &rel_path, key, &yaml_value);
    release_write_lock(&app.vault_root, true);
    match result {
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

// Supprime une propriété du frontmatter de la note ouverte, puis re-parse
// localement la note (métadonnées + index).
fn remove_property(app: &mut App, key: &str) {
    let Some(open) = &app.open else {
        return;
    };
    let rel_path = open.path.clone();
    if !acquire_write_lock(&app.vault_root) {
        app.error =
            Some("Vault verrouillé par une autre instance (fichier .obsidian/lock).".to_string());
        return;
    }
    let result = remove_frontmatter_property(&app.vault_root, &rel_path, key);
    release_write_lock(&app.vault_root, true);
    match result {
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

    #[test]
    fn rewrite_wikilink_target_couvre_alias_section_embed() {
        let src = "[[ancien]] [[ancien|alias]] [[ancien#titre]] ![[ancien]]";
        let out = rewrite_wikilink_target(src, "ancien", "nouveau");
        assert_eq!(
            out,
            "[[nouveau]] [[nouveau|alias]] [[nouveau#titre]] ![[nouveau]]"
        );
    }

    #[test]
    fn rewrite_wikilink_target_ignore_prefixes() {
        let out = rewrite_wikilink_target("a [[anciens]] b", "ancien", "nouveau");
        assert_eq!(out, "a [[anciens]] b");
    }

    #[test]
    fn is_ignored_note_exclut_gabarits_et_archive() {
        fn note(path: &str) -> Note {
            Note {
                path: PathBuf::from(path),
                name: "n".to_string(),
                metadata: NoteMetadata {
                    frontmatter: None,
                    frontmatter_error: None,
                    wikilinks: Vec::new(),
                    tags: Vec::new(),
                },
            }
        }
        assert!(is_ignored_note(&note("Templates/modele.md"), "Templates"));
        assert!(is_ignored_note(&note(".archive/ancienne.md"), "Templates"));
        assert!(!is_ignored_note(&note("Projets/a.md"), "Templates"));
        assert!(!is_ignored_note(&note("a.md"), ""));
    }

    #[test]
    fn rescan_filtre_les_notes_ignorees() {
        let mut dir = std::env::temp_dir();
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        dir.push(format!("obsidian-rs-ignore-{unique}"));
        std::fs::create_dir_all(dir.join("Templates")).unwrap();
        std::fs::write(dir.join("a.md"), "x").unwrap();
        std::fs::write(dir.join("Templates/modele.md"), "y").unwrap();
        let (notes, _index) = App::rescan(&dir, "Templates");
        let notes = notes.unwrap();
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].name, "a");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn count_occurrences_compte_sans_chevauchement() {
        assert_eq!(count_occurrences("aXaXa", "X"), 2);
        assert_eq!(count_occurrences("aaaa", "aa"), 2);
        assert_eq!(count_occurrences("abc", ""), 0);
        assert_eq!(count_occurrences("abc", "zz"), 0);
    }

    #[test]
    fn occurrence_line_col_localise_bonne_ligne_colonne() {
        assert_eq!(occurrence_line_col("un deux un", "un", 0), Some((0, 0)));
        assert_eq!(occurrence_line_col("un deux un", "un", 1), Some((0, 8)));
        assert_eq!(occurrence_line_col("a\nun b\nun", "un", 0), Some((1, 0)));
        assert_eq!(occurrence_line_col("a\nun b\nun", "un", 1), Some((2, 0)));
        assert_eq!(occurrence_line_col("abc", "x", 0), None);
        assert_eq!(occurrence_line_col("abc", "", 0), None);
    }

    #[test]
    fn find_next_prev_cycle_dans_les_occurrences() {
        let (mut app, _task) = boot();
        app.open = Some(OpenNote {
            path: PathBuf::from("a.md"),
            name: "a".to_string(),
            content: text_editor::Content::with_text("un deux un deux un"),
            metadata: NoteMetadata {
                frontmatter: None,
                frontmatter_error: None,
                wikilinks: Vec::new(),
                tags: Vec::new(),
            },
            property_drafts: PropertyDrafts::default(),
        });
        let _ = update(&mut app, Message::FindOpen);
        let _ = update(&mut app, Message::FindQuery("un".to_string()));
        let find = app.find.as_ref().unwrap();
        assert_eq!(find.occurrences, 3);
        assert_eq!(find.current, 0);
        let _ = update(&mut app, Message::FindNext);
        assert_eq!(app.find.as_ref().unwrap().current, 1);
        let _ = update(&mut app, Message::FindNext);
        assert_eq!(app.find.as_ref().unwrap().current, 2);
        let _ = update(&mut app, Message::FindNext);
        assert_eq!(app.find.as_ref().unwrap().current, 0);
        let _ = update(&mut app, Message::FindPrev);
        assert_eq!(app.find.as_ref().unwrap().current, 2);
        let pos = app.open.as_ref().unwrap().content.cursor().position;
        assert_eq!(pos.line, 0);
        assert_eq!(pos.column, 16); // 3e « un » : 0, 8, 16
    }

    #[test]
    fn toggle_select_ajoute_et_retire() {
        let (mut app, _task) = boot();
        let path = PathBuf::from("a.md");
        let _ = update(&mut app, Message::ToggleSelectNote(path.clone()));
        assert!(app.selected_note_paths.contains(&path));
        let _ = update(&mut app, Message::ToggleSelectNote(path.clone()));
        assert!(!app.selected_note_paths.contains(&path));
        let _ = update(&mut app, Message::ToggleSelectNote(path.clone()));
        let _ = update(&mut app, Message::ClearSelection);
        assert!(app.selected_note_paths.is_empty());
    }

    #[test]
    fn delete_selected_deplace_vers_corbeille() {
        let mut dir = std::env::temp_dir();
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        dir.push(format!("obsidian-rs-sel-{unique}"));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.md"), "a").unwrap();
        std::fs::write(dir.join("b.md"), "b").unwrap();
        let (notes, _index) = App::rescan(&dir, "Templates");
        let (mut app, _task) = boot();
        app.vault_root = dir.clone();
        app.notes = notes;
        let a = PathBuf::from("a.md");
        let b = PathBuf::from("b.md");
        let _ = update(&mut app, Message::ToggleSelectNote(a.clone()));
        let _ = update(&mut app, Message::ToggleSelectNote(b.clone()));
        let _ = update(&mut app, Message::DeleteSelected);
        assert!(!dir.join("a.md").exists());
        assert!(!dir.join("b.md").exists());
        assert!(dir.join(".trash").is_dir());
        assert!(app.selected_note_paths.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn snapshot_vault_copie_toutes_les_notes() {
        let mut dir = std::env::temp_dir();
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        dir.push(format!("obsidian-rs-snap-{unique}"));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.md"), "contenu a").unwrap();
        std::fs::write(dir.join("b.md"), "contenu b").unwrap();
        let (notes, _index) = App::rescan(&dir, "Templates");
        let (mut app, _task) = boot();
        app.vault_root = dir.clone();
        app.notes = notes;
        let _ = update(&mut app, Message::TakeVaultSnapshot);
        let snap_root = dir.join(".obsidian").join("snapshots").join("vault");
        assert!(snap_root.is_dir());
        let mut found = false;
        for entry in std::fs::read_dir(&snap_root).unwrap() {
            let stamp_dir = entry.unwrap().path();
            if stamp_dir.is_dir()
                && stamp_dir.join("a.md").exists()
                && stamp_dir.join("b.md").exists()
            {
                found = true;
            }
        }
        assert!(found, "le snapshot doit contenir a.md et b.md");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn outline_scroll_fixe_le_curseur_sur_la_ligne_du_titre() {
        let (mut app, _task) = boot();
        app.open = Some(OpenNote {
            path: PathBuf::from("a.md"),
            name: "a".to_string(),
            content: text_editor::Content::with_text("# Un\n## Deux\n# Trois"),
            metadata: NoteMetadata {
                frontmatter: None,
                frontmatter_error: None,
                wikilinks: Vec::new(),
                tags: Vec::new(),
            },
            property_drafts: PropertyDrafts::default(),
        });
        app.show_outline = true;
        let _ = update(&mut app, Message::OutlineScroll(1));
        assert!(!app.show_outline, "le Plan doit se fermer après le clic");
        let pos = app.open.as_ref().unwrap().content.cursor().position;
        assert_eq!(pos.line, 1);
        assert_eq!(pos.column, 0);
    }

    #[test]
    fn tab_recherche_initialise_le_panneau_search() {
        let (mut app, _task) = boot();
        assert!(app.search_panel.is_none());
        let _ = update(&mut app, Message::SetSidebarTab(SidebarTab::Recherche));
        assert_eq!(app.sidebar_tab, SidebarTab::Recherche);
        assert!(app.search_panel.is_some());
    }

    #[test]
    fn ouvrir_deux_notes_enregistre_onglets() {
        let mut dir = std::env::temp_dir();
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        dir.push(format!("obsidian-rs-tabs-{unique}"));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.md"), "a").unwrap();
        std::fs::write(dir.join("b.md"), "b").unwrap();
        let (notes, _index) = App::rescan(&dir, "Templates");
        let (mut app, _task) = boot();
        app.vault_root = dir.clone();
        app.notes = notes;
        app.open = None;
        app.open_tabs.clear();
        app.open_tabs_active = 0;
        let a = PathBuf::from("a.md");
        let b = PathBuf::from("b.md");
        let _ = update(&mut app, Message::OpenNote(a.clone()));
        let _ = update(&mut app, Message::OpenNote(b.clone()));
        assert_eq!(app.open_tabs, vec![a, b]);
        assert_eq!(app.open_tabs_active, 1);
        assert_eq!(app.open.as_ref().unwrap().name, "b");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn changer_donglet_preserve_le_tampon_non_sauvegardé() {
        let mut dir = std::env::temp_dir();
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        dir.push(format!("obsidian-rs-tabs-dirty-{unique}"));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.md"), "original a").unwrap();
        std::fs::write(dir.join("b.md"), "original b").unwrap();
        let (notes, _index) = App::rescan(&dir, "Templates");
        let (mut app, _task) = boot();
        app.vault_root = dir.clone();
        app.notes = notes;
        let a = PathBuf::from("a.md");
        let b = PathBuf::from("b.md");
        let _ = update(&mut app, Message::OpenNote(a.clone()));
        app.open.as_mut().unwrap().content = text_editor::Content::with_text("édité a");
        let _ = update(&mut app, Message::MarkDirty);
        let _ = update(&mut app, Message::OpenNote(b.clone()));
        let _ = update(&mut app, Message::OpenNote(a.clone()));
        assert!(
            app.open_dirty,
            "le tampon édité doit être restauré comme sale"
        );
        assert_eq!(app.open.as_ref().unwrap().content.text(), "édité a");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn fermer_longlet_actif_active_le_voisin() {
        let mut dir = std::env::temp_dir();
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        dir.push(format!("obsidian-rs-tabs-close-{unique}"));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.md"), "a").unwrap();
        std::fs::write(dir.join("b.md"), "b").unwrap();
        let (notes, _index) = App::rescan(&dir, "Templates");
        let (mut app, _task) = boot();
        app.vault_root = dir.clone();
        app.notes = notes;
        app.open = None;
        app.open_tabs.clear();
        app.open_tabs_active = 0;
        let a = PathBuf::from("a.md");
        let b = PathBuf::from("b.md");
        let _ = update(&mut app, Message::OpenNote(a.clone()));
        let _ = update(&mut app, Message::OpenNote(b.clone()));
        let _ = update(&mut app, Message::CloseTab(b.clone()));
        assert_eq!(app.open_tabs, vec![a.clone()]);
        assert_eq!(app.open.as_ref().unwrap().name, "a");
        let _ = update(&mut app, Message::CloseTab(a.clone()));
        assert!(app.open_tabs.is_empty());
        assert!(app.open.is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn renommage_met_a_jour_onglet_et_tampon() {
        let mut dir = std::env::temp_dir();
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        dir.push(format!("obsidian-rs-tabs-rename-{unique}"));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.md"), "a").unwrap();
        std::fs::write(dir.join("b.md"), "b").unwrap();
        let (notes, _index) = App::rescan(&dir, "Templates");
        let (mut app, _task) = boot();
        app.vault_root = dir.clone();
        app.notes = notes;
        let a = PathBuf::from("a.md");
        let b = PathBuf::from("b.md");
        let _ = update(&mut app, Message::OpenNote(a.clone()));
        app.open.as_mut().unwrap().content = text_editor::Content::with_text("édité");
        let _ = update(&mut app, Message::MarkDirty);
        let _ = update(&mut app, Message::OpenNote(b.clone()));
        app.rename_dialog = Some((a.clone(), "a2".to_string()));
        let _ = update(&mut app, Message::RenameCommit);
        let a2 = PathBuf::from("a2.md");
        assert!(app.open_tabs.contains(&a2));
        assert!(!app.open_tabs.contains(&a));
        assert!(app.open_buffers.contains_key(&a2));
        assert!(!app.open_buffers.contains_key(&a));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn suppression_retire_longlet_et_le_tampon() {
        let mut dir = std::env::temp_dir();
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        dir.push(format!("obsidian-rs-tabs-del-{unique}"));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.md"), "a").unwrap();
        let (notes, _index) = App::rescan(&dir, "Templates");
        let (mut app, _task) = boot();
        app.vault_root = dir.clone();
        app.notes = notes;
        app.open = None;
        app.open_tabs.clear();
        app.open_tabs_active = 0;
        let a = PathBuf::from("a.md");
        let _ = update(&mut app, Message::OpenNote(a.clone()));
        let _ = update(&mut app, Message::ToggleDelete(a.clone()));
        let _ = update(&mut app, Message::DeleteConfirm);
        assert!(app.open_tabs.is_empty());
        assert!(app.open.is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn sauvegarde_vide_le_tampon_cache() {
        let mut dir = std::env::temp_dir();
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        dir.push(format!("obsidian-rs-tabs-save-{unique}"));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.md"), "a").unwrap();
        std::fs::write(dir.join("b.md"), "b").unwrap();
        let (notes, _index) = App::rescan(&dir, "Templates");
        let (mut app, _task) = boot();
        app.vault_root = dir.clone();
        app.notes = notes;
        let a = PathBuf::from("a.md");
        let b = PathBuf::from("b.md");
        let _ = update(&mut app, Message::OpenNote(a.clone()));
        app.open.as_mut().unwrap().content = text_editor::Content::with_text("édité a");
        let _ = update(&mut app, Message::MarkDirty);
        let _ = update(&mut app, Message::OpenNote(b.clone()));
        assert!(app.open_buffers.contains_key(&a));
        let _ = update(&mut app, Message::OpenNote(a.clone()));
        let _ = update(&mut app, Message::SaveNote);
        assert!(!app.open_buffers.contains_key(&a));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn pane_defaut_contient_quatre_panneaux() {
        let (state, splits) = default_pane_state();
        assert_eq!(state.len(), 4);
        assert_eq!(splits.len(), 3);
        let ratios = pane_ratios(&state);
        assert_eq!(ratios.len(), 3);
        assert!(ratios[0] < 0.4, "sidebar ne doit pas être géante");
    }

    #[test]
    fn verrou_ecriture_exclusif() {
        let mut dir = std::env::temp_dir();
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        dir.push(format!("obsidian-rs-lock-{unique}"));
        std::fs::create_dir_all(&dir).unwrap();
        assert!(
            acquire_write_lock(&dir),
            "première acquisition doit réussir"
        );
        assert!(
            !acquire_write_lock(&dir),
            "seconde acquisition doit échouer (verrou tenu)"
        );
        release_write_lock(&dir, true);
        assert!(acquire_write_lock(&dir), "réacquisition après libération");
        release_write_lock(&dir, true);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    #[ignore = "benchmark manuel : scan d'un gros vault"]
    fn benchmark_scan_gros_vault() {
        let mut dir = std::env::temp_dir();
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        dir.push(format!("obsidian-rs-bench-{unique}"));
        std::fs::create_dir_all(&dir).unwrap();
        for i in 0..10_000 {
            std::fs::write(dir.join(format!("note_{i}.md")), "# T\nTexte\n").unwrap();
        }
        let t0 = std::time::Instant::now();
        let (notes, _index) = App::rescan(&dir, "Templates");
        let elapsed = t0.elapsed();
        assert!(notes.is_ok());
        eprintln!("scan 10k notes : {:?}", elapsed);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn rouvrir_meme_note_preserve_les_editions() {
        let mut dir = std::env::temp_dir();
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        dir.push(format!("obsidian-rs-reopen-{unique}"));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.md"), "a").unwrap();
        let (notes, _index) = App::rescan(&dir, "Templates");
        let (mut app, _task) = boot();
        app.vault_root = dir.clone();
        app.notes = notes;
        let a = PathBuf::from("a.md");
        let _ = update(&mut app, Message::OpenNote(a.clone()));
        app.open.as_mut().unwrap().content = text_editor::Content::with_text("édité a");
        let _ = update(&mut app, Message::MarkDirty);
        // Recliquer l'onglet actif ne doit pas écraser les éditions.
        let _ = update(&mut app, Message::OpenNote(a.clone()));
        assert_eq!(app.open.as_ref().unwrap().content.text(), "édité a");
        assert!(app.open_dirty);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn aide_raccourcis_s_ouvre_et_se_ferme() {
        let (mut app, _task) = boot();
        assert!(!app.show_shortcuts);
        let _ = update(&mut app, Message::ToggleShortcuts);
        assert!(app.show_shortcuts);
        let _ = update(&mut app, Message::ToggleShortcuts);
        assert!(!app.show_shortcuts);

        // Échap referme l'aide.
        let _ = update(&mut app, Message::ToggleShortcuts);
        let _ = update(&mut app, Message::Escape);
        assert!(!app.show_shortcuts);
    }

    #[test]
    fn sidebar_tab_cycle_avant_arriere() {
        use SidebarTab::{Attachments, Backlinks, Calendrier, Explorer, Recherche, Tags};
        assert_eq!(Explorer.next(), Recherche);
        assert_eq!(Recherche.next(), Tags);
        assert_eq!(Tags.next(), Backlinks);
        assert_eq!(Backlinks.next(), Calendrier);
        assert_eq!(Calendrier.next(), Attachments);
        assert_eq!(Attachments.next(), Explorer);
        // Retour : l'inverse exact.
        assert_eq!(Explorer.prev(), Attachments);
        assert_eq!(Recherche.prev(), Explorer);
        for tab in SidebarTab::ORDER {
            assert_eq!(tab.next().prev(), tab);
        }
    }

    #[test]
    fn navigation_onglets_notes_cycle() {
        let mut dir = std::env::temp_dir();
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        dir.push(format!("obsidian-rs-keys-{unique}"));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.md"), "a").unwrap();
        std::fs::write(dir.join("b.md"), "b").unwrap();
        let (notes, _index) = App::rescan(&dir, "Templates");
        let (mut app, _task) = boot();
        app.vault_root = dir.clone();
        app.notes = notes;
        app.open = None;
        app.open_tabs.clear();
        app.open_tabs_active = 0;
        let a = PathBuf::from("a.md");
        let b = PathBuf::from("b.md");
        let _ = update(&mut app, Message::OpenNote(a.clone()));
        let _ = update(&mut app, Message::OpenNote(b.clone()));
        assert_eq!(app.open.as_ref().unwrap().path, b);

        // Ctrl+Tab -> revient sur a (cyclique).
        let _ = update(&mut app, Message::NextTab);
        assert_eq!(app.open.as_ref().unwrap().path, a);
        // Ctrl+Shift+Tab -> repart sur b.
        let _ = update(&mut app, Message::PrevTab);
        assert_eq!(app.open.as_ref().unwrap().path, b);
        // Ctrl+W ferme l'onglet actif et active le voisin.
        let _ = update(&mut app, Message::CloseActiveTab);
        assert_eq!(app.open_tabs, vec![a.clone()]);
        assert_eq!(app.open.as_ref().unwrap().path, a);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Vérifie que « revenir en arrière » et « avancer » suivent
    /// l'historique de navigation.
    #[test]
    fn la_navigation_arriere_et_avant_suivent_l_historique() {
        let mut dir = std::env::temp_dir();
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        dir.push(format!("obsidian-rs-nav-{unique}"));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.md"), "a").unwrap();
        std::fs::write(dir.join("b.md"), "b").unwrap();
        let (notes, _index) = App::rescan(&dir, "Templates");
        let (mut app, _task) = boot();
        app.vault_root = dir.clone();
        app.notes = notes;
        let a = PathBuf::from("a.md");
        let b = PathBuf::from("b.md");
        let _ = update(&mut app, Message::OpenNote(a.clone()));
        let _ = update(&mut app, Message::OpenNote(b.clone()));
        assert_eq!(app.open.as_ref().unwrap().path, b);

        // Retour : la note précédente est rouverte.
        let _ = update(&mut app, Message::NavigateBack);
        assert_eq!(app.open.as_ref().unwrap().path, a);
        // Avance : on revient à la note courante.
        let _ = update(&mut app, Message::NavigateForward);
        assert_eq!(app.open.as_ref().unwrap().path, b);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Vérifie que « enregistrer sous » duplique la note ouverte.
    #[test]
    fn enregistrer_sous_cree_une_copie_de_la_note() {
        let mut dir = std::env::temp_dir();
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        dir.push(format!("obsidian-rs-saveas-{unique}"));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.md"), "contenu").unwrap();
        let (notes, _index) = App::rescan(&dir, "Templates");
        let (mut app, _task) = boot();
        app.vault_root = dir.clone();
        app.notes = notes;
        let _ = update(&mut app, Message::OpenNote(PathBuf::from("a.md")));

        // Le dialogue de nouvelle note s'ouvre, pré-rempli.
        let _ = update(&mut app, Message::SaveNoteAs);
        let state = app.new_note.as_ref().expect("dialogue ouvert");
        assert_eq!(state.name, "a-copie");
        assert_eq!(state.copy_from, Some(PathBuf::from("a.md")));

        // La validation crée la copie avec le contenu de la note source.
        let _ = update(&mut app, Message::NewNoteCreate);
        let copie = dir.join("a-copie.md");
        assert!(copie.exists());
        assert_eq!(std::fs::read_to_string(&copie).unwrap(), "contenu");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Vérifie que le mode lecture retire le panneau de prévisualisation.
    #[test]
    fn le_mode_lecture_retire_le_panneau_de_previsualisation() {
        let (mut app, _task) = boot();
        assert!(find_pane(&app.pane_state, PaneKind::Preview).is_some());
        let _ = update(&mut app, Message::ToggleEditorMode);
        assert!(find_pane(&app.pane_state, PaneKind::Preview).is_none());
        // Le mode édition le réaffiche.
        let _ = update(&mut app, Message::ToggleEditorMode);
        assert!(find_pane(&app.pane_state, PaneKind::Preview).is_some());
    }

    /// Vérifie que la palette exécute l'action de la commande choisie.
    #[test]
    fn la_palette_execute_l_action_de_la_commande() {
        let (mut app, _task) = boot();
        let _ = update(&mut app, Message::OpenCommandPalette);
        assert!(app.command_palette.is_some());
        let index = app
            .commands
            .iter()
            .position(|command| command.id == "toggle-graph")
            .expect("commande « toggle-graph »");
        let _ = update(&mut app, Message::CommandPaletteSelect(index));
        assert!(app.show_graph);
        assert!(app.command_palette.is_none());
    }

    /// Vérifie que la palette liste chaque action du catalogue une fois.
    #[test]
    fn la_palette_liste_chaque_action_du_catalogue() {
        let (app, _task) = boot();
        for action in crate::vault::keymap::all_actions() {
            let count = app
                .commands
                .iter()
                .filter(|command| command.shortcut == Some(*action))
                .count();
            assert_eq!(count, 1, "l'action {action:?} apparaît {count} fois");
        }
    }

    #[test]
    fn benchmark_recherche_full_text_p95() {
        let mut dir = std::env::temp_dir();
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        dir.push(format!("obsidian-rs-bench-ft-{unique}"));
        std::fs::create_dir_all(&dir).unwrap();
        let mut queries = Vec::new();
        for i in 0..5_000 {
            let content =
                format!("# Note {i}\nle contenu de la note {i} pour la recherche full-text.");
            std::fs::write(dir.join(format!("note_{i}.md")), content).unwrap();
            queries.push(format!("contenu note {i}"));
        }
        let (notes, _index) = App::rescan(&dir, "Templates");
        let notes = notes.expect("scan");
        let items: Vec<(String, String)> = notes
            .iter()
            .map(|n| {
                (
                    n.name.clone(),
                    std::fs::read_to_string(dir.join(&n.path)).unwrap(),
                )
            })
            .collect();
        let index = crate::vault::search::build_text_index(&items);
        let mut times = Vec::new();
        for q in &queries {
            let t0 = std::time::Instant::now();
            index.search_text(q, 20);
            times.push(t0.elapsed());
        }
        times.sort();
        let p95 = times[(times.len() as f64 * 0.95) as usize];
        eprintln!("recherche full-text : p95 = {:?}", p95);
        let _ = std::fs::remove_dir_all(&dir);
    }

    // Construit une app isolée dont le vault pointe vers un répertoire
    // temporaire contenant les notes `noms` (fichiers `.md` créés). N'écrit
    // jamais dans le `demo-vault` du dépôt.
    fn app_temp_vault(noms: &[&str]) -> (App, std::path::PathBuf) {
        let mut dir = std::env::temp_dir();
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        dir.push(format!("obsidian-rs-test-{}-{unique}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for nom in noms {
            std::fs::write(dir.join(format!("{nom}.md")), format!("# {nom}\n")).unwrap();
        }
        let (notes, _index) = App::rescan(&dir, "Templates");
        let (mut app, _task) = boot();
        app.vault_root = dir.clone();
        app.notes = notes;
        app.open = None;
        app.open_tabs.clear();
        app.open_tabs_active = 0;
        (app, dir)
    }

    #[test]
    fn scan_termine_ouvre_la_premiere_note_sans_etat() {
        let (mut app, dir) = app_temp_vault(&["a", "b"]);
        let premiere = app.notes.as_ref().unwrap()[0].path.clone();
        ensure_open_note(&mut app);
        assert_eq!(
            app.open.as_ref().map(|o| o.path.clone()),
            Some(premiere),
            "une note est ouverte à la fin du scan"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn scan_termine_rouvre_la_note_enregistree() {
        let (mut app, dir) = app_temp_vault(&["a", "b"]);
        save_open_note(&app.vault_root, Some(std::path::Path::new("b.md")));
        ensure_open_note(&mut app);
        assert_eq!(
            app.open.as_ref().map(|o| o.path.clone()),
            Some(PathBuf::from("b.md")),
            "la note enregistrée prime sur la première"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn etat_corrompu_ou_illisible_ne_panique_pas() {
        // État corrompu (JSON invalide) : on retombe sur la première note.
        let (mut app, dir) = app_temp_vault(&["a", "b"]);
        let premiere = app.notes.as_ref().unwrap()[0].path.clone();
        let state_path = open_note_state_path(&app.vault_root);
        std::fs::create_dir_all(state_path.parent().unwrap()).unwrap();
        std::fs::write(&state_path, "pas du json {{{").unwrap();
        ensure_open_note(&mut app);
        assert_eq!(
            app.open.as_ref().unwrap().path,
            premiere,
            "état corrompu : retombe sur la première note"
        );
        let _ = std::fs::remove_dir_all(&dir);

        // État illisible (le chemin de l'état est un dossier) : idem, sans
        // provoquer de panique.
        let (mut app, dir) = app_temp_vault(&["a"]);
        let premiere = app.notes.as_ref().unwrap()[0].path.clone();
        let state_path = open_note_state_path(&app.vault_root);
        std::fs::create_dir_all(&state_path).unwrap();
        ensure_open_note(&mut app);
        assert_eq!(app.open.as_ref().unwrap().path, premiere);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn toggle_edit_bascule_et_open_note_reinitialise() {
        let (mut app, dir) = app_temp_vault(&["a"]);
        assert!(!app.editing, "lecture seule au démarrage");
        let _ = update(&mut app, Message::ToggleEdit);
        assert!(app.editing);
        let _ = update(&mut app, Message::ToggleEdit);
        assert!(!app.editing);
        let _ = update(&mut app, Message::ToggleEdit);
        assert!(app.editing);
        let _ = update(&mut app, Message::OpenNote(PathBuf::from("a.md")));
        assert!(!app.editing, "OpenNote remet editing à false");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
