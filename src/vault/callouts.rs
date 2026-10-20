//! Module `callouts` — extraction des blocs de citation spéciaux « callout »
//! (`> [!TYPE]`) à la manière d'Obsidian.
//!
//! Un callout est une citation Markdown dont la première ligne porte le
//! marqueur `[!TYPE]` (et éventuellement `-` pour un état replié et un titre).
//! Ce module parse ces blocs sans aucun rendu : il retourne les métadonnées et
//! les lignes de corps brutes (sans le préfixe `> `).

/// Un callout `> [!TYPE]` tel qu'extrait d'un document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Callout {
    /// Type du callout, normalisé en minuscules (`note`, `warning`, …).
    pub kind: String,
    /// Titre optionnel : reste de la ligne d'ouverture après `]`, trimé.
    pub title: Option<String>,
    /// `true` si la syntaxe `> [!TYPE]-` (callout replié) a été utilisée.
    pub collapsed: bool,
    /// Index (0-based) de la ligne d'ouverture.
    pub start_line: usize,
    /// Index (0-based) de la dernière ligne du callout, **inclus**.
    pub end_line: usize,
    /// Lignes de corps, sans le préfixe de citation `> ` (une ligne `>` seule
    /// devient une ligne vide dans le corps).
    pub body: Vec<String>,
}

/// Extrait tous les callouts d'un contenu Markdown.
///
/// Règles :
/// - Un callout commence par une ligne de citation `> [!TYPE]` : `TYPE` est le
///   texte entre `[!` et `]` (non vide, trimé), normalisé en minuscules.
/// - `> [!TYPE]-` produit un callout replié (`collapsed: true`) ; le tiret
///   doit suivre immédiatement `]`.
/// - Le titre optionnel est le reste de la ligne d'ouverture après `]`, trimé
///   (`None` si vide).
/// - Le corps continue tant que les lignes suivantes sont des citations
///   (`> ` ou `>` seul) ; toute autre ligne (texte ordinaire, ligne vide,
///   fence, …) termine le callout.
/// - Une citation imbriquée (`> > texte`) est conservée telle quelle dans le
///   corps : seul le premier marqueur `> ` est retiré.
/// - Les blockquotes ordinaires (sans `[!...]` juste après `> `) sont ignorés.
/// - Les blocs de code — fences ``` / ~~~ et code indenté (quatre espaces ou
///   plus, ou tabulation) — ne peuvent ni ouvrir ni prolonger un callout.
///
/// Les indices `start_line`/`end_line` sont 0-based et `end_line` est inclus :
/// pour un callout jamais fermé allant jusqu'à la fin du document,
/// `end_line` vaut l'index de la dernière ligne du document.
pub fn extract_callouts(content: &str) -> Vec<Callout> {
    let lines: Vec<&str> = content.lines().collect();
    let mut callouts = Vec::new();
    let mut index = 0;

    while index < lines.len() {
        // Bloc de code délimité par une fence : on saute tout le bloc pour ne
        // pas interpréter ses lignes (ouvertures de callout incluses).
        if let Some(fence) = fence_open(lines[index]) {
            index += 1;
            while index < lines.len() && !fence_closes(lines[index], &fence) {
                index += 1;
            }
            if index < lines.len() {
                index += 1; // consomme la fence fermante
            }
            continue;
        }

        // Tentative d'ouverture d'un callout.
        if let Some((kind, title, collapsed)) = callout_opener(lines[index]) {
            let start = index;
            let mut body = Vec::new();
            index += 1;
            while let Some(body_line) = lines.get(index).and_then(|line| quote_body(line)) {
                body.push(body_line.trim_end().to_string());
                index += 1;
            }
            callouts.push(Callout {
                kind,
                title,
                collapsed,
                start_line: start,
                end_line: index - 1,
                body,
            });
            continue;
        }

        index += 1;
    }

    callouts
}

/// Une fence de code : caractère et longueur minimale.
struct Fence {
    char: char,
    len: usize,
}

