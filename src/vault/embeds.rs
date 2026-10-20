//! Module `embeds` — extraction des embeds Markdown `![[...]]` et
//! classification par type (note, image, pdf, autre).

use super::index::NoteIndex;
use super::note::Note;

/// Nature d'un embed, déduite de l'extension de la cible.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmbedKind {
    /// Note du vault (cible sans extension).
    Note,
    /// Image (png, jpg, jpeg, gif, svg, webp, bmp).
    Image,
    /// Document PDF.
    Pdf,
    /// Toute autre extension.
    Other,
}

/// Un embed `![[...]]` extrait d'un document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Embed {
    /// Cible de l'embed, avant toute section `#` et tout alias `|`.
    pub target: String,
    /// Alias après `|`, si présent (trimé).
    pub alias: Option<String>,
    /// Section après `#` : heading ou identifiant de bloc `^id`, si présente.
    pub section: Option<String>,
    /// Nature de la cible (déduite de l'extension).
    pub kind: EmbedKind,
}

/// Extrait tous les embeds `![[...]]` d'un document, dans l'ordre, en
/// ignorant les blocs de code délimités par ``` ou ~~~.
///
/// Un wikilink `[[...]]` sans `!` n'est pas un embed ; le contenu d'un embed
/// ne traverse pas les lignes.
pub fn extract_embeds(content: &str) -> Vec<Embed> {
    let bytes = content.as_bytes();
    let code_ranges = code_block_ranges(content);
    let mut embeds = Vec::new();
    let mut pos = 0;
    let mut ci = 0;

    while pos + 2 < bytes.len() {
        // Oublie les blocs de code déjà entièrement parcourus.
        while ci < code_ranges.len() && pos >= code_ranges[ci].1 {
            ci += 1;
        }
        // Position à l'intérieur d'un bloc de code : saute le bloc entier.
        if ci < code_ranges.len() && pos >= code_ranges[ci].0 {
            pos = code_ranges[ci].1;
            continue;
        }

        if bytes[pos] == b'!' && bytes[pos + 1] == b'[' && bytes[pos + 2] == b'[' {
            // Cherche la fermeture `]]` avant tout `\n`.
            let mut end = pos + 3;
            while end + 1 < bytes.len() && bytes[end] != b'\n' {
                if bytes[end] == b']' && bytes[end + 1] == b']' {
                    break;
                }
                end += 1;
            }

            if end + 1 < bytes.len() && bytes[end] == b']' && bytes[end + 1] == b']' {
                let raw = &content[pos + 3..end];
                embeds.push(parse_embed(raw));
                pos = end + 2;
            } else {
                // Aucune fermeture avant la fin de la ligne : fragment ignoré.
                pos += 3;
            }
            continue;
        }

        pos += 1;
    }

    embeds
}

/// Retourne les intervalles d'octets occupés par les blocs de code délimités
/// par ``` ou ~~~ (fermeture : même caractère, longueur ≥ celle de
/// l'ouverture, suivie uniquement d'espaces). Une clôture jamais refermée
/// s'étend jusqu'à la fin du document.
fn code_block_ranges(source: &str) -> Vec<(usize, usize)> {
    let bytes = source.as_bytes();
    let mut ranges = Vec::new();
    let mut in_fence = false;
    let mut fence_char = 0u8;
    let mut fence_len = 0usize;
    let mut fence_start = 0usize;
    let mut pos = 0usize;

    while pos < bytes.len() {
        let line_start = pos == 0 || bytes[pos - 1] == b'\n';
        if line_start {
            let mut i = pos;
            let mut indent = 0;
            while i < bytes.len() && indent < 3 && (bytes[i] == b' ' || bytes[i] == b'\t') {
                i += 1;
                indent += 1;
            }

            if !in_fence {
                if i + 2 < bytes.len() && (bytes[i] == b'`' || bytes[i] == b'~') {
                    let c = bytes[i];
                    let mut n = 0;
                    while i + n < bytes.len() && bytes[i + n] == c {
                        n += 1;
                    }
                    if n >= 3 {
                        in_fence = true;
                        fence_char = c;
                        fence_len = n;
                        fence_start = pos;
                        pos = i + n;
                        continue;
                    }
                }
            } else if i < bytes.len() && bytes[i] == fence_char {
                let mut n = 0;
                while i + n < bytes.len() && bytes[i + n] == fence_char {
                    n += 1;
                }
                if n >= fence_len {
                    // Seuls des espaces/tabulations peuvent suivre la clôture.
                    let mut j = i + n;
                    let mut valide = true;
                    while j < bytes.len() && bytes[j] != b'\n' {
                        if bytes[j] != b' ' && bytes[j] != b'\t' {
                            valide = false;
                            break;
                        }
                        j += 1;
                    }
                    if valide {
                        ranges.push((fence_start, j));
                        in_fence = false;
                        pos = j;
                        continue;
                    }
                }
            }
        }
        pos += 1;
    }

    if in_fence {
        ranges.push((fence_start, bytes.len()));
    }
    ranges
}

