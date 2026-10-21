//! Confinement des chemins fournis par les clients.
//!
//! Tout chemin reçu par l'API est relatif à la racine du vault. Le
//! confinement est fait en deux temps :
//!
//! 1. **normalisation lexicale** : le chemin ne doit être ni vide, ni
//!    absolu, ni contenir de composant `..` ;
//! 2. **vérification physique** : le chemin le plus proche de la racine qui
//!    existe est canonisé, et le résultat doit rester sous la racine
//!    canonique du vault. Un lien symbolique pointant hors du vault est
//!    donc refusé.

use std::fs;
use std::path::{Component, Path, PathBuf};

/// Erreurs de confinement.
#[derive(Debug, thiserror::Error)]
pub enum PathError {
    /// Chemin vide (ou réduit à vide après normalisation).
    #[error("chemin relatif vide")]
    Empty,
    /// Chemin absolu : interdit.
    #[error("chemin absolu interdit : {0}")]
    Absolute(String),
    /// Composant `..` : remontée hors du vault.
    #[error("composant « .. » interdit : {0}")]
    Escape(String),
    /// Le chemin canonique sort de la racine du vault.
    #[error("le chemin « {0} » sort de la racine du vault")]
    Outside(String),
}

/// Normalise un chemin relatif fourni par un client.
///
/// Refuse le chemin vide, le chemin absolu et tout composant `..` ; les
/// composants `.` redondants sont supprimés.
pub fn normalize_relative(input: &str) -> Result<PathBuf, PathError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(PathError::Empty);
    }
    let path = Path::new(trimmed);
    if path.is_absolute() {
        return Err(PathError::Absolute(trimmed.to_string()));
    }
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::ParentDir => return Err(PathError::Escape(trimmed.to_string())),
            Component::CurDir => {}
            Component::Normal(part) => normalized.push(part),
            Component::RootDir | Component::Prefix(_) => {
                return Err(PathError::Absolute(trimmed.to_string()));
            }
        }
    }
    if normalized.as_os_str().is_empty() {
        return Err(PathError::Empty);
    }
    Ok(normalized)
}

/// Vérifie que `target` (chemin absolu) reste sous `root` (racine canonique).
///
/// Le composant inexistant le plus proche de la racine est canonisé ; le
/// résultat doit être la racine du vault ou l'un de ses descendants.
pub fn ensure_inside(root: &Path, target: &Path, display: &str) -> Result<(), PathError> {
    let mut existing = target.to_path_buf();
    loop {
        match fs::canonicalize(&existing) {
            Ok(canonical) => {
                if canonical.starts_with(root) {
                    return Ok(());
                }
                return Err(PathError::Outside(display.to_string()));
            }
            Err(_) => {
                let Some(parent) = existing.parent().map(Path::to_path_buf) else {
                    return Err(PathError::Outside(display.to_string()));
                };
                if parent.as_os_str().is_empty() || parent == existing {
                    return Err(PathError::Outside(display.to_string()));
                }
                existing = parent;
            }
        }
    }
}
