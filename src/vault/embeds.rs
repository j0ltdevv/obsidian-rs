//! Module `embeds` — extraction des embeds Markdown `![[...]]` et
//! classification par type (note, image, pdf, autre).

use super::index::NoteIndex;
use crate::vault::Note;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use thiserror::Error;

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

// ---------------------------------------------------------------------------
// Résolution d'une cible d'embed vers un fichier concret du vault
// ---------------------------------------------------------------------------

/// Extensions reconnues comme des images (minuscules).
const IMAGE_EXTS: &[&str] = &["png", "jpg", "jpeg", "gif", "svg", "webp", "bmp", "ico"];

/// Extensions reconnues comme des pistes audio (minuscules).
const AUDIO_EXTS: &[&str] = &["mp3", "wav", "flac", "ogg"];

/// Extensions reconnues comme des vidéos (minuscules).
const VIDEO_EXTS: &[&str] = &["mp4", "webm"];

/// Type d'un embed résolu vers un fichier du vault.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmbedType {
    /// Image (png, jpg, jpeg, gif, svg, webp, bmp, ico).
    Image,
    /// Document PDF.
    Document,
    /// Piste audio (mp3, wav, flac, ogg).
    Audio,
    /// Vidéo (mp4, webm).
    Video,
    /// Note Markdown (`cible.md`).
    Note,
    /// Cible absente du vault.
    NotFound,
}

/// Un embed résolu vers un fichier concret du vault.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedEmbed {
    /// Type de l'embed.
    pub kind: EmbedType,
    /// Chemin du fichier, relatif au root du vault.
    pub path: PathBuf,
}

/// Résultat de la résolution d'une cible d'embed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolveOutcome {
    /// Fichier trouvé dans le vault.
    Found(ResolvedEmbed),
    /// Cible absente du vault.
    NotFound,
}

/// Erreurs de résolution ou d'inspection d'un embed.
#[derive(Debug, Error)]
pub enum EmbedError {
    /// Le fichier ne peut pas être lu.
    #[error("fichier illisible : {0}")]
    Io(#[from] io::Error),
    /// Format de fichier non pris en charge.
    #[error("format non pris en charge : {0}")]
    UnsupportedFormat(String),
    /// Fichier corrompu ou structure invalide.
    #[error("fichier corrompu : {0}")]
    Corrupt(String),
}

/// Dimensions d'une image (en pixels).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImageDimensions {
    /// Largeur en pixels.
    pub width: u32,
    /// Hauteur en pixels.
    pub height: u32,
}

/// Extrait la référence de fichier d'une cible d'embed brute (le texte entre
/// `[[` et `]]`). Retire l'alias après `|`, la section après `#`, puis élague
/// les espaces.
fn file_target(raw: &str) -> &str {
    let no_alias = raw.split_once('|').map(|(t, _)| t).unwrap_or(raw);
    let no_section = no_alias.split_once('#').map(|(t, _)| t).unwrap_or(no_alias);
    no_section.trim()
}

/// Type d'embed associé à une extension connue (comparée insensible à la
/// casse). `None` si l'extension n'est reconnue ni comme image, ni comme PDF,
/// ni comme audio, ni comme vidéo.
fn kind_for_ext(ext: &str) -> Option<EmbedType> {
    if IMAGE_EXTS.iter().any(|e| ext.eq_ignore_ascii_case(e)) {
        Some(EmbedType::Image)
    } else if ext.eq_ignore_ascii_case("pdf") {
        Some(EmbedType::Document)
    } else if AUDIO_EXTS.iter().any(|e| ext.eq_ignore_ascii_case(e)) {
        Some(EmbedType::Audio)
    } else if VIDEO_EXTS.iter().any(|e| ext.eq_ignore_ascii_case(e)) {
        Some(EmbedType::Video)
    } else {
        None
    }
}

/// `true` si la cible forme un chemin relatif sûr (aucun composant `..`,
/// aucun chemin absolu).
fn is_safe_relative(target: &str) -> bool {
    if target.starts_with('/') || target.starts_with('\\') {
        return false;
    }
    !target.split(['/', '\\']).any(|c| c == "..")
}

