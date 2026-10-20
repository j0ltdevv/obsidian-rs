//! Extraction du plan d'une note (titres Markdown).
//!
//! Un titre ATX est une ligne commençant par 1 à 6 `#` suivis d'un espace.
//! Les lignes à l'intérieur d'un bloc de code délimité par des fences
//! (```` ``` ```` ou `~~~`) et les lignes d'un bloc de citation (`>`) sont
//! ignorées.

use std::fs;

use super::note::Note;

/// Une entrée du plan.
#[derive(Debug, Clone, PartialEq)]
pub struct OutlineEntry {
    /// Niveau du titre (1..=6).
    pub level: u8,
    /// Texte du titre sans les `#` ni le trim.
    pub title: String,
    /// Ligne (index 0-based) où commence le titre.
    pub line: usize,
}

/// Extrait toutes les entrées de plan d'un contenu Markdown.
///
/// Un titre est une ligne commençant par 1 à 6 `#` suivis d'un espace.
/// Les lignes dans les blocs de code (``` ... ```) sont ignorées.
/// Les titres à l'intérieur d'un bloc de citation (>) sont ignorés.
pub fn extract_outline(content: &str) -> Vec<OutlineEntry> {
    let mut entries = Vec::new();
    let mut code_fence: Option<u8> = None;

    for (line_index, raw) in content.lines().enumerate() {
        let line = raw.trim();

        // Bascule des blocs de code délimités par ``` ou ~~~ (au moins 3
        // caractères identiques ; la fermeture doit utiliser le même type).
        let bytes = line.as_bytes();
        let is_delimiter = bytes.len() >= 3
            && (bytes[0] == b'`' || bytes[0] == b'~')
            && bytes[0] == bytes[1]
            && bytes[1] == bytes[2];

        if is_delimiter {
            match code_fence {
                None => code_fence = Some(bytes[0]),
                Some(opening) if opening == bytes[0] => code_fence = None,
                _ => {}
            }
            continue;
        }

        if code_fence.is_some() {
            continue;
        }

        // Les lignes d'un bloc de citation sont ignorées.
        if line.starts_with('>') {
            continue;
        }

        if let Some((level, title)) = parse_heading(raw) {
            entries.push(OutlineEntry {
                level,
                title,
                line: line_index,
            });
        }
    }

    entries
}

/// Reconnaît un titre ATX : 1 à 6 `#` suivis d'un espace.
fn parse_heading(line: &str) -> Option<(u8, String)> {
    let level = line.bytes().take_while(|byte| *byte == b'#').count();
    if !(1..=6).contains(&level) {
        return None;
    }
    // Le `#` doit être suivi d'un espace (`#titre` reste du texte).
    if !line[level..].starts_with(' ') {
        return None;
    }
    Some((level as u8, line[level..].trim().to_string()))
}

