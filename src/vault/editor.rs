//! Parsing Markdown léger en blocs, pour l'aperçu de l'éditeur.
//!
//! Le parser est volontairement tolérant et sans allocation superflue : il
//! découpe le document ligne par ligne et regroupe les lignes consécutives en
//! blocs. Les blocs de code (fences ou indentation) conservent leur contenu
//! brut ; les autres blocs exposent un texte normalisé (marqueurs retirés,
//! lignes de contenu fusionnées par `\n`).
//!
//! Les tableaux ne sont reconnus que lorsque la première ligne de contenu
//! commence par `|` et qu'une ligne séparatrice (`|---|`, `| :---: |`, …) la
//! suit avec le même nombre de cellules. Une ligne `---` seule reste donc un
//! paragraphe.

/// Nature d'un bloc Markdown reconnu par [`parse_markdown`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlockKind {
    /// Titre ATX, niveau 1 à 6.
    Heading(u8),
    /// Paragraphe : lignes consécutives fusionnées.
    Paragraph,
    /// Bloc de code (fence ou indentation), contenu brut.
    CodeBlock,
    /// Liste à puces (`- `, `* `) ou ordonnée (`1. `).
    List,
    /// Citation `>`.
    Quote,
    /// Callout `> [!TYPE]`, avec le type tel qu'écrit (casse conservée).
    Callout { kind: String },
    /// Tableau avec ligne séparatrice.
    Table,
}

/// Un bloc Markdown : sa nature et son texte.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    /// Nature du bloc.
    pub kind: BlockKind,
    /// Contenu du bloc, sans les marqueurs Markdown de tête.
    pub text: String,
}

/// Découpe `src` en blocs Markdown.
///
/// Les lignes vides séparent les blocs et ne produisent aucune entrée ; un
/// document vide retourne un vecteur vide.
pub fn parse_markdown(src: &str) -> Vec<Block> {
    let lines: Vec<&str> = src.lines().collect();
    let mut blocks = Vec::new();
    let mut index = 0;

    while index < lines.len() {
        let line = lines[index];

        if line.trim().is_empty() {
            index += 1;
            continue;
        }

        // Les fences sont reconnues avant les titres et les listes : une ligne
        // de fence n'est jamais du texte courant.
        if let Some(fence) = fence_open(line) {
            let mut content = Vec::new();
            index += 1;
            while index < lines.len() && !fence_close(lines[index], &fence) {
                content.push(lines[index]);
                index += 1;
            }
            // Fence non fermée : le bloc court jusqu'à la fin du document.
            if index < lines.len() {
                index += 1;
            }
            blocks.push(Block {
                kind: BlockKind::CodeBlock,
                text: content.join("\n"),
            });
            continue;
        }

        if is_code_indented(line) {
            let mut content: Vec<&str> = Vec::new();
            while index < lines.len() {
                let current = lines[index];
                if current.trim().is_empty() {
                    content.push("");
                    index += 1;
                } else if let Some(stripped) = strip_code_indent(current) {
                    content.push(stripped);
                    index += 1;
                } else {
                    break;
                }
            }
            // Les lignes vides finales séparent le bloc du suivant : elles ne
            // font pas partie du contenu.
            while content.last().is_some_and(|line| line.is_empty()) {
                content.pop();
            }
            blocks.push(Block {
                kind: BlockKind::CodeBlock,
                text: content.join("\n"),
            });
            continue;
        }

        if let Some((level, text)) = heading(line) {
            blocks.push(Block {
                kind: BlockKind::Heading(level),
                text,
            });
            index += 1;
            continue;
        }

        if let Some(first) = dequote(line) {
            let mut quoted: Vec<&str> = vec![first];
            index += 1;
            while index < lines.len() {
                match dequote(lines[index]) {
                    Some(body) => {
                        quoted.push(body);
                        index += 1;
                    }
                    None => break,
                }
            }

            if let Some((kind, title)) = callout_marker(quoted[0]) {
                let mut parts = Vec::new();
                if !title.is_empty() {
                    parts.push(title);
                }
                for body in &quoted[1..] {
                    parts.push(body.trim_end().to_string());
                }
                blocks.push(Block {
                    kind: BlockKind::Callout { kind },
                    text: parts.join("\n"),
                });
            } else {
                let text: Vec<&str> = quoted.iter().map(|line| line.trim_end()).collect();
                blocks.push(Block {
                    kind: BlockKind::Quote,
                    text: text.join("\n"),
                });
            }
            continue;
        }

        if list_item(line).is_some() {
            let mut items = Vec::new();
            while index < lines.len() {
                match list_item(lines[index]) {
                    Some(item) => {
                        items.push(item.trim());
                        index += 1;
                    }
                    None => break,
                }
            }
            blocks.push(Block {
                kind: BlockKind::List,
                text: items.join("\n"),
            });
            continue;
        }

        if table_layout(&lines, index).is_some() {
            let mut rows = vec![lines[index].trim().to_string()];
            index += 2; // ligne d'en-tête puis ligne séparatrice
            while index < lines.len() {
                let Some(body) = unindent_upto3(lines[index]) else {
                    break;
                };
                if !body.trim_start().starts_with('|') {
                    break;
                }
                rows.push(body.trim_end().to_string());
                index += 1;
            }
            blocks.push(Block {
                kind: BlockKind::Table,
                text: rows.join("\n"),
            });
            continue;
        }

        // Paragraphe : fusionne les lignes consécutives jusqu'à une ligne vide
        // ou le début d'un autre bloc.
        let mut parts: Vec<&str> = Vec::new();
        while index < lines.len() {
            if !parts.is_empty() && starts_block(&lines, index) {
                break;
            }
            parts.push(lines[index].trim());
            index += 1;
        }
        blocks.push(Block {
            kind: BlockKind::Paragraph,
            text: parts.join("\n"),
        });
    }

    blocks
}

