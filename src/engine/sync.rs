//! Pont synchrone entre le renderer d'Obsidian et le moteur Rust.
//!
//! # Pourquoi un pont synchrone
//!
//! Le renderer d'Obsidian (`app.js`) appelle `ipcRenderer.sendSync` (31
//! canaux distincts) et des fonctions `fs` synchrones (`readFileSync`,
//! `existsSync`, …). La réponse doit donc revenir **dans la pile d'appel**.
//! L'IPC de Tauri est asynchrone : il ne peut pas servir ce chemin. Le pont
//! passe donc par un appel HTTP synchrone (`XMLHttpRequest` en mode
//! synchrone) vers `POST /sync` sur le moteur local.
//!
//! # Contrat
//!
//! Requête (`Content-Type: text/plain`, corps JSON) :
//! `{"op":"fs.readFileSync","args":["Note.md","utf8"]}`.
//!
//! Réponse :
//! - `{"ok":true,"value":…}` — succès ;
//! - `{"ok":false,"error":"message en français","code":"not_found"}` — échec
//!   d'opération.
//!
//! Le champ `code` est un complément utile : il permet au shim de distinguer
//! un fichier absent (`not_found`) d'un refus de confinement
//! (`path_refused`). Seule une requête *malformée* (JSON invalide, champ `op`
//! absent) renvoie un statut HTTP 4xx ; une opération refusée reste un
//! `200` avec `ok:false`, pour que le shim puisse propager le message.
//!
//! # Confinement
//!
//! Tout chemin qui touche au disque passe par `paths::normalize_relative`
//! (normalisation lexicale) puis `paths::ensure_inside` (vérification
//! physique : un lien symbolique sortant du vault est refusé). C'est
//! **exactement** la logique de `src/engine/paths.rs`, celle déjà utilisée par
//! `api.rs` : aucune exception, aucune reimplémentation.
//!
//! Les opérations `path.*` sont des calculs de chaînes purs (aucun fichier
//! n'est ouvert) : elles ne sont donc pas confinées, mais le résultat ne peut
//! servir qu'à une opération `fs.*`, qui l'est.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{json, Value};

use super::api;
use super::http::{self, Request, Response};
use super::paths;
use super::{display_path, Engine, EngineError};

/// Code d'erreur : opération inconnue du pont.
const CODE_UNSUPPORTED_OP: &str = "unsupported_op";
/// Code d'erreur : arguments absents ou mal typés.
const CODE_INVALID_ARGS: &str = "invalid_args";
/// Code d'erreur : chemin refusé par le confinement.
const CODE_PATH_REFUSED: &str = "path_refused";
/// Code d'erreur : fichier ou dossier absent.
const CODE_NOT_FOUND: &str = "not_found";
/// Code d'erreur : autre erreur d'E/S.
const CODE_IO: &str = "io";

/// Erreur d'une opération synchrone, avec un code lisible par le shim.
#[derive(Debug, Clone)]
struct SyncError {
    /// Message en français, destiné au renderer.
    message: String,
    /// Code machine (`not_found`, `path_refused`, …).
    code: &'static str,
}

impl SyncError {
    /// Crée une erreur de pont.
    fn new(message: impl Into<String>, code: &'static str) -> Self {
        Self {
            message: message.into(),
            code,
        }
    }
}

/// Résultat d'une opération synchrone.
///
/// Le type de succès est paramétrable : les helpers qui lisent un argument
/// rendent un `&str`, les opérations de fond rendent un `Value` JSON.
type SyncResult<T = Value> = Result<T, SyncError>;

/// Traite une requête `POST /sync`.
///
/// Réponse : `{"ok":true,"value":…}` ou `{"ok":false,"error":"…","code":"…"}`.
pub fn handle(engine: &mut Engine, request: &Request) -> Response {
    let Ok(body) = request.body_text() else {
        return Response::error(400, "corps de requête non UTF-8".to_string());
    };
    let payload: Value = match serde_json::from_str(body) {
        Ok(payload) => payload,
        Err(err) => return Response::error(400, format!("JSON invalide : {err}")),
    };
    let Some(op) = payload.get("op").and_then(Value::as_str) else {
        return Response::error(400, "champ « op » manquant".to_string());
    };
    let args = payload
        .get("args")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    match dispatch(engine, op, &args) {
        Ok(value) => Response::json(200, &json!({ "ok": true, "value": value })),
        Err(err) => Response::json(
            200,
            &json!({
                "ok": false,
                "error": err.message,
                "code": err.code,
            }),
        ),
    }
}

