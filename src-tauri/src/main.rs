//! Coquille de bureau d'`obsidian-rs`.
//!
//! La fenêtre est un **webview** qui charge le front `web/` (notre HTML/CSS),
//! et les données viennent du **moteur Rust** `obsidian-engine`, lancé ici
//! comme processus enfant : il n'écoute que sur `127.0.0.1` et confine tous
//! les chemins au vault.
//!
//! # Choix du chemin du moteur
//!
//! Le binaire `obsidian-engine` est cherché, dans cet ordre :
//! 1. la variable d'environnement `OBSIDIAN_RS_ENGINE` ;
//! 2. à côté de l'exécutable courant (`<exe-dir>/obsidian-engine`) — c'est le
//!    cas d'un binaire installé à côté ;
//! 3. dans le `PATH`.
//!
//! # Choix du vault
//!
//! Le vault est lu dans la variable d'environnement `OBSIDIAN_RS_VAULT`, ou
//! passé en premier argument de la ligne de commande ; à défaut, c'est le
//! dossier `demo-vault` à côté de l'exécutable.

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

/// Port d'écoute par défaut du moteur (voir `src/engine/mod.rs`).
const PORT_PAR_DEFAUT: u16 = 27123;

fn main() {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let racine_vault = racine_vault(&arguments);
    let port = port(&arguments);
    let adresse = format!("http://127.0.0.1:{port}");

    let mut moteur = match demarrer_moteur(&racine_vault, port) {
        Ok(moteur) => moteur,
        Err(erreur) => {
            eprintln!("obsidian-rs : impossible de démarrer le moteur : {erreur}");
            std::process::exit(1);
        }
    };

    // Le front lit l'adresse du moteur depuis cette variable : on la publie
    // dans l'environnement du webview via la configuration de la fenêtre.
    std::env::set_var("OBSIDIAN_RS_ENGINE_URL", &adresse);

    tauri::Builder::default()
        .setup(move |application| {
            // L'adresse du moteur doit être connue du front : on l'injecte
            // dans la page avant tout script (un webview ne voit pas
            // l'environnement du processus parent).
            if let Some(fenetre) = application.get_webview_window("main") {
                let _ = fenetre.eval(&format!("window.OBSIDIAN_ENGINE_URL = {adresse:?};"));
            }
            println!("obsidian-rs : moteur prêt sur {adresse}");
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("fenêtre obsidian-rs")
        .run(move |_application, evenement| {
            // La fenêtre se ferme : on arrête le moteur — uniquement à la
            // sortie de l'application, pas au premier événement.
            if let tauri::RunEvent::Exit = evenement {
                let _ = moteur.kill();
                let _ = moteur.wait();
            }
        });
}

/// Démarre le moteur et attend qu'il réponde.
fn demarrer_moteur(racine: &Path, port: u16) -> Result<Child, String> {
    let binaire = trouver_moteur().ok_or_else(|| {
        "binaire `obsidian-engine` introuvable (OBSIDIAN_RS_ENGINE, à côté de \
         l'exécutable, ou dans le PATH)"
            .to_string()
    })?;

    let mut commande = Command::new(&binaire);
    commande
        .arg("--root")
        .arg(racine)
        .arg("--port")
        .arg(port.to_string())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    let mut moteur = commande
        .spawn()
        .map_err(|erreur| format!("démarrage de {} : {erreur}", binaire.display()))?;

    if !attendre_moteur(port) {
        let _ = moteur.kill();
        return Err(format!(
            "le moteur ne répond pas sur 127.0.0.1:{port} (le vault « {} » est-il lisible ?)",
            racine.display()
        ));
    }
    Ok(moteur)
}

/// Attend que le moteur réponde à `/health` (au plus `tentatives` fois).
fn attendre_moteur(port: u16) -> bool {
    let hote = format!("127.0.0.1:{port}");
    for _ in 0..60 {
        if requete_sante(&hote) {
            return true;
        }
        std::thread::sleep(std::time::Duration::from_millis(250));
    }
    false
}

/// Interroge `/health` avec une requête HTTP minimale (sans dépendance).
fn requete_sante(hote: &str) -> bool {
    use std::io::{Read, Write};
    let Ok(mut flux) = std::net::TcpStream::connect(hote) else {
        return false;
    };
    let requete = "GET /health HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n";
    if flux.write_all(requete.as_bytes()).is_err() {
        return false;
    }
    let mut reponse = Vec::new();
    let _ = flux.read_to_end(&mut reponse);
    reponse.starts_with(b"HTTP/1.1 200")
}

/// Cherche le binaire du moteur.
fn trouver_moteur() -> Option<PathBuf> {
    if let Ok(chemin) = std::env::var("OBSIDIAN_RS_ENGINE") {
        let chemin = PathBuf::from(chemin);
        if chemin.is_file() {
            return Some(chemin);
        }
    }
    if let Ok(executable) = std::env::current_exe() {
        if let Some(dossier) = executable.parent() {
            let candidat = dossier.join("obsidian-engine");
            if candidat.is_file() {
                return Some(candidat);
            }
        }
    }
    let chemin = which("obsidian-engine")?;
    if chemin.is_file() {
        Some(chemin)
    } else {
        None
    }
}

/// Cherche un exécutable dans le `PATH`.
fn which(nom: &str) -> Option<PathBuf> {
    let chemin = std::env::var_os("PATH")?;
    std::env::split_paths(&chemin)
        .map(|dossier| dossier.join(nom))
        .find(|candidat| candidat.is_file())
}

/// Racine du vault : variable d'environnement, argument, puis `demo-vault`.
fn racine_vault(arguments: &[String]) -> PathBuf {
    if let Ok(racine) = std::env::var("OBSIDIAN_RS_VAULT") {
        return PathBuf::from(racine);
    }
    if let Some(premier) = arguments.first() {
        if !premier.starts_with("--") {
            return PathBuf::from(premier);
        }
    }
    std::env::current_exe()
        .ok()
        .and_then(|executable| executable.parent().map(|dossier| dossier.to_path_buf()))
        .map(|dossier| dossier.join("demo-vault"))
        .unwrap_or_else(|| PathBuf::from("demo-vault"))
}

/// Port du moteur : argument `--port`, sinon variable d'environnement, sinon
/// le port par défaut.
fn port(arguments: &[String]) -> u16 {
    let mut suite = arguments.iter();
    while let Some(argument) = suite.next() {
        if argument == "--port" {
            if let Some(valeur) = suite.next() {
                if let Ok(port) = valeur.parse::<u16>() {
                    return port;
                }
            }
        }
    }
    std::env::var("OBSIDIAN_ENGINE_PORT")
        .ok()
        .and_then(|valeur| valeur.parse::<u16>().ok())
        .unwrap_or(PORT_PAR_DEFAUT)
}
