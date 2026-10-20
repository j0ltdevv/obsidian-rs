//! Références de blocs : ancres `^id` et résolution d'un bloc vers son
//! extrait de texte (utilisé par les embeds de bloc `![[note#^id]]`).

/// Une ancre de bloc `^id`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockRef {
    /// Identifiant (sans le préfixe `^`).
    pub id: String,
    /// Ligne (index 0-based) où se trouve l'ancre.
    pub line: usize,
}

/// Extrait d'un bloc ciblé par son ancre.
#[derive(Debug, Clone, PartialEq)]
pub struct BlockSnippet {
    /// Identifiant du bloc.
    pub id: String,
    /// Texte du bloc (marqueur `^id` retiré).
    pub text: String,
    /// Première ligne (0-based).
    pub start_line: usize,
    /// Dernière ligne incluse (0-based).
    pub end_line: usize,
}

/// Détecte le marqueur `^id` en fin d'une ligne. Retourne l'id ou `None`.
fn trailing_block_id(line: &str) -> Option<String> {
    let trimmed = line.trim_end();
    let idx = trimmed.rfind("^")?;
    let id = &trimmed[idx + 1..];
    if id.is_empty()
        || !id
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
    {
        return None;
    }
    Some(id.to_string())
}

/// Extrait toutes les ancres `^id` d'un contenu, en ignorant les blocs de
/// code (fences ``` et ~~~).
pub fn extract_block_refs(content: &str) -> Vec<BlockRef> {
    let mut out = Vec::new();
    let mut in_fence: Option<u8> = None;
    for (line_no, raw) in content.lines().enumerate() {
        if let Some(fence) = in_fence {
            if is_fence(raw, fence) {
                in_fence = None;
            }
            continue;
        }
        if let Some(fence) = fence_char(raw) {
            in_fence = Some(fence);
            continue;
        }
        if let Some(id) = trailing_block_id(raw) {
            out.push(BlockRef { id, line: line_no });
        }
    }
    out
}

/// `Some(c)` si la ligne ouvre une fence de code avec le caractère `c`.
fn fence_char(line: &str) -> Option<u8> {
    let trimmed = line.trim_start();
    if trimmed.starts_with("```") {
        return Some(b'`');
    }
    if trimmed.starts_with("~~~") {
        return Some(b'~');
    }
    None
}

/// `true` si `line` ferme une fence ouverte par `fence`.
fn is_fence(line: &str, fence: u8) -> bool {
    let trimmed = line.trim_start();
    let marker = if fence == b'`' { "```" } else { "~~~" };
    trimmed.starts_with(marker)
}

/// Résout un bloc par son id : ligne ancre plus lignes suivantes du même bloc
/// (jusqu'à la ligne vide). L'id inconnu → `None`.
pub fn resolve_block(content: &str, id: &str) -> Option<BlockSnippet> {
    let lines: Vec<&str> = content.lines().collect();
    let anchor = extract_block_refs(content)
        .into_iter()
        .find(|r| r.id == id)?;

    let anchor_idx = anchor.line;
    // Ligne de l'ancre sans le marqueur.
    let anchor_text = lines
        .get(anchor_idx)?
        .trim_end()
        .strip_suffix(&format!("^{id}"))
        .map(str::trim_end)
        .unwrap_or(lines.get(anchor_idx)?)
        .to_string();

    let mut end = anchor_idx;
    for (i, line) in lines.iter().enumerate().skip(anchor_idx + 1) {
        if line.trim().is_empty() {
            break;
        }
        end = i;
    }
    let text = [anchor_text]
        .into_iter()
        .chain(
            lines[anchor_idx + 1..=end]
                .iter()
                .copied()
                .map(str::to_string),
        )
        .collect::<Vec<_>>()
        .join("\n");

    Some(BlockSnippet {
        id: id.to_string(),
        text,
        start_line: anchor_idx,
        end_line: end,
    })
}

/// Parse une cible d'embed de bloc `note#^id` → `(note, id)`.
pub fn parse_block_embed(target: &str) -> Option<(String, String)> {
    let idx = target.find("#^")?;
    let note = target[..idx].to_string();
    let id = target[idx + 2..].to_string();
    if id.is_empty() {
        return None;
    }
    Some((note, id))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ancres_simples_detectees() {
        let content = "ligne 1\nbloc ^abc\nligne 3\n";
        let refs = extract_block_refs(content);
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].id, "abc");
        assert_eq!(refs[0].line, 1);
    }

    #[test]
    fn ancres_avec_tirets_et_underscores() {
        assert_eq!(
            trailing_block_id("x ^mon-id_1"),
            Some("mon-id_1".to_string())
        );
    }

    #[test]
    fn ancres_ignorees_dans_code_fence() {
        let content = "avant\n```\n^cache\n```\naprès ^ok\n";
        let refs = extract_block_refs(content);
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].id, "ok");
    }

    #[test]
    fn ancres_ignorees_dans_fence_tildes() {
        let content = "~~~\n^a\n~~~\n^b\n";
        let refs = extract_block_refs(content);
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].id, "b");
    }

    #[test]
    fn resolve_block_retourne_le_bloc() {
        let content = "début\nélément ^bloc\nélément deux\n\nfin\n";
        let snippet = resolve_block(content, "bloc").unwrap();
        assert_eq!(snippet.text, "élément\nélément deux");
        assert_eq!(snippet.start_line, 1);
        assert_eq!(snippet.end_line, 2);
    }

    #[test]
    fn resolve_block_id_inconnu_retourne_none() {
        assert!(resolve_block("a ^x\n", "inconnu").is_none());
    }

    #[test]
    fn resolve_block_s_arrete_a_ligne_vide() {
        let content = "a ^b\nc\n\nd\n";
        let snippet = resolve_block(content, "b").unwrap();
        assert_eq!(snippet.text, "a\nc");
        assert_eq!(snippet.end_line, 1);
    }

    #[test]
    fn parse_block_embed_note_et_id() {
        assert_eq!(
            parse_block_embed("note#^id"),
            Some(("note".to_string(), "id".to_string()))
        );
    }

    #[test]
    fn parse_block_embed_sans_marqueur_retourne_none() {
        assert!(parse_block_embed("note#heading").is_none());
        assert!(parse_block_embed("note^id").is_none());
    }
}