/// Encode une réponse `/sync` avec les en-têtes CORS.
///
/// Le shim appelle le moteur depuis la page du renderer. Si la page n'est pas
/// servie par le moteur, la requête est cross-origin : sans
/// `Access-Control-Allow-Origin`, le navigateur bloque la réponse. L'étoile
/// est sans risque ici : le moteur n'écoute que sur `127.0.0.1`.
pub fn encode_cors(response: &Response) -> Vec<u8> {
    let text = if response.status == 204 {
        "No Content"
    } else {
        http::status_text(response.status)
    };
    let head = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: application/json; charset=utf-8\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Headers: Content-Type\r\nAccess-Control-Allow-Methods: POST, OPTIONS\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
        response.status,
        text,
        response.body.len()
    );
    let mut bytes = head.into_bytes();
    bytes.extend_from_slice(response.body.as_bytes());
    bytes
}

/// Répartit une opération vers son implémentation.
fn dispatch(engine: &mut Engine, op: &str, args: &[Value]) -> SyncResult {
    match op {
        "fs.readFileSync" => read_file_sync(engine, args),
        "fs.writeFileSync" => write_file_sync(engine, args),
        "fs.existsSync" => exists_sync(engine, args),
        "fs.readdirSync" => readdir_sync(engine, args),
        "fs.statSync" => stat_sync(engine, args),
        "fs.mkdirSync" => mkdir_sync(engine, args),
        "fs.unlinkSync" => unlink_sync(engine, args),
        "fs.renameSync" => rename_sync(engine, args),
        "path.join" => path_join(args),
        "path.dirname" => path_dirname(args),
        "path.basename" => path_basename(args),
        "path.extname" => path_extname(args),
        "path.resolve" => path_resolve(engine, args),
        "crypto.randomUUID" => Ok(Value::String(random_uuid())),
        op if op.starts_with("ipc.vault.") => vault_op(engine, op, args),
        other => Err(SyncError::new(
            format!("opération non supportée : {other}"),
            CODE_UNSUPPORTED_OP,
        )),
    }
}

/// Argument textuel à l'indice demandé.
fn arg_str(args: &[Value], index: usize) -> SyncResult<&str> {
    args.get(index).and_then(Value::as_str).ok_or_else(|| {
        SyncError::new(
            format!("argument #{index} manquant ou non textuel"),
            CODE_INVALID_ARGS,
        )
    })
}

/// Vérifie l'encodage demandé : seul `utf8` est accepté (le moteur ne fait
/// pas de conversion binaire).
fn check_encoding(args: &[Value], index: usize) -> SyncResult<()> {
    let Some(encoding) = args.get(index) else {
        return Ok(());
    };
    let Some(encoding) = encoding.as_str() else {
        return Err(SyncError::new(
            format!("encodage #{index} non textuel"),
            CODE_INVALID_ARGS,
        ));
    };
    if encoding.eq_ignore_ascii_case("utf8") || encoding.eq_ignore_ascii_case("utf-8") {
        return Ok(());
    }
    Err(SyncError::new(
        format!("encodage non supporté : {encoding} (seul « utf8 » est accepté)"),
        CODE_INVALID_ARGS,
    ))
}

/// Confinement d'un chemin fourni par le shim.
///
/// Le chemin passe par exactement la même logique que le reste du moteur :
/// `paths::normalize_relative` (lexicale : ni vide, ni absolu, ni `..`) puis
/// `paths::ensure_inside` (physique : le chemin canonisé doit rester sous
/// la racine du vault, un lien symbolique sortant est donc refusé).
fn confined(engine: &Engine, raw: &str) -> SyncResult<PathBuf> {
    let relative = match paths::normalize_relative(raw) {
        Ok(relative) => relative,
        Err(paths::PathError::Empty) => {
            // « . » (ou « ./ ») désigne la racine du vault : c'est le
            // dossier courant du renderer, il est sous la racine par
            // définition. Les autres chemins vides restent refusés.
            if is_vault_root(raw) {
                return Ok(PathBuf::from("."));
            }
            return Err(SyncError::new(
                paths::PathError::Empty.to_string(),
                CODE_PATH_REFUSED,
            ));
        }
        Err(err) => {
            return Err(SyncError::new(err.to_string(), CODE_PATH_REFUSED));
        }
    };
    let target = engine.root().join(&relative);
    paths::ensure_inside(engine.root(), &target, &display_path(&relative))
        .map_err(|err| SyncError::new(err.to_string(), CODE_PATH_REFUSED))?;
    Ok(relative)
}

