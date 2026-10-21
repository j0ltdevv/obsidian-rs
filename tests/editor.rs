//! Tests de la coloration de la **source** Markdown et des constantes
//! typographiques de l'éditeur.
//!
//! L'API testée est celle de `obsidian_rs::vault::editor` : les fragments
//! colorés, la région de frontmatter, la position du curseur et les
//! constantes typographiques. Ces tests n'`unwrap()` que sur des valeurs
//! construites dans le test, jamais sur une lecture de fichier.

use obsidian_rs::vault::editor::{
    colorize_source, cursor_line_col, frontmatter_region, heading_level, heading_style, typography,
    SourceKind, SourceToken,
};
use obsidian_rs::vault::frontmatter::parse_frontmatter;
use obsidian_rs::vault::live_preview::{render_markdown, SegmentStyle};

/// Kinds des fragments, dans l'ordre du document.
fn kinds(tokens: &[SourceToken]) -> Vec<SourceKind> {
    tokens.iter().map(|token| token.kind).collect()
}

/// Fragment de la nature `kind` dont le texte vaut `text`.
fn fragment<'a>(
    tokens: &'a [SourceToken],
    kind: SourceKind,
    text: &str,
) -> Option<&'a SourceToken> {
    tokens
        .iter()
        .find(|token| token.kind == kind && token.text == text)
}

/// Fragments de contenu (hors fins de ligne).
fn contenu(tokens: &[SourceToken]) -> Vec<&SourceToken> {
    tokens
        .iter()
        .filter(|token| token.kind != SourceKind::LineBreak)
        .collect()
}

