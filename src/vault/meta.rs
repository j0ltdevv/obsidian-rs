//! Lecture et modification du frontmatter YAML d'une note.
//!
//! Le bloc frontmatter est le YAML délimité par `---` en tête de document.
//! Toutes les écritures passent par `ops::write_note` (atomique). L'ordre des
//! clés existantes est préservé ; les nouvelles clés sont ajoutées en fin de
//! bloc. Un YAML invalide ou un bloc non terminé ne sont jamais réécrits.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};

use super::frontmatter::parse_frontmatter;
use super::note::{Note, NoteMetadata};
use super::ops::write_note;
use super::tags::parse_tags;
use super::wikilinks::parse_wikilinks;

/// Met à jour une propriété du frontmatter YAML d'une note (clé existante
/// remplacée, clé absente ajoutée en fin de bloc). Écrit via
/// `ops::write_note` (atomique). Retourne la note re-parsée.
///
/// Aucune écriture n'est effectuée si la valeur est déjà celle demandée.
/// Le corps Markdown (tout ce qui suit le bloc) est préservé tel quel.
pub fn set_frontmatter_property(
    root: &Path,
    rel_path: &Path,
    key: &str,
    value: &serde_yaml::Value,
) -> Result<Note> {
    edit_frontmatter(root, rel_path, |mapping| {
        if mapping.get(key).is_some_and(|old| old == value) {
            // Valeur identique → aucun changement, pas d'écriture.
            return false;
        }
        mapping.insert(serde_yaml::Value::String(key.to_string()), value.clone());
        true
    })
}

/// Supprime une propriété du frontmatter (clé absente → Ok sans changement).
pub fn remove_frontmatter_property(root: &Path, rel_path: &Path, key: &str) -> Result<Note> {
    edit_frontmatter(root, rel_path, |mapping| {
        if !mapping.contains_key(key) {
            // Clé absente → aucun changement, pas d'écriture.
            return false;
        }
        // `shift_remove` préserve l'ordre relatif des clés restantes.
        mapping.shift_remove(key);
        true
    })
}

/// Retourne le frontmatter parsé actuel (Option), en utilisant le même
/// parsing que la note scannée (`scan::read_note`).
pub fn read_frontmatter(root: &Path, rel_path: &Path) -> Result<Option<serde_yaml::Value>> {
    let source = read_source(root, rel_path)?;
    Ok(parse_frontmatter(&source)?)
}

/// Applique `edit` au block frontmatter puis réécrit la note si un changement
/// a eu lieu. `edit` retourne `true` si le mapping a été modifié.
fn edit_frontmatter(
    root: &Path,
    rel_path: &Path,
    edit: impl FnOnce(&mut serde_yaml::Mapping) -> bool,
) -> Result<Note> {
    let source = read_source(root, rel_path)?;
    let (had_bom, source) = split_bom(&source);

    // Un YAML invalide ou un bloc non terminé → erreur, fichier intact.
    let parsed = parse_frontmatter(source)?;
    let mut mapping = match parsed {
        None => serde_yaml::Mapping::new(),
        Some(serde_yaml::Value::Mapping(map)) => map,
        // Un bloc vide (`---` / `---`) équivaut à un mapping vide.
        Some(serde_yaml::Value::Null) => serde_yaml::Mapping::new(),
        Some(_) => bail!("le frontmatter n'est pas un mapping"),
    };

    if !edit(&mut mapping) {
        // Aucun changement → la note n'est pas réécrite.
        return Ok(build_note(rel_path, source));
    }

    let interior = if mapping.is_empty() {
        String::new()
    } else {
        serde_yaml::to_string(&mapping)
            .map_err(|e| anyhow!("sérialisation du frontmatter : {e}"))?
            .trim_end_matches('\n')
            .to_string()
    };

    let body = body_after_block(source);
    let mut content = String::with_capacity(source.len() + interior.len() + 16);
    if had_bom {
        content.push('\u{FEFF}');
    }
    content.push_str("---\n");
    content.push_str(&interior);
    content.push('\n');
    content.push_str("---\n");
    content.push_str(&body);

    write_note(root, rel_path, &content)?;
    Ok(build_note(rel_path, &content))
}

/// Retourne tout ce qui suit le bloc frontmatter (corps Markdown), tel quel.
/// Sans bloc, retourne le document entier.
fn body_after_block(source: &str) -> String {
    let lines: Vec<&str> = source.split('\n').collect();
    if !lines.first().is_some_and(|line| line.trim() == "---") {
        return source.to_string();
    }
    match lines
        .iter()
        .enumerate()
        .skip(1)
        .find(|(_, line)| line.trim() == "---")
    {
        Some((closing, _)) => lines[closing + 1..].join("\n"),
        // Inatteignable : `parse_frontmatter` aurait déjà échoué.
        None => source.to_string(),
    }
}

/// Sépare un éventuel BOM UTF-8 en tête de document.
fn split_bom(source: &str) -> (bool, &str) {
    match source.strip_prefix('\u{FEFF}') {
        Some(rest) => (true, rest),
        None => (false, source),
    }
}

/// Lit le contenu d'une note, après vérification du root et confinement du
/// chemin relatif (mêmes garanties que `ops`).
fn read_source(root: &Path, rel_path: &Path) -> Result<String> {
    if !root.is_dir() {
        bail!("la racine du vault n'existe pas : {}", root.display());
    }
    let rel_path = confine_rel_path(rel_path)?;
    let target = root.join(&rel_path);
    fs::read_to_string(&target).with_context(|| format!("lecture du fichier {}", target.display()))
}

/// Vérifie que `rel_path` est un chemin relatif sûr : rejette tout chemin
/// absolu, tout composant ``..`` (remontée hors du vault) et le chemin vide,
/// puis retourne la forme normalisée (composants `.` redondants supprimés).
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
            std::path::Component::Normal(part) => normalized.push(part),
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                bail!("composant \"..\" interdit : {}", rel_path.display());
            }
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

/// Construit la `Note` correspondant à un chemin relatif et à son contenu,
/// en réutilisant les parseurs du vault (même logique que `ops::build_note`).
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
