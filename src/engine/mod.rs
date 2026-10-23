//! Moteur de vault exposé comme service HTTP JSON local.
//!
//! Ce module rend le cœur de `crate::vault` accessible à un front end non
//! Rust (le moteur de rendu d'Obsidian, ou Ignis) via une petite API HTTP
//! JSON servie sur `127.0.0.1` uniquement.
//!
//! # Principes
//!
//! - **Aucune dépendance supplémentaire** : le protocole HTTP/1.1 est
//!   implémenté à la main sur `std::net::TcpListener` (voir
//!   [`crate::engine::http`]).
//! - **Aucune duplication du parsing** : le frontmatter, les tags et les
//!   wikilinks sont analysés par `crate::vault` (`scan_vault`, `NoteIndex`,
//!   `ops::write_note`, `search_ops::filter_notes`). Le moteur ne fait que
//!   projeter ces résultats en JSON.
//! - **Confinement des chemins** : tout chemin fourni par un client est
//!   relatif, sans `..`, et vérifié comme restant sous la racine du vault
//!   (un lien symbolique sortant du vault est refusé).
//!
//! # Endpoints
//!
//! | Méthode | Chemin | Réponse |
//! |---|---|---|
//! | `GET` | `/health` | `{"ok":true,"root":"…","notes":N}` |
//! | `GET` | `/files` | `{"files":[{"path":…,"size":N,"mtime":N}]}` |
//! | `GET` | `/file?path=…` | `{"path":…,"content":"…"}` |
//! | `PUT` | `/file?path=…` | `{"path":…,"bytes":N}` |
//! | `GET` | `/metadata?path=…` | `{"path":…,"frontmatter":{…},"tags":[…],"links":[…],"unresolved":[…],"properties":{…}}` |
//! | `GET` | `/backlinks?path=…` | `{"path":…,"backlinks":[{"from":…,"count":N}]}` |
//! | `GET` | `/search?q=…&limit=N` | `{"results":[{"path":…,"line":N,"text":…}]}` |
//! | `GET` | `/graph` | `{"nodes":[…],"edges":[…]}` |
//! | `GET` | `/tags` | `{"tags":[{"tag":…,"count":N}]}` |
//! | `GET` | `/outline?path=…` | `{"path":…,"outline":[{"level":N,"title":…,"line":N}]}` |
//! | `GET` | `/tasks[?path=…]` | `{"tasks":[{"path":…,"line":N,"text":…,"done":bool}]}` |
//! | `GET` | `/aliases` | `{"aliases":[{"alias":…,"path":…}]}` |
//! | `POST` | `/sync` | `{"ok":true,"value":…}` ou `{"ok":false,"error":"…"}` |
//!
//! Les numéros de ligne exposés en JSON (`/outline`, `/tasks`, `/search`) sont
//! **1-based**, comme dans un éditeur ; le vault manipule des indices 0-based.
//!
//! Les erreurs renvoient un statut HTTP et `{"error":"message"}` ; les
//! messages sont en français.
//!
//! # Pont synchrone
//!
//! `POST /sync` est le pont utilisé par le renderer d'Obsidian, qui appelle
//! `ipcRenderer.sendSync` et des fonctions `fs` synchrones : la réponse doit
//! revenir dans la pile d'appel, ce que l'IPC asynchrone ne permet pas. Voir
//! [`crate::engine::sync`] pour le contrat des opérations.

pub mod api;
pub mod http;
pub mod json;
pub mod paths;
pub mod sync;

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use crate::vault::aliases::extract_aliases;
use crate::vault::index::NoteIndex;
use crate::vault::note::Note;
use crate::vault::ops::{create_note, write_note};
use crate::vault::scan_vault;
use crate::vault::search::{build_text_index, FullTextIndex};
use crate::vault::search_ops::{filter_notes, parse_query};
use crate::vault::tasks::{extract_tasks, TaskStatus};

/// Port d'écoute par défaut du moteur.
pub const DEFAULT_PORT: u16 = 27123;

/// Variable d'environnement lue pour le port d'écoute.
pub const PORT_ENV: &str = "OBSIDIAN_ENGINE_PORT";