/// Indentation en espaces en tête de ligne.
fn leading_spaces(line: &str) -> usize {
    line.bytes().take_while(|byte| *byte == b' ').count()
}

/// Retire jusqu'à trois espaces d'indentation.
///
/// Retourne `None` si la ligne est indentée de quatre espaces ou plus : elle
/// relève alors du code indenté, pas d'un marqueur de bloc.
fn unindent_upto3(line: &str) -> Option<&str> {
    let spaces = leading_spaces(line);
    if spaces >= 4 {
        None
    } else {
        Some(&line[spaces..])
    }
}

/// Une fence ouvrante : caractère et longueur (```` ``` ```` ou `~~~`).
#[derive(Debug, Clone, Copy)]
pub(crate) struct Fence {
    char: char,
    len: usize,
}

/// Reconnaît une fence ouvrante (au moins trois backticks ou tildes).
pub(crate) fn fence_open(line: &str) -> Option<Fence> {
    let body = unindent_upto3(line)?;
    let char = body.chars().next()?;
    if char != '`' && char != '~' {
        return None;
    }
    let len = body.chars().take_while(|c| *c == char).count();
    if len < 3 {
        None
    } else {
        Some(Fence { char, len })
    }
}

/// Vrai si `line` ferme la fence `open` (même caractère, longueur suffisante,
/// rien d'autre que des blancs ensuite).
pub(crate) fn fence_close(line: &str, open: &Fence) -> bool {
    let Some(body) = unindent_upto3(line) else {
        return false;
    };
    let trimmed = body.trim_end();
    let len = trimmed.chars().take_while(|c| *c == open.char).count();
    len >= open.len && trimmed.chars().all(|c| c == open.char)
}

/// Vrai si la ligne est du code indenté (quatre espaces ou une tabulation).
fn is_code_indented(line: &str) -> bool {
    line.starts_with('\t') || leading_spaces(line) >= 4
}

/// Retire l'indentation d'une ligne de code (quatre espaces ou une tabulation).
fn strip_code_indent(line: &str) -> Option<&str> {
    if let Some(rest) = line.strip_prefix("    ") {
        Some(rest)
    } else {
        line.strip_prefix('\t')
    }
}

/// Préfixe de titre ATX reconnu en tête de ligne.
///
/// Les offsets sont exprimés en octets relativement à la ligne fournie.
/// La règle est celle de [`heading`] : au plus trois espaces d'indentation,
/// un à six `#` suivis d'un blanc (ou de la fin de la ligne).
pub(crate) struct HeadingPrefix {
    /// Niveau du titre (1 à 6).
    pub level: u8,
    /// Offset de la fin du marqueur (juste après le dernier `#`).
    pub marker_end: usize,
    /// Offset du début du texte du titre.
    pub text_start: usize,
}

/// Reconnaît un titre ATX en tête de ligne et retourne son préfixe.
///
/// Une ligne indentée de quatre espaces ou plus est du code indenté, pas un
/// titre.
pub(crate) fn heading_prefix(line: &str) -> Option<HeadingPrefix> {
    let bytes = line.as_bytes();
    // Au plus trois espaces d'indentation.
    let mut index = 0usize;
    while index < bytes.len() && bytes[index] == b' ' && index < 3 {
        index += 1;
    }
    if index < bytes.len() && bytes[index] == b' ' {
        // Quatre espaces ou plus : code indenté, pas un titre.
        return None;
    }
    let mut level = 0u8;
    while index < bytes.len() && bytes[index] == b'#' {
        level += 1;
        index += 1;
    }
    if level == 0 || level > 6 {
        return None;
    }
    let marker_end = index;
    // Les dièses ne forment un titre que s'ils sont suivis d'un blanc ou de
    // la fin de ligne (`#Titre` reste du texte).
    if index < bytes.len() && bytes[index] != b' ' && bytes[index] != b'\t' {
        return None;
    }
    while index < bytes.len() && (bytes[index] == b' ' || bytes[index] == b'\t') {
        index += 1;
    }
    Some(HeadingPrefix {
        level,
        marker_end,
        text_start: index,
    })
}

/// Reconnaît un titre ATX et retourne son niveau (1 à 6) et son texte.
fn heading(line: &str) -> Option<(u8, String)> {
    let prefix = heading_prefix(line)?;
    let text = strip_closing_hashes(line[prefix.text_start..].trim());
    Some((prefix.level, text.to_string()))
}

/// Retire une éventuelle séquence de dièses fermante (`# Titre ##`).
fn strip_closing_hashes(text: &str) -> &str {
    let trimmed = text.trim_end();
    let without = trimmed.trim_end_matches('#');
    if without.len() == trimmed.len() {
        return trimmed;
    }
    if without.is_empty() {
        ""
    } else if without.ends_with(' ') || without.ends_with('\t') {
        without.trim_end()
    } else {
        trimmed
    }
}

/// Retire un marqueur de citation `>` (et l'espace optionnel qui le suit).
fn dequote(line: &str) -> Option<&str> {
    let body = unindent_upto3(line)?;
    let rest = body.strip_prefix('>')?;
    Some(rest.strip_prefix(' ').unwrap_or(rest))
}

/// Reconnaît `[!TYPE]` et retourne le type et le titre éventuel.
fn callout_marker(quoted: &str) -> Option<(String, String)> {
    let inner = quoted.strip_prefix("[!")?;
    let end = inner.find(']')?;
    let kind = inner[..end].trim();
    if kind.is_empty() {
        return None;
    }
    let title = inner[end + 1..].trim();
    Some((kind.to_string(), title.to_string()))
}

