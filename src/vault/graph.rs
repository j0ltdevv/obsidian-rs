//! Graphe non orienté des notes du vault : nœuds = notes (dans l'ordre du
//! vault), arêtes = paires de notes reliées par un wikilink résolu.

use std::collections::VecDeque;

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
