//! Opérations d'écriture sur le vault (création, déplacement, écriture).
//!
//! Toutes les écritures sont atomiques : un fichier temporaire est écrit dans
//! le même dossier, puis déplacé sur la cible via `rename`.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};

use super::frontmatter::parse_frontmatter;
use super::note::{Note, NoteMetadata};
use super::tags::parse_tags;
use super::wikilinks::parse_wikilinks;

/// Crée une note `rel_path` sous la racine `root`, en créant les dossiers
/// intermédiaires au besoin. Échoue si la note existe déjà (pas d'écrasement
/// silencieux). Écriture atomique (fichier temporaire + rename).
pub fn create_note(root: &Path, rel_path: &Path, content: &str) -> Result<Note> {
    require_root(root)?;
    let target = root.join(rel_path);
    if target.exists() {
        bail!("la note existe déjà : {}", target.display());
    }
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).with_context(|| {
            format!(
                "création des dossiers intermédiaires pour {}",
                target.display()
            )
        })?;
    }
    write_atomic(&target, content)?;
    Ok(build_note(rel_path, content))
}

/// Renomme/déplace une note de `from` vers `to` (chemins relatifs au root).
/// Vérifie que `from` existe et que `to` n'existe pas déjà. S'appuie sur un
/// déplacement atomique (`rename`).
pub fn move_note(root: &Path, from: &Path, to: &Path) -> Result<()> {
    require_root(root)?;
    let source = root.join(from);
    let dest = root.join(to);
    if !source.is_file() {
        bail!("la note source n'existe pas : {}", source.display());
    }
    if dest.exists() {
        bail!("la note cible existe déjà : {}", dest.display());
    }
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).with_context(|| {
            format!(
                "création des dossiers intermédiaires pour {}",
                dest.display()
            )
        })?;
    }
    fs::rename(&source, &dest).with_context(|| {
        format!(
            "déplacement de {} vers {}",
            source.display(),
            dest.display()
        )
    })?;
    Ok(())
}

/// Écrit le contenu d'une note existante. Écriture atomique (fichier
/// temporaire + rename), sans jamais laisser de fichier temporaire.
pub fn write_note(root: &Path, rel_path: &Path, content: &str) -> Result<()> {
    require_root(root)?;
    let target = root.join(rel_path);
    if !target.is_file() {
        bail!("la note n'existe pas : {}", target.display());
    }
    write_atomic(&target, content)
}

/// Vérifie que la racine du vault existe et est un dossier.
fn require_root(root: &Path) -> Result<()> {
    if !root.is_dir() {
        bail!("la racine du vault n'existe pas : {}", root.display());
    }
    Ok(())
}

/// Écrit `content` dans `target` de façon atomique : fichier temporaire dans le
/// même dossier, puis `rename`. En cas d'erreur, le temporaire est nettoyé.
fn write_atomic(target: &Path, content: &str) -> Result<()> {
    let dir = target
        .parent()
        .ok_or_else(|| anyhow!("chemin sans dossier parent : {}", target.display()))?;
    let (temp, mut file) = open_unique_temp(dir, target)?;
    let result = (|| -> Result<()> {
        file.write_all(content.as_bytes())
            .with_context(|| format!("écriture du fichier temporaire {}", temp.display()))?;
        file.flush()
            .with_context(|| format!("flush du fichier temporaire {}", temp.display()))?;
        Ok(())
    })();
    if let Err(err) = result {
        let _ = fs::remove_file(&temp);
        return Err(err);
    }
    fs::rename(&temp, target)
        .with_context(|| format!("déplacement atomique vers {}", target.display()))?;
    Ok(())
}

/// Ouvre un fichier temporaire unique (`.{nom}.tmp-{pid}[-n]`) dans `dir`, en
/// `create_new` pour garantir l'unicité même en cas d'accès concurrent.
fn open_unique_temp(dir: &Path, target: &Path) -> Result<(PathBuf, fs::File)> {
    let name = target
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "note".to_string());
    let base = format!(".{name}.tmp-{}", std::process::id());
    for i in 0..100u32 {
        let temp = if i == 0 {
            dir.join(&base)
        } else {
            dir.join(format!("{base}-{i}"))
        };
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
        {
            Ok(file) => return Ok((temp, file)),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => {
                return Err(e)
                    .with_context(|| format!("création du fichier temporaire {}", temp.display()));
            }
        }
    }
    bail!(
        "impossible de créer un fichier temporaire unique dans {}",
        dir.display()
    )
}

/// Construit la `Note` correspondant à un chemin relatif et à son contenu,
/// en réutilisant les parseurs du vault (frontmatter, tags, wikilinks).
fn build_note(rel_path: &Path, content: &str) -> Note {
    let (frontmatter, frontmatter_error) = match parse_frontmatter(content) {
        Ok(parsed) => (parsed, None),
        Err(err) => (None, Some(err.to_string())),
    };
    let tags = merge_tags(parse_tags(content), frontmatter.as_ref());
    let metadata = NoteMetadata {
        frontmatter,
        frontmatter_error,
        wikilinks: parse_wikilinks(content),
        tags,
    };
    let name = rel_path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or_default()
        .to_string();
    Note {
        path: rel_path.to_path_buf(),
        name,
        metadata,
    }
}

/// Fusionne les tags inline avec ceux du frontmatter (liste ou scalaire),
/// dédoublonnés en conservant l'ordre de première apparition.
fn merge_tags(inline: Vec<String>, frontmatter: Option<&serde_yaml::Value>) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut merged = Vec::new();
    for tag in inline {
        if seen.insert(tag.clone()) {
            merged.push(tag);
        }
    }
    if let Some(value) = frontmatter.and_then(|fm| fm.get("tags")) {
        match value {
            serde_yaml::Value::Sequence(items) => {
                for item in items {
                    if let serde_yaml::Value::String(tag) = item {
                        if seen.insert(tag.clone()) {
                            merged.push(tag.clone());
                        }
                    }
                }
            }
            serde_yaml::Value::String(tag) if seen.insert(tag.clone()) => {
                merged.push(tag.clone());
            }
            _ => {}
        }
    }
    merged
}