/// Reconnaît un item de liste (`- `, `* `, `1. `) et retourne son contenu.
fn list_item(line: &str) -> Option<&str> {
    let body = unindent_upto3(line)?;
    if let Some(rest) = body.strip_prefix("- ") {
        return Some(rest);
    }
    if let Some(rest) = body.strip_prefix("* ") {
        return Some(rest);
    }
    let digits = body.bytes().take_while(u8::is_ascii_digit).count();
    if digits == 0 {
        return None;
    }
    body.get(digits..)?.strip_prefix(". ")
}

/// Vrai si `line` est une ligne séparatrice de tableau valide.
fn is_separator_cell(cell: &str) -> bool {
    let core = cell.strip_prefix(':').unwrap_or(cell);
    let core = core.strip_suffix(':').unwrap_or(core);
    !core.is_empty() && core.bytes().all(|byte| byte == b'-')
}

/// Retourne le nombre de cellules si `line` est une ligne séparatrice de tableau.
fn separator_cells(line: &str) -> Option<usize> {
    let body = unindent_upto3(line)?;
    let trimmed = body.trim();
    if !trimmed.contains('|') {
        return None;
    }
    let inner = trimmed.trim_start_matches('|').trim_end_matches('|');
    let cells: Vec<&str> = inner.split('|').map(str::trim).collect();
    if cells.iter().all(|cell| is_separator_cell(cell)) {
        Some(cells.len())
    } else {
        None
    }
}

/// Nombre de cellules d'une ligne de tableau (pipes de bord ignorés).
fn table_cell_count(line: &str) -> usize {
    let trimmed = line.trim();
    let inner = trimmed.trim_start_matches('|').trim_end_matches('|');
    inner.split('|').count()
}

/// Vrai si un tableau commence à `index` (en-tête `| … |` puis séparatrice de
/// même largeur).
fn table_layout(lines: &[&str], index: usize) -> Option<usize> {
    let header = unindent_upto3(lines[index])?;
    if !header.trim_start().starts_with('|') {
        return None;
    }
    let separator = separator_cells(lines.get(index + 1)?)?;
    if separator != table_cell_count(header) {
        return None;
    }
    Some(separator)
}

/// Vrai si la ligne `index` commence un bloc autonome (interrompt un paragraphe).
///
/// Le code indenté n'interrompt pas un paragraphe : il en est la continuation,
/// conformément au Markdown.
fn starts_block(lines: &[&str], index: usize) -> bool {
    let line = lines[index];
    if line.trim().is_empty() {
        return true;
    }
    fence_open(line).is_some()
        || heading(line).is_some()
        || dequote(line).is_some()
        || list_item(line).is_some()
        || table_layout(lines, index).is_some()
}

// ─────────────────────────────────────────────────────────────────────────────
// Coloration de la SOURCE Markdown
// ─────────────────────────────────────────────────────────────────────────────

use super::live_preview::SegmentStyle;

/// Nature d'un fragment de source Markdown.
///
/// Les fragments sont reconnus par les **mêmes** fonctions que l'aperçu
/// (`crate::vault::live_preview`) : un wikilink, un tag, un lien, une
/// emphase ou une fence sont classés à l'identique dans la source et dans
/// l'aperçu, qui ne peuvent donc pas diverger.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceKind {
    /// Texte ordinaire : ni marqueur ni construction reconnue.
    Plain,
    /// Marqueur de titre en début de ligne (`#`, `##`, …).
    HeadingMarker,
    /// Puce (`-`, `*`, `+`) ou numéro (`1.`) d'une liste.
    ListMarker,
    /// Case à cocher d'une tâche (`[ ]`, `[x]`).
    TaskCheckbox,
    /// Marqueur de citation (`>`).
    BlockquoteMarker,
    /// Délimiteur de frontmatter (`---`).
    FrontmatterDelimiter,
    /// Corps YAML d'un bloc de frontmatter : exclu de la coloration du corps.
    Frontmatter,
    /// Fence de code (`` ``` `` ou `~~~`) et lignes du bloc de code.
    CodeFence,
    /// Emphase (`*texte*`).
    Emphasis,
    /// Gras (`**texte**`).
    Strong,
    /// Texte barré (`~~texte~~`).
    Strikethrough,
    /// Code inline (`` `texte` ``).
    InlineCode,
    /// Lien markdown (`[texte](url)`).
    Link,
    /// Wikilink (`[[cible]]`).
    WikiLink,
    /// Embed (`![[cible]]`).
    Embed,
    /// Tag (`#tag`).
    Tag,
    /// Commentaire HTML (`<!-- … -->`).
    HtmlComment,
    /// Fin de ligne : le terminateur (`\n`, `\r\n`) n'appartient à aucun
    /// fragment de contenu.
    LineBreak,
}

/// Un fragment de source Markdown, coloré.
///
/// Les fragments sont ordonnés et contigus : la concaténation de leurs
/// [`SourceToken::text`] reproduit la source à l'identique, terminateurs de
/// ligne compris.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceToken {
    /// Nature du fragment.
    pub kind: SourceKind,
    /// Texte exact du fragment dans la source (marqueurs compris).
    pub text: String,
    /// Ligne 0-based du début du fragment.
    pub line: usize,
    /// Colonne 0-based du début du fragment, en octets depuis le début de la
    /// ligne (convention de [`cursor_line_col`]).
    pub column: usize,
}