/// Erreurs du moteur.
#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    /// La racine du vault n'existe pas ou n'est pas un dossier.
    #[error("racine du vault introuvable : {0}")]
    RootMissing(PathBuf),
    /// Le scan du vault a échoué.
    #[error("scan du vault impossible : {0}")]
    Scan(String),
    /// Chemin refusé (vide, absolu, `..` ou hors du vault).
    #[error(transparent)]
    Path(#[from] paths::PathError),
    /// Opération de vault refusée (écriture ou création impossible).
    #[error("opération de vault refusée : {0}")]
    Vault(String),
    /// Erreur d'E/S.
    #[error("erreur d'E/S : {0}")]
    Io(#[from] std::io::Error),
}

/// Données de recherche : index full-text, contenus bruts et nombre de
/// lignes par note (indice de note → lignes).
struct SearchData {
    /// Contenu texte de chaque note indexée.
    contents: HashMap<PathBuf, String>,
    /// Nombre de lignes par note (indice dans `Engine::notes`).
    line_counts: HashMap<usize, usize>,
    /// Index full-text construit sur `(titre, texte)`.
    fulltext: FullTextIndex,
}

/// Résultat de recherche exposé par l'API JSON.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchHit {
    /// Chemin relatif de la note dans le vault.
    pub path: String,
    /// Numéro de ligne (1-based) de la première correspondance ; `0` quand la
    /// note correspond sans qu'une ligne précise ne soit identifiée (par
    /// exemple une recherche par tag ou par chemin).
    pub line: usize,
    /// Texte de la ligne correspondante (vide si `line` vaut `0`).
    pub text: String,
}

/// Nœud du graphe exposé par l'API JSON.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphNode {
    /// Chemin relatif de la note dans le vault.
    pub path: String,
    /// Tags de la note (fusion corps + frontmatter).
    pub tags: Vec<String>,
    /// Nombre de wikilinks résolus vers d'autres notes.
    pub links: usize,
}

/// Arête orientée du graphe exposée par l'API JSON.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphEdge {
    /// Chemin relatif de la note source.
    pub from: String,
    /// Chemin relatif de la note cible.
    pub to: String,
}

/// Tag du vault et nombre de notes qui le portent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagCount {
    /// Tag tel qu'écrit dans les notes (sans `#`).
    pub tag: String,
    /// Nombre de notes portant ce tag (au plus une fois par note).
    pub count: usize,
}

/// Tâche (case à cocher) exposée par l'API JSON.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskEntry {
    /// Chemin relatif de la note d'origine.
    pub path: PathBuf,
    /// Ligne (0-based) de la case dans la note.
    pub line: usize,
    /// Texte de la tâche, après la case.
    pub text: String,
    /// `true` si la case est cochée.
    pub done: bool,
}

/// Alias déclaré dans le frontmatter d'une note.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AliasEntry {
    /// Alias tel que déclaré (trimé, casse conservée).
    pub alias: String,
    /// Chemin relatif de la note cible.
    pub path: PathBuf,
}

/// Le moteur : racine du vault, notes scannées et index de résolution.
pub struct Engine {
    /// Racine canonique du vault (chemin absolu, sans lien symbolique).
    root: PathBuf,
    /// Notes du vault, dans l'ordre du scan.
    notes: Vec<Note>,
    /// Index de résolution (wikilinks, backlinks) construit par le vault.
    index: NoteIndex,
    /// Données de recherche, construites à la demande.
    search: Option<SearchData>,
}

