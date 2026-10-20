//! Filtrage et tri des notes par champ de frontmatter (style « Dataview »).
//!
//! [`Query`] exprime une condition sur une propriété du frontmatter YAML d'une
//! note. [`filter_notes`] retourne les indices des notes qui satisfont **toutes**
//! les requêtes (conjonction). [`sort_by_field`] réordonne une liste d'indices
//! selon la valeur d'un champ.
//!
//! # Champ absent
//!
//! Une note qui ne possède pas le champ demandé (pas de frontmatter, frontmatter
//! qui n'est pas un mapping, ou clé absente) est :
//!
//! - exclue par [`filter_notes`] ;
//! - placée en **fin** de liste par [`sort_by_field`], quel que soit le sens du
//!   tri.
//!
//! # Valeurs numériques
//!
//! `Gt`, `Lt` et le tri tentent de lire la valeur en tant que `f64` : un nombre
//! YAML est lu directement, une chaîne est parsée. Toute autre forme (booléen,
//! séquence, mapping, `null`, chaîne non numérique) n'est pas numérique : la
//! comparaison échoue (la note est exclue) et le tri la traite comme du texte.

use std::cmp::Ordering;

use serde_yaml::Value;

use super::note::Note;

/// Comparaison appliquée à la valeur d'un champ.
#[derive(Debug, Clone, PartialEq)]
pub enum FieldCmp {
    /// Égalité, sur la forme texte de la valeur.
    Eq(String),
    /// La forme texte de la valeur contient la sous-chaîne.
    Contains(String),
    /// Strictement supérieur, valeur numérique.
    Gt(f64),
    /// Strictement inférieur, valeur numérique.
    Lt(f64),
}

/// Condition sur un champ du frontmatter d'une note.
#[derive(Debug, Clone, PartialEq)]
pub struct Query {
    /// Nom du champ.
    pub field: String,
    /// Comparaison à appliquer à la valeur du champ.
    pub cmp: FieldCmp,
}

impl Query {
    /// Crée une condition sur `field`.
    pub fn new(field: impl Into<String>, cmp: FieldCmp) -> Self {
        Self {
            field: field.into(),
            cmp,
        }
    }
}

/// Retourne la valeur du champ `field` dans le frontmatter de `note`, ou `None`
/// si le champ est absent ou si le frontmatter n'est pas un mapping.
fn field_value<'a>(note: &'a Note, field: &str) -> Option<&'a Value> {
    note.metadata
        .frontmatter
        .as_ref()
        .and_then(Value::as_mapping)
        .and_then(|mapping| mapping.get(field))
}

/// Forme texte d'une valeur YAML, utilisée par `Eq` et `Contains`.
fn value_text(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Number(number) => number.to_string(),
        Value::Bool(boolean) => boolean.to_string(),
        Value::Null => String::new(),
        // Séquences et mappings : sérialisation YAML compacte, de sorte que le
        // contenu reste lisible (une liste `tags: [rust, test]` devient
        // « [rust, test] »).
        _ => serde_yaml::to_string(value)
            .unwrap_or_default()
            .trim()
            .to_string(),
    }
}

/// Tente de convertir une valeur YAML en `f64`.
fn as_f64(value: &Value) -> Option<f64> {
    match value {
        Value::Number(number) => number.as_f64(),
        Value::String(text) => text.parse().ok(),
        _ => None,
    }
}

/// Compare deux valeurs présentes : numérique si les deux sont lisibles en
/// `f64`, texte sinon.
fn compare_values(left: &Value, right: &Value) -> Ordering {
    match (as_f64(left), as_f64(right)) {
        (Some(left), Some(right)) => left.partial_cmp(&right).unwrap_or(Ordering::Equal),
        _ => value_text(left).cmp(&value_text(right)),
    }
}

/// Vérifie que `note` satisfait `query`. Une note sans le champ ne le satisfait
/// jamais.
fn matches_query(note: &Note, query: &Query) -> bool {
    let Some(value) = field_value(note, &query.field) else {
        return false;
    };

    match &query.cmp {
        FieldCmp::Eq(expected) => value_text(value) == *expected,
        FieldCmp::Contains(needle) => value_text(value).contains(needle),
        FieldCmp::Gt(threshold) => as_f64(value).is_some_and(|v| v > *threshold),
        FieldCmp::Lt(threshold) => as_f64(value).is_some_and(|v| v < *threshold),
    }
}