/// Colore une source Markdown fragment par fragment.
///
/// # Contrat
///
/// - Les fragments sont ordonnés et contigus : la concaténation de leurs
///   [`SourceToken::text`] reproduit la source à l'identique.
/// - [`SourceToken::line`] est 0-based ; [`SourceToken::column`] est 0-based
///   et compté en octets depuis le début de la ligne.
/// - Le bloc de frontmatter est coloré par [`SourceKind::Frontmatter`] et
///   [`SourceKind::FrontmatterDelimiter`] : aucun marqueur du corps n'y est
///   reconnu (une liste YAML n'est pas une liste Markdown).
/// - Un bloc de code, fence comprise, est uniformément
///   [`SourceKind::CodeFence`] : aucun markup inline n'y est reconnu.
///
/// Les wikilinks, tags, liens, emphases et fences sont reconnus par les
/// fonctions de `live_preview` : la source et l'aperçu ne peuvent pas
/// diverger sur la nature d'une construction.
pub fn colorize_source(source: &str) -> Vec<SourceToken> {
    let mut tokens: Vec<SourceToken> = Vec::new();
    let region = frontmatter_region(source);
    let mut state = SourceState::default();

    for (line, (content, terminator)) in split_lines(source).into_iter().enumerate() {
        let context = LineContext::new(content, line);
        colorize_line(&context, region, &mut state, &mut tokens);
        if !terminator.is_empty() {
            push_token(
                &mut tokens,
                SourceKind::LineBreak,
                terminator,
                line,
                content.len(),
            );
        }
    }

    tokens
}

/// Région de frontmatter d'un document : lignes ouvrante et fermante.
///
/// Mêmes règles que [`crate::vault::frontmatter::parse_frontmatter`] : la
/// première ligne doit être `---` (après trim) et une ligne ultérieure dont
/// le trim vaut `---` ferme le bloc. Les lignes sont 0-based ; `None` si le
/// document n'a pas de frontmatter (ou un frontmatter jamais fermé).
pub fn frontmatter_region(source: &str) -> Option<(usize, usize)> {
    let mut lines = source.lines();
    let opening = lines.next()?;
    if opening.trim() != "---" {
        return None;
    }
    for (index, line) in lines.enumerate() {
        if line.trim() == "---" {
            return Some((0, index + 1));
        }
    }
    None
}

/// Niveau de titre d'une ligne source (1 à 6), ou `None`.
///
/// Même reconnaissance que la coloration : une ligne reconnue comme titre par
/// [`colorize_source`] a le même niveau ici.
pub fn heading_level(line: &str) -> Option<u8> {
    heading_prefix(line).map(|prefix| prefix.level)
}

/// Style de prévisualisation correspondant à un niveau de titre.
///
/// Miroir de [`SegmentStyle`] : le niveau 1 donne `H1`, …, le niveau 6 donne
/// `H6`. Un niveau hors bornes est ramené au niveau 1 ou 6.
pub fn heading_style(level: u8) -> SegmentStyle {
    match level.clamp(1, 6) {
        1 => SegmentStyle::H1,
        2 => SegmentStyle::H2,
        3 => SegmentStyle::H3,
        4 => SegmentStyle::H4,
        5 => SegmentStyle::H5,
        _ => SegmentStyle::H6,
    }
}

/// Position (ligne, colonne) d'un offset dans `content`.
///
/// Mêmes conventions que la recherche en note (`occurrence_line_col` de la
/// couche application) : ligne 0-based et colonne 0-based comptée en octets
/// depuis le début de la ligne.
///
/// Retourne `None` si l'offset est hors du document ou s'il tombe au milieu
/// d'un caractère UTF-8.
pub fn cursor_line_col(content: &str, byte_offset: usize) -> Option<(usize, usize)> {
    if byte_offset > content.len() || !content.is_char_boundary(byte_offset) {
        return None;
    }
    let prefix = &content[..byte_offset];
    let line = prefix.matches('\n').count();
    let column = prefix.len() - prefix.rfind('\n').map(|index| index + 1).unwrap_or(0);
    Some((line, column))
}

/// Position (ligne, colonne) de la `n`-ième occurrence de `query`.
///
/// Même convention que [`cursor_line_col`] : ligne 0-based, colonne 0-based
/// en octets. `None` si la requête est vide ou si l'occurrence n'existe pas.
pub fn occurrence_line_col(content: &str, query: &str, nth: usize) -> Option<(usize, usize)> {
    if query.is_empty() {
        return None;
    }
    let offset = content
        .match_indices(query)
        .nth(nth)
        .map(|(index, _)| index)?;
    cursor_line_col(content, offset)
}

/// Constantes typographiques calquées sur le thème par défaut d'Obsidian.
///
/// # Provenance des valeurs
///
/// Chaque constante indique la variable CSS d'Obsidian qu'elle reproduit :
/// les variables `--…` sont publiques et surchargeables par un thème ou un
/// snippet CSS. Les constantes sans variable publique reprennent la mise en
/// page du thème par défaut.
///
/// Ce module ne contient que des **données** : ni couleur, ni police
/// effective, ni rendu. La couche UI en tire ses dimensions.
pub mod typography {
    /// Interligne du texte : 1.6.
    ///
    /// Miroir de `--line-height` (1.6 dans le thème par défaut d'Obsidian).
    pub const LINE_HEIGHT: f32 = 1.6;

    /// Taille du texte du corps, en pixels : 16.
    ///
    /// Miroir de la taille de police du texte (`--font-text`), 16 px par
    /// défaut dans Obsidian.
    pub const BODY_FONT_SIZE_PX: f32 = 16.0;

    /// Taille du texte de l'interface, en pixels : 13.
    ///
    /// Miroir de `--font-interface` (13 px par défaut dans Obsidian).
    pub const INTERFACE_FONT_SIZE_PX: f32 = 13.0;

