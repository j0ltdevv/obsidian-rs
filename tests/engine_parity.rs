//! Tests d'intégration des routes de parité du moteur (`/tags`, `/outline`,
//! `/tasks`, `/aliases`).
//!
//! Chaque test crée un vault temporaire unique sous `std::env::temp_dir()`,
//! démarre le moteur sur un port éphémère (`127.0.0.1:0`) et lui envoie des
//! requêtes HTTP écrites à la main (aucune dépendance de test
//! supplémentaire). Le vault temporaire est supprimé à la fin du test.

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
        let root = std::env::temp_dir().join(format!("obsy-parity-{}", unique_suffix()));
        fs::create_dir_all(&root).expect("création du vault temporaire");
        Self { root }
    }

    /// Écrit un fichier du vault (dossiers intermédiaires créés au besoin).
    fn write(&self, rel: &str, content: &str) {
        let path = self.root.join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("création des dossiers intermédiaires");
        }
        fs::write(&path, content).expect("écriture du fichier de test");
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
fn request(port: u16, method: &str, target: &str) -> (u16, String) {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connexion au moteur");
    let head = format!(
        "{method} {target} HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
    );
    stream
        .write_all(head.as_bytes())
        .expect("écriture de la requête");
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

/// Corps JSON décodé d'une requête GET.
fn get_json(port: u16, target: &str) -> (u16, Value) {
    let (status, body) = request(port, "GET", target);
    let value = serde_json::from_str(&body).unwrap_or_else(|err| {
        panic!("corps JSON invalide pour {target} : {err} — corps : {body}");
    });
    (status, value)
}

/// Vault de référence : deux notes, tags, alias, titres et tâches (dont une
/// dans un bloc de code, qui doit être ignorée).
fn sample_vault() -> TempVault {
    let vault = TempVault::new();
    vault.write(
        "Notes/Alpha.md",
        "---\naliases:\n  - alpha\n  - Première\ntags: [projet/alpha, test]\n---\n# Titre\n\n## Sous-titre\n\n- [ ] Tâche à faire\n- [x] Tâche faite\n\n#autre\n\n```md\n# pas un titre\n- [ ] pas une tâche\n```\n",
    );
    vault.write(
        "Beta.md",
        "---\naliases: beta\ntags:\n  - test\n---\n# Beta\n- [ ] Une tâche\n",
    );
    vault
}

#[test]
fn tags_returns_counts_sorted_by_tag() {
    let vault = sample_vault();
    let server = TestServer::start(&vault.root);

    let (status, body) = get_json(server.port, "/tags");
    assert_eq!(status, 200);
    assert_eq!(
        body,
        json!({
            "tags": [
                { "tag": "autre", "count": 1 },
                { "tag": "projet/alpha", "count": 1 },
                { "tag": "test", "count": 2 },
            ]
        })
    );

    // Ordre déterministe : deux appels produisent le même corps.
    let (_, again) = get_json(server.port, "/tags");
    assert_eq!(body, again);
}

#[test]
fn outline_returns_heading_tree_with_editor_lines() {
    let vault = sample_vault();
    let server = TestServer::start(&vault.root);

    let (status, body) = get_json(server.port, "/outline?path=Notes/Alpha.md");
    assert_eq!(status, 200);
    assert_eq!(
        body,
        json!({
            "path": "Notes/Alpha.md",
            "outline": [
                { "level": 1, "title": "Titre", "line": 7 },
                { "level": 2, "title": "Sous-titre", "line": 9 },
            ]
        })
    );
}

#[test]
fn tasks_returns_vault_tasks_sorted_then_filtered_by_path() {
    let vault = sample_vault();
    let server = TestServer::start(&vault.root);

    let (status, body) = get_json(server.port, "/tasks");
    assert_eq!(status, 200);
    assert_eq!(
        body,
        json!({
            "tasks": [
                { "path": "Beta.md", "line": 7, "text": "Une tâche", "done": false },
                { "path": "Notes/Alpha.md", "line": 11, "text": "Tâche à faire", "done": false },
                { "path": "Notes/Alpha.md", "line": 12, "text": "Tâche faite", "done": true },
            ]
        })
    );

    let (status, filtered) = get_json(server.port, "/tasks?path=Notes/Alpha.md");
    assert_eq!(status, 200);
    assert_eq!(
        filtered,
        json!({
            "tasks": [
                { "path": "Notes/Alpha.md", "line": 11, "text": "Tâche à faire", "done": false },
                { "path": "Notes/Alpha.md", "line": 12, "text": "Tâche faite", "done": true },
            ]
        })
    );
}

#[test]
fn aliases_lists_frontmatter_aliases_sorted() {
    let vault = sample_vault();
    let server = TestServer::start(&vault.root);

    let (status, body) = get_json(server.port, "/aliases");
    assert_eq!(status, 200);
    assert_eq!(
        body,
        json!({
            "aliases": [
                { "alias": "alpha", "path": "Notes/Alpha.md" },
                { "alias": "beta", "path": "Beta.md" },
                { "alias": "Première", "path": "Notes/Alpha.md" },
            ]
        })
    );
}

#[test]
fn path_routes_reject_missing_escaping_and_unknown_paths() {
    let vault = sample_vault();
    let server = TestServer::start(&vault.root);

    for target in [
        "/outline",
        "/outline?path=",
        "/outline?path=..%2Fsecret.md",
        "/tasks?path=..%2Fsecret.md",
    ] {
        let (status, body) = request(server.port, "GET", target);
        assert_eq!(status, 400, "cible : {target} — corps : {body}");
        assert!(
            body.contains("\"error\""),
            "corps d'erreur attendu pour {target} : {body}"
        );
    }

    for target in ["/outline?path=absent.md", "/tasks?path=absent.md"] {
        let (status, body) = request(server.port, "GET", target);
        assert_eq!(status, 404, "cible : {target} — corps : {body}");
        assert!(
            body.contains("introuvable"),
            "message d'absence attendu pour {target} : {body}"
        );
    }
}

#[test]
fn new_routes_reject_wrong_method() {
    let vault = sample_vault();
    let server = TestServer::start(&vault.root);

    for target in ["/tags", "/outline", "/tasks", "/aliases"] {
        let (status, body) = request(server.port, "POST", target);
        assert_eq!(status, 405, "cible : {target} — corps : {body}");
    }
}
