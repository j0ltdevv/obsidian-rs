//! Routage des requêtes HTTP vers les handlers de l'API JSON.
//!
//! Chaque handler retourne une [`Response`](super::http::Response) : un
//! statut HTTP et un corps JSON. Les messages d'erreur sont en français.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde_json::{json, Value};

use super::http::{Request, Response};
use super::json as engine_json;
use super::paths;
use super::sync;
use super::{display_path, Engine, EngineError};

/// Chemins exposés par l'API.
const ROUTES: [&str; 8] = [
    "/health",
    "/files",
    "/file",
    "/metadata",
    "/backlinks",
    "/search",
    "/graph",
    "/sync",
];

/// Nombre de résultats renvoyés par défaut par `/search`.
const DEFAULT_SEARCH_LIMIT: usize = 50;

/// Plafond dur du nombre de résultats renvoyés par `/search`.
const MAX_SEARCH_LIMIT: usize = 500;

/// Route une requête vers son handler.
pub fn route(engine: &mut Engine, request: &Request) -> Response {
    match (request.method.as_str(), request.path.as_str()) {
        ("GET", "/health") => health(engine),
        ("GET", "/files") => files(engine),
        ("GET", "/file") => read_file(engine, request),
        ("PUT", "/file") => write_file(engine, request),
        ("GET", "/metadata") => metadata(engine, request),
        ("GET", "/backlinks") => backlinks(engine, request),
        ("GET", "/search") => search(engine, request),
        ("GET", "/graph") => graph(engine),
        ("POST", "/sync") => sync::handle(engine, request),
        ("OPTIONS", "/sync") => Response {
            status: 204,
            body: String::new(),
        },
        (method, path) if ROUTES.contains(&path) => Response::error(
            405,
            format!("méthode « {method} » non autorisée pour {path}"),
        ),
        _ => Response::error(404, format!("ressource inconnue : {}", request.path)),
    }
}

/// `GET /health` — état du moteur.
fn health(engine: &Engine) -> Response {
    Response::json(
        200,
        &json!({
            "ok": true,
            "root": engine.root().to_string_lossy(),
            "notes": engine.note_count(),
        }),
    )
}

/// `GET /files` — liste des fichiers du vault (hors fichiers et dossiers
/// cachés).
fn files(engine: &Engine) -> Response {
    let mut entries: Vec<Value> = Vec::new();
    let mut pending = vec![engine.root().to_path_buf()];
    while let Some(directory) = pending.pop() {
        let Ok(reader) = fs::read_dir(&directory) else {
            continue;
        };
        for entry in reader.flatten() {
            let name = entry.file_name();
            let Some(name) = name.to_str() else {
                continue;
            };
            if name.starts_with('.') {
                // Fichiers et dossiers cachés (`.obsidian`, `.git`, …).
                continue;
            }
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            let path = entry.path();
            if file_type.is_dir() {
                pending.push(path);
                continue;
            }
            if !file_type.is_file() {
                // Les liens symboliques ne sont pas listés : leur cible
                // pourrait se trouver hors du vault.
                continue;
            }
            let Ok(metadata) = entry.metadata() else {
                continue;
            };
            let mtime = metadata
                .modified()
                .ok()
                .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
                .map(|delta| delta.as_secs())
                .unwrap_or(0);
            let Some(relative) = relative_path(engine.root(), &path) else {
                continue;
            };
            entries.push(json!({
                "path": relative,
                "size": metadata.len(),
                "mtime": mtime,
            }));
        }
    }
    entries.sort_by(|left, right| {
        let left_path = left.get("path").and_then(Value::as_str).unwrap_or("");
        let right_path = right.get("path").and_then(Value::as_str).unwrap_or("");
        left_path.cmp(right_path)
    });
    Response::json(200, &json!({ "files": entries }))
}

/// `GET /file?path=…` — contenu brut d'un fichier.
fn read_file(engine: &Engine, request: &Request) -> Response {
    let relative = match requested_path(request) {
        Ok(relative) => relative,
        Err(response) => return response,
    };
    let target = engine.root().join(&relative);
    if let Err(err) = paths::ensure_inside(engine.root(), &target, &display_path(&relative)) {
        return Response::error(400, err.to_string());
    }
    match fs::read_to_string(&target) {
        Ok(content) => Response::json(
            200,
            &json!({
                "path": display_path(&relative),
                "content": content,
            }),
        ),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Response::error(
            404,
            format!("fichier introuvable : {}", display_path(&relative)),
        ),
        Err(err) => Response::error(500, format!("lecture impossible : {err}")),
    }
}

/// `PUT /file?path=…` — écriture atomique d'un fichier (corps = Markdown brut).
fn write_file(engine: &mut Engine, request: &Request) -> Response {
    let relative = match requested_path(request) {
        Ok(relative) => relative,
        Err(response) => return response,
    };
    let target = engine.root().join(&relative);
    if let Err(err) = paths::ensure_inside(engine.root(), &target, &display_path(&relative)) {
        return Response::error(400, err.to_string());
    }
    let content = match request.body_text() {
        Ok(content) => content,
        Err(err) => return Response::error(400, err.to_string()),
    };
    match engine.write_file(&relative, content) {
        Ok(bytes) => Response::json(
            200,
            &json!({
                "path": display_path(&relative),
                "bytes": bytes,
            }),
        ),
        Err(err) => Response::error(status_for(&err), err.to_string()),
    }
}

