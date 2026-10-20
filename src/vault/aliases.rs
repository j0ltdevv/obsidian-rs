//! Aliases de notes : extraits du frontmatter (`aliases:`), utilisés pour
//! résoudre des liens alternatifs vers une note.

use std::collections::HashMap;
use std::path::PathBuf;

use super::note::Note;

/// Une référence d'alias vers une note.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AliasRef {
    /// Alias tel que déclaré (trimé, casse conservée).
    pub alias: String,
    /// Chemin relatif de la note cible.
    pub note_path: PathBuf,
}

/// Extrait la liste des alias d'un frontmatter YAML.
///
/// Lit la clé `aliases`. Valeur chaîne → un seul alias (trimé). Séquence →
/// chaque élément chaîne (trimé), les éléments non-chaîne sont ignorés. Clé
/// absente, valeur non-chaîne/non-séquence, ou frontmatter absent → liste vide.
pub fn extract_aliases(frontmatter: &Option<serde_yaml::Value>) -> Vec<String> {
    let Some(serde_yaml::Value::Mapping(map)) = frontmatter else {
        return Vec::new();
    };
    let Some(serde_yaml::Value::String(key)) = map
        .keys()
        .find(|k| matches!(k, serde_yaml::Value::String(s) if s == "aliases"))
    else {
        return Vec::new();
    };
    let _ = key;
    let Some(value) = map.get(serde_yaml::Value::String("aliases".to_string())) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    match value {
        serde_yaml::Value::String(s) => {
            let t = s.trim();
            if !t.is_empty() {
                out.push(t.to_string());
            }
        }
        serde_yaml::Value::Sequence(seq) => {
            for item in seq {
                if let serde_yaml::Value::String(s) = item {
                    let t = s.trim();
                    if !t.is_empty() {
                        out.push(t.to_string());
                    }
                }
            }
        }
        _ => {}
    }
    out
}

/// Index des alias → notes. Clé normalisée en minuscules.
#[derive(Debug)]
pub struct AliasIndex {
    by_alias: HashMap<String, Vec<usize>>,
    notes: Vec<Note>,
}

impl AliasIndex {
    /// Construit l'index depuis la liste des notes (scan déjà effectué).
    pub fn build(notes: &[Note]) -> Self {
        let mut by_alias: HashMap<String, Vec<usize>> = HashMap::new();
        for (i, note) in notes.iter().enumerate() {
            for alias in extract_aliases(&note.metadata.frontmatter) {
                by_alias.entry(alias.to_lowercase()).or_default().push(i);
            }
        }
        Self {
            by_alias,
            notes: notes.to_vec(),
        }
    }

    /// Résout un alias (insensible à la casse) vers les notes correspondantes.
    /// Alias inconnu → liste vide.
    pub fn resolve<'a>(&'a self, alias: &str) -> Vec<&'a Note> {
        self.by_alias
            .get(&alias.to_lowercase())
            .into_iter()
            .flatten()
            .filter_map(|&i| self.notes.get(i))
            .collect()
    }

    /// Liste de tous les alias déclarés (pour débogage).
    pub fn aliases(&self) -> Vec<&str> {
        self.by_alias.keys().map(|s| s.as_str()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn note(name: &str, aliases: Option<&[&str]>) -> Note {
        let frontmatter = aliases.map(|list| {
            let seq: Vec<serde_yaml::Value> = list
                .iter()
                .map(|s| serde_yaml::Value::String(s.to_string()))
                .collect();
            serde_yaml::Value::Mapping(
                [(
                    serde_yaml::Value::String("aliases".to_string()),
                    serde_yaml::Value::Sequence(seq),
                )]
                .into_iter()
                .collect(),
            )
        });
        Note {
            path: PathBuf::from(format!("{name}.md")),
            name: name.to_string(),
            metadata: super::super::note::NoteMetadata {
                frontmatter,
                frontmatter_error: None,
                wikilinks: Vec::new(),
                tags: Vec::new(),
            },
        }
    }

    fn no_frontmatter() -> Note {
        Note {
            path: PathBuf::from("x.md"),
            name: "x".to_string(),
            metadata: super::super::note::NoteMetadata {
                frontmatter: None,
                frontmatter_error: None,
                wikilinks: Vec::new(),
                tags: Vec::new(),
            },
        }
    }

    #[test]
    fn extraction_cle_absente_retourne_vide() {
        assert_eq!(extract_aliases(&None), Vec::<String>::new());
        assert_eq!(
            extract_aliases(&Some(serde_yaml::Value::Mapping(Default::default()))),
            Vec::<String>::new()
        );
    }

    #[test]
    fn extraction_sequence_retourne_les_chaines_trimees() {
        let fm = Some(serde_yaml::Value::Mapping(
            [(
                serde_yaml::Value::String("aliases".to_string()),
                serde_yaml::Value::Sequence(vec![
                    serde_yaml::Value::String("  b  ".to_string()),
                    serde_yaml::Value::String("c".to_string()),
                ]),
            )]
            .into_iter()
            .collect(),
        ));
        assert_eq!(extract_aliases(&fm), vec!["b".to_string(), "c".to_string()]);
    }

    #[test]
    fn extraction_ignore_les_elements_non_chaine() {
        let fm = Some(serde_yaml::Value::Mapping(
            [(
                serde_yaml::Value::String("aliases".to_string()),
                serde_yaml::Value::Sequence(vec![
                    serde_yaml::Value::String("a".to_string()),
                    serde_yaml::Value::Number(serde_yaml::Number::from(42)),
                ]),
            )]
            .into_iter()
            .collect(),
        ));
        assert_eq!(extract_aliases(&fm), vec!["a".to_string()]);
    }

    #[test]
    fn extraction_retourne_une_simple_chaine() {
        let fm = Some(serde_yaml::Value::Mapping(
            [(
                serde_yaml::Value::String("aliases".to_string()),
                serde_yaml::Value::String("a".to_string()),
            )]
            .into_iter()
            .collect(),
        ));
        assert_eq!(extract_aliases(&fm), vec!["a".to_string()]);
    }

    #[test]
    fn index_resout_case_insensitive() {
        let notes = vec![note("A", Some(&["Alias Un"])), no_frontmatter()];
        let index = AliasIndex::build(&notes);
        let hits = index.resolve("alias un");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].name, "A");
    }

    #[test]
    fn index_alias_inconnu_retourne_vide() {
        let notes = vec![note("A", Some(&["x"]))];
        let index = AliasIndex::build(&notes);
        assert!(index.resolve("zzz").is_empty());
    }

    #[test]
    fn index_multiple_notes_partagent_un_alias() {
        let notes = vec![note("A", Some(&["commun"])), note("B", Some(&["commun"]))];
        let index = AliasIndex::build(&notes);
        assert_eq!(index.resolve("commun").len(), 2);
    }
}
