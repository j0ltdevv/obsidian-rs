//! Rendu coloré de la source Markdown pour l'éditeur (mode lecture).
//!
//! Ce module s'appuie sur [`crate::vault::editor::colorize_source`] pour
//! fragmenter la source en [`SourceKind`] : chaque fragment est traduit en un
//! [`StyledSpan`] (texte + couleur), et les lignes sont regroupées par
//! [`styled_lines`]. Le rendu effectif, [`render_source`], construit un widget
//! `iced` (`Rich`) affichant la source avec sa syntaxe colorée.
//!
//! Les couleurs reprennent les teintes du thème par défaut d'Obsidian (voir
//! `src/ui/theme_vars.rs`) : texte, texte atténué, texte très discret, accent
//! et accent au survol. Les teintes complémentaires (embeds, tags, code)
//! restent dans la même famille pour rester lisibles dans les deux modes.

use iced::font::Style as FontStyle;
use iced::font::Weight;
use iced::widget::text::{Rich, Span};
use iced::{Color, Element, Font, Length};

use crate::app::Message;
use crate::ui::theme;
use crate::vault::editor::{colorize_source, typography, SourceKind};

/// Un fragment de source coloré : texte possédé et couleur associée.
#[derive(Debug, Clone)]
pub struct StyledSpan {
    /// Texte exact du fragment dans la source.
    pub text: String,
    /// Couleur d'affichage du fragment.
    pub color: Color,
}

/// Texte principal (`--text-normal`, thème sombre par défaut).
const COLOR_TEXTE: Color = Color::from_rgb8(0xda, 0xda, 0xda);
/// Texte atténué (`--text-muted`).
const COLOR_ATTENUE: Color = Color::from_rgb8(0xa0, 0xa0, 0xa0);
/// Texte très discret (`--text-faint`).
const COLOR_DISCRET: Color = Color::from_rgb8(0x66, 0x66, 0x66);
/// Accent (`--accent` / `--text-accent`) : liens, titres, cases.
const COLOR_ACCENT: Color = Color::from_rgb8(0x8a, 0x5c, 0xf5);
/// Accent au survol (`--accent-hover`) : wikilinks.
const COLOR_ACCENT_SURVOL: Color = Color::from_rgb8(0x9b, 0x6f, 0xff);
/// Teinte des embeds, dans la famille de l'accent.
const COLOR_EMBED: Color = Color::from_rgb8(0x7c, 0x5c, 0xd6);
/// Teinte des tags, proche de celle du thème par défaut.
const COLOR_TAG: Color = Color::from_rgb8(0xd6, 0x69, 0xbc);
/// Teinte du code, proche de celle du thème par défaut.
const COLOR_CODE: Color = Color::from_rgb8(0xa8, 0xb8, 0xff);

/// Associe à chaque [`SourceKind`] une couleur stable.
///
/// Les couleurs sont constantes (indépendantes du thème actif) pour rester
/// stables entre deux rendus. `WikiLink`, `Embed`, `Tag` et `Link` ont des
/// teintes distinctes ; `Frontmatter`, `FrontmatterDelimiter` et `HtmlComment`
/// sont atténués.
pub fn token_color(kind: SourceKind) -> Color {
    match kind {
        SourceKind::Plain => COLOR_TEXTE,
        SourceKind::HeadingMarker => COLOR_ACCENT,
        SourceKind::ListMarker => COLOR_ATTENUE,
        SourceKind::TaskCheckbox => COLOR_ACCENT,
        SourceKind::BlockquoteMarker => COLOR_ACCENT,
        SourceKind::FrontmatterDelimiter | SourceKind::Frontmatter | SourceKind::HtmlComment => {
            COLOR_DISCRET
        }
        SourceKind::CodeFence | SourceKind::InlineCode => COLOR_CODE,
        SourceKind::Emphasis | SourceKind::Strong => COLOR_TEXTE,
        SourceKind::Strikethrough => COLOR_ATTENUE,
        SourceKind::Link => COLOR_ACCENT,
        SourceKind::WikiLink => COLOR_ACCENT_SURVOL,
        SourceKind::Embed => COLOR_EMBED,
        SourceKind::Tag => COLOR_TAG,
        SourceKind::LineBreak => COLOR_TEXTE,
    }
}