/// Parse le contenu brut d'un embed (entre `![[` et `]]`).
///
/// L'ordre canonique est `cible#section|alias` : le premier `|` introduit
/// l'alias, le premier `#` de la cible introduit la section.
fn parse_embed(raw: &str) -> Embed {
    let (target_part, alias) = match raw.split_once('|') {
        Some((target, alias)) => (target, Some(alias.trim())),
        None => (raw, None),
    };

    let alias = match alias {
        Some(alias) if !alias.is_empty() => Some(alias.to_string()),
        _ => None,
    };

    let (target, section) = match target_part.split_once('#') {
        Some((target, section)) => (target.trim(), Some(section.trim())),
        None => (target_part.trim(), None),
    };

    let section = match section {
        Some(section) if !section.is_empty() => Some(section.to_string()),
        _ => None,
    };

    let kind = embed_kind(target);

    Embed {
        target: target.to_string(),
        alias,
        section,
        kind,
    }
}

/// Détermine la nature d'une cible d'embed d'après son extension, comparée
/// insensible à la casse. Une cible sans extension est une note.
pub fn embed_kind(target: &str) -> EmbedKind {
    match extension_of(target) {
        Some(ext)
            if ext.eq_ignore_ascii_case("png")
                || ext.eq_ignore_ascii_case("jpg")
                || ext.eq_ignore_ascii_case("jpeg")
                || ext.eq_ignore_ascii_case("gif")
                || ext.eq_ignore_ascii_case("svg")
                || ext.eq_ignore_ascii_case("webp")
                || ext.eq_ignore_ascii_case("bmp") =>
        {
            EmbedKind::Image
        }
        Some(ext) if ext.eq_ignore_ascii_case("pdf") => EmbedKind::Pdf,
        Some(_) => EmbedKind::Other,
        None => EmbedKind::Note,
    }
}

/// Extension de la cible : après le dernier `/` et le dernier `.` du nom de
/// fichier. `None` si aucune extension (ou extension vide).
fn extension_of(target: &str) -> Option<&str> {
    let base = match target.rfind('/') {
        Some(idx) => &target[idx + 1..],
        None => target,
    };
    let dot = base.rfind('.')?;
    let ext = &base[dot + 1..];
    if ext.is_empty() {
        None
    } else {
        Some(ext)
    }
}

