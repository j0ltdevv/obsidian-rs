//! Tests du rendu Markdown coloré (`src/ui/markdown.rs`).
//!
//! L'API testée est celle de `obsy::ui::markdown` : `styled_lines`
//! découpe une source en lignes de fragments colorés (`StyledSpan`), et
//! `token_color` associe une couleur stable à chaque `SourceKind`.
//!
//! Invariants vérifiés :
//! 1. `styled_lines(src).len() == src.split('\n').count()`.
//! 2. Pour chaque ligne `i`, la concaténation des `text` des spans vaut
//!    exactement `src.split('\n').nth(i)` (aucun caractère perdu ni ajouté,
//!    `\r` compris).
//! 3. Les couleurs sont stables par kind, distinctes pour
//!    `WikiLink`/`Tag`/`Link`/`Plain`, et les frontmatters sont atténués
//!    (même couleur, différente de `Plain`).

use obsy::ui::markdown::{styled_lines, token_color, StyledSpan};
use obsy::vault::editor::SourceKind;

/// Source représentative couvrant frontmatter, titre, paragraphe, liste,
/// wikilink, tag, code inline, bloc de code et citation.
const SOURCE_REPRESENTATIVE: &str = "\
---
titre: Exemple
tags: [sujet]
---

# Titre

Un paragraphe avec `code` inline, [[Note]] et #sujet.

- item

```
let code = true;
```

> citation
";

/// Concatène les textes des spans d'une ligne.
fn concat_ligne(ligne: &[StyledSpan]) -> String {
    ligne.iter().map(|span| span.text.as_str()).collect()
}

#[test]
fn styled_lines_respecte_le_nombre_de_lignes() {
    let source = SOURCE_REPRESENTATIVE;
    let lignes = styled_lines(source);
    assert_eq!(lignes.len(), source.split('\n').count());
}

#[test]
fn chaque_ligne_reproduit_exactement_la_source() {
    let source = SOURCE_REPRESENTATIVE;
    let lignes = styled_lines(source);
    let attends: Vec<&str> = source.split('\n').collect();
    assert_eq!(lignes.len(), attends.len());

    for (i, (ligne, attendu)) in lignes.iter().zip(attends.iter()).enumerate() {
        assert_eq!(
            concat_ligne(ligne),
            *attendu,
            "ligne {i} : perte ou ajout de caractères"
        );
    }
}

#[test]
fn source_vide_produit_une_ligne_vide() {
    let lignes = styled_lines("");
    assert_eq!(lignes.len(), 1);
    assert_eq!(concat_ligne(&lignes[0]), "");
}

#[test]
fn source_sans_fin_de_ligne_conserve_son_texte() {
    let lignes = styled_lines("juste un texte");
    assert_eq!(lignes.len(), 1);
    assert_eq!(concat_ligne(&lignes[0]), "juste un texte");
}

#[test]
fn source_finissant_par_saut_de_ligne_a_une_derniere_ligne_vide() {
    let lignes = styled_lines("ligne\n");
    assert_eq!(lignes.len(), 2);
    assert_eq!(concat_ligne(&lignes[0]), "ligne");
    assert_eq!(concat_ligne(&lignes[1]), "");
}

#[test]
fn sauts_de_ligne_crlf_conservent_le_carriage_return() {
    let lignes = styled_lines("a\r\nb\r\n");
    assert_eq!(lignes.len(), 3);
    assert_eq!(concat_ligne(&lignes[0]), "a\r");
    assert_eq!(concat_ligne(&lignes[1]), "b\r");
    assert_eq!(concat_ligne(&lignes[2]), "");
}

