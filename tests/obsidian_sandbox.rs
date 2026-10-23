//! Tests d'intégration : dialecte Markdown d'Obsidian documenté par les guides.
//!
//! Fixtures : `tests/fixtures/obsidian-sandbox/` (extraits de l'ASAR d'Obsidian,
//! documents de démonstration). Aucune chaîne des guides n'est copiée ici :
//! seules les sous-chaînes minimales nécessaires aux assertions (cibles de liens,
//! marqueurs de callout, etc.) sont utilisées.

use std::fs;

use obsy::vault::tags::parse_tags;
use obsy::vault::wikilinks::parse_wikilinks;
use obsy::vault::Wikilink;

/// Lit une fixture du sandbox (chemin relatif à
/// `tests/fixtures/obsidian-sandbox/`).
fn lire_fixture(chemin_relatif: &str) -> String {
    let racine = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/obsidian-sandbox/"
    );
    fs::read_to_string(format!("{racine}{chemin_relatif}")).unwrap_or_else(|err| {
        panic!("lecture de la fixture « {chemin_relatif} » impossible : {err}")
    })
}

fn lien(cible: &str) -> Wikilink {
    Wikilink {
        target: cible.to_string(),
        alias: None,
    }
}

fn lien_avec_alias(cible: &str, alias: &str) -> Wikilink {
    Wikilink {
        target: cible.to_string(),
        alias: Some(alias.to_string()),
    }
}

/// 1. Wikilink — `Formatting/Internal link.md`.
#[test]
fn wikilinks_extraits_de_internal_link_md() {
    let source = lire_fixture("Formatting/Internal link.md");

    // « Link to a page: [[Embeds]]. » figure deux fois (bloc de code + copie réelle).
    assert_eq!(
        parse_wikilinks(&source),
        vec![lien("Embeds"), lien("Embeds")]
    );

    // Aucun tag dans ce document.
    assert!(parse_tags(&source).is_empty());
}

/// 2. Embed — `Formatting/Embeds.md`.
///
/// LIMITE : pas de distinction embed pour l'instant — `![[…]]` produit un
/// Wikilink identique à `[[…]]`, dont la cible ne contient pas le « ! ».
#[test]
fn embeds_parses_comme_wikilinks_sans_bang() {
    let source = lire_fixture("Formatting/Embeds.md");

    assert_eq!(
        parse_wikilinks(&source),
        vec![
            lien("Embeds"),
            lien("Plugins makes Obsidian special for you"),
            lien("Plugins make Obsidian special for you"),
        ]
    );
}

/// 3. Callout — `Formatting/Callout.md`.
///
/// Le marqueur `> [!type]` ne doit générer ni tag ni wikilink parasite.
#[test]
fn callouts_ne_generent_pas_de_tags_ni_de_liens() {
    // Marqueurs de callout tels qu'ils apparaissent dans la fixture.
    for marker in [
        "> [!NOTE]",
        "> [!INFO]",
        "> [!TIP] Callouts can have custom titles, which also supports **markdown**!",
        "> [!FAQ]- Are callouts foldable?",
    ] {
        assert!(
            parse_wikilinks(marker).is_empty(),
            "marker callout : {marker}"
        );
    }

    let source = lire_fixture("Formatting/Callout.md");

    // Seuls les wikilinks cités par le guide existent (bloc de code + copie réelle).
    assert_eq!(
        parse_wikilinks(&source),
        vec![
            lien_avec_alias("Internal link", "wikilinks"),
            lien_avec_alias("Internal link", "wikilinks"),
            lien_avec_alias("#Customizations", "customized"),
        ]
    );

    // LIMITE : l'ancre de lien `[[#Customizations|customized]]` est interprétée
    // comme un tag par parse_tags (le `#` suit une ponctuation).
    assert_eq!(parse_tags(&source), vec!["Customizations".to_string()]);
}

/// 4. Tableau — `Formatting/Table.md`.
#[test]
fn tableaux_ne_generent_pas_de_liens_ni_de_tags() {
    // Des rangées simples (sans wikilink) ne produisent ni lien ni tag.
    for rangee in [
        "First Header | Second Header",
        "Content from cell 1 | Content from cell 2",
        ":----------------|-------------:",
        "because of the `:` | these will be justified",
    ] {
        assert!(parse_wikilinks(rangee).is_empty(), "rangée : {rangee}");
        assert!(parse_tags(rangee).is_empty(), "rangée : {rangee}");
    }

    let source = lire_fixture("Formatting/Table.md");

    // LIMITE : le pipe échappé `\|` n'est pas décodé — la cible du lien garde
    // le backslash.
    assert_eq!(
        parse_wikilinks(&source),
        vec![
            lien_avec_alias("Format your notes\\", "Formatting"),
            lien_avec_alias("Callouts\\", "Callouts"),
            lien_avec_alias("Format your notes\\", "Formatting"),
            lien_avec_alias("Callout\\", "Callouts"),
        ]
    );

    assert!(parse_tags(&source).is_empty());
}

/// 5. Note de bas de page — `Formatting/Footnote.md`.
#[test]
fn footnotes_ne_sont_ni_liens_ni_tags() {
    let source = lire_fixture("Formatting/Footnote.md");

    assert!(parse_wikilinks(&source).is_empty());
    assert!(parse_tags(&source).is_empty());

    for ligne in [
        "Here's a simple footnote,[^1] and here's a longer one.[^bignote]",
        "[^1]: meaningful!",
        "[^bignote]: Here's one with multiple paragraphs and code.",
    ] {
        assert!(parse_wikilinks(ligne).is_empty(), "ligne : {ligne}");
        assert!(parse_tags(ligne).is_empty(), "ligne : {ligne}");
    }
}

/// 6. Tag — `Formatting/Task.md`.
///
/// La fixture « Tag.md » n'existe pas dans le sandbox ; le seul exemple de tag
/// des guides figure dans `Task.md` (« #tags, [links](), **formatting**
/// supported »). Aucune fixture ne contient d'imbrication `#a/b` (couverte par
/// les tests unitaires de `src/vault/tags.rs`).
#[test]
fn tags_extraits_de_task_md() {
    let source = lire_fixture("Formatting/Task.md");

    // `#tags` apparaît deux fois (bloc de code + copie réelle) ; doublons conservés.
    assert_eq!(
        parse_tags(&source),
        vec!["tags".to_string(), "tags".to_string()]
    );

    // Les cases à cocher et `[links]()` ne créent pas de wikilink.
    assert!(parse_wikilinks(&source).is_empty());
}

/// 7. Commentaire — `Formatting/Comment.md`.
///
/// LIMITE : `parse_tags` ignore les délimiteurs `%%..%%` — un tag situé à
/// l'intérieur d'un commentaire est néanmoins détecté. La fixture elle-même ne
/// contient aucun tag.
#[test]
fn tags_dans_les_commentaires_sont_detectes() {
    let source = lire_fixture("Formatting/Comment.md");

    assert!(parse_tags(&source).is_empty());

    assert_eq!(
        parse_tags("Avant %%#cache%% après"),
        vec!["cache".to_string()]
    );
}
