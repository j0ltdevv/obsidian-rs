//! Versions locales (snapshots) des notes.
//!
//! Les snapshots sont stockés dans un dossier caché à la racine du vault
//! (`<root>/.obsy-history`), donc ignorés par le scan du vault. Chaque
//! note possède son propre sous-dossier, avec la même arborescence relative
//! que la note : `dossier/note.md/`. Les snapshots ne sont jamais supprimés.
//!
//! Le nom d'un snapshot commence par l'horodatage système en millisecondes,
//! suivi d'un numéro de séquence pour garantir l'unicité et l'ordre même
//! lorsque deux snapshots sont pris dans la même milliseconde :
//! `<millis>-<séquence>.md`. L'écriture est atomique : le contenu est écrit
//! dans un fichier temporaire, puis lié sous son nom définitif (le lien échoue
//! plutôt que d'écraser un snapshot existant).

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{bail, Context, Result};

/// Nom du dossier caché contenant les snapshots.
const HISTORY_DIR: &str = ".obsy-history";

/// Retourne le dossier racine des snapshots pour le vault `root`
/// (`<root>/.obsy-history`), sans le créer.
pub fn snapshot_root(root: &Path) -> PathBuf {
    root.join(HISTORY_DIR)
}

/// Enregistre `content` comme snapshot de la note `rel_path` (chemin relatif au
/// vault) et retourne le chemin du fichier créé. Les dossiers intermédiaires
/// sont créés à la demande. Écriture atomique, sans jamais écraser un snapshot
/// existant.
pub fn snapshot_note(root: &Path, rel_path: &Path, content: &str) -> Result<PathBuf> {
    require_root(root)?;
    let rel_path = confine_rel_path(rel_path)?;
    let dir = snapshot_root(root).join(&rel_path);
    fs::create_dir_all(&dir)
        .with_context(|| format!("création du dossier de snapshots pour {}", dir.display()))?;
    let stamp = timestamp_millis()?;
    write_snapshot(&dir, stamp, content)
}

/// Liste les snapshots de la note `rel_path`, du plus ancien au plus récent.
/// Retourne une liste vide si aucun snapshot n'existe pour cette note.
pub fn list_snapshots(root: &Path, rel_path: &Path) -> Result<Vec<PathBuf>> {
    require_root(root)?;
    let rel_path = confine_rel_path(rel_path)?;
    let dir = snapshot_root(root).join(&rel_path);
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut snapshots = Vec::new();
    for entry in
        fs::read_dir(&dir).with_context(|| format!("lecture du dossier {}", dir.display()))?
    {
        let entry = entry.with_context(|| format!("entrée illisible dans {}", dir.display()))?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .with_context(|| format!("type de fichier illisible pour {}", path.display()))?;
        if !file_type.is_file() {
            continue;
        }
        // Les fichiers temporaires (et tout autre fichier caché) ne sont pas
        // des snapshots : ils sont ignorés, notamment après un arrêt brutal.
        let hidden = path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with('.'));
        if !hidden {
            snapshots.push(path);
        }
    }
    snapshots.sort();
    Ok(snapshots)
}

/// Lit le snapshot `snap` de la note `rel_path` et retourne son contenu exact.
///
/// `snap` peut être le chemin retourné par [`list_snapshots`], ou simplement le
/// nom du fichier de snapshot (relatif au dossier de snapshots de la note).
/// Le chemin est confiné : tout fichier situé hors du dossier de snapshots de
/// la note est refusé. L'écriture de la note restaurée reste à la charge de
/// l'appelant (`ops::write_note`).
pub fn restore_snapshot(root: &Path, rel_path: &Path, snap: &Path) -> Result<String> {
    require_root(root)?;
    let rel_path = confine_rel_path(rel_path)?;
    let dir = snapshot_root(root).join(&rel_path);
    let snapshot = resolve_snapshot_path(&dir, snap)?;
    fs::read_to_string(&snapshot)
        .with_context(|| format!("lecture du snapshot {}", snapshot.display()))
}

/// Vérifie que la racine du vault existe et est un dossier.
fn require_root(root: &Path) -> Result<()> {
    if !root.is_dir() {
        bail!("la racine du vault n'existe pas : {}", root.display());
    }
    Ok(())
}

/// Même confinement que `ops` : rejette les chemins absolus, vides ou
/// contenant ``..``, puis retourne la forme normalisée (composants `.`
/// redondants supprimés).
fn confine_rel_path(rel_path: &Path) -> Result<PathBuf> {
    if rel_path.as_os_str().is_empty() {
        bail!("chemin relatif vide");
    }
    if rel_path.is_absolute() {
        bail!("chemin absolu interdit : {}", rel_path.display());
    }
    let mut normalized = PathBuf::new();
    for component in rel_path.components() {
        match component {
            std::path::Component::ParentDir => {
                bail!("composant \"..\" interdit : {}", rel_path.display());
            }
            std::path::Component::CurDir => {}
            std::path::Component::Normal(part) => normalized.push(part),
            std::path::Component::RootDir | std::path::Component::Prefix(_) => {
                bail!("chemin absolu interdit : {}", rel_path.display());
            }
        }
    }
    if normalized.as_os_str().is_empty() {
        bail!("chemin relatif vide après normalisation");
    }
    Ok(normalized)
}

