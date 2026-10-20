//! Opérateurs de recherche complets pour le vault.
//!
//! Analyse une requête utilisateur en termes structurés
//! ([`parse_query`]) puis filtre les notes du vault selon ces termes
//! ([`filter_notes`]), au-dessus de l'index full-text existant
//! ([`super::search::FullTextIndex`]).

use std::collections::{HashMap, HashSet};

use super::note::Note;
use super::search::FullTextIndex;

/// Terme de recherche structuré après parsing des opérateurs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SearchTerm {
    /// Mot-clé simple (recherche full-text).
    Keyword(String),
    /// Contrainte de chemin : `path:"sous/dossier"`.
    Path(String),
    /// Contrainte de tag : `tag:#perso` (sans le # ou avec).
    Tag(String),
    /// Contrainte de nom de fichier : `file:nom`.
    File(String),
    /// Contrainte de ligne : `line:N` (la note contient au moins N lignes).
    Line(usize),
    /// Exclusion : `-mot` exclut les notes contenant le mot.
    Exclude(String),
}

/// Requête structurée disséquée.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ParsedQuery {
    pub positive: Vec<SearchTerm>,
    pub negative: Vec<SearchTerm>,
}

/// Analyse une chaîne de requête utilisateur en termes structurés.
///
/// Syntaxe : séparateur espace ; `path:...`, `tag:...`, `file:...`,
/// `line:N`, `-terme` (exclusion). Un terme inconnu devient `Keyword` ; une
/// valeur vide après un préfixe connu (ex. `path:`) est aussi traitée comme
/// `Keyword`. `line` invalide est ignoré.
pub fn parse_query(query: &str) -> ParsedQuery {
    let mut parsed = ParsedQuery::default();
    for token in query.split_whitespace() {
        if let Some(rest) = token.strip_prefix('-') {
            if rest.is_empty() {
                parsed.positive.push(SearchTerm::Keyword(token.to_string()));
            } else {
                parsed.negative.push(SearchTerm::Exclude(rest.to_string()));
            }
            continue;
        }
        if let Some(value) = token.strip_prefix("line:") {
            // `line` invalide → terme ignoré.
            if let Ok(lines) = value.parse::<usize>() {
                parsed.positive.push(SearchTerm::Line(lines));
            }
            continue;
        }
        if let Some(value) = token.strip_prefix("path:") {
            if !value.is_empty() {
                parsed.positive.push(SearchTerm::Path(value.to_string()));
                continue;
            }
        }
        if let Some(value) = token.strip_prefix("tag:") {
            if !value.is_empty() {
                parsed.positive.push(SearchTerm::Tag(normalize_tag(value)));
                continue;
            }
        }
        if let Some(value) = token.strip_prefix("file:") {
            if !value.is_empty() {
                parsed.positive.push(SearchTerm::File(value.to_string()));
                continue;
            }
        }
        parsed.positive.push(SearchTerm::Keyword(token.to_string()));
    }
    parsed
}

/// Filtre les notes selon une requête structurée.
///
/// - `notes` : liste des notes du vault.
/// - `text_index` : [`FullTextIndex`] construit sur `(titre, texte)` — pour
///   les termes `Keyword`/`Exclude`. Si `None`, les mots-clés matchent par
///   sous-séquence dans le nom.
/// - `line_counts` : mapping indice note → nombre de lignes (fourni par
///   l'appelant).
///
/// Retourne les indices (dans `notes`) triés par score décroissant pour les
/// termes positifs, puis exclusions appliquées. À scores égaux, l'ordre
/// d'origine est conservé (tri stable).
pub fn filter_notes(
    notes: &[crate::vault::note::Note],
    text_index: Option<&FullTextIndex>,
    line_counts: &HashMap<usize, usize>,
    query: &ParsedQuery,
) -> Vec<usize> {
    // 1. Sélection des notes par mots-clés positifs et calcul de leur score.
    let keywords: Vec<&str> = positive_keywords(query);
    let selected: Vec<(usize, u64)> = if keywords.is_empty() {
        // Aucun mot-clé : toutes les notes passent, score nul.
        (0..notes.len()).map(|index| (index, 0)).collect()
    } else if let Some(index) = text_index {
        // L'index classe les correspondances par score décroissant (tri
        // stable) ; l'ordre de `search_text` est conservé tel quel.
        index
            .search_text(&keywords.join(" "), usize::MAX)
            .into_iter()
            .map(|index| (index, 0))
            .collect()
    } else {
        // Sans index : chaque mot-clé est une sous-séquence du nom (insensible
        // à la casse) ; le score compte les mots-clés matchés.
        let lowered: Vec<String> = keywords.iter().map(|word| word.to_lowercase()).collect();
        let mut scored = Vec::new();
        for (index, note) in notes.iter().enumerate() {
            let name = note.name.to_lowercase();
            let mut score = 0u64;
            for word in &lowered {
                if is_subsequence(word, &name) {
                    score += 1;
                }
            }
            if score > 0 {
                scored.push((index, score));
            }
        }
        scored
    };

    // 2. Filtrage par contraintes structurelles (ET).
    let mut selected: Vec<(usize, u64)> = selected
        .into_iter()
        .filter(|&(index, _)| matches_constraints(notes, line_counts, query, index))
        .collect();

    // 3. Tri final par score décroissant, stable (ordre d'origine à égalité).
    //    Avec l'index full-text, le tri stable à scores égaux conserve l'ordre
    //    déjà produit par `search_text`.
    selected.sort_by_key(|&(index, score)| (std::cmp::Reverse(score), index));

    // 4. Exclusions : retrait des notes matchant un terme négatif, quel que
    //    soit le reste de la requête.
    if query.negative.is_empty() {
        return selected.into_iter().map(|(index, _)| index).collect();
    }
    let excluded = excluded_indices(notes, text_index, query);
    selected
        .into_iter()
        .map(|(index, _)| index)
        .filter(|index| !excluded.contains(index))
        .collect()
}

