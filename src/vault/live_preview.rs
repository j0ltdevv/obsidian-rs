//! Moteur de rendu Markdown -> segments stylés (données pures).
//!
//! Indépendant d'iced : `render_markdown` produce une liste ordonnée de
//! segments typographiques, que l'UI traduira ensuite en widgets ou en
//! texte stylisé. Aucun widget iced ici : uniquement des segments.
//!
//! # Sémantique des segments
//!
//! - L'ordre des segments suit l'ordre d'apparition dans le document.
//! - `line` est 0-based.
//! - `text` ne contient jamais les marqueurs (`**`, `*`, `~~`, `` ` ``).
//! - Un bloc de code produit **un seul** segment `CodeBlock` dont `text`
//!   est le contenu brut du bloc (marqueurs d'ouverture et de fermeture
//!   exclus).
//! - L'imbrication simple (gras dans italique) est tolérée mais pas
//!   requise : le premier marqueur reconnu gagne.

/// Style typographique d'un segment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SegmentStyle {
    /// Texte brut sans style particulier.
    Plain,
    /// Titre de niveau 1 (`# x`).
    H1,
    /// Titre de niveau 2 (`## x`).
    H2,
    /// Titre de niveau 3 (`### x`).
    H3,
    /// Titre de niveau 4 (`#### x`).
    H4,
    /// Titre de niveau 5 (`##### x`).
    H5,
    /// Titre de niveau 6 (`###### x`).
    H6,
    /// Gras `**x**`.
    Bold,
    /// Italique `*x*`.
    Italic,
    /// Gras et italique `***x***`.
    BoldItalic,
    /// Texte barré `~~x~~`.
    Strikethrough,
    /// Code inline `` `x` ``.
    InlineCode,
    /// Bloc de code (un seul segment par bloc, contenu brut).
    CodeBlock,
    /// Citation `> x`.
    Blockquote,
    /// Lien markdown `[texte](url)`.
    Link,
    /// URL nue (`https://...` ou `http://...`).
    ExternalUrl,
    /// Wikilink `[[cible]]`.
    Wikilink,
    /// Tag `#tag`.
    Tag,
    /// Case à cocher d'une tâche non faite (`- [ ]`).
    TaskCheckboxTodo,
    /// Case à cocher d'une tâche faite (`- [x]`).
    TaskCheckboxDone,
    /// Puce de liste non ordonnée (`-`, `*` ou `+`).
    ListBullet,
    /// Numéro de liste ordonnée (`1.`).
    ListNumbered,
    /// Règle horizontale (`---`).
    HorizontalRule,
    /// Ligne d'un tableau simple (`| a | b |`).
    Table,
    /// Note de bas de page `[^id]` ou définition `[^id]: texte`.
    Footnote,

    /// Callout : ligne de corps (`> [!type]`…, ligne d'ouverture sans titre incluse).
    Callout,
    /// Callout : ligne d'ouverture avec titre (`> [!type] titre`).
    CalloutTitle,
    /// Math inline `$...$`.
    MathInline,
    /// Math display `$$...$$` (peut couvrir une plage de lignes).
    MathDisplay,
    /// Embed `![[cible]]` (note ou autre).
    Embed,
    /// Embed d'image `![[fichier.png]]`.
    EmbedImage,
    /// Embed PDF `![[fichier.pdf]]`.
    EmbedPdf,
}

/// Un segment de rendu.
#[derive(Debug, Clone, PartialEq)]
pub struct Segment {
    /// Style.
    pub style: SegmentStyle,
    /// Texte brut du segment (sans le marqueur).
    pub text: String,
    /// Ligne de départ (0-based).
    pub line: usize,
}

/// Rend un contenu Markdown en segments.
///
/// Couvre : titres, gras `**x**`, italique `*x*`, barré `~~x~~`, code
/// inline `` `x` ``, blocs de code, citations `>`, listes à puces /
/// numérotées, tâches `- [ ]` / `- [x]`, liens `[texte](url)`, URLs
/// nues (`http://...`), wikilinks `[[cible]]` (avec alias
/// `[[cible|alias]]` -> `text` = alias), tags `#tag`, règle horizontale
/// `---`, tableaux simples (`| a | b |`), callouts `> [!type]`, maths
/// `$...$` / `$$...$$`, embeds `![[cible]]`.
///
/// L'imbrication simple (gras dans italique) est tolérée mais pas
/// requise : le premier marqueur reconnu sur la ligne gagne.
pub fn render_markdown(content: &str) -> Vec<Segment> {
    let lines: Vec<&str> = content.lines().collect();
    // Parsers existants : plages de callouts et de maths display. Les blocs
    // de code en sont exclus par les modules eux-mêmes.
    let callouts = super::callouts::extract_callouts(content);
    let math = super::math::extract_math(content);
    let mut segments: Vec<Segment> = Vec::new();
    let mut index: usize = 0;
    let mut callout_idx = 0usize;
    let mut math_idx = 0usize;

    while index < lines.len() {
        let line = lines[index];
        let line_no = index;

        // Avance les curseurs vers le premier segment couvrant `index`.
        while callout_idx < callouts.len() && index > callouts[callout_idx].end_line {
            callout_idx += 1;
        }
        while math_idx < math.len() && index > math[math_idx].end_line {
            math_idx += 1;
        }

        // Ligne appartenant à un callout : rendue en segments Callout
        // (jamais en Blockquote).
        if callouts
            .get(callout_idx)
            .is_some_and(|co| (co.start_line..=co.end_line).contains(&index))
        {
            push_callout_line(&callouts[callout_idx], line_no, line, &mut segments);
            index += 1;
            continue;
        }

        // Plage d'une math display : un seul segment `MathDisplay` pour tout
        // le bloc (texte = source brute, sauts de ligne internes compris).
        if math.get(math_idx).is_some_and(|m| {
            m.kind == super::math::MathKind::Display && (m.start_line..=m.end_line).contains(&index)
        }) {
            let m = &math[math_idx];
            segments.push(Segment {
                style: SegmentStyle::MathDisplay,
                text: m.source.clone(),
                line: m.start_line,
            });
            index = m.end_line + 1;
            continue;
        }

        // Bloc de code : trois apostrophes inverses (langue optionnelle).
        if line.trim_start().starts_with("```") {
            index = push_code_block(&lines, index, &mut segments);
            continue;
        }

        push_line_segments(line, line_no, &mut segments);
        index += 1;
    }

    segments
}