impl Engine {
    /// Ouvre un vault : scanne les notes, construit l'index de résolution.
    ///
    /// La racine est canonisée : elle devient le point de référence absolu
    /// pour le confinement des chemins.
    pub fn open(root: &Path) -> Result<Self, EngineError> {
        let canonical = match fs::canonicalize(root) {
            Ok(canonical) => canonical,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                return Err(EngineError::RootMissing(root.to_path_buf()))
            }
            Err(err) => return Err(EngineError::Io(err)),
        };
        if !canonical.is_dir() {
            return Err(EngineError::RootMissing(root.to_path_buf()));
        }
        let mut engine = Engine {
            root: canonical,
            notes: Vec::new(),
            index: NoteIndex::build_index(&[]),
            search: None,
        };
        engine.refresh()?;
        Ok(engine)
    }

    /// Recharge complètement le vault : nouvelles notes, nouvel index, données
    /// de recherche invalidées.
    pub fn refresh(&mut self) -> Result<(), EngineError> {
        let notes = scan_vault(&self.root).map_err(|err| EngineError::Scan(format!("{err:#}")))?;
        self.index = NoteIndex::build_index(&notes);
        self.notes = notes;
        self.search = None;
        Ok(())
    }

    /// Racine canonique du vault.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Notes du vault, dans l'ordre du scan.
    pub fn notes(&self) -> &[Note] {
        &self.notes
    }

    /// Index de résolution du vault (résolution, backlinks).
    pub fn index(&self) -> &NoteIndex {
        &self.index
    }

    /// Nombre de notes du vault.
    pub fn note_count(&self) -> usize {
        self.notes.len()
    }

    /// Note correspondant à un chemin relatif, si elle existe.
    pub fn find_note(&self, rel: &Path) -> Option<&Note> {
        self.notes.iter().find(|note| note.path == rel)
    }

    /// Résout la cible d'un wikilink vers une note du vault.
    ///
    /// La résolution s'appuie sur l'index du vault (nom de fichier sans
    /// extension, insensible à la casse, cible unique). Une ancre (`#`) ou une
    /// référence de bloc (`^`) est retirée de la cible : `[[Note#Section]]`
    /// pointe la note `Note`. Une cible réduite à une ancre (`[[#Section]]`)
    /// pointe la note courante (`from`).
    pub fn resolve_link<'a>(&'a self, from: Option<&'a Note>, target: &str) -> Option<&'a Note> {
        let trimmed = target.trim();
        if trimmed.is_empty() {
            return None;
        }
        let base = trimmed
            .split(['#', '^'])
            .next()
            .unwrap_or(trimmed)
            .trim()
            .to_string();
        if base.is_empty() {
            // Ancre locale : le lien pointe la note qui le contient.
            return from;
        }
        let matches = self.index.resolve(&base);
        if matches.len() == 1 {
            Some(matches[0])
        } else {
            // Cible inconnue ou ambiguë (plusieurs notes homonymes).
            None
        }
    }

    /// Lit le contenu d'un fichier du vault (chemin relatif déjà confiné).
    pub fn read_file(&self, rel: &Path) -> Result<String, EngineError> {
        let target = self.root.join(rel);
        fs::read_to_string(&target).map_err(EngineError::Io)
    }

    /// Écrit un fichier du vault de façon atomique (fichier temporaire +
    /// `rename`, via `vault::ops`), puis recharge le vault.
    ///
    /// Le fichier est créé s'il n'existe pas encore ; les dossiers
    /// intermédiaires sont créés au besoin. Le contenu est écrit tel quel
    /// (Markdown brut).
    pub fn write_file(&mut self, rel: &Path, content: &str) -> Result<usize, EngineError> {
        let target = self.root.join(rel);
        if target.is_file() {
            write_note(&self.root, rel, content)
                .map_err(|err| EngineError::Vault(format!("{err:#}")))?;
        } else {
            create_note(&self.root, rel, content)
                .map_err(|err| EngineError::Vault(format!("{err:#}")))?;
        }
        self.refresh()?;
        Ok(content.len())
    }

    /// Construit les données de recherche (index full-text, contenus, lignes).
    fn build_search_data(&self) -> SearchData {
        let mut contents = HashMap::with_capacity(self.notes.len());
        let mut line_counts = HashMap::with_capacity(self.notes.len());
        let mut items = Vec::with_capacity(self.notes.len());
        for (index, note) in self.notes.iter().enumerate() {
            let path = self.root.join(&note.path);
            let text = match fs::read_to_string(&path) {
                Ok(text) => text,
                Err(err) => {
                    // Une note illisible ne casse pas la recherche : elle est
                    // indexée vide.
                    eprintln!(
                        "[obsidian-engine] lecture impossible de {} : {err}",
                        note.path.display()
                    );
                    String::new()
                }
            };
            line_counts.insert(index, text.lines().count());
            contents.insert(note.path.clone(), text.clone());
            items.push((note.name.clone(), text));
        }
        SearchData {
            contents,
            line_counts,
            fulltext: build_text_index(&items),
        }
    }

    /// Données de recherche, construites à la demande puis conservées.
    ///
    /// Les données sont prises en charge le temps de la recherche puis
    /// remises en place : l'emprunt du moteur ne bloque pas l'index.
    fn take_search_data(&mut self) -> SearchData {
        match self.search.take() {
            Some(data) => data,
            None => self.build_search_data(),
        }
    }

    /// Recherche dans le vault : la sélection des notes reuse la sémantique du
    /// vault (`search_ops::parse_query` puis `search_ops::filter_notes`).
    ///
    /// Chaque note retenue produit une ligne : la première ligne contenant le
    /// mot-clé de la requête, ou la ligne `0` si la note a été retenue par un
    /// critère qui ne porte pas sur son texte (tag, chemin, nom).
    pub fn search_results(&mut self, query: &str, limit: usize) -> Vec<SearchHit> {
        if query.trim().is_empty() || limit == 0 {
            return Vec::new();
        }
        let parsed = parse_query(query);
        let keyword = first_keyword(query);
        let data = self.take_search_data();
        let indices = filter_notes(
            &self.notes,
            Some(&data.fulltext),
            &data.line_counts,
            &parsed,
        );
        let mut hits = Vec::new();
        for index in indices.into_iter().take(limit) {
            let Some(note) = self.notes.get(index) else {
                continue;
            };
            let content = data
                .contents
                .get(&note.path)
                .map(String::as_str)
                .unwrap_or("");
            let (line, text) = first_matching_line(content, keyword);
            hits.push(SearchHit {
                path: display_path(&note.path),
                line,
                text,
            });
        }
        self.search = Some(data);
        hits
    }

    /// Graphe orienté du vault : un nœud par note, une arête par wikilink
    /// résolu.
    ///
    /// La résolution est celle de [`Engine::resolve_link`] : le graphe est
    /// donc cohérent avec `/metadata` (et avec `/backlinks`).
    pub fn graph_data(&self) -> (Vec<GraphNode>, Vec<GraphEdge>) {
        let mut nodes = Vec::with_capacity(self.notes.len());
        let mut edges: Vec<GraphEdge> = Vec::new();
        let mut seen: HashMap<(String, String), ()> = HashMap::new();
        for note in &self.notes {
            let mut links = 0usize;
            for link in &note.metadata.wikilinks {
                let Some(target) = self.resolve_link(Some(note), &link.target) else {
                    continue;
                };
                if target.path == note.path {
                    // Boucle : une note ne se relie pas à elle-même.
                    continue;
                }
                links += 1;
                let from = display_path(&note.path);
                let to = display_path(&target.path);
                if seen.insert((from.clone(), to.clone()), ()).is_none() {
                    edges.push(GraphEdge { from, to });
                }
            }
            nodes.push(GraphNode {
                path: display_path(&note.path),
                tags: note.metadata.tags.clone(),
                links,
            });
        }
        (nodes, edges)
    }

    /// Tags du vault et nombre de notes qui les portent, triés par tag.
    ///
    /// Un tag n'est compté qu'une fois par note, même s'il y apparaît
    /// plusieurs fois (corps et frontmatter fusionnés). La comparaison est
    /// exacte (la casse est significative, comme dans le vault).
    pub fn tags_data(&self) -> Vec<TagCount> {
        let mut counts: BTreeMap<String, usize> = BTreeMap::new();
        for note in &self.notes {
            let mut seen: Vec<&str> = Vec::new();
            for tag in &note.metadata.tags {
                if seen.contains(&tag.as_str()) {
                    continue;
                }
                seen.push(tag.as_str());
                *counts.entry(tag.clone()).or_insert(0) += 1;
            }
        }
        counts
            .into_iter()
            .map(|(tag, count)| TagCount { tag, count })
            .collect()
    }

    /// Tâches de toutes les notes, triées par chemin puis par ligne.
    ///
    /// Une note illisible est ignorée (signalée sur la sortie d'erreur) : une
    /// seule note cassée ne fait pas échouer `/tasks`.
    pub fn tasks_data(&self) -> Vec<TaskEntry> {
        let mut entries = Vec::new();
        for note in &self.notes {
            let path = self.root.join(&note.path);
            let content = match fs::read_to_string(&path) {
                Ok(content) => content,
                Err(err) => {
                    eprintln!(
                        "[obsidian-engine] lecture impossible de {} : {err}",
                        note.path.display()
                    );
                    continue;
                }
            };
            for task in extract_tasks(&content, &note.path) {
                entries.push(TaskEntry {
                    path: note.path.clone(),
                    line: task.line,
                    text: task.text,
                    done: task.status == TaskStatus::Done,
                });
            }
        }
        entries.sort_by(|left, right| {
            left.path
                .cmp(&right.path)
                .then_with(|| left.line.cmp(&right.line))
        });
        entries
    }

    /// Alias déclarés par les notes, triés par alias (insensible à la casse)
    /// puis par chemin.
    pub fn aliases_data(&self) -> Vec<AliasEntry> {
        let mut entries = Vec::new();
        for note in &self.notes {
            for alias in extract_aliases(&note.metadata.frontmatter) {
                entries.push(AliasEntry {
                    alias,
                    path: note.path.clone(),
                });
            }
        }
        entries.sort_by(|left, right| {
            left.alias
                .to_lowercase()
                .cmp(&right.alias.to_lowercase())
                .then_with(|| left.alias.cmp(&right.alias))
                .then_with(|| left.path.cmp(&right.path))
        });
        entries
    }
}

