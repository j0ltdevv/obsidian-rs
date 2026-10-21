//! Corbeille du vault : inventaire, restauration et vidage.
//!
//! Format produit par l'application (`src/app/mod.rs`, `Message::DeleteConfirm`
//! et `Message::DeleteSelected`) : une note supprimée est déplacée de
//! `<vault>/.trash/<horodatage>/<chemin>` où `<horodatage>` est le nombre de
//! secondes depuis l'epoch Unix et `<chemin>` le chemin relatif de la note dans
//! le vault. Ce module lit ce format tel quel (sans le modifier) et fournit la
//! restauration et le vidage qui manquent aujourd'hui.
//!
//! Sécurité : aucune opération ne suit de lien symbolique, ne remonte hors du
//! vault (composant `..`, chemin absolu) ni n'écrase un fichier existant.

use std::fs;
use std::path::{Component, Path, PathBuf};

/// Erreurs de manipulation de la corbeille.
#[derive(Debug, thiserror::Error)]
pub enum TrashError {
    /// La racine du vault n'existe pas ou n'est pas un dossier.
    #[error("la racine du vault n'existe pas : {0}")]
    RootNotFound(String),
    /// Un chemin relatif est piégé (absolu, `..`, vide).
    #[error("chemin relatif invalide : {0}")]
    InvalidRelativePath(String),
    /// Une entrée de la corbeille contient un lien symbolique.
    #[error("entrée de corbeille protégée (lien symbolique) : {0}")]
    SymlinkEntry(String),
    /// Une entrée de la corbeille est illisible ou malformée.
    #[error("entrée de corbeille illisible : {0}")]
    Unreadable(String),
    /// La cible de restauration existe déjà dans le vault.
    #[error("la note existe déjà à la destination : {0}")]
    DestinationExists(String),
    /// Erreur d'E/S sous-jacente.
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// Une entrée de la corbeille.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrashEntry {
    /// Chemin d'origine de l'élément supprimé, relatif au vault.
    pub original_path: PathBuf,
    /// Date de suppression : horodatage Unix en secondes.
    pub deleted_at: u64,
    /// Taille en octets de l'élément supprimé.
    pub size: u64,
}

/// Inventorie la corbeille du vault.
///
/// Parcourt `.trash/<horodatage>/<chemin>` et retourne une entrée par élément
/// supprimé, triée par date de suppression croissante. Retourne une liste vide
/// si la corbeille n'existe pas.
///
/// # Sécurité
///
/// Toute entrée non conforme est refusée par une erreur (jamais ignorée) :
/// - un nom de dossier d'horodatage qui n'est pas un nombre entier ;
/// - un lien symbolique où que ce soit dans l'arborescence de la corbeille ;
/// - un fichier ou dossier directement sous `.trash` hors d'un dossier
///   d'horodatage ;
/// - un chemin relatif piégé (`..`, absolu, vide).
pub fn list_trash(root: &Path) -> Result<Vec<TrashEntry>, TrashError> {
    let trash_root = root.join(".trash");
    if !trash_root.exists() {
        return Ok(Vec::new());
    }
    let meta = fs::symlink_metadata(&trash_root)?;
    if meta.file_type().is_symlink() {
        return Err(TrashError::SymlinkEntry(trash_root.display().to_string()));
    }
    if !meta.is_dir() {
        return Err(TrashError::Unreadable(trash_root.display().to_string()));
    }

    let mut stamps: Vec<(u64, PathBuf)> = Vec::new();
    for entry in fs::read_dir(&trash_root)? {
        let entry = entry?;
        let path = entry.path();
        let meta = fs::symlink_metadata(&path)?;
        if meta.file_type().is_symlink() {
            return Err(TrashError::SymlinkEntry(path.display().to_string()));
        }
        if !meta.is_dir() {
            return Err(TrashError::Unreadable(path.display().to_string()));
        }
        let stamp: u64 = entry
            .file_name()
            .to_string_lossy()
            .parse()
            .map_err(|_| TrashError::Unreadable(path.display().to_string()))?;
        stamps.push((stamp, path));
    }
    // Tri stable par date de suppression pour un inventaire déterministe.
    stamps.sort_by_key(|(stamp, _)| *stamp);

    let mut out = Vec::new();
    for (stamp, path) in stamps {
        walk_stamp(&path, stamp, &PathBuf::new(), &mut out)?;
    }
    Ok(out)
}

