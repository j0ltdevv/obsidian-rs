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

use iced::widget::text_editor;
use iced::{Element, Task};

use crate::vault::index::NoteIndex;
use crate::vault::meta::{remove_frontmatter_property, set_frontmatter_property};
use crate::vault::ops::write_note;
use crate::vault::{scan_vault, Note, NoteMetadata};

/// Nombre maximal de résultats affichés par la recherche rapide.
const SEARCH_LIMIT: usize = 10;

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
}

/// Point d'entrée iced : construit l'état initial en scannant le vault.
pub fn boot() -> (App, Task<Message>) {
    let (notes, index) = App::rescan(Path::new(DEMO_VAULT));
    let app = App {
        notes,
        index,
        vault_root: PathBuf::from(DEMO_VAULT),
        open: None,
        expanded: HashSet::new(),
        error: None,
        switcher: None,
        show_graph: false,
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
            app.notes = notes;
            app.index = index;
            app.error = None;
            if let Some(state) = &mut app.switcher {
                state.results = switcher_results(&app.notes, &app.index, &state.query);
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
                    Ok(()) => app.error = None,
                    Err(error) => {
                        app.error = Some(format!("Sauvegarde impossible : {error}"));
                    }
                }
            }
        }
        Message::OpenWikilink(target) => {
            // Résolution via l'index : premier match (les doublons de nom
            // sont possibles ; on prend la première correspondance).
            let resolved = app
                .index
                .as_ref()
                .and_then(|index| index.resolve(&target).first().map(|note| note.path.clone()));
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
        Message::ToggleGraph => {
            app.show_graph = !app.show_graph;
            if app.show_graph {
                app.switcher = None;
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
pub fn subscription(_state: &App) -> iced::Subscription<Message> {
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
                _ => {}
            }
        }

        if key.as_ref() == Key::Named(key::Named::Escape) {
            return Some(Message::CloseSwitcher);
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
