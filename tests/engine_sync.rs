//! Tests d'intégration du pont synchrone `POST /sync` (`src/engine/sync.rs`).
//!
//! Chaque test crée un vault temporaire unique sous `std::env::temp_dir()`,
//! démarre le moteur sur un port éphémère (`127.0.0.1:0`) et lui envoie des
//! requêtes HTTP écrites à la main (aucune dépendance de test supplémentaire).
//! Le vault temporaire est supprimé à la fin du test.

use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use obsy::engine::{serve, Engine};
use serde_json::{json, Value};

/// Génère un suffixe unique par vault (pid + compteur atomique).
fn unique_suffix() -> String {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    format!("{}-{n}", std::process::id())
}

/// Vault temporaire supprimé à la fin du test (y compris en cas de panique).
struct TempVault {
    root: PathBuf,
}

impl TempVault {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("obsy-sync-{}", unique_suffix()));
        fs::create_dir_all(&root).expect("création du vault temporaire");
        Self { root }
    }

    fn join(&self, rel: &str) -> PathBuf {
        self.root.join(rel)
    }

    /// Écrit un fichier du vault (dossiers intermédiaires créés au besoin).
    fn write(&self, rel: &str, content: &str) {
        let path = self.join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("création des dossiers intermédiaires");
        }
        fs::write(&path, content).expect("écriture du fichier de test");
    }

    /// Noms des entrées du dossier racine du vault.
    fn root_entries(&self) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(&self.root)
            .expect("lecture du dossier du vault")
            .map(|entry| {
                entry
                    .expect("entrée du dossier")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        names.sort();
        names
    }
}

impl Drop for TempVault {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

/// Moteur démarré sur un port éphémère.
///
/// Le fil de service n'est jamais arrêté : il est détaché et meurt avec le
/// processus de test (le port est propre à ce test).
struct TestServer {
    port: u16,
}

impl TestServer {
    fn start(vault: &Path) -> Self {
        let engine = Engine::open(vault).expect("ouverture du vault de test");
        let listener = TcpListener::bind("127.0.0.1:0").expect("écoute sur un port éphémère");
        let port = listener.local_addr().expect("adresse d'écoute").port();
        std::thread::spawn(move || {
            let _ = serve(engine, listener);
        });
        Self { port }
    }
}

/// Envoie une requête HTTP brute et retourne (statut, corps brut).
fn request(port: u16, method: &str, target: &str, body: Option<&str>) -> (u16, String) {
    let payload = body.unwrap_or("");
    let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connexion au moteur");
    let head = format!(
        "{method} {target} HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        payload.len()
    );
    stream
        .write_all(head.as_bytes())
        .expect("écriture de la requête");
    stream
        .write_all(payload.as_bytes())
        .expect("écriture du corps");
    stream.flush().expect("vidage du socket");
    let mut raw = String::new();
    stream
        .read_to_string(&mut raw)
        .expect("lecture de la réponse HTTP");
    let (head, body) = raw.split_once("\r\n\r\n").expect("réponse HTTP complète");
    let status_line = head.lines().next().unwrap_or_default();
    let status = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|code| code.parse::<u16>().ok())
        .expect("code de statut HTTP");
    (status, body.to_string())
}

/// Envoie une requête HTTP brute et retourne (statut, en-têtes, corps).
fn request_full(
    port: u16,
    method: &str,
    target: &str,
    body: Option<&str>,
) -> (u16, String, String) {
    let payload = body.unwrap_or("");
    let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connexion au moteur");
    let head = format!(
        "{method} {target} HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        payload.len()
    );
    stream
        .write_all(head.as_bytes())
        .expect("écriture de la requête");
    stream
        .write_all(payload.as_bytes())
        .expect("écriture du corps");
    stream.flush().expect("vidage du socket");
    let mut raw = String::new();
    stream
        .read_to_string(&mut raw)
        .expect("lecture de la réponse HTTP");
    let (head, body) = raw.split_once("\r\n\r\n").expect("réponse HTTP complète");
    let status_line = head.lines().next().unwrap_or_default();
    let status = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|code| code.parse::<u16>().ok())
        .expect("code de statut HTTP");
    (status, head.to_string(), body.to_string())
}

/// Appelle une opération du pont synchrone et retourne le corps JSON décodé.
fn sync_op(port: u16, op: &str, args: Value) -> Value {
    let payload = json!({ "op": op, "args": args });
    let (status, body) = request(port, "POST", "/sync", Some(&payload.to_string()));
    assert_eq!(status, 200, "le pont doit répondre 200 : {body}");
    serde_json::from_str(&body).unwrap_or(Value::Null)
}