/// Indices des notes qui satisfont **toutes** les requêtes (AND).
///
/// L'ordre d'origine est conservé. Une requête vide ne filtre rien : toutes les
/// notes sont retenues. Une note qui ne possède pas le champ d'une requête est
/// exclue.
pub fn filter_notes(notes: &[Note], queries: &[Query]) -> Vec<usize> {
    notes
        .iter()
        .enumerate()
        .filter(|(_, note)| queries.iter().all(|query| matches_query(note, query)))
        .map(|(index, _)| index)
        .collect()
}

/// Réordonne `indices` selon la valeur du champ `field`.
///
/// - `ascending = true` : valeurs croissantes, puis les notes sans champ.
/// - `ascending = false` : valeurs décroissantes, puis les notes sans champ.
///
/// Les notes qui partagent la même valeur conservent leur ordre relatif (tri
/// stable). Les indices hors de `notes` provoquent un `panic`.
pub fn sort_by_field(notes: &[Note], field: &str, ascending: bool, indices: &mut [usize]) {
    indices.sort_by(|&left, &right| {
        let left_value = field_value(&notes[left], field);
        let right_value = field_value(&notes[right], field);

        // Les notes sans le champ passent toujours en fin, quel que soit le
        // sens du tri.
        let presence = match (left_value.is_some(), right_value.is_some()) {
            (true, true) => Ordering::Equal,
            (true, false) => Ordering::Less,
            (false, true) => Ordering::Greater,
            (false, false) => Ordering::Equal,
        };
        if presence != Ordering::Equal {
            return presence;
        }

        let value_order = match (left_value, right_value) {
            (Some(left), Some(right)) => compare_values(left, right),
            _ => Ordering::Equal,
        };
        if value_order == Ordering::Equal {
            // Stabilité : à valeur égale, on restaure l'ordre des notes
            // (indices croissants), indépendamment du sens du tri.
            left.cmp(&right)
        } else if ascending {
            value_order
        } else {
            value_order.reverse()
        }
    });
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::*;
    use crate::vault::frontmatter::parse_frontmatter;
    use crate::vault::note::NoteMetadata;

    /// Construit une note à partir de son frontmatter YAML.
    fn note(path: &str, frontmatter: &str) -> Note {
        let source = format!("---\n{frontmatter}---\n\nCorps du document.");
        let frontmatter = parse_frontmatter(&source)
            .expect("frontmatter valide")
            .expect("frontmatter présent");

        Note {
            path: PathBuf::from(path),
            name: Path::new(path)
                .file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap()
                .to_string(),
            metadata: NoteMetadata {
                frontmatter: Some(frontmatter),
                frontmatter_error: None,
                wikilinks: Vec::new(),
                tags: Vec::new(),
            },
        }
    }

    /// Notes de test :
    ///
    /// | indice | path      | priority | title       |
    /// |--------|-----------|----------|-------------|
    /// | 0      | a.md      | 5        | Alpha       |
    /// | 1      | b.md      | 3        | Beta        |
    /// | 2      | c.md      | 5        | Alpha Beta  |
    /// | 3      | d.md      | —        | Delta       |
    /// | 4      | e.md      | 10       | Epsilon     |
    fn notes() -> Vec<Note> {
        vec![
            note("a.md", "priority: 5\ntitle: Alpha\n"),
            note("b.md", "priority: 3\ntitle: Beta\n"),
            note("c.md", "priority: 5\ntitle: Alpha Beta\n"),
            note("d.md", "title: Delta\n"),
            note("e.md", "priority: 10\ntitle: Epsilon\n"),
        ]
    }

    #[test]
    fn eq_reussit_sur_valeur_texte() {
        let notes = notes();
        assert_eq!(
            filter_notes(&notes, &[Query::new("title", FieldCmp::Eq("Alpha".into()))]),
            vec![0]
        );
    }

    #[test]
    fn eq_echec_sur_valeur_differente() {
        let notes = notes();
        assert!(filter_notes(
            &notes,
            &[Query::new("title", FieldCmp::Eq("Inexistant".into()))]
        )
        .is_empty());
    }

    #[test]
    fn eq_insensible_a_la_valeur_d_un_autre_champ() {
        let notes = notes();
        assert_eq!(
            filter_notes(&notes, &[Query::new("priority", FieldCmp::Eq("5".into()))]),
            vec![0, 2]
        );
    }

    #[test]
    fn contient_reussit_en_sous_chaine() {
        let notes = notes();
        assert_eq!(
            filter_notes(
                &notes,
                &[Query::new("title", FieldCmp::Contains("Alpha".into()))]
            ),
            vec![0, 2]
        );
    }

    #[test]
    fn contient_echec_si_absent() {
        let notes = notes();
        assert!(filter_notes(
            &notes,
            &[Query::new("title", FieldCmp::Contains("Z".into()))]
        )
        .is_empty());
    }

    #[test]
    fn superieur_strict_numerique() {
        let notes = notes();
        assert_eq!(
            filter_notes(&notes, &[Query::new("priority", FieldCmp::Gt(4.0))]),
            vec![0, 2, 4]
        );
    }

    #[test]
    fn superieur_strict_exclut_la_borne() {
        let notes = notes();
        assert_eq!(
            filter_notes(&notes, &[Query::new("priority", FieldCmp::Gt(5.0))]),
            vec![4]
        );
    }

    #[test]
    fn inferieur_strict_numerique() {
        let notes = notes();
        assert_eq!(
            filter_notes(&notes, &[Query::new("priority", FieldCmp::Lt(4.0))]),
            vec![1]
        );
    }

    #[test]
    fn champ_absent_exclu_du_filtrage() {
        let notes = notes();
        // `d` n'a pas de `priority` : elle doit disparaître du résultat.
        assert_eq!(
            filter_notes(&notes, &[Query::new("priority", FieldCmp::Gt(0.0))]),
            vec![0, 1, 2, 4]
        );
    }

    #[test]
    fn frontmatter_absent_exclu_du_filtrage() {
        let notes = notes();
        let sans = note("f.md", "");
        let mut notes = notes;
        notes.push(sans);
        assert_eq!(
            filter_notes(&notes, &[Query::new("priority", FieldCmp::Gt(0.0))]),
            vec![0, 1, 2, 4]
        );
    }

    #[test]
    fn conjonction_et_logique() {
        let notes = notes();
        // `Alpha` dans le titre ET priority > 4 → a (5) et c (5).
        assert_eq!(
            filter_notes(
                &notes,
                &[
                    Query::new("title", FieldCmp::Contains("Alpha".into())),
                    Query::new("priority", FieldCmp::Gt(4.0)),
                ]
            ),
            vec![0, 2]
        );
    }

    #[test]
    fn conjonction_exclut_dès_qu_une_requete_echoue() {
        let notes = notes();
        // priority > 4 exclut b (3) ; title == Beta exclut le reste.
        assert!(filter_notes(
            &notes,
            &[
                Query::new("priority", FieldCmp::Gt(4.0)),
                Query::new("title", FieldCmp::Eq("Beta".into())),
            ]
        )
        .is_empty());
    }

    #[test]
    fn requete_vide_retourne_toutes_les_notes() {
        let notes = notes();
        assert_eq!(filter_notes(&notes, &[]), vec![0, 1, 2, 3, 4]);
    }

    #[test]
    fn tri_numerique_croissant_et_sans_champ_en_fin() {
        let notes = notes();
        let mut indices = vec![2, 0, 4, 1, 3];
        sort_by_field(&notes, "priority", true, &mut indices);
        // 3 (b), puis 5 (a, c, ordre d'origine), puis 10 (e), enfin d (absent).
        assert_eq!(indices, vec![1, 0, 2, 4, 3]);
    }

    #[test]
    fn tri_numerique_decroissant_et_sans_champ_en_fin() {
        let notes = notes();
        let mut indices = vec![0, 1, 2, 3, 4];
        sort_by_field(&notes, "priority", false, &mut indices);
        // 10, 5, 5, 3, puis d (absent) toujours en fin.
        assert_eq!(indices, vec![4, 0, 2, 1, 3]);
    }

    #[test]
    fn tri_texte_croissant() {
        let notes = notes();
        let mut indices = vec![4, 3, 2, 1, 0];
        sort_by_field(&notes, "title", true, &mut indices);
        assert_eq!(indices, vec![0, 2, 1, 3, 4]);
    }

    #[test]
    fn tri_texte_decroissant() {
        let notes = notes();
        let mut indices = vec![0, 1, 2, 3, 4];
        sort_by_field(&notes, "title", false, &mut indices);
        assert_eq!(indices, vec![4, 3, 1, 2, 0]);
    }

    #[test]
    fn tri_stable_a_egalite() {
        let notes = notes();
        // `a` et `c` partagent priority = 5 ; placés dans l'ordre inverse,
        // l'ordre d'origine doit être restauré par la stabilité.
        let mut indices = vec![2, 0];
        sort_by_field(&notes, "priority", true, &mut indices);
        assert_eq!(indices, vec![0, 2]);
    }

    #[test]
    fn tri_stable_a_egalite_sens_decroissant() {
        let notes = notes();
        let mut indices = vec![2, 0];
        sort_by_field(&notes, "priority", false, &mut indices);
        assert_eq!(indices, vec![0, 2]);
    }

    #[test]
    fn tri_vide_ne_fait_rien() {
        let notes = notes();
        let mut indices = Vec::new();
        sort_by_field(&notes, "priority", true, &mut indices);
        assert!(indices.is_empty());
    }

    #[test]
    fn type_mixte_nombre_et_chaine_numerique() {
        let notes = vec![
            note("a.md", "priority: 5\n"),
            note("b.md", "priority: \"5\"\n"),
            note("c.md", "priority: \"10\"\n"),
            note("d.md", "priority: \"abc\"\n"),
        ];
        let mut indices = vec![3, 2, 1, 0];
        sort_by_field(&notes, "priority", true, &mut indices);
        // 5 (nombre), 5 (chaîne), 10 (chaîne), puis « abc » en texte.
        assert_eq!(indices, vec![0, 1, 2, 3]);
    }

    #[test]
    fn filtre_sur_chaine_numerique() {
        let notes = vec![
            note("a.md", "priority: \"5\"\n"),
            note("b.md", "priority: \"12\"\n"),
            note("c.md", "priority: abc\n"),
        ];
        // Gt(4) : 5 et 12 sont numériques, abc ne l'est pas → exclue.
        assert_eq!(
            filter_notes(&notes, &[Query::new("priority", FieldCmp::Gt(4.0))]),
            vec![0, 1]
        );
    }

    #[test]
    fn valeur_null_est_presente_mais_vide() {
        let notes = vec![note("a.md", "title: \n"), note("b.md", "title: ok\n")];
        // `title: ` donne un `Value::Null` : présent, donc non exclu par Eq(""),
        // mais différent de « ok ».
        assert_eq!(
            filter_notes(&notes, &[Query::new("title", FieldCmp::Eq("".into()))]),
            vec![0]
        );
        assert_eq!(
            filter_notes(
                &notes,
                &[Query::new("title", FieldCmp::Contains("".into()))]
            ),
            vec![0, 1]
        );
    }

    #[test]
    fn sequence_serialisee_en_yaml_compact() {
        let notes = vec![
            note("a.md", "tags:\n  - rust\n  - test\n"),
            note("b.md", "tags: [rust, docs]\n"),
        ];
        assert_eq!(
            filter_notes(
                &notes,
                &[Query::new("tags", FieldCmp::Contains("rust".into()))]
            ),
            vec![0, 1]
        );
        assert_eq!(
            filter_notes(
                &notes,
                &[Query::new("tags", FieldCmp::Contains("docs".into()))]
            ),
            vec![1]
        );
    }
}
