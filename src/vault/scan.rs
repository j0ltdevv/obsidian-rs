use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

use super::frontmatter::parse_frontmatter;
use super::note::{Note, NoteMetadata};
use super::tags::parse_tags;
use super::wikilinks::parse_wikilinks;

/// Parcourt récursivement `root` et retourne toutes les notes `*.md`,
/// à l'exception des dossiers cachés (`.obsidian`, `.git`, …).
pub fn scan_vault(root: &Path) -> Result<Vec<Note>> {
    let mut notes = Vec::new();
    scan_dir(root, root, &mut notes)?;
    Ok(notes)
}

fn scan_dir(root: &Path, dir: &Path, notes: &mut Vec<Note>) -> Result<()> {
    for entry in
        fs::read_dir(dir).with_context(|| format!("lecture du dossier {}", dir.display()))?
    {
        let entry = entry.with_context(|| format!("entrée illisible dans {}", dir.display()))?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .with_context(|| format!("type de fichier illisible pour {}", path.display()))?;

        if file_type.is_dir() {
            let is_hidden = path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with('.'));
            if !is_hidden {
                scan_dir(root, &path, notes)?;
            }
        } else if file_type.is_file() && is_markdown(&path) {
            notes.push(read_note(root, &path)?);
        }
    }
    Ok(())
}

fn is_markdown(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("md"))
}

fn read_note(root: &Path, path: &Path) -> Result<Note> {
    let source = fs::read_to_string(path)
        .with_context(|| format!("lecture du fichier {}", path.display()))?;

    let (frontmatter, frontmatter_error) = match parse_frontmatter(&source) {
        Ok(parsed) => (parsed, None),
        Err(err) => (None, Some(err.to_string())),
    };

    let tags = merge_tags(parse_tags(&source), frontmatter.as_ref());

    let metadata = NoteMetadata {
        frontmatter,
        frontmatter_error,
        wikilinks: parse_wikilinks(&source),
        tags,
    };

    let name = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or_default()
        .to_string();

    let relative = path.strip_prefix(root).unwrap_or(path);

    Ok(Note {
        path: relative.to_path_buf(),
        name,
        metadata,
    })
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

/// Retourne les noms de notes en doublon (même `name` dans des dossiers
/// différents), triés, chaque nom n'apparaissant qu'une fois.
pub fn duplicate_names(notes: &[Note]) -> Vec<String> {
    let mut by_name = std::collections::BTreeMap::<String, usize>::new();
    for note in notes {
        *by_name.entry(note.name.clone()).or_insert(0) += 1;
    }
    by_name
        .into_iter()
        .filter_map(|(name, count)| (count > 1).then_some(name))
        .collect()
}