    /// Interligne des titres : 1.4.
    ///
    /// Miroir de `--h1-line-height` … `--h6-line-height` (1.4 dans Obsidian).
    pub const HEADING_LINE_HEIGHT: f32 = 1.4;

    /// Tailles relatives des titres, en `em`, pour les niveaux 1 à 6.
    ///
    /// Miroir de `--h1-size` … `--h6-size` d'Obsidian : 1.6, 1.4, 1.2, 1.1,
    /// 1 et 1.
    pub const HEADING_SIZES_EM: [f32; 6] = [1.6, 1.4, 1.2, 1.1, 1.0, 1.0];

    /// Espacement entre deux paragraphes, en `em` : 0.75 (12 px pour un corps
    /// à 16 px).
    ///
    /// Obsidian n'expose pas de variable publique pour cet espacement : la
    /// valeur reprend le thème par défaut.
    pub const PARAGRAPH_SPACING_EM: f32 = 0.75;

    /// Taille du texte des blocs de code, en `em` : 0.9.
    ///
    /// Miroir de `--code-size` d'Obsidian (0.9em).
    pub const CODE_SIZE_EM: f32 = 0.9;

    /// Rayon des arrondis du code, en pixels : 4.
    ///
    /// Miroir de `--code-radius` d'Obsidian (4px).
    pub const CODE_RADIUS_PX: f32 = 4.0;

    /// Marge intérieure du code inline, en `em` : 0.2.
    ///
    /// Obsidian n'expose pas de variable publique : la valeur reprend le
    /// thème par défaut.
    pub const INLINE_CODE_PADDING_EM: f32 = 0.2;

    /// Retrait des citations, en pixels : 20.
    ///
    /// Obsidian n'expose pas de variable publique : la valeur reprend le
    /// thème par défaut.
    pub const BLOCKQUOTE_INDENT_PX: f32 = 20.0;

    /// Épaisseur de la bordure de citation, en pixels : 2.
    ///
    /// Miroir de `--blockquote-border-thickness` d'Obsidian (2px).
    pub const BLOCKQUOTE_BORDER_PX: f32 = 2.0;

    /// Retrait d'un niveau de liste, en `em` : 2.
    ///
    /// Obsidian n'expose pas de variable publique : la valeur reprend le
    /// thème par défaut.
    pub const LIST_INDENT_EM: f32 = 2.0;

    /// Taille de la case à cocher d'une tâche, en `em` : 1.15.
    ///
    /// Obsidian n'expose pas de variable publique : la valeur reprend le
    /// thème par défaut.
    pub const TASK_CHECKBOX_SIZE_EM: f32 = 1.15;

    /// Rayon de la case à cocher d'une tâche, en pixels : 4.
    ///
    /// Obsidian n'expose pas de variable publique : la valeur reprend le
    /// thème par défaut.
    pub const TASK_CHECKBOX_RADIUS_PX: f32 = 4.0;

    /// Taille relative (en `em`) du titre de niveau `level`.
    ///
    /// Un niveau hors bornes (0, 7 ou plus) est ramené au niveau 1 ou 6.
    pub fn heading_size_em(level: u8) -> f32 {
        let index = usize::from(level.clamp(1, 6)) - 1;
        HEADING_SIZES_EM[index]
    }
}

/// État de coloration partagé entre les lignes d'un document.
#[derive(Default)]
struct SourceState {
    /// Fence de code ouverte : le contenu est du code jusqu'à sa fermeture.
    fence: Option<Fence>,
    /// Commentaire HTML ouvert : le contenu est un commentaire jusqu'à sa
    /// fermeture.
    in_comment: bool,
}

/// Contexte de coloration d'une ligne : texte, caractères et offsets.
struct LineContext<'a> {
    /// Texte de la ligne, sans son terminateur.
    text: &'a str,
    /// Caractères de la ligne.
    chars: Vec<char>,
    /// Offset d'octet de chaque caractère ; la dernière entrée est la
    /// longueur de la ligne.
    offsets: Vec<usize>,
    /// Numéro de la ligne (0-based).
    line: usize,
}

impl<'a> LineContext<'a> {
    /// Construit le contexte d'une ligne.
    fn new(text: &'a str, line: usize) -> Self {
        let mut chars: Vec<char> = Vec::with_capacity(text.len());
        let mut offsets: Vec<usize> = Vec::with_capacity(text.len() + 1);
        for (offset, ch) in text.char_indices() {
            chars.push(ch);
            offsets.push(offset);
        }
        offsets.push(text.len());
        LineContext {
            text,
            chars,
            offsets,
            line,
        }
    }

    /// Texte des caractères `from` (inclus) à `to` (exclu).
    fn slice(&self, from: usize, to: usize) -> &'a str {
        let text: &'a str = self.text;
        &text[self.offsets[from]..self.offsets[to]]
    }

    /// Offset d'octet du caractère `index`.
    fn offset(&self, index: usize) -> usize {
        self.offsets[index]
    }

    /// Index du caractère dont l'offset d'octet est `byte_offset`.
    fn char_index(&self, byte_offset: usize) -> usize {
        self.offsets
            .iter()
            .position(|&offset| offset >= byte_offset)
            .unwrap_or(self.chars.len())
    }
}

/// Découpe une source en lignes, en conservant le terminateur de chacune.
fn split_lines(source: &str) -> Vec<(&str, &str)> {
    let mut out: Vec<(&str, &str)> = Vec::new();
    let mut rest = source;
    loop {
        match rest.find('\n') {
            Some(position) => {
                let line = &rest[..position];
                match line.strip_suffix('\r') {
                    Some(stripped) => out.push((stripped, "\r\n")),
                    None => out.push((line, "\n")),
                }
                rest = &rest[position + 1..];
            }
            None => {
                if rest.is_empty() {
                    break;
                }
                match rest.strip_suffix('\r') {
                    Some(stripped) => out.push((stripped, "\r")),
                    None => out.push((rest, "")),
                }
                break;
            }
        }
    }
    out
}

