//! Tests d'intégration du langage de requête Dataview (sous-ensemble DQL).
//!
//! Toutes les notes sont synthétiques : aucun accès au vrai vault ni au
//! système de fichiers (les champs `file.size`, `file.mtime` et
//! `file.ctime` sont donc absents ici, car les chemins n'existent pas sur
//! disque).

use obsy::vault::dataview::query_notes;
use obsy::vault::note::{Note, NoteMetadata, Wikilink};
use std::path::{Path, PathBuf};

/// Construit une note synthétique.
fn note(path: &str, frontmatter: Option<&str>, tags: &[&str], links: &[&str]) -> Note {
    let frontmatter =
        frontmatter.map(|yaml| serde_yaml::from_str(yaml).expect("frontmatter YAML valide"));

    Note {
        path: PathBuf::from(path),
        name: Path::new(path)
            .file_stem()
            .and_then(|stem| stem.to_str())
            .expect("stem valide")
            .to_string(),
        metadata: NoteMetadata {
            frontmatter,
            frontmatter_error: None,
            wikilinks: links
                .iter()
                .map(|target| Wikilink {
                    target: target.to_string(),
                    alias: None,
                })
                .collect(),
            tags: tags.iter().map(|tag| tag.to_string()).collect(),
        },
    }
}

/// Notes de test :
///
/// | indice | path          | name | priority | title   | tags            | links |
/// |--------|---------------|------|----------|---------|-----------------|-------|
/// | 0      | proj/e.md     | e    | 10       | Epsilon | project/high    | —     |
/// | 1      | docs/c.md     | c    | 2        | Gamma   | personal        | B     |
/// | 2      | proj/a.md     | a    | 5        | Alpha   | project, urgent | A, B |
/// | 3      | proj/b.md     | b    | 3        | Beta    | project         | C     |
/// | 4      | proj/d.md     | d    | —        | Delta   | project         | A     |
fn notes() -> Vec<Note> {
    vec![
        note(
            "proj/e.md",
            Some("priority: 10\ntitle: Epsilon\n"),
            &["project/high"],
            &[],
        ),
        note(
            "docs/c.md",
            Some("priority: 2\ntitle: Gamma\n"),
            &["personal"],
            &["B"],
        ),
        note(
            "proj/a.md",
            Some("priority: 5\ntitle: Alpha\n"),
            &["project", "urgent"],
            &["A", "B"],
        ),
        note(
            "proj/b.md",
            Some("priority: 3\ntitle: Beta\n"),
            &["project"],
            &["C"],
        ),
        note("proj/d.md", Some("title: Delta\n"), &["project"], &["A"]),
    ]
}

// ---------------------------------------------------------------------------
// requête vide
// ---------------------------------------------------------------------------

#[test]
fn requete_vide_renvoie_toutes_les_notes() {
    let notes = notes();
    assert_eq!(query_notes(&notes, "").unwrap(), vec![0, 1, 2, 3, 4]);
}

// ---------------------------------------------------------------------------
// FROM
// ---------------------------------------------------------------------------

#[test]
fn from_tag_ne_renvoie_que_les_notes_portant_ce_tag() {
    let notes = notes();
    assert_eq!(
        query_notes(&notes, "FROM #project").unwrap(),
        vec![0, 2, 3, 4]
    );
}

#[test]
fn from_tag_imbrique_concerne_aussi_le_sous_tag() {
    let notes = notes();
    assert_eq!(
        query_notes(&notes, "FROM #project").unwrap(),
        vec![0, 2, 3, 4]
    );
}

#[test]
fn from_tag_precis_filtre_sur_le_sous_tag() {
    let notes = notes();
    assert_eq!(query_notes(&notes, "FROM #project/high").unwrap(), vec![0]);
}

#[test]
fn from_dossier_inclut_les_sous_dossiers() {
    let notes = notes();
    assert_eq!(
        query_notes(&notes, "FROM \"proj\"").unwrap(),
        vec![0, 2, 3, 4]
    );
    assert_eq!(query_notes(&notes, "FROM \"docs\"").unwrap(), vec![1]);
}

#[test]
fn from_etoile_renvoie_tout() {
    let notes = notes();
    assert_eq!(query_notes(&notes, "FROM *").unwrap(), vec![0, 1, 2, 3, 4]);
}

// ---------------------------------------------------------------------------
// WHERE
// ---------------------------------------------------------------------------

#[test]
fn where_egalite_texte() {
    let notes = notes();
    assert_eq!(
        query_notes(&notes, "WHERE title = \"Alpha\"").unwrap(),
        vec![2]
    );
}

#[test]
fn where_contient_sur_liste_de_tags() {
    let notes = notes();
    assert_eq!(
        query_notes(&notes, "WHERE file.tags contains \"urgent\"").unwrap(),
        vec![2]
    );
    assert_eq!(
        query_notes(&notes, "WHERE file.tags contains \"project\"").unwrap(),
        vec![0, 2, 3, 4]
    );
}