/// Résout un embed vers les notes du vault.
///
/// Seuls les embeds de type [`EmbedKind::Note`] **sans section** sont résolus :
/// la résolution est délèguée à [`NoteIndex::resolve`] sur la cible. Tout autre
/// embed (image, pdf, autre, ou avec une section) retourne une liste vide.
pub fn resolve_embed<'a>(index: &'a NoteIndex, embed: &Embed) -> Vec<&'a Note> {
    if embed.kind == EmbedKind::Note && embed.section.is_none() {
        index.resolve(&embed.target)
    } else {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vault::note::{Note, NoteMetadata};
    use std::path::PathBuf;

    fn note(nom: &str) -> Note {
        Note {
            path: PathBuf::from(format!("{nom}.md")),
            name: nom.to_string(),
            metadata: NoteMetadata {
                frontmatter: None,
                frontmatter_error: None,
                wikilinks: Vec::new(),
                tags: Vec::new(),
            },
        }
    }

    fn embed(target: &str, alias: Option<&str>, section: Option<&str>) -> Embed {
        Embed {
            target: target.to_string(),
            alias: alias.map(str::to_string),
            section: section.map(str::to_string),
            kind: embed_kind(target),
        }
    }

    #[test]
    fn extrait_embeds_simples() {
        assert_eq!(
            extract_embeds("![[NoteA]]"),
            vec![embed("NoteA", None, None)]
        );
        assert_eq!(
            extract_embeds("texte ![[NoteB]] fin"),
            vec![embed("NoteB", None, None)]
        );
    }

    #[test]
    fn extrait_embeds_avec_alias_et_section() {
        assert_eq!(
            extract_embeds("![[Note#Intro|Alias]]"),
            vec![embed("Note", Some("Alias"), Some("Intro"))]
        );
        assert_eq!(
            extract_embeds("![[Note|Alias]]"),
            vec![embed("Note", Some("Alias"), None)]
        );
        assert_eq!(
            extract_embeds("![[Note#^bloc]]"),
            vec![embed("Note", None, Some("^bloc"))]
        );
        // Le trim s'applique à la cible, à la section et à l'alias.
        assert_eq!(
            extract_embeds("![[ Note | Alias ]]"),
            vec![embed("Note", Some("Alias"), None)]
        );
    }

    #[test]
    fn extrait_embeds_dans_l_ordre() {
        assert_eq!(
            extract_embeds("![[A]] puis ![[B]]"),
            vec![embed("A", None, None), embed("B", None, None)]
        );
        assert_eq!(
            extract_embeds("![[A]]\n![[B]]"),
            vec![embed("A", None, None), embed("B", None, None)]
        );
    }

    #[test]
    fn wikilink_sans_bang_non_embed() {
        assert_eq!(extract_embeds("[[X]]"), Vec::new());
        assert_eq!(
            extract_embeds("voir [[X]] et ![[Y]]"),
            vec![embed("Y", None, None)]
        );
        assert_eq!(extract_embeds("[[X]] ![[Y]]"), vec![embed("Y", None, None)]);
    }

    #[test]
    fn fragment_non_ferme_ignore() {
        assert_eq!(extract_embeds("![[A\nB]]"), Vec::new());
        assert_eq!(extract_embeds("![[A"), Vec::new());
        assert_eq!(extract_embeds("![[A]] ![[B"), vec![embed("A", None, None)]);
    }

    #[test]
    fn ignore_blocs_de_code() {
        // Backticks.
        assert_eq!(extract_embeds("```rust\n![[X]]\n```"), Vec::new());
        // Tildes.
        assert_eq!(extract_embeds("~~~\n![[X]]\n~~~"), Vec::new());
        // L'embed hors bloc est conservé, pas celui du bloc.
        assert_eq!(
            extract_embeds("```\n![[X]]\n```\n![[Y]]"),
            vec![embed("Y", None, None)]
        );
        // Un bloc non refermé court jusqu'à la fin du document.
        assert_eq!(extract_embeds("```\n![[X]]"), Vec::new());
    }

    #[test]
    fn classifie_les_kinds() {
        // Images.
        for ext in ["png", "jpg", "jpeg", "gif", "svg", "webp", "bmp"] {
            assert_eq!(embed_kind(&format!("photo.{ext}")), EmbedKind::Image);
        }
        // Insensible à la casse.
        assert_eq!(embed_kind("photo.PNG"), EmbedKind::Image);
        // PDF.
        assert_eq!(embed_kind("doc.pdf"), EmbedKind::Pdf);
        // Sans extension → note.
        assert_eq!(embed_kind("Note"), EmbedKind::Note);
        assert_eq!(embed_kind("dossier/Note"), EmbedKind::Note);
        // Extension inconnue → autre.
        assert_eq!(embed_kind("fichier.xyz"), EmbedKind::Other);
        // Seule la dernière extension du nom de fichier compte.
        assert_eq!(embed_kind("archive.tar.gz"), EmbedKind::Other);
    }

    #[test]
    fn resout_embed_note_sans_section() {
        let notes = vec![note("Projet"), note("Quotidien")];
        let index = NoteIndex::build_index(&notes);

        let emb = embed("Projet", None, None);
        let res = resolve_embed(&index, &emb);
        assert_eq!(res.len(), 1);
        assert_eq!(res[0].name, "Projet");
    }

    #[test]
    fn l_alias_n_affecte_pas_la_resolution() {
        let notes = vec![note("Projet")];
        let index = NoteIndex::build_index(&notes);

        let emb = embed("Projet", Some("Alias"), None);
        let res = resolve_embed(&index, &emb);
        assert_eq!(res.len(), 1);
        assert_eq!(res[0].name, "Projet");
    }

    #[test]
    fn resout_tous_les_homonymes() {
        let mut notes = vec![note("Commun")];
        notes[0].path = PathBuf::from("a/Commun.md");
        notes.push(note("Commun"));
        notes[1].path = PathBuf::from("b/Commun.md");
        let index = NoteIndex::build_index(&notes);

        let emb = embed("Commun", None, None);
        let res = resolve_embed(&index, &emb);
        assert_eq!(res.len(), 2);
    }

    #[test]
    fn ne_resout_pas_images_ni_sections() {
        let notes = vec![note("Projet")];
        let index = NoteIndex::build_index(&notes);

        let img = embed("photo.png", None, None);
        assert!(resolve_embed(&index, &img).is_empty());

        let pdf = embed("doc.pdf", None, None);
        assert!(resolve_embed(&index, &pdf).is_empty());

        let avec_section = embed("Projet", None, Some("Intro"));
        assert!(resolve_embed(&index, &avec_section).is_empty());
    }

    #[test]
    fn nom_inconnu_retourne_vide() {
        let notes = vec![note("Projet")];
        let index = NoteIndex::build_index(&notes);

        let inconnu = embed("Inexistant", None, None);
        assert!(resolve_embed(&index, &inconnu).is_empty());
    }
}
