//! Construction de la coquille de bureau : le script de build de Tauri.
//!
//! Il ne fait que générer les ressources embarquées (fenêtre, icônes,
//! configuration) à partir de `tauri.conf.json`.

fn main() {
    tauri_build::build();
}