/// Le chemin désigne-t-il la racine du vault (« . », « ./ », …) ?
///
/// Un chemin absolu ou remontant (`..`) n'est jamais considéré comme la
/// racine : seuls des composants `.` (ou vides) sont acceptés.
fn is_vault_root(raw: &str) -> bool {
    let trimmed = raw.trim();
    !trimmed.is_empty()
        && trimmed
            .split('/')
            .all(|piece| piece.is_empty() || piece == ".")
}

/// Erreur d'E/S convertie en erreur de pont (`not_found` si absent).
fn io_error(err: &std::io::Error, relative: &Path) -> SyncError {
    if err.kind() == std::io::ErrorKind::NotFound {
        SyncError::new(
            format!("introuvable : {}", display_path(relative)),
            CODE_NOT_FOUND,
        )
    } else {
        SyncError::new(format!("erreur d'E/S : {err}"), CODE_IO)
    }
}

/// `fs.readFileSync` — lecture d'un fichier texte (encodage `utf8`).
fn read_file_sync(engine: &Engine, args: &[Value]) -> SyncResult {
    let relative = confined(engine, arg_str(args, 0)?)?;
    check_encoding(args, 1)?;
    let target = engine.root().join(&relative);
    match fs::read_to_string(&target) {
        Ok(content) => Ok(Value::String(content)),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Err(SyncError::new(
            format!("fichier introuvable : {}", display_path(&relative)),
            CODE_NOT_FOUND,
        )),
        Err(err) => Err(SyncError::new(
            format!("lecture impossible : {err}"),
            CODE_IO,
        )),
    }
}

/// `fs.writeFileSync` — écriture atomique d'un fichier texte.
///
/// L'écriture passe par `Engine::write_file` (donc par `vault::ops`) : fichier
/// temporaire + `rename`, dossiers intermédiaires créés au besoin.
fn write_file_sync(engine: &mut Engine, args: &[Value]) -> SyncResult {
    let relative = confined(engine, arg_str(args, 0)?)?;
    let content = arg_str(args, 1)?;
    check_encoding(args, 2)?;
    let bytes = engine
        .write_file(&relative, content)
        .map_err(|err| match err {
            EngineError::Path(inner) => SyncError::new(inner.to_string(), CODE_PATH_REFUSED),
            other => SyncError::new(other.to_string(), CODE_IO),
        })?;
    Ok(json!({ "bytes": bytes }))
}

/// `fs.existsSync` — existence d'un chemin (le confinement s'applique aussi).
fn exists_sync(engine: &Engine, args: &[Value]) -> SyncResult {
    let relative = confined(engine, arg_str(args, 0)?)?;
    let target = engine.root().join(&relative);
    Ok(Value::Bool(target.exists()))
}