/// Parcourt récursivement un dossier d'horodatage et collecte les entrées.
///
/// `stamp_dir` est le chemin du dossier d'horodatage (pour les messages
/// d'erreur), `stamp` sa valeur numérique, `prefix` le chemin relatif accumulé
/// dans le dossier et `out` le vecteur d'entrées à remplir.
fn walk_stamp(
    stamp_dir: &Path,
    stamp: u64,
    prefix: &Path,
    out: &mut Vec<TrashEntry>,
) -> Result<(), TrashError> {
    for entry in fs::read_dir(stamp_dir)? {
        let entry = entry?;
        let path = entry.path();
        let meta = fs::symlink_metadata(&path)?;
        if meta.file_type().is_symlink() {
            return Err(TrashError::SymlinkEntry(path.display().to_string()));
        }
        let name = entry.file_name();
        if meta.is_dir() {
            walk_stamp(&path, stamp, &prefix.join(&name), out)?;
        } else if meta.is_file() {
            let original_path = confine_child(&prefix.join(&name))?;
            out.push(TrashEntry {
                original_path,
                deleted_at: stamp,
                size: meta.len(),
            });
        }
        // Les autres types de fichier (tubes, sockets…) sont ignorés.
    }
    Ok(())
}

/// Restaure un élément supprimé à son chemin d'origine dans le vault.
///
/// Déplace l'élément de `.trash/<horodatage>/<chemin>` vers `<chemin>` (relatif
/// au vault). La restauration échoue proprement si la cible existe déjà
/// (aucun écrasement) ou si l'entrée est piégée (lien symbolique, chemin
/// absolu, `..`). L'écriture est atomique (déplacement via `rename`).
pub fn restore_trash(root: &Path, entry: &TrashEntry) -> Result<(), TrashError> {
    if !root.is_dir() {
        return Err(TrashError::RootNotFound(root.display().to_string()));
    }
    let original = confine_child(&entry.original_path)?;

    let source = root
        .join(".trash")
        .join(entry.deleted_at.to_string())
        .join(&original);
    let src_meta = fs::symlink_metadata(&source)?;
    if src_meta.file_type().is_symlink() || !src_meta.is_file() {
        return Err(TrashError::Unreadable(source.display().to_string()));
    }

    let dest = root.join(&original);
    // `symlink_metadata` détecte aussi les liens symboliques morts : toute
    // présence est refusée, sans écrasement. Résidu TOCTOU (cible apparue entre
    // la vérification et le `rename`) accepté en mono-utilisateur.
    if fs::symlink_metadata(&dest).is_ok() {
        return Err(TrashError::DestinationExists(dest.display().to_string()));
    }
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::rename(&source, &dest)?;
    Ok(())
}

/// Vide la corbeille : supprime définitivement tout son contenu.
///
/// # Confirmation obligatoire
///
/// Cette opération est irréversible : elle détruit les éléments supprimés sans
/// possibilité de restauration. L'appelant DOIT s'assurer que l'utilisateur a
/// explicitement confirmé avant d'appeler cette fonction.
pub fn empty_trash(root: &Path) -> Result<(), TrashError> {
    let trash_root = root.join(".trash");
    let meta = fs::symlink_metadata(&trash_root)?;
    if meta.file_type().is_symlink() {
        return Err(TrashError::SymlinkEntry(trash_root.display().to_string()));
    }
    if !meta.is_dir() {
        return Err(TrashError::Unreadable(trash_root.display().to_string()));
    }
    fs::remove_dir_all(&trash_root)?;
    Ok(())
}

/// Vérifie que `rel_path` est un chemin relatif sûr : rejette tout chemin
/// absolu, tout composant `..` et le chemin vide, puis retourne la forme
/// normalisée (composants `.` redondants supprimés).
fn confine_child(rel_path: &Path) -> Result<PathBuf, TrashError> {
    if rel_path.as_os_str().is_empty() {
        return Err(TrashError::InvalidRelativePath("(vide)".to_string()));
    }
    if rel_path.is_absolute() {
        return Err(TrashError::InvalidRelativePath(
            rel_path.display().to_string(),
        ));
    }
    let mut normalized = PathBuf::new();
    for component in rel_path.components() {
        match component {
            Component::Normal(part) => normalized.push(part),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(TrashError::InvalidRelativePath(
                    rel_path.display().to_string(),
                ));
            }
        }
    }
    if normalized.as_os_str().is_empty() {
        return Err(TrashError::InvalidRelativePath(
            rel_path.display().to_string(),
        ));
    }
    Ok(normalized)
}