/// Reconnaît une fence ouvrante (au moins trois backticks ou tildes), après
/// retrait d'une indentation d'au plus trois espaces.
fn fence_open(line: &str) -> Option<Fence> {
    let body = strip_up_to_3_spaces(line)?;
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
fn fence_closes(line: &str, open: &Fence) -> bool {
    let Some(body) = strip_up_to_3_spaces(line) else {
        return false;
    };
    let trimmed = body.trim_end();
    let len = trimmed.chars().take_while(|c| *c == open.char).count();
    len >= open.len && trimmed.chars().all(|c| c == open.char)
}

/// Retire une indentation d'au plus trois espaces ; `None` pour une ligne de
/// code indenté (quatre espaces ou plus, ou tabulation).
fn strip_up_to_3_spaces(line: &str) -> Option<&str> {
    if line.starts_with('\t') {
        return None;
    }
    let spaces = line.bytes().take_while(|byte| *byte == b' ').count();
    if spaces >= 4 {
        None
    } else {
        Some(&line[spaces..])
    }
}

/// Retourne le contenu d'une ligne de citation après retrait du marqueur
/// `> ` (ou `>` seul pour une ligne vide de citation) ; `None` si la ligne
/// n'est pas une citation.
fn quote_body(line: &str) -> Option<&str> {
    let body = strip_up_to_3_spaces(line)?;
    let rest = body.strip_prefix('>')?;
    Some(rest.strip_prefix(' ').unwrap_or(rest))
}

/// Reconnaît une ouverture de callout `> [!TYPE]` (avec `-` optionnel et titre
/// éventuel) et retourne `(type minuscules, titre, replié)`.
fn callout_opener(line: &str) -> Option<(String, Option<String>, bool)> {
    let body = quote_body(line)?;
    let inner = body.strip_prefix("[!")?;
    let end = inner.find(']')?;
    let kind = inner[..end].trim();
    if kind.is_empty() {
        return None;
    }
    let rest = &inner[end + 1..];
    let (collapsed, title) = match rest.strip_prefix('-') {
        Some(after) => (true, after.trim()),
        None => (false, rest.trim()),
    };
    let title = (!title.is_empty()).then(|| title.to_string());
    Some((kind.to_lowercase(), title, collapsed))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Construit le callout attendu pour des assertions compactes.
    fn callout(
        kind: &str,
        title: Option<&str>,
        collapsed: bool,
        start: usize,
        end: usize,
        body: &[&str],
    ) -> Callout {
        Callout {
            kind: kind.to_string(),
            title: title.map(str::to_string),
            collapsed,
            start_line: start,
            end_line: end,
            body: body.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn callout_simple_type_normalise_en_minuscules() {
        assert_eq!(
            extract_callouts("> [!WARNING]\n> Attention\n> suite"),
            vec![callout(
                "warning",
                None,
                false,
                0,
                2,
                &["Attention", "suite"]
            )]
        );
    }

    #[test]
    fn callout_sans_corps() {
        assert_eq!(
            extract_callouts("> [!note]"),
            vec![callout("note", None, false, 0, 0, &[])]
        );
    }

    #[test]
    fn titre_optionnel_apres_crochet_fermant() {
        assert_eq!(
            extract_callouts("> [!note] Mon titre\n> corps"),
            vec![callout("note", Some("Mon titre"), false, 0, 1, &["corps"])]
        );
    }

    #[test]
    fn callout_replie_avec_tiret() {
        assert_eq!(
            extract_callouts("> [!todo]- Tâches\n> À faire"),
            vec![callout("todo", Some("Tâches"), true, 0, 1, &["À faire"])]
        );
        // Sans titre : `-` seul.
        assert_eq!(
            extract_callouts("> [!danger]-"),
            vec![callout("danger", None, true, 0, 0, &[])]
        );
    }

    #[test]
    fn corps_multilignes_sans_prefixe_de_citation() {
        assert_eq!(
            extract_callouts("> [!info]\n> Un\n> Deux\n> Trois"),
            vec![callout("info", None, false, 0, 3, &["Un", "Deux", "Trois"])]
        );
    }

    #[test]
    fn deux_callouts_adjacents_separes_par_ligne_vide() {
        let source = "> [!note]\n> un\n\n> [!tip]\n> deux";
        assert_eq!(
            extract_callouts(source),
            vec![
                callout("note", None, false, 0, 1, &["un"]),
                callout("tip", None, false, 3, 4, &["deux"]),
            ]
        );
    }

    #[test]
    fn citation_imbriquee_conservee_dans_le_corps() {
        assert_eq!(
            extract_callouts("> [!note]\n> niveau un\n> > niveau deux\n> retour"),
            vec![callout(
                "note",
                None,
                false,
                0,
                3,
                &["niveau un", "> niveau deux", "retour"],
            )]
        );
    }

    #[test]
    fn ligne_de_citation_vide_conservee_dans_le_corps() {
        assert_eq!(
            extract_callouts("> [!note]\n> un\n>\n> deux"),
            vec![callout("note", None, false, 0, 3, &["un", "", "deux"])]
        );
    }

    #[test]
    fn blockquote_ordinaire_ignoree() {
        assert!(extract_callouts("> Simple citation\n> suite").is_empty());
        // Une citation ordinaire avant un callout ne crée pas d'entrée.
        assert_eq!(
            extract_callouts("> simple\n> [!note]\n> ok"),
            vec![callout("note", None, false, 1, 2, &["ok"])]
        );
    }

    #[test]
    fn marqueur_non_en_tete_de_citation_ignore() {
        assert!(extract_callouts("> attention [!note] pas un callout").is_empty());
    }

    #[test]
    fn code_fence_ignoree() {
        let source = "```rust\n> [!BUG] pret\ngarde\n```\n> [!note]\n> réel";
        assert_eq!(
            extract_callouts(source),
            vec![callout("note", None, false, 4, 5, &["réel"])]
        );
    }

    #[test]
    fn fence_tildes_et_fence_non_fermee_ignorees() {
        assert!(extract_callouts("~~~\n> [!note] caché\n").is_empty());
        assert!(extract_callouts("```\n> [!note] caché\n").is_empty());
    }

    #[test]
    fn fence_termine_un_callout_en_cours() {
        assert_eq!(
            extract_callouts("> [!note]\n> avant\n```\n> dans code\n```"),
            vec![callout("note", None, false, 0, 1, &["avant"])]
        );
    }

    #[test]
    fn code_indente_ignore() {
        assert!(extract_callouts("    > [!note] hors scope\n").is_empty());
    }

    #[test]
    fn callout_jamais_ferme_va_jusqu_a_la_fin() {
        assert_eq!(
            extract_callouts("> [!note]\n> un\n> deux"),
            vec![callout("note", None, false, 0, 2, &["un", "deux"])]
        );
    }
}