/// Horodatage courant en millisecondes depuis l'époque UNIX.
fn timestamp_millis() -> Result<u128> {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("horloge système antérieure à l'époque UNIX")?;
    Ok(elapsed.as_millis())
}

/// Écrit un snapshot atomique dans `dir` et retourne son chemin définitif.
fn write_snapshot(dir: &Path, stamp: u128, content: &str) -> Result<PathBuf> {
    let (temp_path, mut temp) = open_unique_temp(dir)?;
    let written = temp
        .write_all(content.as_bytes())
        .with_context(|| format!("écriture du snapshot temporaire {}", temp_path.display()))
        .and_then(|()| {
            temp.flush()
                .with_context(|| format!("flush du snapshot temporaire {}", temp_path.display()))
        });
    if let Err(err) = written {
        drop(temp);
        let _ = fs::remove_file(&temp_path);
        return Err(err);
    }
    drop(temp);
    let target = match bind_snapshot(dir, stamp, &temp_path) {
        Ok(target) => target,
        Err(err) => {
            let _ = fs::remove_file(&temp_path);
            return Err(err);
        }
    };
    let _ = fs::remove_file(&temp_path);
    Ok(target)
}

/// Ouvre un fichier temporaire unique (`.snapshot-{pid}[-n].tmp`) dans `dir`,
/// en `create_new` pour garantir l'unicité même en cas d'accès concurrent.
fn open_unique_temp(dir: &Path) -> Result<(PathBuf, fs::File)> {
    let base = format!(".snapshot-{}.tmp", std::process::id());
    for i in 0..100u32 {
        let path = if i == 0 {
            dir.join(&base)
        } else {
            dir.join(format!("{base}-{i}"))
        };
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(file) => return Ok((path, file)),
            Err(err) if err.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(err) => {
                return Err(err)
                    .with_context(|| format!("création du fichier temporaire {}", path.display()));
            }
        }
    }
    bail!(
        "impossible de créer un fichier temporaire unique dans {}",
        dir.display()
    )
}

/// Lie le fichier temporaire `temp` sous un nom définitif unique
/// (`<millis>-<séquence>.md`). Le lien échoue si le nom existe déjà, ce qui
/// garantit qu'aucun snapshot n'est écrasé, même en cas de concurrence.
fn bind_snapshot(dir: &Path, stamp: u128, temp: &Path) -> Result<PathBuf> {
    for seq in 0..1000u32 {
        let target = dir.join(format!("{stamp}-{seq:03}.md"));
        match fs::hard_link(temp, &target) {
            Ok(()) => return Ok(target),
            Err(err) if err.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(err) => {
                return Err(err).with_context(|| {
                    format!("création du snapshot définitif {}", target.display())
                });
            }
        }
    }
    bail!(
        "impossible de nommer un snapshot unique dans {}",
        dir.display()
    )
}