/// Découpe une source Markdown en lignes colorées.
///
/// # Invariants
///
/// - `styled_lines(src).len() == src.split('\n').count()` ;
/// - pour tout index `i`, la concaténation des `text` des spans de la ligne
///   `i` est exactement `src.split('\n').nth(i)`.
///
/// Le découpage en lignes se fait exclusivement sur `\n` :
///
/// - un terminateur contenant `\n` (`"\n"` ou `"\r\n"`) clôt la ligne
///   courante et en ouvre une nouvelle ; la partie précédant le `\n` (le
///   `\r` d'un CRLF) appartient à la ligne qui se termine et y est
///   conservée ;
/// - un retour chariot seul (`"\r"`) n'ouvre pas de nouvelle ligne : il
///   reste dans la ligne courante.
///
/// Une source finissant par `\n` produit ainsi une dernière ligne vide.
pub fn styled_lines(source: &str) -> Vec<Vec<StyledSpan>> {
    let mut lines: Vec<Vec<StyledSpan>> = vec![Vec::new()];
    for token in colorize_source(source) {
        if token.kind == SourceKind::LineBreak {
            // Le découpage en lignes se fait sur `\n` uniquement.
            if let Some(prefix) = token.text.strip_suffix('\n') {
                // Terminateur contenant `\n` : la partie précédant le `\n`
                // (le `\r` d'un CRLF) appartient à la ligne qui se termine.
                if !prefix.is_empty() {
                    if let Some(line) = lines.last_mut() {
                        if let Some(last) = line.last_mut() {
                            last.text.push_str(prefix);
                        } else {
                            line.push(StyledSpan {
                                text: prefix.to_string(),
                                color: token_color(SourceKind::LineBreak),
                            });
                        }
                    }
                }
                // On clôt la ligne courante et on en ouvre une nouvelle (vide
                // si la source se termine sur un retour à la ligne).
                lines.push(Vec::new());
            } else {
                // Retour chariot seul (`"\r"`) : pas de nouvelle ligne, le
                // `\r` est conservé dans la ligne courante.
                if let Some(line) = lines.last_mut() {
                    if let Some(last) = line.last_mut() {
                        last.text.push_str(&token.text);
                    } else {
                        line.push(StyledSpan {
                            text: token.text,
                            color: token_color(SourceKind::LineBreak),
                        });
                    }
                }
            }
        } else if let Some(last) = lines.last_mut() {
            last.push(StyledSpan {
                text: token.text,
                color: token_color(token.kind),
            });
        }
    }
    lines
}

/// Construit le widget iced affichant la source Markdown colorée.
///
/// Chaque ligne de la source devient une ligne de texte, les fragments étant
/// rendus via [`Rich`]. Le gras, l'italique et le texte barré sont appliqués
/// lorsque le fragment le permet (respectivement `Strong`, `Emphasis`,
/// `Strikethrough`). Le widget possède ses textes : il est donc `'static`.
pub fn render_source(source: &str) -> Element<'static, Message> {
    let mut spans: Vec<Span<'static, (), Font>> = Vec::new();

    for token in colorize_source(source) {
        if token.kind == SourceKind::LineBreak {
            // Terminateur de ligne : on affiche le texte exact du token pour
            // reproduire la source à l'identique (CRLF inclus).
            spans.push(Span::new(token.text));
            continue;
        }

        let mut span = Span::new(token.text.clone()).color(token_color(token.kind));
        match token.kind {
            SourceKind::Strong => {
                let mut bold = theme::font();
                bold.weight = Weight::Bold;
                span = span.font(bold);
            }
            SourceKind::Emphasis => {
                let mut italic = theme::font();
                italic.style = FontStyle::Italic;
                span = span.font(italic);
            }
            SourceKind::Strikethrough => {
                span = span.strikethrough(true);
            }
            _ => {}
        }
        spans.push(span);
    }

    Rich::with_spans(spans)
        .size(typography::BODY_FONT_SIZE_PX)
        .line_height(typography::LINE_HEIGHT)
        .width(Length::Fill)
        .into()
}

/// Découpe les lignes d'un tableau (barres `|` conservées) en grille de
/// cellules. Les lignes de séparation (`| --- | --- |`) sont ignorées.
///
/// Une ligne sans bordure d'entrée (`|`) n'est pas une ligne de tableau :
/// elle est ignorée plutôt que déstructurée.
pub fn table_grid(lignes: &[String]) -> Vec<Vec<String>> {
    lignes
        .iter()
        .filter_map(|ligne| {
            let trim = ligne.trim();
            let interieur = trim.strip_prefix('|')?;
            let interieur = interieur.strip_suffix('|').unwrap_or(interieur);
            let cellules: Vec<String> =
                interieur.split('|').map(|c| c.trim().to_string()).collect();
            if cellules.is_empty() {
                return None;
            }
            let separateur = cellules.iter().all(|c| {
                let corps = c.trim_matches(':');
                !corps.is_empty() && corps.chars().all(|ch| ch == '-')
            });
            (!separateur).then_some(cellules)
        })
        .collect()
}