/// Absorbe un bloc de code ouvert à `start` jusqu'au fermant suivant.
///
/// Retourne l'index de la ligne à traiter ensuite. Un bloc jamais
/// fermé produit tout de même un segment (contenu jusqu'à la fin).
fn push_code_block(lines: &[&str], start: usize, out: &mut Vec<Segment>) -> usize {
    let mut body: Vec<&str> = Vec::new();
    let mut index = start + 1;

    while index < lines.len() {
        if lines[index].trim_start().starts_with("```") {
            break;
        }
        body.push(lines[index]);
        index += 1;
    }

    out.push(Segment {
        style: SegmentStyle::CodeBlock,
        text: body.join("\n"),
        line: start,
    });

    if index < lines.len() {
        // Le fermant ```` ``` ```` existe : on reprend après lui.
        index + 1
    } else {
        // Bloc jamais fermé : toute la fin du document a été consommée.
        index
    }
}

/// Découpe une ligne hors bloc de code en segments.
fn push_line_segments(line: &str, line_no: usize, out: &mut Vec<Segment>) {
    let cleaned = sanitize_inline_html(line);
    let expanded = expand_emoji(&cleaned);
    let line = expanded.as_str();
    // Ligne de séparation de tableau : ignorée (aucun segment).
    if est_ligne_separation_tableau(line) {
        return;
    }
    // Ligne vide : aucun segment.
    if line.trim().is_empty() {
        return;
    }

    // Règle horizontale : `---`, `***` ou `___`.
    if est_regle_horizontale(line) {
        push_marqueur(SegmentStyle::HorizontalRule, line_no, out);
        return;
    }

    // Titres : de un à six `#` suivis d'un espace.
    if let Some((niveau, reste)) = parse_titre(line) {
        let style = match niveau {
            1 => SegmentStyle::H1,
            2 => SegmentStyle::H2,
            3 => SegmentStyle::H3,
            4 => SegmentStyle::H4,
            5 => SegmentStyle::H5,
            _ => SegmentStyle::H6,
        };
        push_texte(style, reste, line_no, out);
        return;
    }

    // Tableau simple : un segment par ligne.
    if est_ligne_tableau(line) {
        push_table(line, line_no, out);
        return;
    }

    // Préfixe de bloc (citation, liste, tâche) : le style s'applique au
    // texte restant, produit en un seul segment.
    if let Some((reste, style)) = parse_prefixe_bloc(line) {
        push_texte(style, reste.trim_start(), line_no, out);
        return;
    }

    push_inline(line, line_no, out);
}

/// Retire le préfixe de citation d'une ligne (`> ` ou `>` seul), en miroir
/// de `callouts::quote_body`.
fn stripper_citation(line: &str) -> &str {
    let espaces = line.bytes().take_while(|b| *b == b' ').count();
    let corps = if espaces < 4 { &line[espaces..] } else { line };
    match corps.strip_prefix('>') {
        Some(reste) => reste.strip_prefix(' ').unwrap_or(reste),
        None => line,
    }
}

/// Empile les segments d'une ligne appartenant à un callout.
///
/// La ligne d'ouverture avec titre produit un segment `CalloutTitle` dont le
/// texte est la ligne sans `> ` (marqueur `[!type]` conservé) ; sans titre,
/// elle produit un `Callout`. Chaque ligne de corps produit un `Callout`.
fn push_callout_line(
    callout: &super::callouts::Callout,
    line_no: usize,
    line: &str,
    out: &mut Vec<Segment>,
) {
    let corps = stripper_citation(line).trim_end();
    let style = if line_no == callout.start_line && callout.title.is_some() {
        SegmentStyle::CalloutTitle
    } else {
        SegmentStyle::Callout
    };
    push_texte(style, corps, line_no, out);
}

/// Empile un segment représentant un marqueur (texte vide).
fn push_marqueur(style: SegmentStyle, line_no: usize, out: &mut Vec<Segment>) {
    out.push(Segment {
        style,
        text: String::new(),
        line: line_no,
    });
}

/// Empile un segment de texte stylisé.
fn push_texte(style: SegmentStyle, texte: &str, line_no: usize, out: &mut Vec<Segment>) {
    out.push(Segment {
        style,
        text: texte.to_string(),
        line: line_no,
    });
}

/// Empile le segment d'une ligne de tableau (cellules join-es par un
/// espace, bordures `|` retirées).
fn push_table(line: &str, line_no: usize, out: &mut Vec<Segment>) {
    push_texte(
        SegmentStyle::Table,
        &nettoyer_ligne_tableau(line),
        line_no,
        out,
    );
}

/// `true` si la ligne est une règle horizontale (`---`, `***`, `___`).
fn est_regle_horizontale(line: &str) -> bool {
    let trim = line.trim();
    if trim.len() < 3 {
        return false;
    }
    let Some(premier) = trim.chars().next() else {
        return false;
    };
    if premier != '-' && premier != '*' && premier != '_' {
        return false;
    }
    trim.chars().all(|c| c == premier)
}

/// `true` si la ligne est une ligne de tableau simple.
///
/// Choix documenté : on requiert au moins deux `|` et on exclut les
/// wikilinks (qui peuvent contenir `[[a|b]]`). Une ligne `| a | b |`
/// produit un seul segment `Table`, une ligne par ligne de tableau.
fn est_ligne_tableau(line: &str) -> bool {
    let trim = line.trim();
    if trim.starts_with("[[") {
        return false;
    }
    // Ligne de séparation d'en-tête (`| --- | --- |`) : ignorée.
    if trim.starts_with('|') {
        let cells: Vec<&str> = trim.trim_matches('|').split('|').collect();
        if !cells.is_empty()
            && cells.iter().all(|c| {
                let c = c.trim();
                c.starts_with(':')
                    || c.ends_with(':')
                    || c.trim_matches(':').chars().all(|ch| ch == '-')
            })
        {
            return false;
        }
    }
    trim.chars().filter(|&c| c == '|').count() >= 2
}

