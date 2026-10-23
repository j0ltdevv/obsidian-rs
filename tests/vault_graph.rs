//! Tests d'intégration du graphe de vault (`src/vault/graph.rs`).

use std::path::PathBuf;

use obsy::vault::graph::{build_graph, Graph};
use obsy::vault::index::NoteIndex;
use obsy::vault::{Note, NoteMetadata, Wikilink};

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

/// Construit le graphe des notes via l'index.
fn graphe(notes: &[Note]) -> Graph {
    let index = NoteIndex::build_index(notes);
    build_graph(notes, &index)
}

#[test]
fn lien_unidirectionnel_cree_l_arete_une_seule_fois() {
    // A → B : une seule arête non orientée.
    let notes = vec![note("A", &[("B", None)]), sans_liens("B")];
    let g = graphe(&notes);
    assert_eq!(g.nodes, vec![0, 1]);
    assert_eq!(g.edges, vec![(0, 1)]);

    // Réciproque B → A : même arête unique (le graphe est non orienté).
    let notes = vec![sans_liens("A"), note("B", &[("A", None)])];
    let g = graphe(&notes);
    assert_eq!(g.edges, vec![(0, 1)]);

    // Double lien A → B et B → A : toujours une seule arête.
    let notes = vec![note("A", &[("B", None)]), note("B", &[("A", None)])];
    let g = graphe(&notes);
    assert_eq!(g.edges, vec![(0, 1)]);
}

#[test]
fn liens_dangling_et_ambigus_ignores() {
    // Lien dangling (cible inexistante) : aucune arête.
    let notes = vec![note("A", &[("Inexistante", None)]), sans_liens("B")];
    assert!(graphe(&notes).edges.is_empty());

    // Lien ambigu (deux notes homonymes) : aucune arête.
    let notes = vec![
        note("A", &[("Dup", None)]),
        sans_liens("Dup"),
        sans_liens("Dup"),
    ];
    let g = graphe(&notes);
    assert!(g.edges.is_empty());
    assert_eq!(g.nodes.len(), 3);

    // Seuls les liens non résolus sont ignorés ; les autres survivent.
    let notes = vec![
        note("A", &[("Inexistante", None), ("B", None)]),
        sans_liens("B"),
    ];
    assert_eq!(graphe(&notes).edges, vec![(0, 1)]);
}

#[test]
fn boucle_sur_lui_meme_ignoree() {
    let notes = vec![note("A", &[("A", None)])];
    let g = graphe(&notes);
    assert_eq!(g.nodes, vec![0]);
    assert!(g.edges.is_empty());
    assert!(g.neighbors(0).is_empty());
}

#[test]
fn chaine_aretes_et_voisins_exacts() {
    let notes = vec![
        note("A", &[("B", None)]),
        note("B", &[("C", None)]),
        sans_liens("C"),
    ];
    let g = graphe(&notes);
    assert_eq!(g.edges, vec![(0, 1), (1, 2)]);
    assert_eq!(g.neighbors(0), vec![1]);
    assert_eq!(g.neighbors(1), vec![0, 2]);
    assert_eq!(g.neighbors(2), vec![1]);
}

#[test]
fn composantes_isoles_et_chaine() {
    // 3 nœuds isolés → 3 composantes, plus grande = 1.
    let notes = vec![sans_liens("A"), sans_liens("B"), sans_liens("C")];
    assert_eq!(graphe(&notes).components(), (3, 1));

    // Chaîne A—B—C → 1 composante de taille 3.
    let notes = vec![
        note("A", &[("B", None)]),
        note("B", &[("C", None)]),
        sans_liens("C"),
    ];
    assert_eq!(graphe(&notes).components(), (1, 3));
}

#[test]
fn degre_du_noeud_central_dans_un_triangle() {
    // Triangle A—B—C—A : chaque nœud a exactement 2 voisins.
    let notes = vec![
        note("A", &[("B", None)]),
        note("B", &[("C", None)]),
        note("C", &[("A", None)]),
    ];
    let g = graphe(&notes);
    assert_eq!(g.edges.len(), 3);
    for node in 0..3 {
        assert_eq!(g.degree(node), 2);
    }
}

#[test]
fn build_graph_stable_sans_liens() {
    let notes = vec![sans_liens("A"), sans_liens("B"), sans_liens("C")];
    let g = graphe(&notes);
    assert_eq!(g.nodes, vec![0, 1, 2]);
    assert!(g.edges.is_empty());
    for node in 0..3 {
        assert_eq!(g.degree(node), 0);
        assert!(g.neighbors(node).is_empty());
    }
}

#[test]
fn ordre_des_noeuds_suit_l_ordre_des_notes() {
    let notes = vec![
        sans_liens("Zeta"),
        sans_liens("Alpha"),
        sans_liens("Milieu"),
    ];
    let g = graphe(&notes);
    assert_eq!(g.nodes, vec![0, 1, 2]);

    // Les arêtes référencent les bons indices, indépendamment de l'ordre
    // alphabétique des noms.
    let notes = vec![
        note("Zeta", &[("Alpha", None)]),
        sans_liens("Alpha"),
        sans_liens("Milieu"),
    ];
    let g = graphe(&notes);
    assert_eq!(g.edges, vec![(0, 1)]);
}