/// Retourne les valeurs des termes positifs `Keyword`.
fn positive_keywords(query: &ParsedQuery) -> Vec<&str> {
    query
        .positive
        .iter()
        .filter_map(|term| match term {
            SearchTerm::Keyword(word) => Some(word.as_str()),
            _ => None,
        })
        .collect()
}

/// Vérifie qu'une note satisfait toutes les contraintes structurelles
/// (`Path`, `Tag`, `File`, `Line`) de la requête.
fn matches_constraints(
    notes: &[Note],
    line_counts: &HashMap<usize, usize>,
    query: &ParsedQuery,
    index: usize,
) -> bool {
    query.positive.iter().all(|term| match term {
        SearchTerm::Path(pattern) => {
            let path = notes[index].path.to_string_lossy().to_lowercase();
            path.contains(&pattern.to_lowercase())
        }
        SearchTerm::Tag(tag) => notes[index]
            .metadata
            .tags
            .iter()
            .any(|stored| normalize_tag(stored) == *tag),
        SearchTerm::File(pattern) => notes[index]
            .name
            .to_lowercase()
            .contains(&pattern.to_lowercase()),
        SearchTerm::Line(min) => line_counts.get(&index).copied().unwrap_or(0) >= *min,
        SearchTerm::Keyword(_) | SearchTerm::Exclude(_) => true,
    })
}

/// Retourne l'ensemble des indices des notes matchant au moins un terme
/// négatif `Exclude`.
fn excluded_indices(
    notes: &[Note],
    text_index: Option<&FullTextIndex>,
    query: &ParsedQuery,
) -> HashSet<usize> {
    let words: Vec<&str> = query
        .negative
        .iter()
        .filter_map(|term| match term {
            SearchTerm::Exclude(word) => Some(word.as_str()),
            _ => None,
        })
        .collect();
    let indices: Vec<usize> = if let Some(index) = text_index {
        index.search_text(&words.join(" "), usize::MAX)
    } else {
        let lowered: Vec<String> = words.iter().map(|word| word.to_lowercase()).collect();
        notes
            .iter()
            .enumerate()
            .filter(|(_, note)| {
                let name = note.name.to_lowercase();
                lowered.iter().any(|word| is_subsequence(word, &name))
            })
            .map(|(index, _)| index)
            .collect()
    };
    indices.into_iter().collect()
}

/// Normalise une valeur de tag en retirant le `#` initial et les `/` finaux.
fn normalize_tag(tag: &str) -> String {
    tag.trim_start_matches('#')
        .trim_end_matches('/')
        .to_string()
}

