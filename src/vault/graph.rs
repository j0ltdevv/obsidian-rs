//! Graphe non orienté des notes du vault : nœuds = notes (dans l'ordre du
//! vault), arêtes = paires de notes reliées par un wikilink résolu.

use std::collections::VecDeque;
use std::path::PathBuf;

use super::index::NoteIndex;
use super::note::Note;

/// Identifiant de nœud : indice de la note dans le vault.
pub type NodeId = usize;

/// Graphe simple non orienté construit depuis les notes et leur index.
#[derive(Debug)]
pub struct Graph {
    /// Nœuds, dans l'ordre des notes (chaque nœud = indice de note).
    pub nodes: Vec<NodeId>,
    /// Arêtes non orientées uniques, triées par paire (a < b).
    pub edges: Vec<(NodeId, NodeId)>,
    /// Liste d'adjacence triée (lookups voisins/degré).
    adjacency: Vec<Vec<NodeId>>,
}

/// Filtres d'affichage du graphe.
#[derive(Debug, Clone, Default)]
pub struct GraphFilters {
    /// Si défini, n'affiche que la note cible et ses voisins directs
    /// (graphe local autour de cette note).
    pub local_note: Option<PathBuf>,
    /// Si défini, n'affiche que les notes portant ce tag.
    pub tag: Option<String>,
    /// Si défini, n'affiche que les notes de ce dossier (ou
    /// sous-dossiers).
    pub folder: Option<PathBuf>,
}

impl GraphFilters {
    /// `true` si la note d'indice `idx` doit être affichée.
    pub fn allows(&self, graph: &Graph, notes: &[Note], idx: usize) -> bool {
        let Some(note) = notes.get(idx) else {
            return false;
        };
        if let Some(local) = &self.local_note {
            let Some(target) = notes.iter().position(|n| n.path == *local) else {
                return true;
            };
            if idx != target && !graph.neighbors(target).contains(&idx) {
                return false;
            }
        }
        if let Some(tag) = &self.tag {
            if !note.metadata.tags.iter().any(|t| t == tag) {
                return false;
            }
        }
        if let Some(folder) = &self.folder {
            if !note.path.starts_with(folder) {
                return false;
            }
        }
        true
    }

    /// `true` si aucun filtre n'est actif.
    pub fn is_empty(&self) -> bool {
        self.local_note.is_none() && self.tag.is_none() && self.folder.is_none()
    }
}

impl Graph {
    /// Retourne les voisins directs d'un nœud, dans l'ordre croissant.
    /// Nœud inconnu → liste vide.
    pub fn neighbors(&self, node: NodeId) -> Vec<NodeId> {
        self.adjacency.get(node).cloned().unwrap_or_default()
    }

    /// Nombre de composantes connexes (BFS) et taille de la plus grande.
    /// Les nœuds isolés comptent comme composantes de taille 1.
    pub fn components(&self) -> (usize, usize) {
        let mut visited = vec![false; self.nodes.len()];
        let mut component_count = 0;
        let mut largest = 0;
        for start in 0..self.nodes.len() {
            if visited[start] {
                continue;
            }
            component_count += 1;
            let mut size = 0;
            let mut queue = VecDeque::new();
            queue.push_back(start);
            visited[start] = true;
            while let Some(node) = queue.pop_front() {
                size += 1;
                for &neighbor in &self.adjacency[node] {
                    if !visited[neighbor] {
                        visited[neighbor] = true;
                        queue.push_back(neighbor);
                    }
                }
            }
            largest = largest.max(size);
        }
        (component_count, largest)
    }

    /// Degré d'un nœud (nombre d'arêtes incidentes). Nœud inconnu → 0.
    pub fn degree(&self, node: NodeId) -> usize {
        self.adjacency.get(node).map_or(0, Vec::len)
    }
}

/// Construit le graphe non orienté des notes : une arête par paire
/// (noteA, noteB) si noteA possède un wikilink résolu vers noteB (ou
/// réciproquement). Les liens dangling/ambigus sont ignorés, ainsi que les
/// boucles (note → elle-même).
pub fn build_graph(notes: &[Note], index: &NoteIndex) -> Graph {
    let mut undirected: Vec<(NodeId, NodeId)> = Vec::new();
    for (i, note) in notes.iter().enumerate() {
        for resolved in index.outgoing(note) {
            let Some(name) = resolved else {
                continue;
            };
            let Some(j) = notes.iter().position(|candidate| candidate.name == name) else {
                continue;
            };
            if i == j {
                continue; // boucle ignorée
            }
            undirected.push(if i < j { (i, j) } else { (j, i) });
        }
    }
    undirected.sort_unstable();
    undirected.dedup();

    let mut adjacency = vec![Vec::new(); notes.len()];
    for &(a, b) in &undirected {
        adjacency[a].push(b);
        adjacency[b].push(a);
    }
    for adj in &mut adjacency {
        adj.sort_unstable();
    }

    Graph {
        nodes: (0..notes.len()).collect(),
        edges: undirected,
        adjacency,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vault::note::NoteMetadata;
    use std::path::Path;

    fn note(path: &str, tags: &[&str]) -> Note {
        Note {
            path: PathBuf::from(path),
            name: Path::new(path)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or(path)
                .to_string(),
            metadata: NoteMetadata {
                frontmatter: None,
                frontmatter_error: None,
                wikilinks: Vec::new(),
                tags: tags.iter().map(|t| t.to_string()).collect(),
            },
        }
    }

    fn sample_notes() -> Vec<Note> {
        vec![
            note("A.md", &["projet"]),
            note("B.md", &["idée"]),
            note("Dossier/C.md", &["projet"]),
        ]
    }

    #[test]
    fn pas_de_filtre_autorise_tout() {
        let notes = sample_notes();
        let index = NoteIndex::build_index(&notes);
        let graph = build_graph(&notes, &index);
        let filters = GraphFilters::default();
        assert!(filters.allows(&graph, &notes, 0));
        assert!(filters.allows(&graph, &notes, 2));
    }

    #[test]
    fn filtre_local_sans_arête_ne_garde_que_la_note_cible() {
        let notes = sample_notes();
        let index = NoteIndex::build_index(&notes);
        let graph = build_graph(&notes, &index);
        let filters = GraphFilters {
            local_note: Some(PathBuf::from("A.md")),
            ..GraphFilters::default()
        };
        assert!(filters.allows(&graph, &notes, 0));
        assert!(!filters.allows(&graph, &notes, 1));
        assert!(!filters.allows(&graph, &notes, 2));
    }

    #[test]
    fn filtre_tag_ne_garde_que_les_notes_porteuses() {
        let notes = sample_notes();
        let index = NoteIndex::build_index(&notes);
        let graph = build_graph(&notes, &index);
        let filters = GraphFilters {
            tag: Some("projet".to_string()),
            ..GraphFilters::default()
        };
        assert!(filters.allows(&graph, &notes, 0));
        assert!(!filters.allows(&graph, &notes, 1));
        assert!(filters.allows(&graph, &notes, 2));
    }

    #[test]
    fn filtre_dossier_restrint_au_dossier_ou_sous_dossiers() {
        let notes = sample_notes();
        let index = NoteIndex::build_index(&notes);
        let graph = build_graph(&notes, &index);
        let filters = GraphFilters {
            folder: Some(PathBuf::from("Dossier")),
            ..GraphFilters::default()
        };
        assert!(!filters.allows(&graph, &notes, 0));
        assert!(filters.allows(&graph, &notes, 2));
    }
}