/// `true` si la ligne est une ligne de séparation de tableau
/// (`| --- | :---: |`…) — ignorée au rendu.
fn est_ligne_separation_tableau(line: &str) -> bool {
    let trim = line.trim();
    if !trim.starts_with('|') {
        return false;
    }
    // Chaque cellule doit être composée uniquement de `:`, `-`, espaces,
    // et contenir au moins un `-` (sinon `| :note: |` serait mangé).
    let cells: Vec<&str> = trim.trim_matches('|').split('|').collect();
    if cells.is_empty() {
        return false;
    }
    let mut any_dash = false;
    for c in &cells {
        let c = c.trim();
        if c.is_empty() || !c.chars().all(|ch| matches!(ch, ':' | '-' | ' ')) {
            return false;
        }
        any_dash |= c.contains('-');
    }
    any_dash
}

/// Nettoie une ligne de tableau : bordures `|` retirées, cellules
/// jointes par un espace.
fn nettoyer_ligne_tableau(line: &str) -> String {
    let trim = line.trim();
    let corps = trim.strip_prefix('|').unwrap_or(trim);
    let corps = corps.strip_suffix('|').unwrap_or(corps);
    corps
        .split('|')
        .map(|c| c.trim())
        .filter(|c| !c.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Détecte un titre en début de ligne : renvoie le niveau et le reste.
///
/// `###x` n'est pas un titre (pas d'espace après les `#`).
fn parse_titre(line: &str) -> Option<(usize, &str)> {
    let trim = line.trim_start();
    let niveau = trim.chars().take_while(|&c| c == '#').count();
    if niveau == 0 || niveau > 6 {
        return None;
    }
    let reste = &trim[niveau..];
    if !reste.starts_with(' ') {
        return None;
    }
    Some((niveau, reste.trim_start()))
}

/// Détecte un préfixe de bloc (tâche, liste, citation).
///
/// Renvoie le reste à découper en inline et le style du marqueur.
fn parse_prefixe_bloc(line: &str) -> Option<(&str, SegmentStyle)> {
    let trim = line.trim_start();

    if let Some((reste, style)) = parse_tache(trim) {
        return Some((reste, style));
    }

    if let Some(reste) = parse_puce(trim) {
        return Some((reste, SegmentStyle::ListBullet));
    }

    if let Some(reste) = parse_liste_numerotee(trim) {
        return Some((reste, SegmentStyle::ListNumbered));
    }

    if let Some(reste) = trim.strip_prefix('>') {
        return Some((reste, SegmentStyle::Blockquote));
    }

    // Définition de note de bas de page : `[^id]: texte`.
    if let Some(reste) = trim.strip_prefix("[^") {
        if let Some(fin) = reste.find(']') {
            let id = &reste[..fin];
            let after = reste[fin + 1..].trim_start();
            if !id.is_empty() && after.starts_with(':') {
                return Some((after[1..].trim_start(), SegmentStyle::Footnote));
            }
        }
    }

    None
}

/// Détecte une tâche `- [ ] x` / `- [x] x` (aussi `*` et `+`).
fn parse_tache(trim: &str) -> Option<(&str, SegmentStyle)> {
    let apres_puce = strip_puce(trim)?;
    let apres = apres_puce.trim_start();
    if !apres.starts_with('[') {
        return None;
    }

    if let Some(reste) = apres.strip_prefix("[ ]") {
        if reste.is_empty() || reste.starts_with(char::is_whitespace) {
            return Some((reste.trim_start(), SegmentStyle::TaskCheckboxTodo));
        }
    }
    if let Some(reste) = apres
        .strip_prefix("[x]")
        .or_else(|| apres.strip_prefix("[X]"))
    {
        if reste.is_empty() || reste.starts_with(char::is_whitespace) {
            return Some((reste.trim_start(), SegmentStyle::TaskCheckboxDone));
        }
    }
    None
}

/// Détecte une puce de liste non ordonnée (`-`, `*` ou `+`).
fn parse_puce(trim: &str) -> Option<&str> {
    let apres = strip_puce(trim)?;
    if apres.is_empty() || apres.starts_with(char::is_whitespace) {
        Some(apres)
    } else {
        None
    }
}

/// Retire une puce `-`, `*` ou `+` en tête d'une ligne déjà trimée.
fn strip_puce(trim: &str) -> Option<&str> {
    let apres = trim
        .strip_prefix('-')
        .or_else(|| trim.strip_prefix('*'))
        .or_else(|| trim.strip_prefix('+'))?;
    // `- texte` ok, mais pas `-- titre` ni `-1. negatif`.
    if apres.starts_with('-') || apres.starts_with('*') || apres.starts_with('+') {
        return None;
    }
    Some(apres)
}

/// Détecte une liste numérotée `1. x` (un ou plusieurs chiffres).
fn parse_liste_numerotee(trim: &str) -> Option<&str> {
    let chiffres = trim.chars().take_while(|c| c.is_ascii_digit()).count();
    if chiffres == 0 {
        return None;
    }
    let apres = &trim[chiffres..];
    if !apres.starts_with('.') {
        return None;
    }
    let corps = &apres[1..];
    if corps.is_empty() || corps.starts_with(char::is_whitespace) {
        Some(corps)
    } else {
        None
    }
}
/// Liste blanche des balises inline autorisées (fermeture incluse).
const SAFE_INLINE_TAGS: &[&str] = &[
    "b", "/b", "strong", "/strong", "i", "/i", "em", "/em", "br", "/br", "code", "/code", "u",
    "/u", "mark", "/mark", "s", "/s",
];

/// Nettoie le HTML inline : retire uniquement les balises de la liste
/// blanche, laisse tout le reste intact (aucune exécution, aucun autre tag).
fn sanitize_inline_html(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    while let Some(lt) = rest.find('<') {
        out.push_str(&rest[..lt]);
        let after = &rest[lt + 1..];
        let Some(gt) = after.find('>') else {
            out.push('<');
            out.push_str(after);
            return out;
        };
        let tag = after[..gt].trim().to_lowercase();
        if SAFE_INLINE_TAGS.contains(&tag.as_str()) {
            rest = &after[gt + 1..];
        } else {
            out.push('<');
            out.push_str(after);
            return out;
        }
    }
    out.push_str(rest);
    out
}

/// Remplace les emoji `:nom:` courants par leur glyphe Unicode.
fn expand_emoji(line: &str) -> String {
    const EMOJIS: &[(&str, &str)] = &[
        (":smile:", "😊"),
        (":grin:", "😁"),
        (":thumbsup:", "👍"),
        (":+1:", "👍"),
        (":warning:", "⚠️"),
        (":x:", "❌"),
        (":check:", "✅"),
        (":star:", "⭐"),
        (":heart:", "❤️"),
        (":fire:", "🔥"),
        (":bulb:", "💡"),
        (":rocket:", "🚀"),
        (":eyes:", "👀"),
        (":clipboard:", "📋"),
        (":calendar:", "📅"),
        (":hourglass:", "⏳"),
    ];
    let mut out = line.to_string();
    for (name, glyph) in EMOJIS {
        out = out.replace(name, glyph);
    }
    out
}
/// Découpe le contenu inline d'une ligne en segments.
fn push_inline(ligne: &str, line_no: usize, out: &mut Vec<Segment>) {
    let chars: Vec<char> = ligne.chars().collect();
    let mut i: usize = 0;
    let mut tampon = String::new();

    while i < chars.len() {
        let c = chars[i];

        // Code inline : prioritaire, aucun marqueur dedans.
        if c == '`' {
            if let Some(fin) = position(chars.as_slice(), i + 1, '`') {
                vider_tampon(&mut tampon, line_no, out);
                push_texte(
                    SegmentStyle::InlineCode,
                    &chars[i + 1..fin].iter().collect::<String>(),
                    line_no,
                    out,
                );
                i = fin + 1;
                continue;
            }
        }

        // Note de bas de page : `[^id]` (référence) ou `[^id]: texte`
        // (définition). Le contenu affiché est `[^id]` (sans `:`).
        if c == '[' && chars.get(i + 1) == Some(&'^') {
            if let Some(fin) = position(chars.as_slice(), i + 2, ']') {
                let id: String = chars[i + 2..fin].iter().collect();
                if !id.is_empty() {
                    vider_tampon(&mut tampon, line_no, out);
                    push_texte(SegmentStyle::Footnote, &format!("[^{id}]"), line_no, out);
                    i = fin + 1;
                    continue;
                }
            }
        }

        // Math inline : `$...$` sur la même ligne. `$$` ne déclenche pas
        // l'inline (display géré au niveau de la plage) ; un `$` non fermé
        // reste littéral.
        if c == '$' && chars.get(i + 1) != Some(&'$') {
            if let Some(fin) = position(chars.as_slice(), i + 1, '$') {
                vider_tampon(&mut tampon, line_no, out);
                push_texte(
                    SegmentStyle::MathInline,
                    &chars[i + 1..fin].iter().collect::<String>(),
                    line_no,
                    out,
                );
                i = fin + 1;
                continue;
            }
        }

        // Gras / italique : `***`, `**` ou `*`.
        if c == '*' {
            if let Some((style, fin, texte)) = parse_emphase(chars.as_slice(), i) {
                vider_tampon(&mut tampon, line_no, out);
                push_texte(style, &texte, line_no, out);
                i = fin;
                continue;
            }
        }

        // Barré : `~~x~~`.
        if c == '~' && chars.get(i + 1) == Some(&'~') {
            if let Some(fin) = position_double(chars.as_slice(), i + 2, '~') {
                vider_tampon(&mut tampon, line_no, out);
                push_texte(
                    SegmentStyle::Strikethrough,
                    &chars[i + 2..fin].iter().collect::<String>(),
                    line_no,
                    out,
                );
                i = fin + 2;
                continue;
            }
        }

        // Lien markdown `[texte](url)`. Non fermé -> tampon (Plain).
        if c == '[' && chars.get(i + 1) != Some(&'[') {
            if let Some((texte, url, fin)) = parse_lien(chars.as_slice(), i) {
                vider_tampon(&mut tampon, line_no, out);
                if url.starts_with("http://") || url.starts_with("https://") {
                    push_texte(SegmentStyle::ExternalUrl, &url, line_no, out);
                } else {
                    push_texte(SegmentStyle::Link, &texte, line_no, out);
                }
                i = fin;
                continue;
            }
        }

        // URL nue.
        if c == 'h' {
            if let Some(fin) = parse_url_nue(chars.as_slice(), i) {
                vider_tampon(&mut tampon, line_no, out);
                push_texte(
                    SegmentStyle::ExternalUrl,
                    &chars[i..fin].iter().collect::<String>(),
                    line_no,
                    out,
                );
                i = fin;
                continue;
            }
        }

        // Embed `![[cible#section|alias]]` : image, pdf, note ou autre.
        if c == '!' && chars.get(i + 1) == Some(&'[') && chars.get(i + 2) == Some(&'[') {
            if let Some((style, texte, fin)) = parse_embed_inline(chars.as_slice(), i) {
                vider_tampon(&mut tampon, line_no, out);
                push_texte(style, &texte, line_no, out);
                i = fin;
                continue;
            }
        }

        // Wikilink `[[cible]]` (alias `[[cible|alias]]` -> alias).
        if c == '[' && chars.get(i + 1) == Some(&'[') {
            if let Some((texte, fin)) = parse_wikilink(chars.as_slice(), i) {
                vider_tampon(&mut tampon, line_no, out);
                push_texte(SegmentStyle::Wikilink, &texte, line_no, out);
                i = fin;
                continue;
            }
        }

        // Tag `#tag` : seulement après un espace ou en début de ligne.
        if c == '#' && debut_de_tag(chars.as_slice(), i) {
            if let Some(fin) = fin_tag(chars.as_slice(), i + 1) {
                vider_tampon(&mut tampon, line_no, out);
                push_texte(
                    SegmentStyle::Tag,
                    &chars[i + 1..fin].iter().collect::<String>(),
                    line_no,
                    out,
                );
                i = fin;
                continue;
            }
        }

        tampon.push(c);
        i += 1;
    }

    vider_tampon(&mut tampon, line_no, out);
}

/// Vide le tampon dans les segments (style `Plain`) s'il est non vide.
fn vider_tampon(tampon: &mut String, line_no: usize, out: &mut Vec<Segment>) {
    if tampon.is_empty() {
        return;
    }
    let texte = std::mem::take(tampon);
    out.push(Segment {
        style: SegmentStyle::Plain,
        text: texte,
        line: line_no,
    });
}

/// Première position de `cible` à partir de `debut`.
fn position(chars: &[char], debut: usize, cible: char) -> Option<usize> {
    (debut..chars.len()).find(|&i| chars[i] == cible)
}

/// Première position d'une paire `cc` à partir de `debut`.
fn position_double(chars: &[char], debut: usize, cible: char) -> Option<usize> {
    (debut..chars.len()).find(|&i| chars[i] == cible && chars.get(i + 1) == Some(&cible))
}

/// Parse une emphase à la position `i` : style, position de fin et texte.
///
/// On préfère `***` à `**` et `**` à `*` ; chaque marqueur doit
/// être fermé par son exact équivalent.
fn parse_emphase(chars: &[char], i: usize) -> Option<(SegmentStyle, usize, String)> {
    let deux_etoiles = chars.get(i + 1) == Some(&'*');
    let trois_etoiles =
        deux_etoiles && chars.get(i + 2) == Some(&'*') && chars.get(i + 3) != Some(&'*');

    if trois_etoiles {
        if let Some(fin) = position_double(chars, i + 3, '*') {
            if chars.get(fin + 2) == Some(&'*') {
                let texte: String = chars[i + 3..fin].iter().collect();
                return Some((SegmentStyle::BoldItalic, fin + 3, texte));
            }
        }
    }

    if deux_etoiles && chars.get(i + 2) != Some(&'*') {
        if let Some(fin) = position_double(chars, i + 2, '*') {
            if chars.get(fin + 2) != Some(&'*') {
                let texte: String = chars[i + 2..fin].iter().collect();
                return Some((SegmentStyle::Bold, fin + 2, texte));
            }
        }
    }

    if !deux_etoiles {
        if let Some(fin) = position(chars, i + 1, '*') {
            let texte: String = chars[i + 1..fin].iter().collect();
            if !texte.trim().is_empty() {
                return Some((SegmentStyle::Italic, fin + 1, texte));
            }
        }
    }

    None
}

/// Parse un lien markdown `[texte](url)` à partir de `i`.
///
/// L'URL d'un lien `http(s)://` génère un segment `ExternalUrl`
/// (le segment `Link` est réservé aux liens internes du vault).
fn parse_lien(chars: &[char], i: usize) -> Option<(String, String, usize)> {
    let fin_texte = position(chars, i + 1, ']')?;
    let texte: String = chars[i + 1..fin_texte].iter().collect();

    let ouverture = position(chars, fin_texte + 1, '(')?;
    if ouverture != fin_texte + 1 {
        return None;
    }
    let fin_url = position(chars, ouverture + 1, ')')?;
    let url: String = chars[ouverture + 1..fin_url].iter().collect();

    Some((texte, url, fin_url + 1))
}

/// Parse une URL nue démarrée à `i` ; renvoie la position de fin.
///
/// L'URL s'étend jusqu'au premier caractère blanc ou la fin de la ligne.
fn parse_url_nue(chars: &[char], i: usize) -> Option<usize> {
    let reste: String = chars[i..].iter().collect();
    let prefixe = if reste.starts_with("https://") {
        "https://"
    } else if reste.starts_with("http://") {
        "http://"
    } else {
        return None;
    };
    let apres_schema = &reste[prefixe.len()..];
    let fin_url = apres_schema
        .find(char::is_whitespace)
        .unwrap_or(apres_schema.len());
    Some(i + prefixe.len() + fin_url)
}

/// Parse un wikilink `[[cible]]` à partir de `i` ; gère l'alias.
fn parse_wikilink(chars: &[char], i: usize) -> Option<(String, usize)> {
    let apres = &chars[i + 2..];
    let fin = position(apres, 0, ']')?;
    if apres.get(fin + 1) != Some(&']') {
        return None;
    }
    let brut: String = apres[..fin].iter().collect();
    let texte = match brut.split_once('|') {
        Some((_, alias)) => alias.to_string(),
        None => brut,
    };
    Some((texte, i + 2 + fin + 2))
}

/// Parse un embed `![[cible#section|alias]]` à partir de `i` (le `!` inclus).
///
/// Miroir de `embeds::parse_embed` : le texte du segment est la cible (sans
/// `![[ ]]`, sans section ni alias) et le style dépend de la nature de la
/// cible (`embed_kind`).
fn parse_embed_inline(chars: &[char], i: usize) -> Option<(SegmentStyle, String, usize)> {
    let apres = &chars[i + 3..];
    let fin = position(apres, 0, ']')?;
    if apres.get(fin + 1) != Some(&']') {
        return None;
    }
    let brut: String = apres[..fin].iter().collect();
    let target_part = brut.split_once('|').map(|(t, _)| t).unwrap_or(&brut);
    let cible = target_part
        .split_once('#')
        .map(|(t, _)| t)
        .unwrap_or(target_part)
        .trim();
    let style = match super::embeds::embed_kind(cible) {
        super::embeds::EmbedKind::Image => SegmentStyle::EmbedImage,
        super::embeds::EmbedKind::Pdf => SegmentStyle::EmbedPdf,
        super::embeds::EmbedKind::Note | super::embeds::EmbedKind::Other => SegmentStyle::Embed,
    };
    Some((style, cible.to_string(), i + 3 + fin + 2))
}

/// `true` si un `#` à la position `i` ouvre un tag.
///
/// Règle : le `#` doit être en début de ligne ou précédé d'un
/// espace, et être suivi d'un caractère de tag.
fn debut_de_tag(chars: &[char], i: usize) -> bool {
    let debut_ligne = i == 0;
    let apres_espace = i > 0 && chars[i - 1] == ' ';
    if !debut_ligne && !apres_espace {
        return false;
    }
    chars
        .get(i + 1)
        .filter(|c| est_caractere_de_tag(**c))
        .is_some()
}

/// Fin (exclue) du tag démarrant à `debut` (premier caractère après
/// le `#`), ou `None` si `debut` n'est pas un caractère de tag.
fn fin_tag(chars: &[char], debut: usize) -> Option<usize> {
    chars.get(debut).filter(|c| est_caractere_de_tag(**c))?;
    let fin = debut
        + chars
            .iter()
            .skip(debut)
            .take_while(|c| est_caractere_de_tag(**c))
            .count();
    Some(fin)
}

/// `true` si `c` peut faire partie d'un tag (même règle que `tags.rs`).
fn est_caractere_de_tag(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '-' || c == '/'
}
#[cfg(test)]
mod tests {
    use super::*;

    /// Construit un `Segment`.
    fn seg(style: SegmentStyle, text: &str, line: usize) -> Segment {
        Segment {
            style,
            text: text.to_string(),
            line,
        }
    }

    /// Construit le vecteur attendu depuis des triplets.
    fn segs(attendus: &[(&str, SegmentStyle, usize)]) -> Vec<Segment> {
        attendus.iter().map(|&(t, s, l)| seg(s, t, l)).collect()
    }

    #[test]
    fn test_texte_plain_simple() {
        assert_eq!(
            render_markdown("Bonjour le monde"),
            segs(&[("Bonjour le monde", SegmentStyle::Plain, 0)])
        );
    }

    #[test]
    fn test_titres_niveau_1_a_3() {
        assert_eq!(
            render_markdown("# Titre 1\n## Titre 2\n### Titre 3"),
            segs(&[
                ("Titre 1", SegmentStyle::H1, 0),
                ("Titre 2", SegmentStyle::H2, 1),
                ("Titre 3", SegmentStyle::H3, 2),
            ])
        );
    }

    #[test]
    fn test_titres_niveau_4_a_6() {
        assert_eq!(
            render_markdown("#### T4\n##### T5\n###### T6"),
            segs(&[
                ("T4", SegmentStyle::H4, 0),
                ("T5", SegmentStyle::H5, 1),
                ("T6", SegmentStyle::H6, 2),
            ])
        );
    }

    #[test]
    fn test_hash_colle_n_est_pas_un_titre() {
        // `###x` : pas d'espace après les `#`, donc pas un titre.
        assert_eq!(
            render_markdown("###x"),
            segs(&[("###x", SegmentStyle::Plain, 0)])
        );
    }

    #[test]
    fn test_gras() {
        assert_eq!(
            render_markdown("du **gras** ici"),
            segs(&[
                ("du ", SegmentStyle::Plain, 0),
                ("gras", SegmentStyle::Bold, 0),
                (" ici", SegmentStyle::Plain, 0),
            ])
        );
    }

    #[test]
    fn test_italique() {
        assert_eq!(
            render_markdown("de l'*italique* pure"),
            segs(&[
                ("de l'", SegmentStyle::Plain, 0),
                ("italique", SegmentStyle::Italic, 0),
                (" pure", SegmentStyle::Plain, 0),
            ])
        );
    }

    #[test]
    fn test_gras_italique_combines() {
        assert_eq!(
            render_markdown("***important***"),
            segs(&[("important", SegmentStyle::BoldItalic, 0)])
        );
    }

    #[test]
    fn test_barrer() {
        assert_eq!(
            render_markdown("texte ~~supprimé~~"),
            segs(&[
                ("texte ", SegmentStyle::Plain, 0),
                ("supprimé", SegmentStyle::Strikethrough, 0),
            ])
        );
    }

    #[test]
    fn test_marqueur_non_ferme_donne_plain() {
        // `~~jamais fermé` : pas de fermant, le texte reste Plain.
        assert_eq!(
            render_markdown("~~jamais fermé"),
            segs(&[("~~jamais fermé", SegmentStyle::Plain, 0)])
        );
    }

    #[test]
    fn test_code_inline() {
        assert_eq!(
            render_markdown("voici `du code` inline"),
            segs(&[
                ("voici ", SegmentStyle::Plain, 0),
                ("du code", SegmentStyle::InlineCode, 0),
                (" inline", SegmentStyle::Plain, 0),
            ])
        );
    }

    #[test]
    fn test_code_inline_segments_complets() {
        assert_eq!(
            render_markdown("a `b` c"),
            segs(&[
                ("a ", SegmentStyle::Plain, 0),
                ("b", SegmentStyle::InlineCode, 0),
                (" c", SegmentStyle::Plain, 0),
            ])
        );
    }

    #[test]
    fn test_bloc_code_multilignes() {
        let contenu = "```rust\nlet x = 1;\nlet y = 2;\n```\nsuite";
        assert_eq!(
            render_markdown(contenu),
            segs(&[
                ("let x = 1;\nlet y = 2;", SegmentStyle::CodeBlock, 0),
                ("suite", SegmentStyle::Plain, 4),
            ])
        );
    }

    #[test]
    fn test_bloc_code_jamais_ferme() {
        assert_eq!(
            render_markdown("```\ncorps\n"),
            segs(&[("corps", SegmentStyle::CodeBlock, 0)])
        );
    }

    #[test]
    fn test_marqueurs_absents_du_texte() {
        for s in &render_markdown("**a** *b* ~~c~~ `d`") {
            assert!(!s.text.contains('*'), " marqueur dans {}", s.text);
            assert!(!s.text.contains('~'), "marqueur dans {}", s.text);
            assert!(!s.text.contains('`'), "marqueur dans {}", s.text);
        }
    }

    #[test]
    fn test_citation() {
        assert_eq!(
            render_markdown("> une citation"),
            segs(&[("une citation", SegmentStyle::Blockquote, 0)])
        );
    }

    #[test]
    fn test_liste_a_puces() {
        assert_eq!(
            render_markdown("- premier\n- deuxième"),
            segs(&[
                ("premier", SegmentStyle::ListBullet, 0),
                ("deuxième", SegmentStyle::ListBullet, 1),
            ])
        );
    }

    #[test]
    fn test_liste_numerotee() {
        assert_eq!(
            render_markdown("1. un\n2. deux"),
            segs(&[
                ("un", SegmentStyle::ListNumbered, 0),
                ("deux", SegmentStyle::ListNumbered, 1),
            ])
        );
    }

    #[test]
    fn test_tache_non_faite() {
        assert_eq!(
            render_markdown("- [ ] à faire"),
            segs(&[("à faire", SegmentStyle::TaskCheckboxTodo, 0)])
        );
    }

    #[test]
    fn test_tache_faite() {
        assert_eq!(
            render_markdown("- [x] terminé"),
            segs(&[("terminé", SegmentStyle::TaskCheckboxDone, 0)])
        );
    }

    #[test]
    fn test_lien_markdown() {
        assert_eq!(
            render_markdown("[texte](https://example.com)"),
            segs(&[("https://example.com", SegmentStyle::ExternalUrl, 0)])
        );
    }

    #[test]
    fn test_lien_interne() {
        // Lien interne au vault : segment `Link`, texte = libellé.
        assert_eq!(
            render_markdown("[va voir](Autre%20note.md)"),
            segs(&[("va voir", SegmentStyle::Link, 0)])
        );
    }

    #[test]
    fn test_lien_non_ferme_donne_plain_et_url() {
        // Le lien n'est pas fermé : `[texte](` reste Plain, l'URL est
        // reconnue comme URL nue.
        assert_eq!(
            render_markdown("[texte](https://incomplet"),
            segs(&[
                ("[texte](", SegmentStyle::Plain, 0),
                ("https://incomplet", SegmentStyle::ExternalUrl, 0),
            ])
        );
    }

    #[test]
    fn test_url_nue() {
        assert_eq!(
            render_markdown("voir https://example.com/page"),
            segs(&[
                ("voir ", SegmentStyle::Plain, 0),
                ("https://example.com/page", SegmentStyle::ExternalUrl, 0),
            ])
        );
    }

    #[test]
    fn test_wikilink_avec_alias() {
        assert_eq!(
            render_markdown("[[cible|affichage]]"),
            segs(&[("affichage", SegmentStyle::Wikilink, 0)])
        );
    }

    #[test]
    fn test_wikilink_simple() {
        assert_eq!(
            render_markdown("[[Note cible]]"),
            segs(&[("Note cible", SegmentStyle::Wikilink, 0)])
        );
    }

    #[test]
    fn test_tag_apres_espace() {
        assert_eq!(
            render_markdown("texte #urgent"),
            segs(&[
                ("texte ", SegmentStyle::Plain, 0),
                ("urgent", SegmentStyle::Tag, 0),
            ])
        );
    }

    #[test]
    fn test_pas_de_tag_dans_un_mot() {
        // `#foo#bar` : le deuxième `#` n'est pas précédé d'un espace,
        // le tag vaut donc `foo` et `#bar` reste Plain.
        assert_eq!(
            render_markdown("mot #foo#bar"),
            segs(&[
                ("mot ", SegmentStyle::Plain, 0),
                ("foo", SegmentStyle::Tag, 0),
                ("#bar", SegmentStyle::Plain, 0),
            ])
        );
    }

    #[test]
    fn test_hash_seul_n_est_pas_un_tag() {
        assert_eq!(render_markdown("#"), segs(&[("#", SegmentStyle::Plain, 0)]));
        assert_eq!(
            render_markdown("texte # "),
            segs(&[("texte # ", SegmentStyle::Plain, 0)])
        );
    }

    #[test]
    fn test_regle_horizontale() {
        assert_eq!(
            render_markdown("avant\n---\naprès"),
            segs(&[
                ("avant", SegmentStyle::Plain, 0),
                ("", SegmentStyle::HorizontalRule, 1),
                ("après", SegmentStyle::Plain, 2),
            ])
        );
    }

    #[test]
    fn test_tableau_simple() {
        assert_eq!(
            render_markdown("| a | b |\n|---|---|\n| 1 | 2 |"),
            segs(&[
                ("a b", SegmentStyle::Table, 0),
                ("1 2", SegmentStyle::Table, 2), // séparateur ignoré
            ])
        );
    }

    #[test]
    fn test_ordre_des_segments() {
        let segments = render_markdown("# T\n**gras**\n- item\n`code`");
        let styles: Vec<SegmentStyle> = segments.iter().map(|s| s.style).collect();
        assert_eq!(
            styles,
            vec![
                SegmentStyle::H1,
                SegmentStyle::Bold,
                SegmentStyle::ListBullet,
                SegmentStyle::InlineCode,
            ]
        );
    }

    #[test]
    fn test_lignes_respectees_multilignes() {
        assert_eq!(
            render_markdown("a\n\nb\nc"),
            segs(&[
                ("a", SegmentStyle::Plain, 0),
                ("b", SegmentStyle::Plain, 2),
                ("c", SegmentStyle::Plain, 3),
            ])
        );
    }

    #[test]
    fn test_chaque_style_au_moins_une_fois() {
        let contenu = [
            "texte simple",
            "# Titre",
            "## Sous-titre",
            "### N3",
            "#### N4",
            "##### N5",
            "###### N6",
            "**gras**",
            "*italique*",
            "***les deux***",
            "~~barré~~",
            "`code`",
            "```",
            "bloc",
            "```",
            "> citation",
            "- puce",
            "1. numéro",
            "- [ ] todo",
            "- [x] fait",
            "[lien](page.md)",
            "https://nu.example.com",
            "[[wiki|alias]]",
            "#tag",
            "---",
            "| a | b |",
        ]
        .join("\n");
        let styles: Vec<SegmentStyle> = render_markdown(&contenu).iter().map(|s| s.style).collect();
        for attendu in [
            SegmentStyle::Plain,
            SegmentStyle::H1,
            SegmentStyle::H2,
            SegmentStyle::H3,
            SegmentStyle::H4,
            SegmentStyle::H5,
            SegmentStyle::H6,
            SegmentStyle::Bold,
            SegmentStyle::Italic,
            SegmentStyle::BoldItalic,
            SegmentStyle::Strikethrough,
            SegmentStyle::InlineCode,
            SegmentStyle::CodeBlock,
            SegmentStyle::Blockquote,
            SegmentStyle::Link,
            SegmentStyle::ExternalUrl,
            SegmentStyle::Wikilink,
            SegmentStyle::Tag,
            SegmentStyle::TaskCheckboxTodo,
            SegmentStyle::TaskCheckboxDone,
            SegmentStyle::ListBullet,
            SegmentStyle::ListNumbered,
            SegmentStyle::HorizontalRule,
            SegmentStyle::Table,
        ] {
            assert!(styles.contains(&attendu), "style manquant : {attendu:?}");
        }
    }

    #[test]
    fn test_callout_avec_titre() {
        assert_eq!(
            render_markdown("> [!note] Titre\n> corps"),
            segs(&[
                ("[!note] Titre", SegmentStyle::CalloutTitle, 0),
                ("corps", SegmentStyle::Callout, 1),
            ])
        );
    }

    #[test]
    fn test_callout_replie_avec_tiret() {
        assert_eq!(
            render_markdown("> [!todo]- Tâches\n> À faire"),
            segs(&[
                ("[!todo]- Tâches", SegmentStyle::CalloutTitle, 0),
                ("À faire", SegmentStyle::Callout, 1),
            ])
        );
    }

    #[test]
    fn test_callout_multilignes_sans_titre() {
        assert_eq!(
            render_markdown("> [!info]\n> Un\n> Deux\n> Trois"),
            segs(&[
                ("[!info]", SegmentStyle::Callout, 0),
                ("Un", SegmentStyle::Callout, 1),
                ("Deux", SegmentStyle::Callout, 2),
                ("Trois", SegmentStyle::Callout, 3),
            ])
        );
    }

    #[test]
    fn test_callouts_adjacents_separes_par_ligne_vide() {
        assert_eq!(
            render_markdown("> [!note]\n> un\n\n> [!tip]\n> deux"),
            segs(&[
                ("[!note]", SegmentStyle::Callout, 0),
                ("un", SegmentStyle::Callout, 1),
                ("[!tip]", SegmentStyle::Callout, 3),
                ("deux", SegmentStyle::Callout, 4),
            ])
        );
    }

    #[test]
    fn test_callout_pas_rendu_en_blockquote() {
        let segments = render_markdown("> [!warning] Attention\n> danger");
        assert!(
            segments.iter().all(|s| s.style != SegmentStyle::Blockquote),
            "un callout ne doit pas produire de segment Blockquote"
        );
    }

    #[test]
    fn test_math_inline() {
        assert_eq!(
            render_markdown("du $x^2$ et $y$"),
            segs(&[
                ("du ", SegmentStyle::Plain, 0),
                ("x^2", SegmentStyle::MathInline, 0),
                (" et ", SegmentStyle::Plain, 0),
                ("y", SegmentStyle::MathInline, 0),
            ])
        );
    }

    #[test]
    fn test_math_inline_non_ferme_laisse_litteral() {
        assert_eq!(
            render_markdown("prix $100"),
            segs(&[("prix $100", SegmentStyle::Plain, 0)])
        );
    }

    #[test]
    fn test_math_display_une_ligne() {
        assert_eq!(
            render_markdown("avant\n$$E=mc^2$$\naprès"),
            segs(&[
                ("avant", SegmentStyle::Plain, 0),
                ("E=mc^2", SegmentStyle::MathDisplay, 1),
                ("après", SegmentStyle::Plain, 2),
            ])
        );
    }

    #[test]
    fn test_math_display_multilignes() {
        assert_eq!(
            render_markdown("avant\n$$\na+b\n$$\naprès"),
            segs(&[
                ("avant", SegmentStyle::Plain, 0),
                ("\na+b\n", SegmentStyle::MathDisplay, 1),
                ("après", SegmentStyle::Plain, 4),
            ])
        );
    }

    #[test]
    fn test_embed_note() {
        assert_eq!(
            render_markdown("voir ![[my note]] ici"),
            segs(&[
                ("voir ", SegmentStyle::Plain, 0),
                ("my note", SegmentStyle::Embed, 0),
                (" ici", SegmentStyle::Plain, 0),
            ])
        );
    }

    #[test]
    fn test_embed_note_avec_alias_et_section() {
        // Le texte du segment reste la cible (pas l'alias ni la section).
        assert_eq!(
            render_markdown("![[note#section|alias]]"),
            segs(&[("note", SegmentStyle::Embed, 0)])
        );
    }

    #[test]
    fn test_embed_image() {
        assert_eq!(
            render_markdown("![[photo.png]]"),
            segs(&[("photo.png", SegmentStyle::EmbedImage, 0)])
        );
    }

    #[test]
    fn test_embed_pdf() {
        assert_eq!(
            render_markdown("![[rapport.pdf]]"),
            segs(&[("rapport.pdf", SegmentStyle::EmbedPdf, 0)])
        );
    }

    #[test]
    fn test_embed_et_wikilink_dans_la_meme_ligne() {
        assert_eq!(
            render_markdown("avant ![[img.PNG]] après [[wiki]]"),
            segs(&[
                ("avant ", SegmentStyle::Plain, 0),
                ("img.PNG", SegmentStyle::EmbedImage, 0),
                (" après ", SegmentStyle::Plain, 0),
                ("wiki", SegmentStyle::Wikilink, 0),
            ])
        );
    }

    #[test]
    fn test_bloc_code_ignore_callouts_math_embeds() {
        let contenu = "```\n> [!note] faux\n$x$\n![[img.png]]\n```";
        assert_eq!(
            render_markdown(contenu),
            segs(&[(
                "> [!note] faux\n$x$\n![[img.png]]",
                SegmentStyle::CodeBlock,
                0,
            )])
        );
    }

    #[test]
    fn html_inline_balises_autorisees_retirees() {
        assert_eq!(
            render_markdown("<b>gras</b> <i>italique</i>"),
            segs(&[("gras italique", SegmentStyle::Plain, 0)])
        );
    }

    #[test]
    fn html_inline_balise_inconnue_conservee() {
        assert_eq!(
            render_markdown("<script>alert(1)</script>"),
            segs(&[("<script>alert(1)</script>", SegmentStyle::Plain, 0)])
        );
    }

    #[test]
    fn emoji_etendus_en_glyphes() {
        assert_eq!(
            render_markdown("réussi :check: !"),
            segs(&[("réussi ✅ !", SegmentStyle::Plain, 0)])
        );
    }

    #[test]
    fn ligne_separation_tableau_ignoree() {
        let segs = render_markdown("| a | b |\n| --- | --- |\n| 1 | 2 |");
        assert_eq!(
            segs,
            [
                ("a b", SegmentStyle::Table, 0),
                ("1 2", SegmentStyle::Table, 2)
            ]
            .iter()
            .map(|(t, s, l)| Segment {
                style: *s,
                text: (*t).to_string(),
                line: *l,
            })
            .collect::<Vec<_>>()
        );
    }

    #[test]
    fn definition_footnote_rendue_en_segment_footnote() {
        let segs = render_markdown("[^1] : texte de note");
        assert_eq!(segs.len(), 1);
        assert_eq!(segs[0].style, SegmentStyle::Footnote);
        assert_eq!(segs[0].text, "texte de note");
    }

    #[test]
    fn editeur_saut_de_ligne_et_selection() {
        use iced::widget::text_editor;
        let mut content: text_editor::Content<iced::Renderer> =
            text_editor::Content::with_text("ligne1");
        content.perform(text_editor::Action::Edit(text_editor::Edit::Enter));
        content.perform(text_editor::Action::SelectAll);
        assert!(content.text().contains('\n'));
    }

    #[test]
    fn ligne_avec_shortcode_non_separateur() {
        let segs = render_markdown("| :note: | x |");
        assert_eq!(segs.len(), 1);
        assert_eq!(segs[0].style, SegmentStyle::Table);
    }
}