/// Appelle une opération et retourne la valeur (échec = panique avec le
/// message d'erreur du moteur).
fn sync_value(port: u16, op: &str, args: Value) -> Value {
    let body = sync_op(port, op, args);
    assert_eq!(
        body["ok"],
        json!(true),
        "l'opération {op} doit réussir : {body}"
    );
    body["value"].clone()
}

/// Appelle une opération et retourne le code d'erreur (échec = panique).
fn sync_error(port: u16, op: &str, args: Value) -> String {
    let body = sync_op(port, op, args);
    assert_eq!(
        body["ok"],
        json!(false),
        "l'opération {op} doit échouer : {body}"
    );
    body["code"].as_str().expect("code d'erreur").to_string()
}

/// 1. `fs.readFileSync` renvoie le contenu du fichier, en UTF-8.
#[test]
fn lecture_synchronise_renvoie_le_contenu_du_fichier() {
    let vault = TempVault::new();
    vault.write("Note.md", "# Titre\n\ncontenu de la note\n");
    let server = TestServer::start(&vault.root);

    let valeur = sync_value(server.port, "fs.readFileSync", json!(["Note.md", "utf8"]));

    assert_eq!(valeur, json!("# Titre\n\ncontenu de la note\n"));
}

/// 2. `fs.existsSync` répond vrai ou faux selon la présence du fichier.
#[test]
fn existence_synchronise_repond_vrai_ou_faux() {
    let vault = TempVault::new();
    vault.write("Present.md", "# Présent\n");
    let server = TestServer::start(&vault.root);

    let present = sync_value(server.port, "fs.existsSync", json!(["Present.md"]));
    let absent = sync_value(server.port, "fs.existsSync", json!(["Absent.md"]));

    assert_eq!(present, json!(true));
    assert_eq!(absent, json!(false));
}

/// 3. Les opérations `path.*` sont calculées par le moteur.
#[test]
fn operations_chemin_sont_calculees_par_le_moteur() {
    let vault = TempVault::new();
    let server = TestServer::start(&vault.root);

    assert_eq!(
        sync_value(server.port, "path.join", json!(["dossier", "Autre.md"])),
        json!("dossier/Autre.md")
    );
    assert_eq!(
        sync_value(server.port, "path.dirname", json!(["dossier/Autre.md"])),
        json!("dossier")
    );
    assert_eq!(
        sync_value(server.port, "path.basename", json!(["dossier/Autre.md"])),
        json!("Autre.md")
    );
    assert_eq!(
        sync_value(
            server.port,
            "path.basename",
            json!(["dossier/Autre.md", ".md"])
        ),
        json!("Autre")
    );
    assert_eq!(
        sync_value(server.port, "path.extname", json!(["dossier/Autre.md"])),
        json!(".md")
    );
    let resolu = sync_value(server.port, "path.resolve", json!(["Note.md"]));
    let racine = vault
        .root
        .canonicalize()
        .expect("racine canonique")
        .to_string_lossy()
        .into_owned();
    assert_eq!(
        resolu,
        json!(format!("{racine}/Note.md")),
        "path.resolve est relatif à la racine du vault"
    );
}

/// 4. La remontée hors du vault est refusée sur toutes les opérations de
///    fichier, et rien n'est écrit hors du vault.
#[test]
fn remontee_hors_du_vault_refusee() {
    let vault = TempVault::new();
    vault.write("Note.md", "# Note\n");
    let server = TestServer::start(&vault.root);

    assert_eq!(
        sync_error(
            server.port,
            "fs.readFileSync",
            json!(["../../etc/passwd", "utf8"])
        ),
        "path_refused"
    );
    assert_eq!(
        sync_error(
            server.port,
            "fs.readFileSync",
            json!(["/etc/passwd", "utf8"])
        ),
        "path_refused",
        "un chemin absolu doit être refusé"
    );
    assert_eq!(
        sync_error(server.port, "fs.existsSync", json!(["../evade.md"])),
        "path_refused"
    );
    assert_eq!(
        sync_error(
            server.port,
            "fs.writeFileSync",
            json!(["../evade.md", "x", "utf8"])
        ),
        "path_refused"
    );
    assert_eq!(
        sync_error(server.port, "fs.readdirSync", json!([".."])),
        "path_refused"
    );
    assert!(
        !vault.join("../evade.md").exists(),
        "aucun fichier ne doit être écrit hors du vault"
    );
}

