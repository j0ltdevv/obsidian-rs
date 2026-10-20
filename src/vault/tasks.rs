//! Extraction des tâches (cases à cocher Markdown).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::note::Note;

/// Statut d'une tâche.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskStatus {
    /// `- [ ]` : à faire.
    Todo,
    /// `- [x]` ou `- [X]` : fait.
    Done,
}

/// Une tâche extraite.
#[derive(Debug, Clone, PartialEq)]
pub struct Task {
    /// Ligne (index 0-based) où se trouve la case.
    pub line: usize,
    /// Texte de la tâche après la case.
    pub text: String,
    /// Statut.
    pub status: TaskStatus,
    /// Note d'origine (chemin relatif).
    pub note_path: PathBuf,
}

/// Extrait les tâches d'un contenu (toutes les lignes `- [ ]` / `- [x]`).
/// Ignore les lignes dans les blocs de code.
pub fn extract_tasks(content: &str, note_path: &Path) -> Vec<Task> {
    let mut tasks = Vec::new();
    let mut bloc_code: Option<u8> = None;

    for (ligne, raw) in content.lines().enumerate() {
        let line = raw.trim();
        let octets = line.as_bytes();

        // Bascule des blocs de code délimités par ``` ou ~~~ (au moins 3
        // caractères identiques ; la fermeture doit utiliser le même type).
        let est_delimiteur = octets.len() >= 3
            && (octets[0] == b'`' || octets[0] == b'~')
            && octets[0] == octets[1]
            && octets[1] == octets[2];

        if est_delimiteur {
            match bloc_code {
                None => bloc_code = Some(octets[0]),
                Some(ouvrant) if ouvrant == octets[0] => bloc_code = None,
                _ => {}
            }
            continue;
        }

        if bloc_code.is_some() {
            continue;
        }

        if let Some((statut, texte)) = parse_case(line) {
            tasks.push(Task {
                line: ligne,
                text: texte.to_string(),
                status: statut,
                note_path: note_path.to_path_buf(),
            });
        }
    }

    tasks
}

/// Parse une ligne `- [...]` et retourne le statut et le texte après la case.
fn parse_case(line: &str) -> Option<(TaskStatus, &str)> {
    let corps = line.strip_prefix("- [")?;
    let octets = corps.as_bytes();
    let statut = match *octets.first()? {
        b' ' => TaskStatus::Todo,
        b'x' | b'X' => TaskStatus::Done,
        _ => return None,
    };
    if octets.get(1) != Some(&b']') {
        return None;
    }
    Some((statut, corps[2..].trim()))
}