/// Premier mot-clé d'une requête (ignore les exclusions `-…` et les
/// opérateurs `x:`), utilisé pour localiser la ligne correspondante.
fn first_keyword(query: &str) -> &str {
    query
        .split_whitespace()
        .find(|token| !token.starts_with('-') && !token.contains(':'))
        .unwrap_or(query)
}

/// Localise la première ligne du texte contenant `keyword` (insensible à la
/// casse). Retourne `(0, "")` si aucune ligne ne correspond.
fn first_matching_line(content: &str, keyword: &str) -> (usize, String) {
    if keyword.trim().is_empty() {
        return (0, String::new());
    }
    let needle = keyword.to_lowercase();
    for (index, line) in content.lines().enumerate() {
        if line.to_lowercase().contains(&needle) {
            return (index + 1, line.trim().to_string());
        }
    }
    (0, String::new())
}

/// Chemin relatif d'un fichier du vault, avec `/` comme séparateur.
pub fn display_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// Sert le moteur sur `listener` jusqu'à la fermeture du socket.
///
/// Chaque connexion est traitée dans son propre fil ; l'état du moteur est
/// partagé derrière un verrou (les écritures rechargent le vault).
pub fn serve(engine: Engine, listener: TcpListener) -> Result<(), EngineError> {
    let engine = Arc::new(Mutex::new(engine));
    for stream in listener.incoming() {
        let stream = stream?;
        let engine = Arc::clone(&engine);
        std::thread::spawn(move || {
            if let Err(err) = handle_connection(&engine, stream) {
                eprintln!("[obsidian-engine] connexion interrompue : {err}");
            }
        });
    }
    Ok(())
}