/// Résout et confine le chemin d'un snapshot au dossier `dir`.
fn resolve_snapshot_path(dir: &Path, snap: &Path) -> Result<PathBuf> {
    if snap.as_os_str().is_empty() {
        bail!("chemin de snapshot vide");
    }
    let candidate = if snap.is_absolute() || snap.components().count() > 1 {
        snap.to_path_buf()
    } else {
        dir.join(confine_rel_path(snap)?)
    };
    if !candidate.is_file() {
        bail!("le snapshot n'existe pas : {}", candidate.display());
    }
    let dir = fs::canonicalize(dir)
        .with_context(|| format!("résolution du dossier de snapshots {}", dir.display()))?;
    let candidate = fs::canonicalize(&candidate)
        .with_context(|| format!("résolution du snapshot {}", candidate.display()))?;
    if candidate.parent() != Some(dir.as_path()) {
        bail!("snapshot hors de la note : {}", candidate.display());
    }
    Ok(candidate)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    /// Vault éphémère, supprimé à la chute du scope.
    struct TempVault(PathBuf);

    impl TempVault {
        fn new() -> Self {
            let count = COUNTER.fetch_add(1, Ordering::SeqCst);
            let path =
                std::env::temp_dir().join(format!("obsy-history-{}-{}", std::process::id(), count));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).expect("création du vault temporaire");
            TempVault(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempVault {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn snapshot_cree_fichier_et_dossier() {
        let vault = TempVault::new();
        let snap = snapshot_note(vault.path(), Path::new("note.md"), "contenu initial")
            .expect("création du snapshot");

        assert!(snap.is_file());
        assert_eq!(
            fs::read_to_string(&snap).expect("lecture du snapshot"),
            "contenu initial"
        );
        assert!(snapshot_root(vault.path()).is_dir());
        assert!(snap.starts_with(snapshot_root(vault.path())));
    }

    #[test]
    fn deux_snapshots_distincts_et_ordonnes() {
        let vault = TempVault::new();
        let first =
            snapshot_note(vault.path(), Path::new("note.md"), "version 1").expect("snapshot 1");
        let second =
            snapshot_note(vault.path(), Path::new("note.md"), "version 2").expect("snapshot 2");

        assert_ne!(first, second);
        let snapshots =
            list_snapshots(vault.path(), Path::new("note.md")).expect("liste des snapshots");
        assert_eq!(snapshots, vec![first, second]);
    }

    #[test]
    fn restore_retourne_le_contenu_exact() {
        let vault = TempVault::new();
        let content = "Ligne 1\nLigne 2 — été 🚀\n";
        let snap = snapshot_note(vault.path(), Path::new("note.md"), content)
            .expect("création du snapshot");
        fs::write(vault.path().join("note.md"), "contenu modifié").expect("écriture de la note");

        let restored = restore_snapshot(vault.path(), Path::new("note.md"), &snap)
            .expect("restauration du snapshot");
        assert_eq!(restored, content);
    }

    #[test]
    fn chemin_relatif_respecte() {
        let vault = TempVault::new();
        let rel_path = Path::new("dossier/sous/note.md");
        let snap = snapshot_note(vault.path(), rel_path, "contenu").expect("création du snapshot");

        assert_eq!(
            snap.parent().expect("dossier parent du snapshot"),
            snapshot_root(vault.path()).join("dossier/sous/note.md")
        );
        assert_eq!(
            list_snapshots(vault.path(), rel_path).expect("liste des snapshots"),
            vec![snap.clone()]
        );
        assert!(list_snapshots(vault.path(), Path::new("note.md"))
            .expect("liste des snapshots d'une note sans historique")
            .is_empty());
        assert_eq!(
            restore_snapshot(vault.path(), rel_path, &snap).expect("restauration"),
            "contenu"
        );
    }

    #[test]
    fn erreur_si_hors_root() {
        let vault = TempVault::new();
        assert!(snapshot_note(vault.path(), Path::new("../evasion.md"), "contenu").is_err());
        assert!(snapshot_note(vault.path(), &vault.path().join("absolu.md"), "contenu").is_err());
        assert!(list_snapshots(vault.path(), Path::new("../note.md")).is_err());

        let snap =
            snapshot_note(vault.path(), Path::new("note.md"), "ok").expect("création du snapshot");
        let outside = vault.path().join("dehors.md");
        fs::write(&outside, "secret").expect("écriture hors du dossier de snapshots");
        assert!(restore_snapshot(vault.path(), Path::new("note.md"), &outside).is_err());
        assert!(restore_snapshot(
            vault.path(),
            Path::new("note.md"),
            Path::new("../dehors.md")
        )
        .is_err());
        assert!(restore_snapshot(vault.path(), Path::new("note.md"), &snap).is_ok());
    }

    #[test]
    fn dossier_cree_a_la_demande() {
        let vault = TempVault::new();
        assert!(!snapshot_root(vault.path()).exists());

        let snap = snapshot_note(vault.path(), Path::new("a/b/c.md"), "contenu")
            .expect("création du snapshot");
        assert!(snapshot_root(vault.path()).is_dir());
        assert!(snap.parent().expect("dossier parent").is_dir());
    }

    #[test]
    fn contenu_vide_autorise() {
        let vault = TempVault::new();
        let snap = snapshot_note(vault.path(), Path::new("note.md"), "")
            .expect("création d'un snapshot vide");

        assert_eq!(
            fs::metadata(&snap).expect("métadonnées du snapshot").len(),
            0
        );
        assert_eq!(
            restore_snapshot(vault.path(), Path::new("note.md"), &snap)
                .expect("restauration du snapshot vide"),
            ""
        );
    }

    #[test]
    fn restore_accepte_le_nom_du_fichier() {
        let vault = TempVault::new();
        let snap = snapshot_note(vault.path(), Path::new("note.md"), "version")
            .expect("création du snapshot");
        let name = snap.file_name().expect("nom du fichier de snapshot");

        assert_eq!(
            restore_snapshot(vault.path(), Path::new("note.md"), Path::new(name))
                .expect("restauration par nom"),
            "version"
        );
    }

    #[test]
    fn racine_cachee_ignoree_par_le_scan() {
        let vault = TempVault::new();
        assert_eq!(
            snapshot_root(vault.path())
                .file_name()
                .and_then(|name| name.to_str()),
            Some(".obsy-history")
        );
        snapshot_note(vault.path(), Path::new("note.md"), "contenu").expect("création du snapshot");

        let notes = super::super::scan_vault(vault.path()).expect("scan du vault");
        assert!(notes.is_empty());
    }
}
