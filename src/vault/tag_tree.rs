//! Arbre de tags : hiérarchie déduite des tags plats (`a/b/c`).

/// Un nœud de l'arbre de tags.
#[derive(Debug, Clone, PartialEq)]
pub struct TagNode {
    /// Segment de tag (sans `/`).
    pub name: String,
    /// Nombre de feuilles distinctes qui passent par ce nœud.
    pub count: usize,
    /// Sous-tags, triés alphabétiquement.
    pub children: Vec<TagNode>,
}

/// Construit l'arbre de tags à partir d'une liste de tags plats.
///
/// Chaque tag est découpé sur `/` ; un chemin `a/b/c` crée `a` > `b` > `c`.
/// `count` d'un nœud = nombre de feuilles distinctes qui le traversent (un
/// tag compte pour 1 à chaque niveau). Les frères sont triés par ordre
/// alphabétique. Liste vide → rien.
pub fn build_tag_tree(tags: &[String]) -> Vec<TagNode> {
    let mut roots: Vec<TagNode> = Vec::new();

    for tag in tags {
        let parts: Vec<&str> = tag.split('/').filter(|p| !p.is_empty()).collect();
        if parts.is_empty() {
            continue;
        }
        insert_path(&mut roots, &parts);
    }

    sort_tree(&mut roots);
    roots
}

/// Insère un chemin (liste de segments) dans l'arbre.
fn insert_path(nodes: &mut Vec<TagNode>, parts: &[&str]) {
    let head = parts[0].to_string();
    let slot = match nodes.iter_mut().find(|n| n.name == head) {
        Some(node) => node,
        None => {
            nodes.push(TagNode {
                name: head.clone(),
                count: 0,
                children: Vec::new(),
            });
            nodes.last_mut().unwrap()
        }
    };
    if parts.len() == 1 {
        slot.count += 1;
    } else {
        insert_path(&mut slot.children, &parts[1..]);
    }
}

/// Trie récursivement les enfants puis recalcule les comptes.
fn sort_tree(nodes: &mut [TagNode]) {
    for node in nodes.iter_mut() {
        sort_tree(&mut node.children);
    }
    nodes.sort_by(|a, b| a.name.cmp(&b.name));
    for node in nodes.iter_mut() {
        node.count = if node.children.is_empty() {
            node.count
        } else {
            node.children.iter().map(|c| c.count).sum()
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hierarchie_simple() {
        let tags = vec!["a/b".to_string(), "a/c".to_string()];
        let tree = build_tag_tree(&tags);
        assert_eq!(tree.len(), 1);
        assert_eq!(tree[0].name, "a");
        assert_eq!(tree[0].count, 2);
        assert_eq!(tree[0].children.len(), 2);
        assert_eq!(tree[0].children[0].name, "b");
        assert_eq!(tree[0].children[1].name, "c");
    }

    #[test]
    fn imbrication_profonde() {
        let tags = vec!["a/b/c".to_string()];
        let tree = build_tag_tree(&tags);
        assert_eq!(tree[0].children[0].name, "b");
        assert_eq!(tree[0].children[0].children[0].name, "c");
        assert_eq!(tree[0].children[0].children[0].count, 1);
    }

    #[test]
    fn tri_alphabétique() {
        let tags = vec!["z".to_string(), "a".to_string()];
        let tree = build_tag_tree(&tags);
        assert_eq!(tree[0].name, "a");
        assert_eq!(tree[1].name, "z");
    }

    #[test]
    fn tag_sans_slash_racine() {
        let tag = "seul".to_string();
        let tree = build_tag_tree(&[tag]);
        assert_eq!(tree.len(), 1);
        assert_eq!(tree[0].name, "seul");
        assert_eq!(tree[0].count, 1);
    }

    #[test]
    fn liste_vide() {
        assert!(build_tag_tree(&[]).is_empty());
    }

    #[test]
    fn doublons_comptes_une_fois_part_chemin() {
        let tags = vec![
            "a/b".to_string(),
            "a/b".to_string(),
            "a".to_string(),
            "a/c".to_string(),
        ];
        let tree = build_tag_tree(&tags);
        assert_eq!(tree[0].count, 3); // a, a/b, a/c
    }

    #[test]
    fn segments_vides_ignores() {
        let tags = vec!["//x".to_string(), "a//b".to_string()];
        let tree = build_tag_tree(&tags);
        // "//x" → "x" ; "a//b" → "a" > "b".
        assert_eq!(tree.len(), 2);
        assert_eq!(tree[0].name, "a");
        assert_eq!(tree[0].children[0].name, "b");
        assert_eq!(tree[1].name, "x");
    }
}
