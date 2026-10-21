//! Binaire `obsidian-engine` : sert le moteur de vault d'obsidian-rs en HTTP
//! JSON local, pour qu'un front end non Rust (moteur de rendu d'Obsidian,
//! Ignis) puisse consommer le cœur Rust du vault.
//!
//! ```sh
//! obsidian-engine --root <chemin du vault> [--port N]
//! ```
//!
//! Le port peut également être fourni par la variable d'environnement
//! `OBSIDIAN_ENGINE_PORT`. À défaut : `27123`. Le binaire n'écoute que sur
//! `127.0.0.1`.

use std::net::TcpListener;
use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{Context, Result};
use obsidian_rs::engine::{serve, Engine, DEFAULT_PORT, PORT_ENV};

/// Affiche l'usage et quitte avec le code 2.
fn usage() -> ! {
    eprintln!("Usage : obsidian-engine --root <chemin du vault> [--port N]");
    eprintln!("  --root <chemin>  racine du vault (obligatoire)");
    eprintln!("  --port <N>       port d'écoute (défaut : {DEFAULT_PORT}, variable {PORT_ENV})");
    std::process::exit(2);
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("[obsidian-engine] erreur : {err:#}");
            ExitCode::FAILURE
        }
    }
}

/// Parse les arguments, ouvre le vault et sert jusqu'à l'arrêt du processus.
fn run() -> Result<()> {
    let mut root: Option<PathBuf> = None;
    let mut port: Option<u16> = None;
    let mut args = std::env::args().skip(1);
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--root" => {
                let value = args.next().unwrap_or_else(|| usage());
                root = Some(PathBuf::from(value));
            }
            "--port" => {
                let value = args.next().unwrap_or_else(|| usage());
                match value.parse::<u16>() {
                    Ok(parsed) => port = Some(parsed),
                    Err(_) => {
                        eprintln!("port invalide : {value}");
                        usage();
                    }
                }
            }
            "--help" | "-h" => usage(),
            other => {
                eprintln!("argument inconnu : {other}");
                usage();
            }
        }
    }

    let Some(root) = root else {
        usage();
    };
    let port = port
        .or_else(|| {
            std::env::var(PORT_ENV)
                .ok()
                .and_then(|value| value.parse::<u16>().ok())
        })
        .unwrap_or(DEFAULT_PORT);

    let engine =
        Engine::open(&root).with_context(|| format!("ouverture du vault {}", root.display()))?;
    let address = format!("127.0.0.1:{port}");
    let listener = TcpListener::bind(&address).with_context(|| format!("écoute sur {address}"))?;
    let bound = listener.local_addr().context("adresse d'écoute")?;
    println!(
        "obsidian-engine : http://{bound} (vault : {})",
        engine.root().display()
    );
    serve(engine, listener).context("service HTTP")?;
    Ok(())
}
