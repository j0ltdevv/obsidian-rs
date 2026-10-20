//! Index de navigation du vault : résolution de wikilinks, backlinks et
//! recherche floue par sous-séquence.

use std::collections::HashMap;

use super::note::Note;

/// Index construit depuis la liste des notes (le scan est déjà effectué).
///
/// Résolution :
/// - chaque note est identifiée par son **nom** (stem du fichier, sans
///   extension), insensible à la casse ;
/// - un wikilink dont la cible correspond à une seule note est **résolu** ;
/// - un wikilink sans cible (aucune note homonyme) ou **ambigu** (plusieurs
///   notes partagent le même nom) est retenu comme dangling/ambigu.
#[derive(Debug)]
pub struct NoteIndex {
    /// Copie des notes, dans l'ordre du vault.
    notes: Vec<Note>,
    /// Nom de note en minuscules → indices dans `notes`.
    by_name: HashMap<String, Vec<usize>>,
}

impl NoteIndex {
    /// Construit l'index depuis la liste des notes (scan déjà effectué).
    pub fn build_index(notes: &[Note]) -> NoteIndex {
        let owned: Vec<Note> = notes.to_vec();
        let mut by_name: HashMap<String, Vec<usize>> = HashMap::new();
        for (index, note) in owned.iter().enumerate() {
            by_name.entry(key(&note.name)).or_default().push(index);
        }
        NoteIndex {
            notes: owned,
            by_name,
        }
    }

    /// Retourne les notes qui pointent vers `note` (backlinks), dans l'ordre
    /// du vault. Les auto-liens et les liens dangling/ambigus sont ignorés.
    /// Si `note` ne fait pas partie de l'index, retourne une liste vide.
    pub fn backlinks<'a>(&'a self, note: &Note) -> Vec<&'a Note> {
        let Some(target) = self.index_of(note) else {
            return Vec::new();
        };
        let mut result = Vec::new();
        for (i, candidate) in self.notes.iter().enumerate() {
            if i == target {
                continue;
            }
            let points_to_target = candidate
                .metadata
                .wikilinks
                .iter()
                .filter_map(|link| resolve_target(&self.by_name, &link.target))
                .any(|resolved| resolved == target);
            if points_to_target {
                result.push(candidate);
            }
        }
        result
    }

    /// Retourne les cibles de `note` : `Some(nom)` pour un lien résolu,
    /// `None` pour un lien sans cible ou ambigu.
    pub fn outgoing<'a>(&'a self, note: &Note) -> Vec<Option<&'a str>> {
        let Some(i) = self.index_of(note) else {
            return Vec::new();
        };
        self.notes[i]
            .metadata
            .wikilinks
            .iter()
            .map(|link| {
                resolve_target(&self.by_name, &link.target)
                    .map(|resolved| self.notes[resolved].name.as_str())
            })
            .collect()
    }

    /// Résolution par nom (insensible à la casse) : retourne toutes les notes
    /// dont le stem correspond, doublons compris. Nom inconnu → liste vide.
    pub fn resolve<'a>(&'a self, name: &str) -> Vec<&'a Note> {
        match self.by_name.get(&key(name)) {
            Some(indices) => indices.iter().map(|&i| &self.notes[i]).collect(),
            None => Vec::new(),
        }
    }

    /// Recherche floue simple (sous-séquence, insensible à la casse) sur les
    /// noms des notes de `notes`. Retourne les notes triées par score
    /// décroissant, limitées à `limit`. Requête vide → notes dans l'ordre du
    /// vault.
    pub fn search<'a>(&'a self, query: &str, notes: &'a [Note], limit: usize) -> Vec<&'a Note> {
        let mut scored: Vec<(&'a Note, u64)> = Vec::new();
        if query.is_empty() {
            scored.extend(notes.iter().map(|note| (note, 0)));
        } else {
            for note in notes {
                if let Some(score) = score(query, &note.name) {
                    scored.push((note, score));
                }
            }
        }
        scored.sort_by_key(|(_, score)| std::cmp::Reverse(*score));
        scored
            .into_iter()
            .take(limit)
            .map(|(note, _)| note)
            .collect()
    }

    /// Indice de `note` dans l'index, si elle y figure (comparaison par valeur).
    fn index_of(&self, note: &Note) -> Option<usize> {
        self.notes.iter().position(|candidate| candidate == note)
    }
}

/// Clé d'indexation pour un nom de note (normalisation minuscules).
fn key(name: &str) -> String {
    name.to_lowercase()
}

/// Résout une cible de wikilink vers un indice de note unique.
///
/// Retourne `None` si aucune note ne correspond au nom, ou si plusieurs notes
/// le partagent (ambiguïté).
fn resolve_target(by_name: &HashMap<String, Vec<usize>>, target: &str) -> Option<usize> {
    let indices = by_name.get(&key(target))?;
    match indices.len() {
        1 => Some(indices[0]),
        _ => None,
    }
}

/// Score de correspondance entre `query` et `name` (sous-séquence,
/// insensible à la casse). Retourne `None` si `query` n'est pas une
/// sous-séquence de `name`. Le score favorise la couverture de la requête et
/// les correspondances précoces.
fn score(query: &str, name: &str) -> Option<u64> {
    let query: Vec<char> = query.chars().flat_map(char::to_lowercase).collect();
    let name: Vec<char> = name.chars().flat_map(char::to_lowercase).collect();

    let mut qi = 0;
    let mut positions_sum = 0usize;
    for (i, &c) in name.iter().enumerate() {
        if qi < query.len() && c == query[qi] {
            positions_sum += i;
            qi += 1;
        }
    }
    if qi < query.len() {
        return None;
    }

    let name_len = name.len().max(1) as u64;
    let query_len = query.len() as u64;
    let coverage = 100 * query_len / name_len;
    let prefix_bonus = if name.starts_with(&query) { 500 } else { 0 };

    Some(10_000 - positions_sum as u64 + coverage + prefix_bonus)
}
