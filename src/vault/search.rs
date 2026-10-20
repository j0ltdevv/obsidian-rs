//! Index inversé full-text naïf.
//!
//! L'index est construit à partir d'une liste de couples `(titre, texte)`. La
//! recherche retourne les indices des entrées correspondantes, triés par score
//! décroissant, à égalité dans l'ordre d'origine.

use std::cmp::Reverse;
use std::collections::HashMap;

/// Bonus ajouté au score d'un token de la requête qui apparaît dans le titre
/// (premier élément du couple) d'une entrée, une seule fois par token.
const TITLE_BONUS: u64 = 2;

/// Index inversé full-text naïf.
///
/// Les entrées sont indexées par leur **texte** (second élément du couple) et
/// par leur **titre** (premier élément). [`FullTextIndex::search_text`]
/// retourne les indices des entrées triés par score décroissant, tronqués à
/// `limit`.
#[derive(Debug)]
pub struct FullTextIndex {
    /// Token → [(indice de l'entrée, fréquence dans le texte)], sans doublon
    /// d'indice.
    body: HashMap<String, Vec<(usize, u64)>>,
    /// Token → indices des entrées dont le titre contient le token, sans
    /// doublon d'indice.
    title: HashMap<String, Vec<usize>>,
}

/// Tokenise une chaîne : passage en minuscules et coupure sur tout caractère
/// non-alphanumérique (Unicode). Les tokens vides sont ignorés.
fn tokenize(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    for ch in text.chars() {
        if ch.is_alphanumeric() {
            current.push(ch);
        } else if !current.is_empty() {
            tokens.push(std::mem::take(&mut current).to_lowercase());
        }
    }
    if !current.is_empty() {
        tokens.push(current.to_lowercase());
    }
    tokens
}

/// Construit l'index à partir d'une liste de couples `(titre, texte)`.
///
/// L'ordre d'origine des entrées est conservé dans les listes inversées, ce
/// qui garantit la stabilité du tri à scores égaux.
pub fn build_text_index(items: &[(String, String)]) -> FullTextIndex {
    let mut body: HashMap<String, Vec<(usize, u64)>> = HashMap::new();
    let mut title: HashMap<String, Vec<usize>> = HashMap::new();

    for (index, (titre, texte)) in items.iter().enumerate() {
        for token in tokenize(titre) {
            let occurrences = title.entry(token).or_default();
            // Les indices sont parcourus par ordre croissant : une répétition
            // du token dans le même titre se trouve en dernière position.
            if occurrences.last() != Some(&index) {
                occurrences.push(index);
            }
        }

        let mut counts: HashMap<String, u64> = HashMap::new();
        for token in tokenize(texte) {
            *counts.entry(token).or_default() += 1;
        }
        for (token, count) in counts {
            body.entry(token).or_default().push((index, count));
        }
    }

    FullTextIndex { body, title }
}

