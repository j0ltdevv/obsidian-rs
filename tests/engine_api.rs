//! Tests d'intégration de l'API HTTP JSON du moteur (`src/engine`).
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
        let root = std::env::temp_dir().join(format!("obsy-engine-{}", unique_suffix()));
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
        "{method} {target} HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
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

/// Envoie une requête HTTP brute et retourne (statut, corps JSON décodé).
fn request_json(port: u16, method: &str, target: &str, body: Option<&str>) -> (u16, Value) {
    let (status, raw) = request(port, method, target, body);
    let value = serde_json::from_str(&raw).unwrap_or(Value::Null);
    (status, value)
}

/// 1. `/health` répond `ok`, la racine du vault et le nombre de notes.
#[test]
fn health_repond_racine_et_nombre_de_notes() {
    let vault = TempVault::new();
    vault.write("Cible.md", "# Cible\n");
    vault.write("Demo.md", "# Démo\n");
    let server = TestServer::start(&vault.root);

    let (status, body) = request_json(server.port, "GET", "/health", None);

    assert_eq!(status, 200);
    assert_eq!(body["ok"], json!(true));
    assert_eq!(body["notes"], json!(2));
    let root = body["root"].as_str().expect("racine du vault");
    assert!(
        root.ends_with(
            vault
                .root
                .file_name()
                .expect("nom du vault")
                .to_string_lossy()
                .as_ref()
        ),
        "la racine annoncée doit être celle du vault : {root}"
    );
}

/// 2. `/files` liste les fichiers du vault avec taille et date de modification.
#[test]
fn files_liste_les_fichiers_avec_taille_et_date() {
    let vault = TempVault::new();
    vault.write("Cible.md", "# Cible\n");
    vault.write("dossier/Autre.md", "# Autre\n");
    let server = TestServer::start(&vault.root);

    let (status, body) = request_json(server.port, "GET", "/files", None);

    assert_eq!(status, 200);
    let files = body["files"].as_array().expect("liste des fichiers");
    assert_eq!(files.len(), 2);
    assert_eq!(files[0]["path"], json!("Cible.md"));
    assert_eq!(files[1]["path"], json!("dossier/Autre.md"));
    assert!(
        files[0]["size"].as_u64().unwrap_or(0) > 0,
        "taille non nulle"
    );
    assert!(files[0]["mtime"].as_u64().is_some(), "date de modification");
}

/// 3. Lecture puis écriture d'un fichier : le contenu écrit est relu sur le
///    disque et aucun fichier temporaire ne reste dans le dossier.
#[test]
fn ecriture_et_lecture_aller_retour_sans_temporaire_residuel() {
    let vault = TempVault::new();
    vault.write("Note.md", "# Ancienne\n");
    let server = TestServer::start(&vault.root);

    let (status, body) = request_json(server.port, "GET", "/file?path=Note.md", None);
    assert_eq!(status, 200);
    assert_eq!(body["path"], json!("Note.md"));
    assert_eq!(body["content"], json!("# Ancienne\n"));

    let nouveau = "# Nouvelle\n";
    let (status, body) = request_json(server.port, "PUT", "/file?path=Note.md", Some(nouveau));
    assert_eq!(status, 200);
    assert_eq!(body["path"], json!("Note.md"));
    assert_eq!(body["bytes"], json!(nouveau.len()));

    let sur_disque = fs::read_to_string(vault.join("Note.md")).expect("lecture du fichier écrit");
    assert_eq!(
        sur_disque, nouveau,
        "le contenu écrit doit être sur le disque"
    );
    assert_eq!(
        vault.root_entries(),
        vec!["Note.md".to_string()],
        "aucun fichier temporaire ne doit rester"
    );

    let (status, body) = request_json(server.port, "GET", "/file?path=Note.md", None);
    assert_eq!(status, 200);
    assert_eq!(body["content"], json!(nouveau));
}

/// 4. `/metadata` : frontmatter parsé, tags fusionnés (corps + frontmatter),
///    lien résolu et lien mort signalé.
#[test]
fn metadata_parse_le_frontmatter_fusionne_les_tags_et_resout_les_liens() {
    let vault = TempVault::new();
    vault.write(
        "Demo.md",
        "---\ntitle: Démo\ntags:\n  - depuis-frontmatter\n---\n\nUn tag #depuis-corps dans le corps.\n\nLien vers [[Cible]] et lien mort [[Inexistant]].\n",
    );
    vault.write("Cible.md", "# Cible\n");
    let server = TestServer::start(&vault.root);

    let (status, body) = request_json(server.port, "GET", "/metadata?path=Demo.md", None);

    assert_eq!(status, 200);
    assert_eq!(body["path"], json!("Demo.md"));
    assert_eq!(body["frontmatter"]["title"], json!("Démo"));

    let tags = body["tags"].as_array().expect("liste des tags");
    assert!(
        tags.contains(&json!("depuis-corps")),
        "le tag du corps doit être présent : {tags:?}"
    );
    assert!(
        tags.contains(&json!("depuis-frontmatter")),
        "le tag du frontmatter doit être présent : {tags:?}"
    );

    let links = body["links"].as_array().expect("liste des liens");
    assert_eq!(links.len(), 1, "un seul lien résolu : {links:?}");
    assert_eq!(links[0]["text"], json!("[[Cible]]"));
    assert_eq!(links[0]["path"], json!("Cible.md"));

    let unresolved = body["unresolved"]
        .as_array()
        .expect("liste des liens morts");
    assert_eq!(unresolved, &vec![json!("[[Inexistant]]")]);

    assert_eq!(body["properties"]["title"], json!("Démo"));
    assert_eq!(body["properties"]["tags"], json!("depuis-frontmatter"));
}