/// 5. `fs.writeFileSync` écrit le fichier, qui est relu à l'identique, et
///    aucun fichier temporaire ne reste dans le dossier.
#[test]
fn ecriture_synchronise_est_relu_sans_temporaire_residuel() {
    let vault = TempVault::new();
    let server = TestServer::start(&vault.root);

    let valeur = sync_value(
        server.port,
        "fs.writeFileSync",
        json!(["dossier/Neuf.md", "# Neuf\n", "utf8"]),
    );
    assert_eq!(valeur["bytes"], json!(7));

    let relu = sync_value(
        server.port,
        "fs.readFileSync",
        json!(["dossier/Neuf.md", "utf8"]),
    );
    assert_eq!(relu, json!("# Neuf\n"));
    assert_eq!(
        vault.root_entries(),
        vec!["dossier".to_string()],
        "seul le dossier créé doit rester"
    );
}

/// 6. Les opérations de dossier et de métadonnées fonctionnent.
#[test]
fn operations_dossier_et_metadonnees() {
    let vault = TempVault::new();
    vault.write("Note.md", "# Note\n");
    let server = TestServer::start(&vault.root);

    sync_value(
        server.port,
        "fs.mkdirSync",
        json!(["sous/dossier", { "recursive": true }]),
    );
    let entrees = sync_value(server.port, "fs.readdirSync", json!(["sous/dossier"]));
    assert_eq!(entrees, json!([]));

    let stats = sync_value(server.port, "fs.statSync", json!(["Note.md"]));
    assert_eq!(stats["isFile"], json!(true));
    assert_eq!(stats["isDirectory"], json!(false));
    assert!(stats["size"].as_u64().unwrap_or(0) > 0, "taille non nulle");

    let types = sync_value(
        server.port,
        "fs.readdirSync",
        json!([".", { "withFileTypes": true }]),
    );
    let types = types.as_array().expect("liste des descripteurs");
    let note = types
        .iter()
        .find(|entry| entry["name"] == json!("Note.md"))
        .expect("descripteur de Note.md");
    assert_eq!(note["isFile"], json!(true));

    sync_value(
        server.port,
        "fs.renameSync",
        json!(["Note.md", "Renommee.md"]),
    );
    assert!(
        vault.join("Renommee.md").exists(),
        "le fichier doit être déplacé"
    );
    assert!(
        !vault.join("Note.md").exists(),
        "l'ancien nom doit disparaître"
    );

    sync_value(server.port, "fs.unlinkSync", json!(["Renommee.md"]));
    assert!(
        !vault.join("Renommee.md").exists(),
        "le fichier doit être supprimé"
    );
}

//  12. La racine du vault est accessible via le chemin relatif vide.
#[test]
fn racine_vault_accessible_par_chemin_vide() {
    let vault = TempVault::new();
    vault.write("Note.md", "# Note\n");
    vault.write("sous/Extra.md", "# Extra\n");
    let server = TestServer::start(&vault.root);

    // `readdir` de la racine via le chemin vide : entrées de premier niveau,
    // et rien en dehors du vault.
    let entrees = sync_value(server.port, "fs.readdirSync", json!([""]));
    assert_eq!(entrees, json!(["Note.md", "sous"]));
    assert_eq!(
        entrees.as_array().expect("liste des entrées").len(),
        2,
        "le readdir de la racine ne doit exposer que les entrées du vault"
    );
    assert_eq!(entrees, json!(vault.root_entries()));

    // Le chemin réduit à des espaces désigne aussi la racine.
    let entrees_espaces = sync_value(server.port, "fs.readdirSync", json!(["  "]));
    assert_eq!(entrees_espaces, entrees);

    // La racine est lisible (`existsSync`) et statistable (`statSync`).
    assert_eq!(
        sync_value(server.port, "fs.existsSync", json!([""])),
        json!(true)
    );
    let stats = sync_value(server.port, "fs.statSync", json!([""]));
    assert_eq!(stats["isDirectory"], json!(true));

    // `readdir` d'un sous-dossier continue de fonctionner.
    let entrees_sous = sync_value(server.port, "fs.readdirSync", json!(["sous"]));
    assert_eq!(entrees_sous, json!(["Extra.md"]));

    // Un chemin absolu et une remontée restent refusés.
    assert_eq!(
        sync_error(server.port, "fs.readdirSync", json!(["/etc"])),
        "path_refused",
        "un chemin absolu doit être refusé"
    );
    assert_eq!(
        sync_error(server.port, "fs.readdirSync", json!(["../hors"])),
        "path_refused",
        "une remontée hors du vault doit être refusée"
    );
}

