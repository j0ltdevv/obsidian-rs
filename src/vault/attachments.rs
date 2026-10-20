//! Pièces jointes : inventaire des fichiers non-note du vault et import
//! atomique d'une pièce jointe externe.

use std::fs;
use std::path::{Path, PathBuf};

/// Extensions de fichiers considérées comme des pièces jointes (minuscules).
pub const ATTACHMENT_EXTS: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "svg", "webp", "bmp", "ico", "pdf", "mp4", "webm", "ogg", "mp3",
    "wav", "flac", "zip", "epub",
];

/// Dossier par défaut où ranger les pièces jointes importées.
pub const DEFAULT_ATTACHMENT_DIR: &str = "attachments";

/// Une pièce jointe découverte dans le vault.
#[derive(Debug, Clone, PartialEq)]
pub struct Attachment {
    /// Chemin relatif au root du vault.
    pub path: PathBuf,
    /// Nom du fichier (avec extension).
    pub name: String,
    /// Extension en minuscules, sans point.
    pub ext: String,
    /// Taille en octets.
    pub size: u64,
}

/// `true` si `name` a une extension de pièce jointe.
pub fn is_attachment_name(name: &str) -> bool {
    let Some(ext) = Path::new(name).extension().and_then(|e| e.to_str()) else {
        return false;
    };
    ATTACHMENT_EXTS.contains(&ext.to_lowercase().as_str())
}

/// Scanne récursivement `root` à la recherche de pièces jointes. Les
/// dossiers cachés (commençant par `.`, dont `.obsidian`) sont ignorés.
pub fn scan_attachments(root: &Path) -> anyhow::Result<Vec<Attachment>> {
    let mut out = Vec::new();
    scan_dir(root, root, &mut out)?;
    out.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(out)
}

fn scan_dir(root: &Path, dir: &Path, out: &mut Vec<Attachment>) -> anyhow::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue;
        }
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            scan_dir(root, &path, out)?;
        } else if file_type.is_file() && is_attachment_name(&name) {
            let rel = path.strip_prefix(root)?.to_path_buf();
            let metadata = fs::metadata(&path)?;
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or_default()
                .to_lowercase();
            out.push(Attachment {
                path: rel,
                name,
                ext,
                size: metadata.len(),
            });
        }
    }
    Ok(())
}

/// Copie `source` dans le dossier `attachments` du vault (atomique), en
/// évitant tout écrasement. Retourne le chemin relatif créé.
///
/// `save_name` (optionnel) force le nom de destination ; sinon le nom du
/// fichier source est conservé. En cas de collision, un suffixe numérique
/// (`-1`, `-2`, …) est ajouté avant l'extension.
pub fn import_attachment(
    root: &Path,
    source: &Path,
    save_name: Option<&str>,
) -> anyhow::Result<PathBuf> {
    if !source.is_file() {
        anyhow::bail!("source absente : {}", source.display());
    }
    let dest_dir = root.join(DEFAULT_ATTACHMENT_DIR);
    fs::create_dir_all(&dest_dir)?;

    let base = match save_name {
        Some(name) => name.to_string(),
        None => source
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| anyhow::anyhow!("nom source invalide"))?
            .to_string(),
    };
    let stem = Path::new(&base)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("attachement");
    let ext: String = Path::new(&base)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| format!(".{e}"))
        .unwrap_or_default();

    let mut candidate = dest_dir.join(&base);
    let mut counter = 1u32;
    while candidate.exists() {
        candidate = dest_dir.join(format!("{stem}-{counter}{ext}"));
        counter += 1;
    }

    let tmp = dest_dir.join(format!(
        ".{}.tmp",
        candidate
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("tmp")
    ));
    fs::copy(source, &tmp)?;
    fs::rename(&tmp, &candidate)?;

    candidate
        .strip_prefix(root)
        .map(|p| p.to_path_buf())
        .map_err(|e| anyhow::anyhow!(e))
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Sandbox {
        dir: PathBuf,
    }

    impl Sandbox {
        fn new() -> Self {
            let mut dir = std::env::temp_dir();
            let unique = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            dir.push(format!("obsidian-rs-att-{unique}"));
            fs::create_dir_all(&dir).unwrap();
            Self { dir }
        }

        fn file(&self, rel: &str, contents: &str) -> PathBuf {
            let p = self.dir.join(rel);
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(&p, contents).unwrap();
            p
        }

        fn drop(&self) {
            let _ = fs::remove_dir_all(&self.dir);
        }
    }

    impl Drop for Sandbox {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.dir);
        }
    }

    #[test]
    fn scan_trouve_les_extensions_connues() {
        let sb = Sandbox::new();
        sb.file("img.png", "x");
        sb.file("doc.pdf", "y");
        sb.file("note.md", "z");
        sb.file("archive.zip", "w");
        let found = scan_attachments(&sb.dir).unwrap();
        let names: Vec<&str> = found.iter().map(|a| a.name.as_str()).collect();
        assert!(names.contains(&"img.png"));
        assert!(names.contains(&"doc.pdf"));
        assert!(names.contains(&"archive.zip"));
        assert!(!names.contains(&"note.md"));
        sb.drop();
    }

    #[test]
    fn scan_ignore_les_dossiers_cachees() {
        let sb = Sandbox::new();
        sb.file(".obsidian/a.png", "x");
        sb.file(".hidden/b.png", "y");
        sb.file("visible/c.png", "z");
        let found = scan_attachments(&sb.dir).unwrap();
        let names: Vec<&str> = found.iter().map(|a| a.name.as_str()).collect();
        assert_eq!(names, vec!["c.png"]);
        sb.drop();
    }

    #[test]
    fn scan_enregistre_la_taille() {
        let sb = Sandbox::new();
        sb.file("a.png", "hello");
        let found = scan_attachments(&sb.dir).unwrap();
        assert_eq!(found[0].size, 5);
        sb.drop();
    }

    #[test]
    fn import_copie_dans_attachments() {
        let sb = Sandbox::new();
        let src = sb.file("ext/source.png", "data");
        let rel = import_attachment(&sb.dir, &src, None).unwrap();
        assert_eq!(rel, PathBuf::from("attachments/source.png"));
        assert!(sb.dir.join(&rel).is_file());
        sb.drop();
    }

    #[test]
    fn import_collision_ajoute_un_suffixe() {
        let sb = Sandbox::new();
        sb.file("attachments/source.png", "old");
        let src = sb.file("ext2/source.png", "new");
        let rel = import_attachment(&sb.dir, &src, None).unwrap();
        assert_eq!(rel, PathBuf::from("attachments/source-1.png"));
        assert_eq!(fs::read_to_string(sb.dir.join(rel)).unwrap(), "new");
        sb.drop();
    }

    #[test]
    fn import_source_absente_erreur() {
        let sb = Sandbox::new();
        let missing = sb.dir.join("absent.png");
        assert!(import_attachment(&sb.dir, &missing, None).is_err());
        sb.drop();
    }
}
