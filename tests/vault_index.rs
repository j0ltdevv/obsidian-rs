//! Tests d'intégration de l'index de vault (`src/vault/index.rs`).

use std::path::PathBuf;

use obsidian_rs::vault::index::NoteIndex;
use obsidian_rs::vault::{Note, NoteMetadata, Wikilink};

/// Construit une note de test avec ses wikilinks (cible, alias).
fn note(name: &str, links: &[(&str, Option<&str>)]) -> Note {
    Note {
        path: PathBuf::from(format!("{}.md", name)),
        name: name.to_string(),
        metadata: NoteMetadata {
            frontmatter: None,
            frontmatter_error: None,
            wikilinks: links
                .iter()
                .map(|(target, alias)| Wikilink {
                    target: target.to_string(),
                    alias: alias.map(str::to_string),
                })
                .collect(),
            tags: Vec::new(),
        },
    }
}

/// Note sans wikilink.
fn sans_liens(name: &str) -> Note {
    note(name, &[])
}

#[test]
fn backlinks_retourne_les_notes_qui_pointent_vers_la_cible() {
    let notes = vec![
        sans_liens("A"),
        note("B", &[("A", None)]),
        note("C", &[("A", None)]),
    ];
    let index = NoteIndex::build_index(&notes);

    let backlinks = index.backlinks(&notes[0]);
    let noms: Vec<&str> = backlinks.iter().map(|n| n.name.as_str()).collect();

    assert_eq!(noms, vec!["B", "C"]);
}

#[test]
fn backlinks_ignore_auto_liens_et_liens_dangling() {
    let notes = vec![
        note("A", &[("A", None)]),       // auto-lien : ignoré
        note("B", &[("Absente", None)]), // dangling : ignoré
        note("C", &[("A", None)]),
    ];
    let index = NoteIndex::build_index(&notes);

    let backlinks = index.backlinks(&notes[0]);
    let noms: Vec<&str> = backlinks.iter().map(|n| n.name.as_str()).collect();

    assert_eq!(noms, vec!["C"]);
}

#[test]
fn outgoing_retourne_some_pour_liens_resolus_et_none_pour_dangling() {
    let notes = vec![
        sans_liens("Cible"),
        note("Source", &[("Cible", None), ("Absente", None), ("", None)]),
    ];
    let index = NoteIndex::build_index(&notes);

    let outgoing = index.outgoing(&notes[1]);

    assert_eq!(outgoing.len(), 3);
    assert_eq!(outgoing[0], Some("Cible"));
    assert_eq!(outgoing[1], None);
    assert_eq!(outgoing[2], None);
}

#[test]
fn lien_ambigu_nest_ni_resolu_ni_backlink() {
    let notes = vec![sans_liens("A"), sans_liens("A"), note("B", &[("A", None)])];
    let index = NoteIndex::build_index(&notes);

    assert_eq!(index.outgoing(&notes[2]), vec![None]);
    assert!(index.backlinks(&notes[0]).is_empty());
    assert!(index.backlinks(&notes[1]).is_empty());
    assert_eq!(index.resolve("A").len(), 2);
}

#[test]
fn resolve_doublons_de_nom_retourne_toutes_les_notes() {
    let notes = vec![sans_liens("A"), sans_liens("A"), sans_liens("B")];
    let index = NoteIndex::build_index(&notes);

    // les doublons sont tous retournés, dans l'ordre du vault
    let resolved = index.resolve("A");
    assert_eq!(resolved.len(), 2);
    assert!(resolved.iter().all(|n| n.name == "A"));

    // résolution insensible à la casse
    assert_eq!(index.resolve("a").len(), 2);
    assert_eq!(index.resolve("B").len(), 1);
}

#[test]
fn resolve_nom_inconnu_retourne_liste_vide() {
    let notes = vec![sans_liens("A")];
    let index = NoteIndex::build_index(&notes);

    assert!(index.resolve("Zzz").is_empty());
}

#[test]
fn search_correspondance_sous_sequence_tri_par_score_et_limit() {
    let notes = vec![sans_liens("Vault"), sans_liens("Valise"), sans_liens("Bob")];
    let index = NoteIndex::build_index(&notes);

    // « lt » est une sous-séquence de « Vault » (l…t) mais pas de « Valise »
    // (aucun t) ni de « Bob ».
    let res = index.search("lt", &notes, 10);
    let noms: Vec<&str> = res.iter().map(|n| n.name.as_str()).collect();
    assert_eq!(noms, vec!["Vault"]);

    // « va » (préfixe commun, insensible à la casse) : « Vault » (plus
    // courte) est triée avant « Valise ».
    let res = index.search("va", &notes, 10);
    let noms: Vec<&str> = res.iter().map(|n| n.name.as_str()).collect();
    assert_eq!(noms, vec!["Vault", "Valise"]);

    // `limit` tronque le résultat.
    let res = index.search("va", &notes, 1);
    let noms: Vec<&str> = res.iter().map(|n| n.name.as_str()).collect();
    assert_eq!(noms, vec!["Vault"]);
}

#[test]
fn search_requete_vide_retourne_les_notes_dans_l_ordre_du_vault() {
    let notes = vec![
        sans_liens("Zebra"),
        sans_liens("Alpha"),
        sans_liens("Michigan"),
    ];
    let index = NoteIndex::build_index(&notes);

    let res = index.search("", &notes, 10);
    let noms: Vec<&str> = res.iter().map(|n| n.name.as_str()).collect();

    assert_eq!(noms, vec!["Zebra", "Alpha", "Michigan"]);
}

#[test]
fn build_index_stable_sur_des_notes_sans_wikilinks() {
    let notes = vec![sans_liens("A"), sans_liens("B"), sans_liens("C")];
    let index = NoteIndex::build_index(&notes);

    assert!(index.backlinks(&notes[0]).is_empty());
    assert!(index.outgoing(&notes[0]).is_empty());
    assert_eq!(index.resolve("A").len(), 1);
}