/// Traite une connexion HTTP : une requête, une réponse, fermeture.
fn handle_connection(
    engine: &Mutex<Engine>,
    mut stream: std::net::TcpStream,
) -> Result<(), EngineError> {
    use std::io::{BufReader, Write};

    let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(30)));
    let _ = stream.set_write_timeout(Some(std::time::Duration::from_secs(30)));
    let mut reader = BufReader::new(stream.try_clone()?);

    let request = match http::read_request(&mut reader, http::MAX_BODY_BYTES) {
        Ok(request) => request,
        Err(http::HttpError::Io(err)) if err.kind() == std::io::ErrorKind::UnexpectedEof => {
            // Le client a fermé la connexion avant d'envoyer sa requête.
            return Ok(());
        }
        Err(err) => {
            let response = http::Response::error(400, err.to_string());
            let _ = stream.write_all(&response.as_bytes());
            let _ = stream.flush();
            return Ok(());
        }
    };

    let response = {
        let mut guard = engine.lock().unwrap_or_else(|err| err.into_inner());
        api::route(&mut guard, &request)
    };
    // Le pont `/sync` est appelé en XHR synchrone depuis la page du renderer :
    // les en-têtes CORS sont obligatoires dès que la page n'est pas servie
    // par le moteur lui-même.
    let bytes = if request.path == "/sync" {
        sync::encode_cors(&response)
    } else {
        response.as_bytes()
    };
    stream.write_all(&bytes)?;
    stream.flush()?;
    Ok(())
}