/// `GET /metadata?path=…` — métadonnées d'une note.
fn metadata(engine: &Engine, request: &Request) -> Response {
    let relative = match requested_path(request) {
        Ok(relative) => relative,
        Err(response) => return response,
    };
    let Some(note) = engine.find_note(&relative) else {
        return Response::error(
            404,
            format!("note introuvable : {}", display_path(&relative)),
        );
    };
    let mut links = Vec::new();
    let mut unresolved = Vec::new();
    for link in &note.metadata.wikilinks {
        let text = link_text(&link.target, link.alias.as_deref());
        match engine.resolve_link(Some(note), &link.target) {
            Some(target) => links.push(json!({
                "text": text,
                "path": display_path(&target.path),
            })),
            None => unresolved.push(Value::String(text)),
        }
    }
    Response::json(
        200,
        &json!({
            "path": display_path(&note.path),
            "frontmatter": engine_json::frontmatter_to_json(note.metadata.frontmatter.as_ref()),
            "frontmatter_error": note.metadata.frontmatter_error.clone(),
            "tags": note.metadata.tags.clone(),
            "links": links,
            "unresolved": unresolved,
            "properties": engine_json::properties_to_json(note.metadata.frontmatter.as_ref()),
        }),
    )
}

/// `GET /backlinks?path=…` — notes pointant vers une note.
fn backlinks(engine: &Engine, request: &Request) -> Response {
    let relative = match requested_path(request) {
        Ok(relative) => relative,
        Err(response) => return response,
    };
    let Some(note) = engine.find_note(&relative) else {
        return Response::error(
            404,
            format!("note introuvable : {}", display_path(&relative)),
        );
    };
    let mut entries = Vec::new();
    for source in engine.index().backlinks(note) {
        let count = source
            .metadata
            .wikilinks
            .iter()
            .filter(|link| {
                engine
                    .resolve_link(Some(source), &link.target)
                    .is_some_and(|target| target.path == note.path)
            })
            .count();
        entries.push(json!({
            "from": display_path(&source.path),
            "count": count,
        }));
    }
    Response::json(
        200,
        &json!({
            "path": display_path(&note.path),
            "backlinks": entries,
        }),
    )
}

/// `GET /search?q=…&limit=N` — recherche dans le vault.
fn search(engine: &mut Engine, request: &Request) -> Response {
    let query = request.param("q").unwrap_or("");
    let limit = request
        .param("limit")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(DEFAULT_SEARCH_LIMIT)
        .min(MAX_SEARCH_LIMIT);
    let hits = engine.search_results(query, limit);
    let results: Vec<Value> = hits
        .iter()
        .map(|hit| {
            json!({
                "path": &hit.path,
                "line": hit.line,
                "text": &hit.text,
            })
        })
        .collect();
    Response::json(200, &json!({ "results": results }))
}

/// `GET /graph` — graphe des notes et de leurs liens.
fn graph(engine: &Engine) -> Response {
    let (nodes, edges) = engine.graph_data();
    let nodes: Vec<Value> = nodes
        .iter()
        .map(|node| {
            json!({
                "path": &node.path,
                "tags": &node.tags,
                "links": node.links,
            })
        })
        .collect();
    let edges: Vec<Value> = edges
        .iter()
        .map(|edge| json!({ "from": &edge.from, "to": &edge.to }))
        .collect();
    Response::json(200, &json!({ "nodes": nodes, "edges": edges }))
}

/// Chemin relatif demandé par le client, normalisé et confiné.
fn requested_path(request: &Request) -> Result<PathBuf, Response> {
    let Some(raw) = request.param("path") else {
        return Err(Response::error(400, "paramètre « path » manquant"));
    };
    paths::normalize_relative(raw).map_err(|err| Response::error(400, err.to_string()))
}

/// Chemin relatif d'un fichier, par rapport à la racine du vault.
fn relative_path(root: &Path, path: &Path) -> Option<String> {
    path.strip_prefix(root).ok().map(display_path)
}

/// Texte brut d'un wikilink (`[[cible]]` ou `[[cible|alias]]`).
fn link_text(target: &str, alias: Option<&str>) -> String {
    match alias {
        Some(alias) => format!("[[{target}|{alias}]]"),
        None => format!("[[{target}]]"),
    }
}

/// Statut HTTP correspondant à une erreur du moteur.
fn status_for(error: &EngineError) -> u16 {
    match error {
        EngineError::Path(_) | EngineError::Vault(_) => 400,
        EngineError::RootMissing(_) => 404,
        EngineError::Io(err) if err.kind() == std::io::ErrorKind::NotFound => 404,
        EngineError::Io(_) => 500,
        EngineError::Scan(_) => 500,
    }
}