/// `fs.readdirSync` — contenu d'un dossier.
///
/// Sans option, la valeur est la liste des noms ; avec
/// `{"withFileTypes":true}`, la valeur est la liste des descripteurs
/// (`name`, `isFile`, `isDirectory`, `isSymbolicLink`). Le tri est par nom
/// (le moteur trie, l'ordre du disque n'est pas garanti).
fn readdir_sync(engine: &Engine, args: &[Value]) -> SyncResult {
    let relative = confined(engine, arg_str(args, 0)?)?;
    let target = engine.root().join(&relative);
    let with_types = args
        .get(1)
        .and_then(|options| options.get("withFileTypes"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let reader = fs::read_dir(&target).map_err(|err| io_error(&err, &relative))?;
    let mut entries: Vec<(String, bool, bool, bool)> = Vec::new();
    for entry in reader {
        let entry = entry.map_err(|err| {
            SyncError::new(format!("lecture du dossier impossible : {err}"), CODE_IO)
        })?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let file_type = entry
            .file_type()
            .map_err(|err| SyncError::new(format!("type de fichier illisible : {err}"), CODE_IO))?;
        entries.push((
            name,
            file_type.is_file(),
            file_type.is_dir(),
            file_type.is_symlink(),
        ));
    }
    entries.sort_by(|left, right| left.0.cmp(&right.0));
    let values = entries
        .into_iter()
        .map(|(name, is_file, is_directory, is_symbolic_link)| {
            if with_types {
                json!({
                    "name": name,
                    "isFile": is_file,
                    "isDirectory": is_directory,
                    "isSymbolicLink": is_symbolic_link,
                })
            } else {
                Value::String(name)
            }
        })
        .collect();
    Ok(Value::Array(values))
}

/// `fs.statSync` — métadonnées d'un chemin.
///
/// `mtime` est en secondes (fractionnaires) depuis l'époque Unix, comme le
/// `Date` de Node.
fn stat_sync(engine: &Engine, args: &[Value]) -> SyncResult {
    let relative = confined(engine, arg_str(args, 0)?)?;
    let target = engine.root().join(&relative);
    let metadata = fs::metadata(&target).map_err(|err| io_error(&err, &relative))?;
    let is_symbolic_link = fs::symlink_metadata(&target)
        .map(|meta| meta.file_type().is_symlink())
        .unwrap_or(false);
    let mtime = metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|delta| delta.as_secs_f64())
        .unwrap_or(0.0);
    Ok(json!({
        "size": metadata.len(),
        "mtime": mtime,
        "isFile": metadata.is_file(),
        "isDirectory": metadata.is_dir(),
        "isSymbolicLink": is_symbolic_link,
        "readonly": metadata.permissions().readonly(),
    }))
}

/// `fs.mkdirSync` — création d'un dossier.
///
/// `{"recursive":true}` crée les dossiers intermédiaires (comme
/// `fs.mkdirSync(dir, {recursive: true})` de Node).
fn mkdir_sync(engine: &Engine, args: &[Value]) -> SyncResult {
    let relative = confined(engine, arg_str(args, 0)?)?;
    let recursive = args
        .get(1)
        .and_then(|options| options.get("recursive"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let target = engine.root().join(&relative);
    let outcome = if recursive {
        fs::create_dir_all(&target)
    } else {
        fs::create_dir(&target)
    };
    outcome.map_err(|err| io_error(&err, &relative))?;
    Ok(Value::Null)
}

/// `fs.unlinkSync` — suppression d'un fichier.
fn unlink_sync(engine: &Engine, args: &[Value]) -> SyncResult {
    let relative = confined(engine, arg_str(args, 0)?)?;
    let target = engine.root().join(&relative);
    fs::remove_file(&target).map_err(|err| io_error(&err, &relative))?;
    Ok(Value::Null)
}

/// `fs.renameSync` — déplacement d'un fichier (les deux chemins sont confinés).
fn rename_sync(engine: &Engine, args: &[Value]) -> SyncResult {
    let from = confined(engine, arg_str(args, 0)?)?;
    let to = confined(engine, arg_str(args, 1)?)?;
    fs::rename(engine.root().join(&from), engine.root().join(&to))
        .map_err(|err| SyncError::new(format!("déplacement impossible : {err}"), CODE_IO))?;
    Ok(Value::Null)
}

/// `path.join` — assemblage de fragments (calcul de chaînes, aucun accès disque).
fn path_join(args: &[Value]) -> SyncResult {
    let mut fragments = Vec::with_capacity(args.len());
    for index in 0..args.len() {
        fragments.push(arg_str(args, index)?.to_string());
    }
    Ok(Value::String(join_fragments(&fragments)))
}

/// Normalisation lexicale d'une liste de fragments, comme `path.join`.
///
/// `path.join` est un calcul de chaînes : aucun fichier n'est ouvert, donc le
/// confinement ne s'applique pas (les seules opérations qui touchent au
/// disque sont confinées). La sémantique suit Node : `.` disparaît, `..`
/// remonte, le résultat est absolu si le premier fragment l'est.
fn join_fragments(fragments: &[String]) -> String {
    let absolute = fragments
        .first()
        .map(|fragment| fragment.starts_with('/'))
        .unwrap_or(false);
    let mut parts: Vec<&str> = Vec::new();
    for fragment in fragments {
        for piece in fragment.split('/') {
            match piece {
                "" | "." => {}
                ".." => match parts.last() {
                    Some(last) if *last != ".." => {
                        parts.pop();
                    }
                    _ => {
                        if !absolute {
                            parts.push("..");
                        }
                    }
                },
                other => parts.push(other),
            }
        }
    }
    let joined = parts.join("/");
    if absolute {
        format!("/{joined}")
    } else if joined.is_empty() {
        ".".to_string()
    } else {
        joined
    }
}

/// `path.dirname` — dossier parent d'un chemin.
fn path_dirname(args: &[Value]) -> SyncResult {
    let raw = arg_str(args, 0)?;
    let parent = Path::new(raw)
        .parent()
        .map(|parent| parent.to_string_lossy().into_owned())
        .unwrap_or_default();
    let parent = if parent.is_empty() {
        ".".to_string()
    } else {
        parent
    };
    Ok(Value::String(parent))
}

/// `path.basename` — nom de fichier, extension facultative retirée.
fn path_basename(args: &[Value]) -> SyncResult {
    let raw = arg_str(args, 0)?;
    let name = Path::new(raw)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let name = match args.get(1).and_then(Value::as_str) {
        Some(extension)
            if !extension.is_empty()
                && name.len() > extension.len()
                && name.ends_with(extension) =>
        {
            name[..name.len() - extension.len()].to_string()
        }
        _ => name,
    };
    Ok(Value::String(name))
}

/// `path.extname` — extension d'un chemin (vide si le nom n'en a pas).
fn path_extname(args: &[Value]) -> SyncResult {
    let raw = arg_str(args, 0)?;
    let extension = Path::new(raw)
        .extension()
        .map(|extension| format!(".{}", extension.to_string_lossy()))
        .unwrap_or_default();
    Ok(Value::String(extension))
}

/// `path.resolve` — chemin absolu, relatif à la racine du vault.
///
/// La racine du vault joue le rôle du `cwd` du processus. Le calcul est
/// lexical : un chemin absolu extérieur reste autorisé comme *chaîne*, mais
/// toute utilisation en `fs.*` sera refusée par le confinement.
fn path_resolve(engine: &Engine, args: &[Value]) -> SyncResult {
    let mut fragments = Vec::with_capacity(args.len());
    for index in 0..args.len() {
        fragments.push(arg_str(args, index)?.to_string());
    }
    let joined = join_fragments(&fragments);
    let resolved = if joined.starts_with('/') {
        PathBuf::from(joined)
    } else if joined == "." {
        engine.root().to_path_buf()
    } else {
        engine.root().join(joined)
    };
    Ok(Value::String(resolved.to_string_lossy().into_owned()))
}

/// `ipc.vault.<nom>` — répartition vers les handlers existants de l'API.
///
/// Le pont réutilise `api::route` tel quel : le confinement, le parsing et
/// les messages d'erreur sont ceux du moteur, sans duplication. La valeur
/// renvoyée est le corps JSON de la route correspondante.
fn vault_op(engine: &mut Engine, op: &str, args: &[Value]) -> SyncResult {
    let (method, path, query, body) = match op {
        "ipc.vault.health" => ("GET", "/health", HashMap::new(), Vec::new()),
        "ipc.vault.list" => ("GET", "/files", HashMap::new(), Vec::new()),
        "ipc.vault.read" => {
            let mut query = HashMap::new();
            query.insert("path".to_string(), arg_str(args, 0)?.to_string());
            ("GET", "/file", query, Vec::new())
        }
        "ipc.vault.write" => {
            let mut query = HashMap::new();
            query.insert("path".to_string(), arg_str(args, 0)?.to_string());
            let content = arg_str(args, 1)?;
            ("PUT", "/file", query, content.as_bytes().to_vec())
        }
        "ipc.vault.metadata" => {
            let mut query = HashMap::new();
            query.insert("path".to_string(), arg_str(args, 0)?.to_string());
            ("GET", "/metadata", query, Vec::new())
        }
        "ipc.vault.backlinks" => {
            let mut query = HashMap::new();
            query.insert("path".to_string(), arg_str(args, 0)?.to_string());
            ("GET", "/backlinks", query, Vec::new())
        }
        "ipc.vault.search" => {
            let mut query = HashMap::new();
            query.insert("q".to_string(), arg_str(args, 0)?.to_string());
            if let Some(limit) = args.get(1).and_then(Value::as_u64) {
                query.insert("limit".to_string(), limit.to_string());
            }
            ("GET", "/search", query, Vec::new())
        }
        "ipc.vault.graph" => ("GET", "/graph", HashMap::new(), Vec::new()),
        "ipc.vault.root" => {
            return Ok(Value::String(engine.root().to_string_lossy().into_owned()));
        }
        other => {
            return Err(SyncError::new(
                format!("opération non supportée : {other}"),
                CODE_UNSUPPORTED_OP,
            ));
        }
    };
    let request = Request {
        method: method.to_string(),
        path: path.to_string(),
        query,
        body,
    };
    let response = api::route(engine, &request);
    let value: Value = serde_json::from_str(&response.body).unwrap_or(Value::Null);
    if response.status == 200 {
        return Ok(value);
    }
    let message = value
        .get("error")
        .and_then(Value::as_str)
        .unwrap_or("opération refusée")
        .to_string();
    let code = match response.status {
        400 => CODE_PATH_REFUSED,
        404 => CODE_NOT_FOUND,
        405 => CODE_UNSUPPORTED_OP,
        _ => CODE_IO,
    };
    Err(SyncError::new(message, code))
}

/// UUID version 4 (RFC 4122), sans dépendance externe.
fn random_uuid() -> String {
    let mut state = seed();
    let mut bytes = [0u8; 16];
    for chunk in bytes.chunks_mut(8) {
        let value = next_u64(&mut state);
        let raw = value.to_le_bytes();
        chunk.copy_from_slice(&raw[..chunk.len()]);
    }
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        bytes[0],
        bytes[1],
        bytes[2],
        bytes[3],
        bytes[4],
        bytes[5],
        bytes[6],
        bytes[7],
        bytes[8],
        bytes[9],
        bytes[10],
        bytes[11],
        bytes[12],
        bytes[13],
        bytes[14],
        bytes[15]
    )
}

/// Graine du générateur : temps + pid + compteur, mélangés.
fn seed() -> u64 {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|delta| delta.as_nanos() as u64)
        .unwrap_or(0);
    let counter = COUNTER.fetch_add(1, Ordering::Relaxed);
    let pid = u64::from(std::process::id());
    let mixed = mix(nanos ^ mix(pid ^ mix(counter)));
    if mixed == 0 {
        0x9e37_79b9_7f4a_7c15
    } else {
        mixed
    }
}

/// Mélangeur SplitMix64 : bonne dispersion, sans état.
fn mix(value: u64) -> u64 {
    let mut mixed = value.wrapping_add(0x9e37_79b9_7f4a_7c15);
    mixed = (mixed ^ (mixed >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    mixed = (mixed ^ (mixed >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    mixed ^ (mixed >> 31)
}

/// Valeur pseudo-aléatoire suivante (xorshift64*).
fn next_u64(state: &mut u64) -> u64 {
    let mut value = *state;
    value ^= value >> 12;
    value ^= value << 25;
    value ^= value >> 27;
    *state = value;
    value.wrapping_mul(0x2545_f491_4f6c_dd1d)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `path.join` suit la sémantique de Node.
    #[test]
    fn join_suit_la_semantique_de_node() {
        assert_eq!(join_fragments(&[]), ".");
        assert_eq!(join_fragments(&["a".to_string()]), "a");
        assert_eq!(
            join_fragments(&["dossier".to_string(), "Autre.md".to_string()]),
            "dossier/Autre.md"
        );
        assert_eq!(
            join_fragments(&["a".to_string(), "..".to_string(), "b".to_string()]),
            "b"
        );
        assert_eq!(join_fragments(&["/a".to_string(), "b".to_string()]), "/a/b");
        assert_eq!(join_fragments(&["a".to_string(), "/b".to_string()]), "a/b");
    }

    /// L'UUID généré a la forme d'un UUID v4.
    #[test]
    fn uuid_est_un_v4_valide() {
        let uuid = random_uuid();
        assert_eq!(uuid.len(), 36, "longueur inattendue : {uuid}");
        let parts: Vec<&str> = uuid.split('-').collect();
        assert_eq!(
            parts.iter().map(|part| part.len()).collect::<Vec<_>>(),
            vec![8, 4, 4, 4, 12]
        );
        assert!(parts[2].starts_with('4'), "version absente : {uuid}");
        assert!(
            matches!(parts[3].as_bytes()[0], b'8' | b'9' | b'a' | b'b'),
            "variante absente : {uuid}"
        );
        assert_ne!(uuid, random_uuid(), "deux UUID doivent différer");
    }
}