/// 7. `ipc.vault.*` est réparti vers les handlers existants de l'API.
#[test]
fn canal_ipc_vault_repartit_vers_les_handlers() {
    let vault = TempVault::new();
    vault.write("Note.md", "# Note\n");
    let server = TestServer::start(&vault.root);

    let racine = sync_value(server.port, "ipc.vault.root", json!([]));
    assert_eq!(
        racine,
        json!(vault.root.canonicalize().expect("racine").to_string_lossy())
    );

    let liste = sync_value(server.port, "ipc.vault.list", json!([]));
    assert_eq!(liste["files"][0]["path"], json!("Note.md"));

    let lecture = sync_value(server.port, "ipc.vault.read", json!(["Note.md"]));
    assert_eq!(lecture["content"], json!("# Note\n"));

    let recherche = sync_value(server.port, "ipc.vault.search", json!(["Note", 5]));
    assert_eq!(recherche["results"][0]["path"], json!("Note.md"));

    assert_eq!(
        sync_error(server.port, "ipc.vault.read", json!(["Absente.md"])),
        "not_found"
    );
    assert_eq!(
        sync_error(server.port, "ipc.vault.read", json!(["../../etc/passwd"])),
        "path_refused"
    );
}

/// 8. `crypto.randomUUID` renvoie un UUID version 4.
#[test]
fn uuid_aleatoire_valide() {
    let vault = TempVault::new();
    let server = TestServer::start(&vault.root);

    let uuid = sync_value(server.port, "crypto.randomUUID", json!([]));
    let uuid = uuid.as_str().expect("chaîne UUID");
    assert_eq!(uuid.len(), 36, "longueur inattendue : {uuid}");
    let parts: Vec<&str> = uuid.split('-').collect();
    assert_eq!(
        parts.iter().map(|part| part.len()).collect::<Vec<_>>(),
        vec![8, 4, 4, 4, 12]
    );
    assert!(parts[2].starts_with('4'), "version absente : {uuid}");
}

/// 9. Une opération inconnue et une requête malformée sont refusées.
#[test]
fn operation_inconnue_et_requete_malformee_refusees() {
    let vault = TempVault::new();
    let server = TestServer::start(&vault.root);

    assert_eq!(
        sync_error(server.port, "fs.explode", json!([])),
        "unsupported_op"
    );
    assert_eq!(
        sync_error(server.port, "ipc.vault.inconnu", json!([])),
        "unsupported_op"
    );

    let (status, body) = request(server.port, "POST", "/sync", Some("{pas du json"));
    assert_eq!(status, 400, "un JSON invalide doit être refusé");
    let body: Value = serde_json::from_str(&body).unwrap_or(Value::Null);
    assert!(
        body["error"].as_str().is_some(),
        "l'erreur doit être décrite"
    );

    let (status, _) = request(server.port, "GET", "/sync", None);
    assert_eq!(status, 405, "GET /sync doit être refusé");
}

/// 10. La réponse du pont porte les en-têtes CORS (le shim appelle le moteur
///     depuis une page qui n'est pas servie par le moteur).
#[test]
fn reponse_du_pont_porte_les_entetes_cors() {
    let vault = TempVault::new();
    let server = TestServer::start(&vault.root);

    let (status, head, _) = request_full(
        server.port,
        "POST",
        "/sync",
        Some(&json!({ "op": "crypto.randomUUID", "args": [] }).to_string()),
    );

    assert_eq!(status, 200);
    assert!(
        head.contains("Access-Control-Allow-Origin: *"),
        "l'en-tête CORS doit être présent : {head}"
    );
    assert!(
        head.contains("Access-Control-Allow-Methods: POST, OPTIONS"),
        "les méthodes autorisées doivent être annoncées : {head}"
    );
}

/// 11. Un lien symbolique sortant du vault est refusé à la lecture.
#[cfg(unix)]
#[test]
fn lien_symbolique_hors_du_vault_refuse() {
    let vault = TempVault::new();
    let outside = std::env::temp_dir().join(format!("obsy-sync-secret-{}", unique_suffix()));
    fs::write(&outside, "secret\n").expect("écriture du fichier hors vault");
    std::os::unix::fs::symlink(&outside, vault.join("Evasion.md"))
        .expect("création du lien symbolique");
    let server = TestServer::start(&vault.root);

    assert_eq!(
        sync_error(
            server.port,
            "fs.readFileSync",
            json!(["Evasion.md", "utf8"])
        ),
        "path_refused"
    );

    let _ = fs::remove_file(&outside);
}