#[test]
fn token_color_renvoie_des_couleurs_opaques_pour_tout_sourcekind() {
    let kinds = [
        SourceKind::Plain,
        SourceKind::HeadingMarker,
        SourceKind::ListMarker,
        SourceKind::TaskCheckbox,
        SourceKind::BlockquoteMarker,
        SourceKind::FrontmatterDelimiter,
        SourceKind::Frontmatter,
        SourceKind::CodeFence,
        SourceKind::Emphasis,
        SourceKind::Strong,
        SourceKind::Strikethrough,
        SourceKind::InlineCode,
        SourceKind::Link,
        SourceKind::WikiLink,
        SourceKind::Embed,
        SourceKind::Tag,
        SourceKind::HtmlComment,
        SourceKind::LineBreak,
    ];
    assert_eq!(kinds.len(), 18);

    for kind in kinds {
        let couleur = token_color(kind);
        assert!(
            couleur.a > 0.0,
            "la couleur de {kind:?} est transparente (alpha = {})",
            couleur.a
        );
    }
}

#[test]
fn token_color_regroupe_les_familles_de_couleurs_annoncees() {
    // Familles partageant la même couleur, telles que documentées dans
    // `token_color` : texte, accent, atténué, discret et code.
    let famille = |a: SourceKind, b: SourceKind| {
        assert_eq!(token_color(a), token_color(b));
    };

    // Texte principal : Plain, Strong et Emphasis (LineBreak inclus).
    famille(SourceKind::Plain, SourceKind::Strong);
    famille(SourceKind::Plain, SourceKind::Emphasis);
    famille(SourceKind::Plain, SourceKind::LineBreak);

    // Accent : titres, tâches, citations et liens.
    famille(SourceKind::HeadingMarker, SourceKind::TaskCheckbox);
    famille(SourceKind::HeadingMarker, SourceKind::BlockquoteMarker);
    famille(SourceKind::HeadingMarker, SourceKind::Link);

    // Atténué : puces et texte barré.
    famille(SourceKind::ListMarker, SourceKind::Strikethrough);

    // Discret : frontmatter et commentaire HTML.
    famille(SourceKind::FrontmatterDelimiter, SourceKind::Frontmatter);
    famille(SourceKind::FrontmatterDelimiter, SourceKind::HtmlComment);

    // Code : fences et code inline.
    famille(SourceKind::CodeFence, SourceKind::InlineCode);

    // Les familles distinctes restent distinctes entre elles.
    let distinct = |a: SourceKind, b: SourceKind| {
        assert_ne!(token_color(a), token_color(b));
    };
    distinct(SourceKind::Plain, SourceKind::HeadingMarker);
    distinct(SourceKind::Plain, SourceKind::Frontmatter);
    distinct(SourceKind::Plain, SourceKind::CodeFence);
    distinct(SourceKind::HeadingMarker, SourceKind::WikiLink);
    distinct(SourceKind::HeadingMarker, SourceKind::Embed);
    distinct(SourceKind::HeadingMarker, SourceKind::Tag);
    distinct(SourceKind::WikiLink, SourceKind::Embed);
    distinct(SourceKind::WikiLink, SourceKind::Tag);
    distinct(SourceKind::Embed, SourceKind::Tag);
}

#[test]
fn wikilink_tag_lien_et_plain_ont_des_couleurs_distinctes() {
    let couleur = token_color(SourceKind::WikiLink);
    let tag = token_color(SourceKind::Tag);
    let lien = token_color(SourceKind::Link);
    let plain = token_color(SourceKind::Plain);

    assert_ne!(couleur, tag);
    assert_ne!(couleur, lien);
    assert_ne!(couleur, plain);
    assert_ne!(tag, lien);
    assert_ne!(tag, plain);
    assert_ne!(lien, plain);
}

#[test]
fn frontmatter_et_son_delimiteur_sont_attemues_et_identiques() {
    assert_eq!(
        token_color(SourceKind::Frontmatter),
        token_color(SourceKind::FrontmatterDelimiter)
    );
    assert_ne!(
        token_color(SourceKind::Frontmatter),
        token_color(SourceKind::Plain)
    );
}