/// Colore une ligne, selon son état (fence ouverte, commentaire ouvert).
fn colorize_line(
    context: &LineContext,
    region: Option<(usize, usize)>,
    state: &mut SourceState,
    out: &mut Vec<SourceToken>,
) {
    // Le frontmatter est coloré à part : aucun markup du corps n'y est
    // reconnu.
    let frontmatter = match region {
        Some((open, close)) if context.line == open || context.line == close => {
            Some(SourceKind::FrontmatterDelimiter)
        }
        Some((open, close)) if context.line > open && context.line < close => {
            Some(SourceKind::Frontmatter)
        }
        _ => None,
    };
    if let Some(kind) = frontmatter {
        push_token(out, kind, context.text, context.line, 0);
        return;
    }

    // Bloc de code ouvert : tout le contenu est du code, fence comprise.
    if let Some(open) = state.fence {
        push_token(out, SourceKind::CodeFence, context.text, context.line, 0);
        if fence_close(context.text, &open) {
            state.fence = None;
        }
        return;
    }

    // Commentaire HTML ouvert : la ligne appartient au commentaire.
    if state.in_comment {
        match context.text.find("-->") {
            Some(position) => {
                push_token(
                    out,
                    SourceKind::HtmlComment,
                    &context.text[..position + 3],
                    context.line,
                    0,
                );
                state.in_comment = false;
                colorize_body(
                    context,
                    context.char_index(position + 3),
                    context.chars.len(),
                    out,
                );
            }
            None => push_token(out, SourceKind::HtmlComment, context.text, context.line, 0),
        }
        return;
    }

    // Fence ouvrante : le bloc de code commence ici.
    if let Some(open) = fence_open(context.text) {
        push_token(out, SourceKind::CodeFence, context.text, context.line, 0);
        state.fence = Some(open);
        return;
    }

    // Commentaire HTML ouvrant : il peut s'ouvrir au milieu de la ligne.
    if let Some(position) = context.text.find("<!--") {
        colorize_body(context, 0, context.char_index(position), out);
        match context.text[position + 4..].find("-->") {
            Some(end) => {
                let stop = position + 4 + end + 3;
                push_token(
                    out,
                    SourceKind::HtmlComment,
                    &context.text[position..stop],
                    context.line,
                    position,
                );
                colorize_body(context, context.char_index(stop), context.chars.len(), out);
            }
            None => {
                push_token(
                    out,
                    SourceKind::HtmlComment,
                    &context.text[position..],
                    context.line,
                    position,
                );
                state.in_comment = true;
            }
        }
        return;
    }

    colorize_body(context, 0, context.chars.len(), out);
}

/// Colore le corps d'une ligne : titre, marqueurs de bloc, puis markup
/// inline.
fn colorize_body(context: &LineContext, from: usize, to: usize, out: &mut Vec<SourceToken>) {
    if from >= to {
        return;
    }

    // Titre : le marqueur est isolé, le texte est colorié comme le reste.
    let heading = if from == 0 {
        heading_prefix(context.text)
    } else {
        None
    };
    if let Some(prefix) = heading {
        let marker_end = context.char_index(prefix.marker_end).min(to);
        let text_start = context
            .char_index(prefix.text_start)
            .min(to)
            .max(marker_end);
        push_token(
            out,
            SourceKind::HeadingMarker,
            context.slice(0, marker_end),
            context.line,
            0,
        );
        if marker_end < text_start {
            push_token(
                out,
                SourceKind::Plain,
                context.slice(marker_end, text_start),
                context.line,
                context.offset(marker_end),
            );
        }
        colorize_body(context, text_start, to, out);
        return;
    }

    // Marqueurs de bloc (citation, liste, tâche) : uniquement en tête de
    // ligne.
    let markers = if from == 0 {
        block_markers(context, from, to)
    } else {
        Vec::new()
    };
    colorize_inline(context, from, to, &markers, out);
}

/// Repère les marqueurs de bloc en tête de ligne : citation, puce, numéro de
/// liste et case à cocher.
///
/// Les listes et les citations sont reconnues par les fonctions de
/// `live_preview` : la source et l'aperçu classent donc une liste de la même
/// façon.
fn block_markers(context: &LineContext, from: usize, to: usize) -> Vec<(usize, usize, SourceKind)> {
    let mut out: Vec<(usize, usize, SourceKind)> = Vec::new();
    let mut index = from;

    loop {
        // Indentation : au plus trois espaces (au-delà, c'est du code
        // indenté).
        let mut spaces = 0usize;
        while index + spaces < to && context.chars[index + spaces] == ' ' && spaces < 3 {
            spaces += 1;
        }
        if index + spaces < to && context.chars[index + spaces] == ' ' {
            break;
        }
        let start = index + spaces;
        if start >= to {
            break;
        }
        let rest = context.slice(start, to);

        // Citation `>`.
        if context.chars[start] == '>' {
            out.push((start, start + 1, SourceKind::BlockquoteMarker));
            index = start + 1;
            continue;
        }

        // Liste à puces (`-`, `*`, `+`).
        if super::live_preview::parse_puce(rest).is_some() {
            out.push((start, start + 1, SourceKind::ListMarker));
            let after = skip_blanks(context, start + 1, to);
            if let Some(len) = task_checkbox_len(context.slice(after, to)) {
                out.push((after, (after + len).min(to), SourceKind::TaskCheckbox));
            }
            break;
        }

        // Liste ordonnée (`1.`).
        if super::live_preview::parse_liste_numerotee(rest).is_some() {
            let digits = context.chars[start..to]
                .iter()
                .take_while(|ch| ch.is_ascii_digit())
                .count();
            out.push((start, start + digits + 1, SourceKind::ListMarker));
            let after = skip_blanks(context, start + digits + 1, to);
            if let Some(len) = task_checkbox_len(context.slice(after, to)) {
                out.push((after, (after + len).min(to), SourceKind::TaskCheckbox));
            }
            break;
        }

        break;
    }

    out
}