#[test]
fn coloration_source_classifie_chaque_sorte_de_marqueur() {
    let source = "\
# Titre

Texte **gras** et *emphase* et `code` et ~~barré~~.

[[Wikilink]] et [lien](https://example.org) et #tag.

- [ ] tâche
- élément

> citation

<!-- commentaire -->
";
    let tokens = colorize_source(source);

    assert!(fragment(&tokens, SourceKind::HeadingMarker, "#").is_some());
    assert!(fragment(&tokens, SourceKind::Strong, "**gras**").is_some());
    assert!(fragment(&tokens, SourceKind::Emphasis, "*emphase*").is_some());
    assert!(fragment(&tokens, SourceKind::InlineCode, "`code`").is_some());
    assert!(fragment(&tokens, SourceKind::Strikethrough, "~~barré~~").is_some());
    assert!(fragment(&tokens, SourceKind::WikiLink, "[[Wikilink]]").is_some());
    assert!(fragment(&tokens, SourceKind::Link, "[lien](https://example.org)").is_some());
    assert!(fragment(&tokens, SourceKind::Tag, "#tag").is_some());
    assert!(fragment(&tokens, SourceKind::ListMarker, "-").is_some());
    assert!(fragment(&tokens, SourceKind::TaskCheckbox, "[ ]").is_some());
    assert!(fragment(&tokens, SourceKind::BlockquoteMarker, ">").is_some());
    assert!(fragment(&tokens, SourceKind::HtmlComment, "<!-- commentaire -->").is_some());
    // Le marqueur de titre est bien en tête de sa ligne.
    let titre = fragment(&tokens, SourceKind::HeadingMarker, "#").expect("titre");
    assert_eq!(titre.line, 0);
    assert_eq!(titre.column, 0);
    assert!(tokens.iter().any(|token| token.kind == SourceKind::Plain));
}

#[test]
fn frontmatter_exclu_de_la_coloration_du_corps() {
    let source = "---\ntitre: x\nliste:\n  - a\n---\n\n# Titre\n";
    let tokens = colorize_source(source);

    // Les deux délimiteurs sont colorés comme tels.
    let delimiteurs: Vec<&SourceToken> = tokens
        .iter()
        .filter(|token| token.kind == SourceKind::FrontmatterDelimiter)
        .collect();
    assert_eq!(delimiteurs.len(), 2);
    assert_eq!(delimiteurs[0].line, 0);
    assert_eq!(delimiteurs[1].line, 4);

    // Le corps YAML est un bloc unique, sans marqueur du corps.
    let yaml: Vec<&SourceToken> = tokens
        .iter()
        .filter(|token| token.kind == SourceKind::Frontmatter)
        .collect();
    assert_eq!(yaml.len(), 3);
    // La liste YAML n'est pas une liste Markdown : aucune puce n'est coloriée
    // dans la région du frontmatter.
    assert!(!tokens
        .iter()
        .any(|token| token.line < 5 && token.kind == SourceKind::ListMarker));

    // Le corps du document est bien coloré.
    assert!(fragment(&tokens, SourceKind::HeadingMarker, "#").is_some());
}

#[test]
fn wikilink_et_lien_markdown_sont_differencies() {
    let tokens = colorize_source("[[Note]] et [Texte](Note.md)");

    assert!(fragment(&tokens, SourceKind::WikiLink, "[[Note]]").is_some());
    assert!(fragment(&tokens, SourceKind::Link, "[Texte](Note.md)").is_some());
    // Le wikilink n'est jamais un lien markdown, et l'inverse non plus.
    assert!(!tokens
        .iter()
        .any(|token| token.kind == SourceKind::Link && token.text.contains("[[")));
    assert!(!tokens
        .iter()
        .any(|token| token.kind == SourceKind::WikiLink && token.text.contains("](")));
}

#[test]
fn tag_dans_un_mot_ignore_et_tag_isole_detecte() {
    let tokens = colorize_source("mot#interne #isolé");

    // Le `#` collé à un mot n'ouvre pas de tag.
    assert!(!tokens
        .iter()
        .any(|token| token.kind == SourceKind::Tag && token.text.contains("interne")));
    // Le `#` précédé d'un blanc ouvre un tag.
    assert!(fragment(&tokens, SourceKind::Tag, "#isolé").is_some());

    // Seul le `#` collé à un mot : aucun tag du tout.
    let seul = colorize_source("mot#interne");
    assert!(!seul.iter().any(|token| token.kind == SourceKind::Tag));
}

#[test]
fn bloc_de_code_entierement_colore_comme_fence() {
    let source = "```rust\n**pas du gras**\n```\n";
    let tokens = colorize_source(source);

    // Tout le contenu du bloc est une fence, marqueurs compris.
    assert!(contenu(&tokens)
        .iter()
        .all(|token| token.kind == SourceKind::CodeFence));
    // Aucun markup inline n'est reconnu dans un bloc de code.
    assert!(!tokens.iter().any(|token| token.kind == SourceKind::Strong));
    assert_eq!(
        kinds(&tokens),
        vec![
            SourceKind::CodeFence,
            SourceKind::LineBreak,
            SourceKind::CodeFence,
            SourceKind::LineBreak,
            SourceKind::CodeFence,
            SourceKind::LineBreak,
        ]
    );
}

#[test]
fn niveau_de_titre_mappe_vers_style_et_taille() {
    assert_eq!(heading_level("### Titre"), Some(3));
    assert_eq!(heading_level("Pas un titre"), None);

    assert_eq!(heading_style(1), SegmentStyle::H1);
    assert_eq!(heading_style(3), SegmentStyle::H3);
    assert_eq!(heading_style(6), SegmentStyle::H6);
    // Un niveau hors bornes est ramené au niveau 1 ou 6.
    assert_eq!(heading_style(0), SegmentStyle::H1);
    assert_eq!(heading_style(9), SegmentStyle::H6);

    // Tailles d'Obsidian : 1.6em, 1.4em, 1.2em, 1.1em, 1em, 1em.
    assert_eq!(typography::heading_size_em(1), 1.6);
    assert_eq!(typography::heading_size_em(2), 1.4);
    assert_eq!(typography::heading_size_em(3), 1.2);
    assert_eq!(typography::heading_size_em(4), 1.1);
    assert_eq!(typography::heading_size_em(5), 1.0);
    assert_eq!(typography::heading_size_em(6), 1.0);
    assert_eq!(typography::LINE_HEIGHT, 1.6);
    assert_eq!(typography::HEADING_LINE_HEIGHT, 1.4);
    assert_eq!(typography::CODE_SIZE_EM, 0.9);
}

#[test]
fn les_fragments_reproduisent_exactement_la_source() {
    let source =
        "---\r\ntitre: x\r\n---\r\n\r\n# Titre\r\n\r\ntexte **gras** et [[Lien]]\r\n- [ ]\r\n>\r\n";
    let tokens = colorize_source(source);

    let reconstruit: String = tokens.iter().map(|token| token.text.as_str()).collect();
    assert_eq!(reconstruit, source);
}

#[test]
fn commentaire_html_multiligne_colorie_comme_un_bloc() {
    let source = "<!--\ncommentaire\nsur deux lignes\n-->\ntexte\n";
    let tokens = colorize_source(source);

    let commentaires: Vec<&SourceToken> = tokens
        .iter()
        .filter(|token| token.kind == SourceKind::HtmlComment)
        .collect();
    assert_eq!(commentaires.len(), 4);
    assert_eq!(commentaires[0].text, "<!--");
    assert_eq!(commentaires[1].text, "commentaire");
    assert_eq!(commentaires[2].text, "sur deux lignes");
    assert_eq!(commentaires[3].text, "-->");
    // Le texte qui suit le commentaire n'est plus un commentaire.
    let contenu_tokens = contenu(&tokens);
    let dernier = contenu_tokens.last().expect("fragment");
    assert_eq!(dernier.kind, SourceKind::Plain);
    assert_eq!(dernier.text, "texte");
}

#[test]
fn frontmatter_region_agree_avec_le_parseur_du_vault() {
    let source = "---\ntitre: x\n---\ncorps";
    assert_eq!(frontmatter_region(source), Some((0, 2)));
    assert!(parse_frontmatter(source).unwrap().is_some());

    // Sans frontmatter : aucune région, comme le parseur du vault.
    let sans = "titre: x\ncorps";
    assert_eq!(frontmatter_region(sans), None);
    assert!(parse_frontmatter(sans).unwrap().is_none());
}

#[test]
fn position_du_curseur_en_ligne_et_colonne() {
    assert_eq!(cursor_line_col("un deux un", 0), Some((0, 0)));
    assert_eq!(cursor_line_col("un deux un", 8), Some((0, 8)));
    assert_eq!(cursor_line_col("a\nun b\nun", 2), Some((1, 0)));
    // Hors document ou au milieu d'un caractère UTF-8 : aucune position.
    assert_eq!(cursor_line_col("abc", 99), None);
    assert_eq!(cursor_line_col("é", 1), None);
}

#[test]
fn source_et_apercu_reconnaisent_le_meme_wikilink() {
    let source = "[[Note]]";

    let segments = render_markdown(source);
    assert!(segments
        .iter()
        .any(|segment| segment.style == SegmentStyle::Wikilink));

    let tokens = colorize_source(source);
    assert!(fragment(&tokens, SourceKind::WikiLink, "[[Note]]").is_some());
}
