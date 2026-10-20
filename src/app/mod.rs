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

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use iced::widget::text_editor;
use iced::{Element, Task};

use crate::vault::index::NoteIndex;
use crate::vault::ops::write_note;
use crate::vault::{scan_vault, Note, NoteMetadata};

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

    match fs::read_to_string(app.vault_root.join(path)) {
        Ok(source) => {
            app.open = Some(OpenNote {
                path: path.to_path_buf(),
                name,
                content: text_editor::Content::with_text(&source),
                metadata,
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