/// 5. `/backlinks` : les notes qui pointent vers la note, avec le nombre
///    d'occurrences.
#[test]
fn backlinks_liste_les_sources_avec_le_nombre_d_occurrences() {
    let vault = TempVault::new();
    vault.write("Cible.md", "# Cible\n");
    vault.write("Demo.md", "Voir [[Cible]] puis encore [[Cible]].\n");
    vault.write("Autre.md", "Voir [[Cible]].\n");
    let server = TestServer::start(&vault.root);

    let (status, body) = request_json(server.port, "GET", "/backlinks?path=Cible.md", None);

    assert_eq!(status, 200);
    assert_eq!(body["path"], json!("Cible.md"));
    let entries = body["backlinks"].as_array().expect("liste des backlinks");
    assert_eq!(entries.len(), 2, "deux sources : {entries:?}");

    let demo = entries
        .iter()
        .find(|entry| entry["from"] == json!("Demo.md"))
        .expect("backlink depuis Demo.md");
    assert_eq!(demo["count"], json!(2), "deux occurrences dans Demo.md");

    let autre = entries
        .iter()
        .find(|entry| entry["from"] == json!("Autre.md"))
        .expect("backlink depuis Autre.md");
    assert_eq!(autre["count"], json!(1));
}

/// 6. `/search` : la note correspondante est renvoyée avec sa ligne et son
///    texte.
#[test]
fn search_renvoie_la_ligne_correspondante() {
    let vault = TempVault::new();
    vault.write("Recherche.md", "# Titre\n\nune ligne alpha ici\n");
    let server = TestServer::start(&vault.root);

    let (status, body) = request_json(server.port, "GET", "/search?q=alpha&limit=5", None);

    assert_eq!(status, 200);
    let results = body["results"].as_array().expect("résultats de recherche");
    assert_eq!(results.len(), 1, "un seul résultat : {results:?}");
    assert_eq!(results[0]["path"], json!("Recherche.md"));
    assert_eq!(results[0]["line"], json!(3));
    assert_eq!(results[0]["text"], json!("une ligne alpha ici"));
}

/// 7. Les chemins qui sortent du vault sont refusés (`..`, chemin absolu).
#[test]
fn chemin_hors_du_vault_refuse() {
    let vault = TempVault::new();
    vault.write("Note.md", "# Note\n");
    let server = TestServer::start(&vault.root);

    let (status, body) = request_json(server.port, "GET", "/file?path=../../etc/passwd", None);
    assert_eq!(status, 400, "la remontée hors vault doit être refusée");
    assert!(
        body["error"].as_str().is_some(),
        "l'erreur doit être décrite : {body}"
    );

    let (status, _) = request_json(server.port, "GET", "/file?path=/etc/passwd", None);
    assert_eq!(status, 400, "un chemin absolu doit être refusé");

    let (status, _) = request_json(server.port, "PUT", "/file?path=../evade.md", Some("x"));
    assert_eq!(status, 400, "l'écriture hors vault doit être refusée");
    assert!(
        !vault.join("../evade.md").exists(),
        "aucun fichier ne doit être écrit hors du vault"
    );
}

/// 8. Un lien symbolique sortant du vault est refusé à la lecture.
#[cfg(unix)]
#[test]
fn lien_symbolique_hors_du_vault_refuse() {
    let vault = TempVault::new();
    vault.write("Note.md", "# Note\n");
    let outside = std::env::temp_dir().join(format!("obsy-engine-secret-{}", unique_suffix()));
    fs::write(&outside, "secret\n").expect("écriture du fichier hors vault");
    std::os::unix::fs::symlink(&outside, vault.join("Evasion.md"))
        .expect("création du lien symbolique");
    let server = TestServer::start(&vault.root);

    let (status, _) = request_json(server.port, "GET", "/file?path=Evasion.md", None);
    assert_eq!(status, 400, "un lien symbolique sortant doit être refusé");

    let _ = fs::remove_file(&outside);
}

/// 9. Les routes inconnues et les méthodes non autorisées sont refusées.
#[test]
fn route_inconnue_et_methode_non_autorisee_refusees() {
    let vault = TempVault::new();
    vault.write("Note.md", "# Note\n");
    let server = TestServer::start(&vault.root);

    let (status, body) = request_json(server.port, "GET", "/inconnu", None);
    assert_eq!(status, 404);
    assert!(body["error"].as_str().is_some());

    let (status, _) = request_json(server.port, "PUT", "/health", Some("x"));
    assert_eq!(status, 405, "PUT /health doit être refusé");

    let (status, _) = request_json(server.port, "GET", "/file", None);
    assert_eq!(status, 400, "le paramètre « path » est obligatoire");
}

/// 10. Un fichier absent renvoie une erreur 404.
#[test]
fn fichier_absent_renvoie_404() {
    let vault = TempVault::new();
    vault.write("Note.md", "# Note\n");
    let server = TestServer::start(&vault.root);

    let (status, body) = request_json(server.port, "GET", "/file?path=Absent.md", None);
    assert_eq!(status, 404);
    assert!(body["error"].as_str().is_some());
}