/// Avance l'index après les blancs, sans les inclure dans un marqueur.
fn skip_blanks(context: &LineContext, from: usize, to: usize) -> usize {
    let mut index = from;
    while index < to && (context.chars[index] == ' ' || context.chars[index] == '\t') {
        index += 1;
    }
    index
}

/// Longueur (en caractères) de la case à cocher d'une tâche en tête de texte.
///
/// Miroir de la règle de l'aperçu : la case (`[ ]`, `[x]`, `[X]`) doit être
/// suivie d'un blanc ou de la fin de la ligne.
fn task_checkbox_len(rest: &str) -> Option<usize> {
    for marqueur in ["[ ]", "[x]", "[X]"] {
        if let Some(reste) = rest.strip_prefix(marqueur) {
            if reste.is_empty() || reste.starts_with(char::is_whitespace) {
                return Some(3);
            }
        }
    }
    None
}

/// Colore le markup inline d'une plage de caractères.
fn colorize_inline(
    context: &LineContext,
    from: usize,
    to: usize,
    markers: &[(usize, usize, SourceKind)],
    out: &mut Vec<SourceToken>,
) {
    let mut marker_index = 0usize;
    let mut plain_start = from;
    let mut index = from;

    while index < to {
        // Marqueur de bloc (puce, citation, case à cocher).
        if marker_index < markers.len() && markers[marker_index].0 == index {
            push_plain(context, plain_start, index, out);
            let (_, end, kind) = markers[marker_index];
            push_token(
                out,
                kind,
                context.slice(index, end),
                context.line,
                context.offset(index),
            );
            index = end;
            plain_start = index;
            marker_index += 1;
            continue;
        }

        // Construction inline (emphase, code, lien, wikilink, tag…).
        if let Some((kind, end)) = inline_span(&context.chars, index) {
            if end <= to {
                push_plain(context, plain_start, index, out);
                push_token(
                    out,
                    kind,
                    context.slice(index, end),
                    context.line,
                    context.offset(index),
                );
                index = end;
                plain_start = index;
                continue;
            }
        }

        index += 1;
    }

    push_plain(context, plain_start, to, out);
}

/// Émet le fragment de texte ordinaire entre deux constructions.
fn push_plain(context: &LineContext, from: usize, to: usize, out: &mut Vec<SourceToken>) {
    if from >= to {
        return;
    }
    push_token(
        out,
        SourceKind::Plain,
        context.slice(from, to),
        context.line,
        context.offset(from),
    );
}

/// Émet un fragment coloré.
fn push_token(
    out: &mut Vec<SourceToken>,
    kind: SourceKind,
    text: &str,
    line: usize,
    column: usize,
) {
    if text.is_empty() {
        return;
    }
    out.push(SourceToken {
        kind,
        text: text.to_string(),
        line,
        column,
    });
}