/// Résout une cible d'embed (le texte entre `[[` et `]]`, alias et section
/// compris) vers le fichier correspondant du vault.
///
/// Ordre de recherche :
/// 1. si la cible porte déjà une extension (ex. `img.png`, `note.md`), le
///    fichier `root/cible` est cherché tel quel ;
/// 2. sinon, `root/cible.md` est cherché d'abord, puis `root/cible.<extension>`
///    pour chaque extension connue (image, pdf, audio, vidéo).
///
/// La présence d'un fichier mais un format non lisible n'est pas une erreur
/// ici : la résolution réussit, et c'est [`image_dimensions`] qui signale
/// l'échec de lecture d'une image.
pub fn resolve_embed_target(root: &Path, raw: &str) -> ResolveOutcome {
    let target = file_target(raw);
    if target.is_empty() || !is_safe_relative(target) {
        return ResolveOutcome::NotFound;
    }

    // Cible avec extension déjà écrite.
    if let Some(ext) = extension_of(target) {
        let kind = if ext.eq_ignore_ascii_case("md") {
            Some(EmbedType::Note)
        } else {
            kind_for_ext(ext)
        };
        let kind = match kind {
            Some(k) => k,
            None => return ResolveOutcome::NotFound,
        };
        let path = root.join(target);
        if path.is_file() {
            return ResolveOutcome::Found(ResolvedEmbed { kind, path });
        }
        return ResolveOutcome::NotFound;
    }

    // Cible sans extension : `cible.md` d'abord, puis `cible.<extension>`.
    let md_path = root.join(format!("{target}.md"));
    if md_path.is_file() {
        return ResolveOutcome::Found(ResolvedEmbed {
            kind: EmbedType::Note,
            path: md_path,
        });
    }

    for (ext, kind) in IMAGE_EXTS
        .iter()
        .map(|e| (*e, EmbedType::Image))
        .chain(std::iter::once(("pdf", EmbedType::Document)))
        .chain(AUDIO_EXTS.iter().map(|e| (*e, EmbedType::Audio)))
        .chain(VIDEO_EXTS.iter().map(|e| (*e, EmbedType::Video)))
    {
        let path = root.join(format!("{target}.{ext}"));
        if path.is_file() {
            return ResolveOutcome::Found(ResolvedEmbed { kind, path });
        }
    }

    ResolveOutcome::NotFound
}

// ---------------------------------------------------------------------------
// Lecture des dimensions d'une image (PNG / JPEG) sans dépendance externe
// ---------------------------------------------------------------------------

/// Lit les dimensions (largeur, hauteur) d'une image depuis son en-tête.
///
/// Formats pris en charge : PNG (signature + bloc `IHDR`) et JPEG (segments
/// `SOF0`/`SOF2`). Un format inconnu ou un fichier corrompu retourne une
/// erreur explicite, jamais une panique.
pub fn image_dimensions(path: &Path) -> Result<ImageDimensions, EmbedError> {
    let data = fs::read(path)?;
    if data.len() >= 8 && data[..8] == [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A] {
        png_dimensions(&data)
    } else if data.len() >= 3 && data[0] == 0xFF && data[1] == 0xD8 {
        jpeg_dimensions(&data)
    } else {
        Err(EmbedError::UnsupportedFormat(
            "ni PNG ni JPEG (signature inconnue)".to_string(),
        ))
    }
}

/// Dimensions d'un PNG : signature 8 octets, puis longueur de bloc (4), type
/// `IHDR` (4), largeur (4) et hauteur (4), en big-endian.
fn png_dimensions(data: &[u8]) -> Result<ImageDimensions, EmbedError> {
    if data.len() < 24 {
        return Err(EmbedError::Corrupt("fichier PNG trop court".to_string()));
    }
    if &data[12..16] != b"IHDR" {
        return Err(EmbedError::Corrupt("bloc IHDR absent".to_string()));
    }
    let width = u32::from_be_bytes([data[16], data[17], data[18], data[19]]);
    let height = u32::from_be_bytes([data[20], data[21], data[22], data[23]]);
    Ok(ImageDimensions { width, height })
}

/// Marqueurs JPEG de début de trame (SOF) qui portent les dimensions.
const SOF_MARKERS: &[u8] = &[
    0xC0, 0xC1, 0xC2, 0xC3, 0xC5, 0xC6, 0xC7, 0xC9, 0xCA, 0xCB, 0xCD, 0xCE, 0xCF,
];

/// Dimensions d'un JPEG : on parcourt les segments après `FF D8` jusqu'à un
/// segment `SOF` (précision 1 octet, puis hauteur 2 octets et largeur 2
/// octets, en big-endian).
fn jpeg_dimensions(data: &[u8]) -> Result<ImageDimensions, EmbedError> {
    let mut i = 2usize; // après le marqueur SOI `FF D8`
    while i + 3 < data.len() {
        if data[i] != 0xFF {
            return Err(EmbedError::Corrupt("marqueur JPEG attendu".to_string()));
        }
        let marker = data[i + 1];
        // Marqueurs sans segment de données : RST0-7, SOI, EOI.
        if marker == 0xD8 || marker == 0xD9 || (0xD0..=0xD7).contains(&marker) {
            i += 2;
            continue;
        }
        let seg_len = u16::from_be_bytes([data[i + 2], data[i + 3]]) as usize;
        if seg_len < 2 || i + 2 + seg_len > data.len() {
            return Err(EmbedError::Corrupt("segment JPEG invalide".to_string()));
        }
        if SOF_MARKERS.contains(&marker) {
            // Payload : longueur (2) + précision (1) + hauteur (2) + largeur (2).
            let payload = i + 4;
            if payload + 4 > data.len() {
                return Err(EmbedError::Corrupt("segment SOF tronqué".to_string()));
            }
            let height = u16::from_be_bytes([data[payload + 1], data[payload + 2]]) as u32;
            let width = u16::from_be_bytes([data[payload + 3], data[payload + 4]]) as u32;
            return Ok(ImageDimensions { width, height });
        }
        i += 2 + seg_len;
    }
    Err(EmbedError::Corrupt("aucun segment SOF trouvé".to_string()))
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