impl FullTextIndex {
    /// Recherche les entrées correspondant à `query`.
    ///
    /// Le score d'une entrée est la somme, pour chaque token de la requête, de
    /// sa fréquence dans le texte de l'entrée, à laquelle s'ajoute
    /// [`TITLE_BONUS`] lorsque le token apparaît dans son titre. Une entrée
    /// qui ne partage aucun token avec la requête est exclue ; un token de la
    /// requête absent de tous les textes n'annule pas les autres.
    ///
    /// Une requête vide ou sans token produit une liste vide, de même que
    /// `limit == 0`. À scores égaux, l'ordre d'origine des entrées est
    /// conservé (tri stable).
    pub fn search_text(&self, query: &str, limit: usize) -> Vec<usize> {
        let tokens = tokenize(query);
        if tokens.is_empty() || limit == 0 {
            return Vec::new();
        }

        // Le score est accumulé depuis les listes inversées : les entrées qui
        // ne partagent que leur titre avec la requête sont donc incluses.
        let mut scores: HashMap<usize, u64> = HashMap::new();
        for token in &tokens {
            if let Some(entries) = self.body.get(token) {
                for &(index, count) in entries {
                    *scores.entry(index).or_default() += count;
                }
            }
            if let Some(entries) = self.title.get(token) {
                for &index in entries {
                    *scores.entry(index).or_default() += TITLE_BONUS;
                }
            }
        }

        let mut scored: Vec<(usize, u64)> = scores.into_iter().collect();
        // Score décroissant, puis indice croissant : le résultat ne dépend pas
        // de l'ordre d'itération du `HashMap`, et l'égalité respecte l'ordre
        // d'entrée.
        scored.sort_by_key(|&(index, score)| (Reverse(score), index));
        scored
            .into_iter()
            .take(limit)
            .map(|(index, _score)| index)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn index() -> FullTextIndex {
        build_text_index(&[
            (
                "Rust Programming".into(),
                "Rust is a systems programming language. Rust focuses on safety and concurrency."
                    .into(),
            ),
            (
                "Python Guide".into(),
                "Python is a high-level language. Programming in Python is fun.".into(),
            ),
            (
                "Rust Notes".into(),
                "Notes about Rust. Safety and concurrency in Rust.".into(),
            ),
            (
                "Cooking".into(),
                "Recipes for cooking pasta and sauce. Cooking is an art.".into(),
            ),
            ("Café".into(), "Le café est chaud. Café! Café?".into()),
            ("Baking Bread".into(), "flour water salt yeast".into()),
        ])
    }

    #[test]
    fn tokenization_lowercase_and_punctuation() {
        assert_eq!(tokenize("Hello, WORLD! 123"), vec!["hello", "world", "123"]);
    }

    #[test]
    fn tokenization_splits_on_non_alphanumeric_unicode() {
        assert_eq!(tokenize("café—café"), vec!["café", "café"]);
        assert_eq!(tokenize("a_b.c-d/e"), vec!["a", "b", "c", "d", "e"]);
    }

    #[test]
    fn tokenization_empty_and_punctuation_only() {
        assert!(tokenize("").is_empty());
        assert!(tokenize("!!! --- ...").is_empty());
    }

    #[test]
    fn simple_match_single_token() {
        let idx = index();
        assert_eq!(idx.search_text("systems", 10), vec![0]);
    }

    #[test]
    fn query_tokens_are_lowercased() {
        let idx = index();
        assert_eq!(idx.search_text("RUST", 10), vec![0, 2]);
    }

    #[test]
    fn multi_token_scores_summed() {
        let idx = index();
        // index 0 : "programming" x1 texte + bonus titre "Programming" = 3 ;
        // index 1 : "programming" x1 texte, pas au titre = 1.
        assert_eq!(idx.search_text("programming", 10), vec![0, 1]);
        // Scores égaux (1 partout), ordre d'origine.
        assert_eq!(idx.search_text("language", 10), vec![0, 1]);
    }

    #[test]
    fn title_bonus_ranks_above_body_only() {
        let idx = index();
        // index 2 : "notes" x1 texte + bonus titre = 3, "safety" x1 = 4 ;
        // index 0 : "safety" x1 texte, absent du titre = 1.
        assert_eq!(idx.search_text("notes safety", 10), vec![2, 0]);
    }

    #[test]
    fn title_only_match_is_returned() {
        let idx = index();
        // "baking" n'apparaît que dans le titre de l'entrée 5 : bonus seul.
        assert_eq!(idx.search_text("baking", 10), vec![5]);
    }

    #[test]
    fn title_bonus_applies_once_per_query_token() {
        let idx = build_text_index(&[
            ("Rust Rust".into(), "unique text".into()),
            ("Other Title".into(), "rust rust rust".into()),
        ]);
        // Entrée 0 : bonus titre une seule fois = 2 ;
        // entrée 1 : fréquence 3 dans le texte = 3. Sans dédoublonnage du
        // titre, l'entrée 0 obtiendrait 4 et passerait devant.
        assert_eq!(idx.search_text("rust", 10), vec![1, 0]);
    }

    #[test]
    fn repeated_query_token_counts_multiple_times() {
        let idx = index();
        // "rust rust" : entrée 0 → 2 * (2 + 2) = 8 ; entrée 2 → 8.
        assert_eq!(idx.search_text("rust rust", 10), vec![0, 2]);
    }

    #[test]
    fn limit_truncates_results() {
        let idx = index();
        assert_eq!(idx.search_text("language", 1), vec![0]);
        assert_eq!(idx.search_text("language", 2), vec![0, 1]);
        assert_eq!(idx.search_text("language", 100), vec![0, 1]);
    }

    #[test]
    fn limit_zero_returns_empty() {
        assert!(index().search_text("rust", 0).is_empty());
    }

    #[test]
    fn empty_query_returns_empty() {
        let idx = index();
        assert!(idx.search_text("", 10).is_empty());
        assert!(idx.search_text("   ...   ", 10).is_empty());
    }

    #[test]
    fn empty_index_returns_empty() {
        let idx = build_text_index(&[]);
        assert!(idx.search_text("rust", 10).is_empty());
    }

    #[test]
    fn no_match_returns_empty() {
        let idx = index();
        assert!(idx.search_text("xyzzyquux", 10).is_empty());
        // Un token sans correspondance n'annule pas les autres.
        assert_eq!(idx.search_text("rust xyzzy", 10), vec![0, 2]);
    }

    #[test]
    fn stable_order_at_equal_scores() {
        let idx = index();
        // "safety" : entrées 0 et 2, score 1 chacune → ordre d'entrée.
        assert_eq!(idx.search_text("safety", 10), vec![0, 2]);
    }

    #[test]
    fn unicode_tokens_and_title_bonus() {
        let idx = index();
        // Entrée 4 : "café" x3 dans le texte + bonus titre = 5.
        assert_eq!(idx.search_text("café", 10), vec![4]);
    }
}