#[test]
fn where_startswith_et_endswith() {
    let notes = notes();
    assert_eq!(
        query_notes(&notes, "WHERE file.name startswith \"a\"").unwrap(),
        vec![2]
    );
    assert_eq!(
        query_notes(&notes, "WHERE file.name endswith \"b\"").unwrap(),
        vec![3]
    );
}

#[test]
fn where_comparaison_numerique() {
    let notes = notes();
    assert_eq!(query_notes(&notes, "WHERE priority > 9").unwrap(), vec![0]);
    assert_eq!(
        query_notes(&notes, "WHERE priority >= 5").unwrap(),
        vec![0, 2]
    );
    assert_eq!(query_notes(&notes, "WHERE priority < 3").unwrap(), vec![1]);
    assert_eq!(query_notes(&notes, "WHERE priority <= 2").unwrap(), vec![1]);
    assert_eq!(
        query_notes(&notes, "WHERE priority != 5").unwrap(),
        vec![0, 1, 3]
    );
}

#[test]
fn where_and() {
    let notes = notes();
    assert_eq!(
        query_notes(&notes, "WHERE priority > 2 AND title contains \"a\"").unwrap(),
        vec![2, 3]
    );
}

#[test]
fn where_or() {
    let notes = notes();
    assert_eq!(
        query_notes(&notes, "WHERE priority > 9 OR title = \"Beta\"").unwrap(),
        vec![0, 3]
    );
}

#[test]
fn where_not() {
    let notes = notes();
    assert_eq!(
        query_notes(&notes, "WHERE NOT file.tags contains \"project\"").unwrap(),
        vec![1]
    );
}

#[test]
fn where_parentheseses() {
    let notes = notes();
    assert_eq!(
        query_notes(
            &notes,
            "WHERE (priority > 2 AND title contains \"a\") OR priority > 9"
        )
        .unwrap(),
        vec![0, 2, 3]
    );
}

#[test]
fn where_champ_absent_exclu() {
    let notes = notes();
    // `d` n'a pas de `priority` : elle ne satisfait jamais une comparaison dessus.
    assert_eq!(
        query_notes(&notes, "WHERE priority > 0").unwrap(),
        vec![0, 1, 2, 3]
    );
}

// ---------------------------------------------------------------------------
// SORT / LIMIT
// ---------------------------------------------------------------------------

#[test]
fn sort_ascendant() {
    let notes = notes();
    assert_eq!(
        query_notes(&notes, "SORT priority ASC").unwrap(),
        vec![1, 3, 2, 0, 4]
    );
}

#[test]
fn sort_descendant() {
    let notes = notes();
    assert_eq!(
        query_notes(&notes, "SORT priority DESC").unwrap(),
        vec![0, 2, 3, 1, 4]
    );
}

#[test]
fn sort_sur_nom_de_fichier() {
    let notes = notes();
    assert_eq!(
        query_notes(&notes, "SORT file.name ASC").unwrap(),
        vec![2, 3, 1, 4, 0]
    );
    assert_eq!(
        query_notes(&notes, "SORT file.name DESC").unwrap(),
        vec![0, 4, 1, 3, 2]
    );
}

#[test]
fn limit_tronque_le_resultat() {
    let notes = notes();
    assert_eq!(
        query_notes(&notes, "FROM #project LIMIT 2").unwrap(),
        vec![0, 2]
    );
    assert_eq!(query_notes(&notes, "LIMIT 0").unwrap(), Vec::<usize>::new());
}

// ---------------------------------------------------------------------------
// requête complète
// ---------------------------------------------------------------------------

#[test]
fn requete_complete_from_where_sort_limit() {
    let notes = notes();
    assert_eq!(
        query_notes(
            &notes,
            "FROM #project WHERE priority >= 3 SORT priority DESC LIMIT 2"
        )
        .unwrap(),
        vec![0, 2]
    );
}

// ---------------------------------------------------------------------------
// erreurs
// ---------------------------------------------------------------------------

#[test]
fn champ_file_inconnu_erreur() {
    let notes = notes();
    let err = query_notes(&notes, "WHERE file.bogus = 1").unwrap_err();
    assert!(err.to_string().contains("champ inconnu"), "{err}");
}

#[test]
fn operateur_manquant_erreur() {
    let notes = notes();
    let err = query_notes(&notes, "WHERE file.name \"x\"").unwrap_err();
    assert!(err.to_string().contains("opérateur manquant"), "{err}");
}

#[test]
fn guillemet_non_ferme_erreur() {
    let notes = notes();
    let err = query_notes(&notes, "WHERE title = \"Alpha").unwrap_err();
    assert!(err.to_string().contains("guillemet non fermé"), "{err}");
}

#[test]
fn limit_invalide_erreur() {
    let notes = notes();
    let err = query_notes(&notes, "LIMIT beaucoup").unwrap_err();
    assert!(err.to_string().contains("nombre invalide"), "{err}");
}

#[test]
fn clause_inconnue_erreur() {
    let notes = notes();
    let err = query_notes(&notes, "FILTRE title = \"x\"").unwrap_err();
    assert!(err.to_string().contains("jeton inattendu"), "{err}");
}