/// Vérifie que `query` est une sous-séquence de `name` (tous deux déjà en
/// minuscules). Une requête vide est une sous-séquence de tout nom.
fn is_subsequence(query: &str, name: &str) -> bool {
    let mut chars = query.chars();
    let Some(mut expected) = chars.next() else {
        return true;
    };
    for ch in name.chars() {
        if ch == expected {
            match chars.next() {
                Some(next) => expected = next,
                None => return true,
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::super::search::build_text_index;

    use super::super::note::{Note, NoteMetadata};
    use super::*;

    fn note(name: &str, path: &str, tags: &[&str]) -> Note {
        Note {
            path: PathBuf::from(path),
            name: name.to_string(),
            metadata: NoteMetadata {
                frontmatter: None,
                frontmatter_error: None,
                wikilinks: Vec::new(),
                tags: tags.iter().map(|tag| tag.to_string()).collect(),
            },
        }
    }

    fn notes() -> Vec<Note> {
        vec![
            note("Rust Intro", "dev/rust.md", &["#perso"]),
            note("Python Guide", "langues/python.md", &["#travail"]),
            note("Rust Notes", "dev/rust/notes.md", &["#perso", "#autre"]),
            note("Cooking", "maison/cuisine.md", &[]),
            note("Liens", "divers/liens.md", &["#perso/x"]),
        ]
    }

    fn text_index() -> FullTextIndex {
        build_text_index(&[
            (
                "Rust Intro".into(),
                "Rust is fast. Safety safety safety.".into(),
            ),
            ("Python Guide".into(), "Python is readable.".into()),
            ("Rust Notes".into(), "Notes about Rust. Safety.".into()),
            ("Cooking".into(), "Recipes.".into()),
            ("Liens".into(), "Various links.".into()),
        ])
    }

    fn line_counts() -> HashMap<usize, usize> {
        [(0, 5), (1, 3), (2, 8), (3, 1), (4, 2)]
            .into_iter()
            .collect()
    }

    fn filter(query: &str, text_index: Option<&FullTextIndex>) -> Vec<usize> {
        filter_notes(&notes(), text_index, &line_counts(), &parse_query(query))
    }

    // --- parsing ---

    #[test]
    fn parse_empty_query_has_no_terms() {
        assert_eq!(parse_query(""), ParsedQuery::default());
        assert_eq!(parse_query("   \t  "), ParsedQuery::default());
    }

    #[test]
    fn parse_simple_keyword() {
        assert_eq!(
            parse_query("bonjour"),
            ParsedQuery {
                positive: vec![SearchTerm::Keyword("bonjour".into())],
                negative: Vec::new(),
            }
        );
    }

    #[test]
    fn parse_path_operator() {
        assert_eq!(
            parse_query("path:sous/dossier"),
            ParsedQuery {
                positive: vec![SearchTerm::Path("sous/dossier".into())],
                negative: Vec::new(),
            }
        );
    }

    #[test]
    fn parse_tag_with_and_without_hash() {
        assert_eq!(
            parse_query("tag:#perso").positive,
            vec![SearchTerm::Tag("perso".into())]
        );
        assert_eq!(
            parse_query("tag:perso").positive,
            vec![SearchTerm::Tag("perso".into())]
        );
        assert_eq!(
            parse_query("tag:perso/").positive,
            vec![SearchTerm::Tag("perso".into())]
        );
    }

    #[test]
    fn parse_file_operator() {
        assert_eq!(
            parse_query("file:Journal").positive,
            vec![SearchTerm::File("Journal".into())]
        );
    }

    #[test]
    fn parse_line_valid() {
        assert_eq!(parse_query("line:12").positive, vec![SearchTerm::Line(12)]);
        assert_eq!(parse_query("line:0").positive, vec![SearchTerm::Line(0)]);
    }

    #[test]
    fn parse_line_invalid_ignored() {
        assert!(parse_query("line:abc").positive.is_empty());
        assert!(parse_query("line:").positive.is_empty());
        assert!(parse_query("line:-1").positive.is_empty());
    }

    #[test]
    fn parse_exclude_operator() {
        assert_eq!(
            parse_query("-excl"),
            ParsedQuery {
                positive: Vec::new(),
                negative: vec![SearchTerm::Exclude("excl".into())],
            }
        );
    }

    #[test]
    fn parse_positive_and_negative_mix() {
        assert_eq!(
            parse_query("bonjour -adieu path:a"),
            ParsedQuery {
                positive: vec![
                    SearchTerm::Keyword("bonjour".into()),
                    SearchTerm::Path("a".into()),
                ],
                negative: vec![SearchTerm::Exclude("adieu".into())],
            }
        );
    }

    #[test]
    fn parse_unknown_prefix_becomes_keyword() {
        assert_eq!(
            parse_query("title:x").positive,
            vec![SearchTerm::Keyword("title:x".into())]
        );
    }

    #[test]
    fn parse_empty_operator_value_becomes_keyword() {
        assert_eq!(
            parse_query("path:").positive,
            vec![SearchTerm::Keyword("path:".into())]
        );
        assert_eq!(
            parse_query("tag:").positive,
            vec![SearchTerm::Keyword("tag:".into())]
        );
        assert_eq!(
            parse_query("file:").positive,
            vec![SearchTerm::Keyword("file:".into())]
        );
    }

    #[test]
    fn parse_minus_alone_becomes_keyword() {
        assert_eq!(
            parse_query("-").positive,
            vec![SearchTerm::Keyword("-".into())]
        );
    }

    // --- filtrage ---

    #[test]
    fn filter_keyword_scores_via_text_index() {
        // "safety" : note 0 = 3 occurrences, note 2 = 1 → note 0 devant.
        assert_eq!(filter("safety", Some(&text_index())), vec![0, 2]);
    }

    #[test]
    fn filter_multi_keyword_cumulative_scores() {
        // "safety python" : note 0 = 3, note 1 = 3, note 2 = 1 → [0, 1, 2].
        assert_eq!(filter("safety python", Some(&text_index())), vec![0, 1, 2]);
    }

    #[test]
    fn filter_keyword_stable_order_at_equal_scores() {
        // "rust" : notes 0 et 2 à score égal → ordre d'origine.
        assert_eq!(filter("rust", Some(&text_index())), vec![0, 2]);
    }

    #[test]
    fn filter_keyword_without_text_index_name_subsequence() {
        assert_eq!(filter("rust", None), vec![0, 2]);
    }

    #[test]
    fn filter_keyword_without_text_index_scores_matches() {
        // "rust notes" : note 2 matche les deux mots-clés (score 2), note 0
        // un seul (score 1) → note 2 devant.
        assert_eq!(filter("rust notes", None), vec![2, 0]);
    }

    #[test]
    fn filter_path_case_insensitive() {
        assert_eq!(filter("path:RUST", Some(&text_index())), vec![0, 2]);
        assert_eq!(filter("path:rust/", Some(&text_index())), vec![2]);
        assert_eq!(
            filter("path:inexistant", Some(&text_index())),
            Vec::<usize>::new()
        );
    }

    #[test]
    fn filter_tag_normalized_with_and_without_hash() {
        assert_eq!(filter("tag:#perso", Some(&text_index())), vec![0, 2]);
        assert_eq!(filter("tag:perso", Some(&text_index())), vec![0, 2]);
        assert_eq!(filter("tag:autre", Some(&text_index())), vec![2]);
    }

    #[test]
    fn filter_tag_exact_not_subtag() {
        // Le tag "#perso/x" (normalisé "perso/x") ne satisfait pas "tag:perso".
        assert_eq!(filter("tag:perso", Some(&text_index())), vec![0, 2]);
    }

    #[test]
    fn filter_file_substring_case_insensitive() {
        assert_eq!(filter("file:RUST", Some(&text_index())), vec![0, 2]);
        assert_eq!(filter("file:guide", Some(&text_index())), vec![1]);
    }

    #[test]
    fn filter_line_min_lines() {
        assert_eq!(filter("line:4", Some(&text_index())), vec![0, 2]);
        assert_eq!(filter("line:1", Some(&text_index())), vec![0, 1, 2, 3, 4]);
        assert_eq!(filter("line:100", Some(&text_index())), Vec::<usize>::new());
    }

    #[test]
    fn filter_exclude_removes_positive() {
        // L'exclusion annule une positive, quel que soit le score.
        assert_eq!(
            filter("safety -safety", Some(&text_index())),
            Vec::<usize>::new()
        );
    }

    #[test]
    fn filter_exclude_only() {
        assert_eq!(filter("-safety", Some(&text_index())), vec![1, 3, 4]);
    }

    #[test]
    fn filter_exclude_without_text_index() {
        // Sans index, "-notes" exclut la note 2 (sous-séquence dans le nom).
        assert_eq!(filter("rust -notes", None), vec![0]);
    }

    #[test]
    fn filter_empty_query_returns_all_notes() {
        assert_eq!(filter("", Some(&text_index())), vec![0, 1, 2, 3, 4]);
        assert_eq!(filter("", None), vec![0, 1, 2, 3, 4]);
    }

    #[test]
    fn filter_constraints_only_original_order() {
        assert_eq!(filter("path:maison", Some(&text_index())), vec![3]);
        assert_eq!(
            filter("tag:perso path:dev", Some(&text_index())),
            vec![0, 2]
        );
    }

    #[test]
    fn filter_mixed_keyword_and_constraint() {
        assert_eq!(filter("safety path:dev", Some(&text_index())), vec![0, 2]);
        assert_eq!(
            filter("safety path:maison", Some(&text_index())),
            Vec::<usize>::new()
        );
    }

    #[test]
    fn filter_no_match_returns_empty() {
        assert_eq!(filter("xyzzy", Some(&text_index())), Vec::<usize>::new());
        assert_eq!(filter("xyzzy", None), Vec::<usize>::new());
    }
}