/// Extrait le plan d'une note (paquet autour de `extract_outline`).
///
/// Le contenu de la note est lu depuis le disque à l'adresse `note.path`.
/// Si le fichier est illisible (absent, permissions, …), retourne un plan
/// vide ; cette fonction ne panique jamais.
pub fn note_outline(note: &Note) -> Vec<OutlineEntry> {
    fs::read_to_string(&note.path)
        .map(|source| extract_outline(&source))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::{extract_outline, note_outline, OutlineEntry};
    use crate::vault::note::{Note, NoteMetadata};
    use std::path::PathBuf;

    /// Construit l'entrée de plan attendue.
    fn entry(level: u8, title: &str, line: usize) -> OutlineEntry {
        OutlineEntry {
            level,
            title: title.to_string(),
            line,
        }
    }

    /// Construit une note avec un chemin donné et des métadonnées vides.
    fn note(path: &str) -> Note {
        Note {
            path: PathBuf::from(path),
            name: String::new(),
            metadata: NoteMetadata {
                frontmatter: None,
                frontmatter_error: None,
                wikilinks: Vec::new(),
                tags: Vec::new(),
            },
        }
    }

    #[test]
    fn niveaux_varies() {
        let source = "# Un\n## Deux\n### Trois\n#### Quatre\n##### Cinq\n###### Six\n";
        let outline = extract_outline(source);

        let levels: Vec<u8> = outline.iter().map(|entry| entry.level).collect();
        assert_eq!(levels, vec![1, 2, 3, 4, 5, 6]);

        let titles: Vec<&str> = outline.iter().map(|entry| entry.title.as_str()).collect();
        assert_eq!(titles, vec!["Un", "Deux", "Trois", "Quatre", "Cinq", "Six"]);
    }

    #[test]
    fn ordre_d_apparition() {
        let source = "## B\n# A\n### C\n";
        let outline = extract_outline(source);

        assert_eq!(
            outline,
            vec![entry(2, "B", 0), entry(1, "A", 1), entry(3, "C", 2)]
        );
    }

    #[test]
    fn bloc_code_ignore() {
        let source = "# Vrai\n```\n# Faux\n## Aussi faux\n```\n## Vrai aussi\n";
        let outline = extract_outline(source);

        assert_eq!(
            outline,
            vec![entry(1, "Vrai", 0), entry(2, "Vrai aussi", 5)]
        );
    }

    #[test]
    fn citation_ignoree() {
        let source = "> # Citée\n> ## Sous-citée\n># Sans espace\n# Réelle\n";
        let outline = extract_outline(source);

        assert_eq!(outline, vec![entry(1, "Réelle", 3)]);
    }

    #[test]
    fn titre_sans_espace_non_reconnu() {
        let source = "#titre\n##x\n### y\n";
        let outline = extract_outline(source);

        assert_eq!(outline, vec![entry(3, "y", 2)]);
    }

    #[test]
    fn sept_dieseges_non_reconnu() {
        let source = "####### Trop profond\n# Ok\n";
        let outline = extract_outline(source);

        assert_eq!(outline, vec![entry(1, "Ok", 1)]);
    }

    #[test]
    fn ligne_vide() {
        let source = "intro\n\n# Titre\n\n## Sous\n";
        let outline = extract_outline(source);

        assert_eq!(outline, vec![entry(1, "Titre", 2), entry(2, "Sous", 4)]);
    }

    #[test]
    fn crlf_ignore() {
        let source = "# Titre\r\nParagraphe\r\n## Sous-titre\r\n";
        let outline = extract_outline(source);

        assert_eq!(
            outline,
            vec![entry(1, "Titre", 0), entry(2, "Sous-titre", 2)]
        );
    }

    #[test]
    fn titre_vide() {
        let source = "# \n##   \n";
        let outline = extract_outline(source);

        assert_eq!(outline, vec![entry(1, "", 0), entry(2, "", 1)]);
    }

    #[test]
    fn titre_conserve_les_espaces_internes() {
        let source = "# Intro  suite\n";
        let outline = extract_outline(source);

        assert_eq!(outline, vec![entry(1, "Intro  suite", 0)]);
    }

    #[test]
    fn contenu_sans_titre() {
        assert_eq!(extract_outline("Juste du texte.\n"), vec![]);
        assert!(extract_outline("").is_empty());
    }

    #[test]
    fn note_outline_lit_le_fichier() {
        let dir = std::env::temp_dir().join(format!("obsidian_outline_{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("création du dossier temporaire");
        let path = dir.join("note.md");
        std::fs::write(&path, "# Titre\n## Sous\n").expect("écriture de la note");

        let note = note(path.to_str().expect("chemin UTF-8"));
        assert_eq!(
            note_outline(&note),
            vec![entry(1, "Titre", 0), entry(2, "Sous", 1)]
        );

        std::fs::remove_dir_all(&dir).expect("nettoyage du dossier temporaire");
    }

    #[test]
    fn note_outline_fichier_absent() {
        let note = note("/chemin/inexistant/note.md");
        assert_eq!(note_outline(&note), vec![]);
    }
}