/// Agrége les tâches de toutes les notes (ordre : par note, puis par ligne).
/// `notes` contient les chemins ; `contents` est un mapping chemin→contenu.
pub fn aggregate_tasks(notes: &[Note], contents: &HashMap<PathBuf, String>) -> Vec<Task> {
    let mut tasks = Vec::new();
    for note in notes {
        let Some(contenu) = contents.get(&note.path) else {
            continue;
        };
        tasks.extend(extract_tasks(contenu, &note.path));
    }
    tasks
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vault::note::NoteMetadata;

    fn note(chemin: &str) -> Note {
        Note {
            path: PathBuf::from(chemin),
            name: String::new(),
            metadata: NoteMetadata {
                frontmatter: None,
                frontmatter_error: None,
                wikilinks: Vec::new(),
                tags: Vec::new(),
            },
        }
    }

    fn tache(ligne: usize, texte: &str, statut: TaskStatus, chemin: &str) -> Task {
        Task {
            line: ligne,
            text: texte.to_string(),
            status: statut,
            note_path: PathBuf::from(chemin),
        }
    }

    #[test]
    fn extraction_simple() {
        let extraites = extract_tasks("- [ ] Acheter du lait", Path::new("notes/a.md"));
        assert_eq!(
            extraites,
            vec![tache(0, "Acheter du lait", TaskStatus::Todo, "notes/a.md")]
        );
    }

    #[test]
    fn done_majuscule_et_minuscule() {
        let extraites = extract_tasks("- [x] Tache basse\n- [X] Tache haute", Path::new("a.md"));
        assert_eq!(
            extraites,
            vec![
                tache(0, "Tache basse", TaskStatus::Done, "a.md"),
                tache(1, "Tache haute", TaskStatus::Done, "a.md"),
            ]
        );
    }

    #[test]
    fn indentation_acceptee() {
        let extraites = extract_tasks("  - [ ] Sous-tache", Path::new("a.md"));
        assert_eq!(
            extraites,
            vec![tache(0, "Sous-tache", TaskStatus::Todo, "a.md")]
        );
    }

    #[test]
    fn bloc_code_ignore() {
        let contenu = "Texte\n```rust\n- [ ] Dans le code\n```\n- [ ] Apres le bloc";
        let extraites = extract_tasks(contenu, Path::new("a.md"));
        assert_eq!(
            extraites,
            vec![tache(4, "Apres le bloc", TaskStatus::Todo, "a.md")]
        );
    }

    #[test]
    fn bloc_code_avec_tildes_ignore() {
        let contenu = "~~~\n- [ ] Dans le code\n~~~\n- [ ] Dehors";
        let extraites = extract_tasks(contenu, Path::new("a.md"));
        assert_eq!(
            extraites,
            vec![tache(3, "Dehors", TaskStatus::Todo, "a.md")]
        );
    }

    #[test]
    fn texte_trime() {
        let extraites = extract_tasks("- [ ]   Contenu avec espaces   ", Path::new("a.md"));
        assert_eq!(
            extraites,
            vec![tache(0, "Contenu avec espaces", TaskStatus::Todo, "a.md")]
        );
    }

    #[test]
    fn ordre_par_ligne() {
        let extraites = extract_tasks(
            "- [x] Premiere\nAutre texte\n- [ ] Deuxieme\n- [x] Troisieme",
            Path::new("a.md"),
        );
        assert_eq!(
            extraites,
            vec![
                tache(0, "Premiere", TaskStatus::Done, "a.md"),
                tache(2, "Deuxieme", TaskStatus::Todo, "a.md"),
                tache(3, "Troisieme", TaskStatus::Done, "a.md"),
            ]
        );
    }

    #[test]
    fn note_sans_tache() {
        let extraites = extract_tasks("Rien ici\n# Titre\n", Path::new("a.md"));
        assert!(extraites.is_empty());
    }

    #[test]
    fn case_non_reconnue_ignoree() {
        let extraites = extract_tasks("- [y] Pas une tache", Path::new("a.md"));
        assert!(extraites.is_empty());
    }

    #[test]
    fn ligne_vide_ignoree() {
        let extraites = extract_tasks("\n- [ ] Tache\n\n", Path::new("a.md"));
        assert_eq!(extraites, vec![tache(1, "Tache", TaskStatus::Todo, "a.md")]);
    }

    #[test]
    fn chemin_preserve() {
        let extraites = extract_tasks("- [ ] Tache", Path::new("dossier/note.md"));
        assert_eq!(extraites[0].note_path, PathBuf::from("dossier/note.md"));
    }

    #[test]
    fn agregation_multi_notes() {
        let a = note("a.md");
        let b = note("b.md");
        let mut contenus = HashMap::new();
        contenus.insert(PathBuf::from("a.md"), "- [ ] Tache A".to_string());
        contenus.insert(PathBuf::from("b.md"), "- [x] Tache B".to_string());

        let agregees = aggregate_tasks(&[a, b], &contenus);
        assert_eq!(
            agregees,
            vec![
                tache(0, "Tache A", TaskStatus::Todo, "a.md"),
                tache(0, "Tache B", TaskStatus::Done, "b.md"),
            ]
        );
    }

    #[test]
    fn ordre_agregation_suit_les_notes() {
        let b = note("b.md");
        let a = note("a.md");
        let mut contenus = HashMap::new();
        contenus.insert(PathBuf::from("a.md"), "- [ ] Tache A".to_string());
        contenus.insert(PathBuf::from("b.md"), "- [x] Tache B".to_string());

        // `notes` liste b avant a : l'ordre suit la liste, pas le mapping.
        let agregees = aggregate_tasks(&[b, a], &contenus);
        assert_eq!(
            agregees,
            vec![
                tache(0, "Tache B", TaskStatus::Done, "b.md"),
                tache(0, "Tache A", TaskStatus::Todo, "a.md"),
            ]
        );
    }

    #[test]
    fn note_absente_du_mapping_ignoree() {
        let a = note("a.md");
        let mut contenus = HashMap::new();
        contenus.insert(PathBuf::from("b.md"), "- [ ] Tache B".to_string());

        let agregees = aggregate_tasks(&[a], &contenus);
        assert!(agregees.is_empty());
    }
}