/// Reconnaît une construction inline à la position `index`.
///
/// Les fonctions de reconnaissance sont celles de `live_preview` : la source
/// et l'aperçu reconnaissent donc exactement les mêmes constructions.
fn inline_span(chars: &[char], index: usize) -> Option<(SourceKind, usize)> {
    match chars[index] {
        '`' => super::live_preview::position(chars, index + 1, '`')
            .map(|end| (SourceKind::InlineCode, end + 1)),
        '*' => super::live_preview::parse_emphase(chars, index).map(|(style, end, _)| {
            let kind = match style {
                SegmentStyle::Bold | SegmentStyle::BoldItalic => SourceKind::Strong,
                _ => SourceKind::Emphasis,
            };
            (kind, end)
        }),
        '~' if chars.get(index + 1) == Some(&'~') => {
            super::live_preview::position_double(chars, index + 2, '~')
                .map(|end| (SourceKind::Strikethrough, end + 2))
        }
        '[' if chars.get(index + 1) != Some(&'[') => {
            super::live_preview::parse_lien(chars, index).map(|(_, _, end)| (SourceKind::Link, end))
        }
        '!' if chars.get(index + 1) == Some(&'[') && chars.get(index + 2) == Some(&'[') => {
            super::live_preview::parse_embed_inline(chars, index)
                .map(|(_, _, end)| (SourceKind::Embed, end))
        }
        '[' if chars.get(index + 1) == Some(&'[') => {
            super::live_preview::parse_wikilink(chars, index)
                .map(|(_, end)| (SourceKind::WikiLink, end))
        }
        '#' if super::live_preview::debut_de_tag(chars, index) => {
            super::live_preview::fin_tag(chars, index + 1).map(|end| (SourceKind::Tag, end))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_markdown, Block, BlockKind};

    #[test]
    fn document_vide_retourne_aucun_bloc() {
        assert_eq!(parse_markdown(""), Vec::new());
        assert_eq!(parse_markdown("\n\n   \n\t\n"), Vec::new());
    }

    #[test]
    fn titres_niveaux_un_a_six() {
        let source = "# Un\n## Deux\n### Trois\n#### Quatre\n##### Cinq\n###### Six";
        let blocks = parse_markdown(source);

        assert_eq!(blocks.len(), 6);
        for (index, block) in blocks.iter().enumerate() {
            let level = index as u8 + 1;
            assert_eq!(block.kind, BlockKind::Heading(level));
        }
        assert_eq!(blocks[0].text, "Un");
        assert_eq!(blocks[5].text, "Six");
    }

    #[test]
    fn dièses_additionnels_ou_collés_ne_sont_pas_des_titres() {
        let blocks = parse_markdown("####### Sept\n#Colle");

        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].kind, BlockKind::Paragraph);
        assert_eq!(blocks[0].text, "####### Sept\n#Colle");
    }

    #[test]
    fn callout_type_et_contenu() {
        let blocks = parse_markdown("> [!WARNING]\n> Ligne un\n> Ligne deux");

        assert_eq!(blocks.len(), 1);
        assert_eq!(
            blocks[0].kind,
            BlockKind::Callout {
                kind: "WARNING".to_string()
            }
        );
        assert_eq!(blocks[0].text, "Ligne un\nLigne deux");
    }

    #[test]
    fn callout_avec_titre() {
        let blocks = parse_markdown("> [!note] Mon titre\n> corps");

        assert_eq!(
            blocks[0].kind,
            BlockKind::Callout {
                kind: "note".to_string()
            }
        );
        assert_eq!(blocks[0].text, "Mon titre\ncorps");
    }

    #[test]
    fn citation_regroupee_puis_paragraphe() {
        let blocks = parse_markdown("> première\n> seconde\n\ntexte");

        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].kind, BlockKind::Quote);
        assert_eq!(blocks[0].text, "première\nseconde");
        assert_eq!(blocks[1].kind, BlockKind::Paragraph);
        assert_eq!(blocks[1].text, "texte");
    }

    #[test]
    fn code_fence_backticks_garde_le_contenu_brut() {
        let source = "```rust\nfn main() {\n    println!(\"# pas un titre\");\n}\n```";
        let blocks = parse_markdown(source);

        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].kind, BlockKind::CodeBlock);
        assert_eq!(
            blocks[0].text,
            "fn main() {\n    println!(\"# pas un titre\");\n}"
        );
    }

    #[test]
    fn code_fence_tildes_et_fence_non_fermee() {
        let blocks = parse_markdown("~~~\n> [!NOTE] brut\n~~~\n---\n~~~\nfin");

        assert_eq!(blocks.len(), 3);
        assert_eq!(blocks[0].kind, BlockKind::CodeBlock);
        assert_eq!(blocks[0].text, "> [!NOTE] brut");
        assert_eq!(blocks[1].kind, BlockKind::Paragraph);
        assert_eq!(blocks[1].text, "---");
        assert_eq!(blocks[2].kind, BlockKind::CodeBlock);
        assert_eq!(blocks[2].text, "fin");
    }

    #[test]
    fn code_indente_quatre_espaces_retire_l_indentation() {
        let blocks = parse_markdown("    let x = 1;\n      imbriqué\n\ntexte");

        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].kind, BlockKind::CodeBlock);
        assert_eq!(blocks[0].text, "let x = 1;\n  imbriqué");
        assert_eq!(blocks[1].kind, BlockKind::Paragraph);
        assert_eq!(blocks[1].text, "texte");
    }

    #[test]
    fn listes_puces_et_ordonnees_regroupees() {
        let blocks = parse_markdown("- un\n* deux\n1. trois\n2. quatre\n\nfin");

        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].kind, BlockKind::List);
        assert_eq!(blocks[0].text, "un\ndeux\ntrois\nquatre");
        assert_eq!(blocks[1].kind, BlockKind::Paragraph);
        assert_eq!(blocks[1].text, "fin");
    }

    #[test]
    fn tableau_avec_separatrice() {
        let source = "| Nom | Valeur |\n|---|:---:|\n| a | 1 |\n| b | 2 |";
        let blocks = parse_markdown(source);

        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].kind, BlockKind::Table);
        assert_eq!(blocks[0].text, "| Nom | Valeur |\n| a | 1 |\n| b | 2 |");
    }

    #[test]
    fn tableau_sans_separatrice_reste_un_paragraphe() {
        let blocks = parse_markdown("| a | b |\n| 1 | 2 |");

        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].kind, BlockKind::Paragraph);
        assert_eq!(blocks[0].text, "| a | b |\n| 1 | 2 |");
    }

    #[test]
    fn paragraphe_fusionne_les_lignes_consecutives() {
        let blocks = parse_markdown("ligne un\nligne deux\n\nligne trois");

        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].kind, BlockKind::Paragraph);
        assert_eq!(blocks[0].text, "ligne un\nligne deux");
        assert_eq!(blocks[1].kind, BlockKind::Paragraph);
        assert_eq!(blocks[1].text, "ligne trois");
    }

    #[test]
    fn ligne_separatrice_seule_est_un_paragraphe() {
        let blocks = parse_markdown("---");

        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].kind, BlockKind::Paragraph);
        assert_eq!(blocks[0].text, "---");
    }

    #[test]
    fn document_mixte_conserve_l_ordre_des_blocs() {
        let source = "\
# Titre

paragraphe
sur deux lignes

- item

> [!TIP]
> indice

```text
code
```
";
        let expected = vec![
            Block {
                kind: BlockKind::Heading(1),
                text: "Titre".to_string(),
            },
            Block {
                kind: BlockKind::Paragraph,
                text: "paragraphe\nsur deux lignes".to_string(),
            },
            Block {
                kind: BlockKind::List,
                text: "item".to_string(),
            },
            Block {
                kind: BlockKind::Callout {
                    kind: "TIP".to_string(),
                },
                text: "indice".to_string(),
            },
            Block {
                kind: BlockKind::CodeBlock,
                text: "code".to_string(),
            },
        ];

        assert_eq!(parse_markdown(source), expected);
    }
}
