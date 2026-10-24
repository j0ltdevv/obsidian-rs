//! Math : extraction des segments `$...$` (inline) et `$$...$$` (display).

/// Type de segment mathématique.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MathKind {
    /// `$...$` : sur une seule ligne.
    Inline,
    /// `$$...$$` : peut s'étendre sur plusieurs lignes.
    Display,
}

/// Un segment mathématique.
#[derive(Debug, Clone, PartialEq)]
pub struct MathSegment {
    /// Type (inline / display).
    pub kind: MathKind,
    /// Source brute entre les délimiteurs (sans les `$`).
    pub source: String,
    /// Première ligne (0-based).
    pub start_line: usize,
    /// Dernière ligne incluse (0-based).
    pub end_line: usize,
}

/// Extrait les segments mathématiques d'un contenu, en ignorant les blocs de
/// code (fences ``` et ~~~).
///
/// Règles : `$$` gagnent sur `$` ; un délimiteur non fermé est ignoré ; un
/// contenu vide (`$$$$`, `$$`) est ignoré ; l'inline ne traverse pas les
/// lignes.
pub fn extract_math(content: &str) -> Vec<MathSegment> {
    let mut out = Vec::new();
    let mut in_fence: Option<u8> = None;
    let mut i = 0;
    let lines: Vec<&str> = content.lines().collect();
    let mut line_no = 0usize;

    while i < lines.len() {
        let line = lines[i];
        if let Some(fence) = in_fence {
            if is_fence(line, fence) {
                in_fence = None;
            }
            i += 1;
            continue;
        }
        if let Some(fence) = fence_char(line) {
            in_fence = Some(fence);
            i += 1;
            continue;
        }

        // Display : cherche `$$` sur cette ligne.
        if let Some(open) = line.find("$$") {
            let rest = &line[open + 2..];
            if let Some(close) = rest.find("$$") {
                let source = rest[..close].to_string();
                if !source.trim().is_empty() {
                    out.push(MathSegment {
                        kind: MathKind::Display,
                        source,
                        start_line: line_no,
                        end_line: line_no,
                    });
                }
                i += 1;
                continue;
            }
            // `$$` non fermé sur la même ligne : cherche sur les suivantes.
            let mut source = rest.to_string();
            let mut end = line_no;
            let mut closed = false;
            let mut j = i + 1;
            while j < lines.len() {
                if let Some(close) = lines[j].find("$$") {
                    source.push('\n');
                    source.push_str(&lines[j][..close]);
                    end = j;
                    closed = true;
                    break;
                }
                source.push('\n');
                source.push_str(lines[j]);
                end = j;
                j += 1;
            }
            if closed && !source.trim().is_empty() {
                out.push(MathSegment {
                    kind: MathKind::Display,
                    source,
                    start_line: line_no,
                    end_line: end,
                });
            }
            i = end + 1;
            line_no = end + 1;
            continue;
        }

        // Inline : `$...$` sur la même ligne, hors `$$`.
        let bytes = line.as_bytes();
        let mut pos = 0;
        while pos < bytes.len() {
            if bytes[pos] == b'$' {
                // Vérifie que ce n'est pas le début d'un `$$` (déjà traité).
                if pos + 1 < bytes.len() && bytes[pos + 1] == b'$' {
                    pos += 2;
                    continue;
                }
                if let Some(rel_close) = line[pos + 1..].find('$') {
                    let source = line[pos + 1..pos + 1 + rel_close].to_string();
                    if !source.trim().is_empty() {
                        out.push(MathSegment {
                            kind: MathKind::Inline,
                            source,
                            start_line: line_no,
                            end_line: line_no,
                        });
                    }
                    pos += 2 + rel_close;
                    continue;
                }
            }
            pos += 1;
        }

        i += 1;
        line_no += 1;
    }

    out
}

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

fn is_fence(line: &str, fence: u8) -> bool {
    let trimmed = line.trim_start();
    let marker = if fence == b'`' { "```" } else { "~~~" };
    trimmed.starts_with(marker)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inline_simple() {
        let segs = extract_math("avant $x+1$ après");
        assert_eq!(segs.len(), 1);
        assert_eq!(segs[0].kind, MathKind::Inline);
        assert_eq!(segs[0].source, "x+1");
        assert_eq!(segs[0].start_line, 0);
    }

    #[test]
    fn display_mono_ligne() {
        let segs = extract_math("$$E=mc^2$$");
        assert_eq!(segs.len(), 1);
        assert_eq!(segs[0].kind, MathKind::Display);
        assert_eq!(segs[0].source, "E=mc^2");
    }

    #[test]
    fn display_multi_lignes() {
        let segs = extract_math("$$\na+b\nc$$");
        assert_eq!(segs.len(), 1);
        assert_eq!(segs[0].source, "\na+b\nc");
        assert_eq!(segs[0].start_line, 0);
        assert_eq!(segs[0].end_line, 2);
    }

    #[test]
    fn dollars_doubles_prioritaires() {
        let segs = extract_math("$$ x $$");
        assert_eq!(segs.len(), 1);
        assert_eq!(segs[0].kind, MathKind::Display);
    }

    #[test]
    fn delimiteur_non_ferme_ignore() {
        assert!(extract_math("$pas fermé").is_empty());
        assert!(extract_math("$$pas fermé").is_empty());
    }

    #[test]
    fn contenu_vide_ignore() {
        assert!(extract_math("$$$$").is_empty());
        assert!(extract_math("$$").is_empty());
    }

    #[test]
    fn code_fence_ignore() {
        assert!(extract_math("```\n$x$\n```").is_empty());
    }

    #[test]
    fn segments_successifs() {
        let segs = extract_math("$a$ puis $b$");
        assert_eq!(segs.len(), 2);
        assert_eq!(segs[0].source, "a");
        assert_eq!(segs[1].source, "b");
    }

    #[test]
    fn positions_de_lignes_trackees() {
        let segs = extract_math("ligne1\n$$d$$\nligne3");
        assert_eq!(segs[0].kind, MathKind::Display);
        assert_eq!(segs[0].start_line, 1);
        assert_eq!(segs[0].end_line, 1);
    }
}

/* ======================= Conversion LaTeX -> Unicode ======================= */

/// Convertit un extrait LaTeX simple en texte Unicode lisible.
///
/// Couverture volontairement pragmatique (ce qu'un appel rend sur une ligne
/// de note) : lettres grecques, opérateurs et relations courants, flèches,
/// ensembles `\mathbb`, exposants et indices `^`/`_` (accolades ou caractère
/// unique), fractions `\frac{a}{b}`, racines `\sqrt`, `\text{…}`, espaces
/// fines. Un `\commande` inconnue abandonne la barre et garde le nom : aucune
/// perte de texte, jamais de panique.
pub fn latex_to_unicode(source: &str) -> String {
    let chars: Vec<char> = source.chars().collect();
    let mut out = String::with_capacity(source.len() + 16);
    let mut i = 0usize;

    while i < chars.len() {
        let c = chars[i];
        if c != '\\' {
            out.push(c);
            i += 1;
            continue;
        }
        // Une commande : \lettre+ ou \symbole unique (\\, \{, \}…).
        let start = i + 1;
        let mut end = start;
        while end < chars.len() && chars[end].is_alphabetic() {
            end += 1;
        }
        if end == start && start < chars.len() {
            // Symbole échappé unique : `\{`, `\}`, `\\` ; et les espaces
            // fines `\,` `\;` `\:` `\!`.
            let echappe = chars[start];
            if matches!(echappe, ',' | ';' | ':' | '!') {
                out.push(' ');
            } else {
                out.push(echappe);
            }
            i = start + 1;
            continue;
        }
        let commande: String = chars[start..end].iter().collect();
        i = end;
        match commande.as_str() {
            "frac" => {
                let numé = lire_groupe(&chars, &mut i);
                let déno = lire_groupe(&chars, &mut i);
                match (&numé, &déno) {
                    (Some(n), Some(d)) if n.chars().count() == 1 && d.chars().count() == 1 => {
                        out.push_str(n);
                        out.push('\u{2044}');
                        out.push_str(d);
                    }
                    (Some(n), Some(d)) => {
                        out.push('(');
                        out.push_str(n);
                        out.push_str(")/(");
                        out.push_str(d);
                        out.push(')');
                    }
                    _ => out.push_str("\\frac"),
                }
            }
            "sqrt" => {
                // \sqrt[3]{x} : indice de racine optionnel.
                let mut indice = String::new();
                if i < chars.len() && chars[i] == '[' {
                    i += 1;
                    while i < chars.len() && chars[i] != ']' {
                        out.push('\0'); // marqueur temporaire inutile
                        i += 1;
                    }
                    i = (i + 1).min(chars.len());
                }
                let _ = &mut indice;
                let contenu = lire_groupe(&chars, &mut i);
                match contenu {
                    Some(texte) => {
                        out.push('√');
                        out.push('(');
                        out.push_str(&texte);
                        out.push(')');
                    }
                    None => out.push_str("\\sqrt"),
                }
            }
            "mathbb" => match lire_groupe(&chars, &mut i) {
                Some(ensemble) => {
                    let double = match ensemble.trim() {
                        "R" => "\u{211D}",
                        "N" => "\u{2115}",
                        "Z" => "\u{2124}",
                        "Q" => "\u{211A}",
                        "C" => "\u{2102}",
                        "H" => "\u{210D}",
                        _ => "",
                    };
                    if double.is_empty() {
                        out.push_str(&ensemble);
                    } else {
                        out.push_str(double);
                    }
                }
                None => out.push_str("\\mathbb"),
            },
            "text" | "mathrm" | "operatorname" => match lire_groupe(&chars, &mut i) {
                Some(texte) => out.push_str(&texte),
                None => out.push('\\'),
            },
            "left" | "right" | "big" | "Big" | "bigl" | "bigr" => {
                // Les délimiteurs suivent directement : on ne fait rien,
                // le caractère suivant est copié tel quel.
            }
            "quad" => out.push_str("  "),
            "qquad" => out.push_str("    "),
            "," | ";" | ":" | "!" => out.push(' '),
            "ldots" | "dots" | "dotsc" | "dotsb" => out.push('…'),
            "cdots" | "cdotp" => out.push('⋯'),
            "prime" => out.push('′'),
            autre => {
                if let Some(symbole) = symbole(&autre) {
                    out.push(symbole);
                } else {
                    // Commande inconnue : garder le nom sans la barre.
                    out.push_str(&commande);
                }
            }
        }
    }

    // Exposants et indices, sur la sortie (les groupes `^{…}` sont déjà
    // des suites de caractères ordinaires).
    scripter(&out)
}

/// Lit un groupe `{…}` à la position `i` (avancée après l'accolade fermante).
/// Un caractère unique sans accolades est aussi accepté.
fn lire_groupe(chars: &[char], i: &mut usize) -> Option<String> {
    while *i < chars.len() && chars[*i].is_whitespace() {
        *i += 1;
    }
    if *i >= chars.len() {
        return None;
    }
    if chars[*i] == '{' {
        *i += 1;
        let mut profondeur = 1usize;
        let mut groupe = String::new();
        while *i < chars.len() {
            let c = chars[*i];
            if c == '{' {
                profondeur += 1;
            } else if c == '}' {
                profondeur -= 1;
                if profondeur == 0 {
                    *i += 1;
                    return Some(groupe);
                }
            }
            groupe.push(c);
            *i += 1;
        }
        return Some(groupe);
    }
    if chars[*i] == '\\' {
        // Groupe d'une commande : la laisser telle quelle.
        let début = *i;
        *i += 1;
        while *i < chars.len() && chars[*i].is_alphabetic() {
            *i += 1;
        }
        if *i == début + 1 && *i < chars.len() {
            *i += 1;
        }
        return Some(chars[début..*i].iter().collect());
    }
    let c = chars[*i];
    *i += 1;
    Some(c.to_string())
}

/// Table des symboles LaTeX connus.
fn symbole(nom: &str) -> Option<char> {
    let s = match nom {
        "alpha" => 'α',
        "beta" => 'β',
        "gamma" => 'γ',
        "delta" => 'δ',
        "epsilon" | "varepsilon" => 'ε',
        "zeta" => 'ζ',
        "eta" => 'η',
        "theta" => 'θ',
        "iota" => 'ι',
        "kappa" => 'κ',
        "lambda" => 'λ',
        "mu" => 'μ',
        "nu" => 'ν',
        "xi" => 'ξ',
        "pi" => 'π',
        "rho" => 'ρ',
        "sigma" => 'σ',
        "tau" => 'τ',
        "upsilon" => 'υ',
        "phi" | "varphi" => 'φ',
        "chi" => 'χ',
        "psi" => 'ψ',
        "omega" => 'ω',
        "Gamma" => 'Γ',
        "Delta" => 'Δ',
        "Theta" => 'Θ',
        "Lambda" => 'Λ',
        "Xi" => 'Ξ',
        "Pi" => 'Π',
        "Sigma" => 'Σ',
        "Phi" => 'Φ',
        "Psi" => 'Ψ',
        "Omega" => 'Ω',
        "sum" => '∑',
        "prod" => '∏',
        "int" => '∫',
        "iint" => '∬',
        "iiint" => '∭',
        "oint" => '∮',
        "partial" => '∂',
        "nabla" => '∇',
        "infty" => '∞',
        "pm" => '±',
        "mp" => '∓',
        "times" => '×',
        "cdot" => '·',
        "div" => '÷',
        "ast" | "star" => '∗',
        "circ" => '∘',
        "bullet" => '•',
        "oplus" => '⊕',
        "otimes" => '⊗',
        "leq" | "le" => '≤',
        "geq" | "ge" => '≥',
        "neq" | "ne" => '≠',
        "approx" => '≈',
        "equiv" => '≡',
        "sim" => '∼',
        "propto" => '∝',
        "ll" => '≪',
        "gg" => '≫',
        "to" | "rightarrow" => '→',
        "leftarrow" | "gets" => '←',
        "leftrightarrow" => '↔',
        "Rightarrow" => '⇒',
        "Leftarrow" => '⇐',
        "Leftrightarrow" => '⇔',
        "mapsto" => '↦',
        "uparrow" => '↑',
        "downarrow" => '↓',
        "in" => '∈',
        "notin" => '∉',
        "ni" => '∋',
        "subset" => '⊂',
        "subseteq" => '⊆',
        "supset" => '⊃',
        "supseteq" => '⊇',
        "cup" => '∪',
        "cap" => '∩',
        "emptyset" | "varnothing" => '∅',
        "forall" => '∀',
        "exists" => '∃',
        "nexists" => '∄',
        "land" | "wedge" => '∧',
        "lor" | "vee" => '∨',
        "neg" | "lnot" => '¬',
        "therefore" => '∴',
        "because" => '∵',
        "perp" => '⊥',
        "parallel" => '∥',
        "angle" => '∠',
        "degree" => '°',
        "hbar" => 'ℏ',
        "ell" => 'ℓ',
        "aleph" => 'ℵ',
        "wp" => '℘',
        "Re" => 'ℜ',
        "Im" => 'ℑ',
        "square" | "Box" => '□',
        "blacksquare" => '■',
        "triangle" => '△',
        "checkmark" => '✓',
        "dagger" => '†',
        "ddagger" => '‡',
        "S" => '§',
        "P" => '¶',
        "copyright" => '©',
        "nbsp" => '\u{00A0}',
        _ => return None,
    };
    Some(s)
}

/// Convertit les suites `^…` et `_…` en exposants/indices Unicode sur un
/// texte déjà débarrassé des commandes. Caractères non disponibles :
/// recopiés tels quels (pas de perte).
fn scripter(texte: &str) -> String {
    let chars: Vec<char> = texte.chars().collect();
    let mut out = String::with_capacity(texte.len());
    let mut i = 0usize;
    while i < chars.len() {
        let c = chars[i];
        if (c == '^' || c == '_') && i + 1 < chars.len() {
            let (contenu, suivant) = if chars[i + 1] == '{' {
                let mut fin = i + 2;
                let mut groupe = String::new();
                while fin < chars.len() && chars[fin] != '}' {
                    groupe.push(chars[fin]);
                    fin += 1;
                }
                (groupe, fin + 1)
            } else {
                (chars[i + 1].to_string(), i + 2)
            };
            let rendu: String = contenu
                .chars()
                .map(|ch| if c == '^' { exposant(ch) } else { indice(ch) })
                .collect();
            if rendu.contains('\u{FFFD}') || rendu == contenu && contenu.chars().count() == 1 {
                // Pas de glyphe disponible : conserver la notation brute.
                out.push(c);
                out.push_str(&contenu);
            } else {
                out.push_str(&rendu);
            }
            i = suivant;
            continue;
        }
        out.push(c);
        i += 1;
    }
    out
}

/// Exposant Unicode d'un caractère (repli : le caractère lui-même).
fn exposant(c: char) -> char {
    match c {
        '0' => '⁰',
        '1' => '¹',
        '2' => '²',
        '3' => '³',
        '4' => '⁴',
        '5' => '⁵',
        '6' => '⁶',
        '7' => '⁷',
        '8' => '⁸',
        '9' => '⁹',
        '+' => '⁺',
        '-' => '⁻',
        '=' => '⁼',
        '(' => '⁽',
        ')' => '⁾',
        'n' => 'ⁿ',
        'i' => 'ⁱ',
        'a' => 'ᵃ',
        'b' => 'ᵇ',
        'c' => 'ᶜ',
        'd' => 'ᵈ',
        'e' => 'ᵉ',
        'f' => 'ᶠ',
        'g' => 'ᵍ',
        'h' => 'ʰ',
        'j' => 'ʲ',
        'k' => 'ᵏ',
        'l' => 'ˡ',
        'm' => 'ᵐ',
        'o' => 'ᵒ',
        'p' => 'ᵖ',
        'r' => 'ʳ',
        's' => 'ˢ',
        't' => 'ᵗ',
        'u' => 'ᵘ',
        'v' => 'ᵛ',
        'w' => 'ʷ',
        'x' => 'ˣ',
        'y' => 'ʸ',
        'z' => 'ᶻ',
        'T' => 'ᵀ',
        autre => autre,
    }
}

/// Indice Unicode d'un caractère (repli : le caractère lui-même).
fn indice(c: char) -> char {
    match c {
        '0' => '₀',
        '1' => '₁',
        '2' => '₂',
        '3' => '₃',
        '4' => '₄',
        '5' => '₅',
        '6' => '₆',
        '7' => '₇',
        '8' => '₈',
        '9' => '₉',
        '+' => '₊',
        '-' => '₋',
        '=' => '₌',
        '(' => '₍',
        ')' => '₎',
        'a' => 'ₐ',
        'e' => 'ₑ',
        'h' => 'ₕ',
        'i' => 'ᵢ',
        'j' => 'ⱼ',
        'k' => 'ₖ',
        'l' => 'ₗ',
        'm' => 'ₘ',
        'n' => 'ₙ',
        'o' => 'ₒ',
        'p' => 'ₚ',
        'r' => 'ᵣ',
        's' => 'ₛ',
        't' => 'ₜ',
        'u' => 'ᵤ',
        'v' => 'ᵥ',
        'x' => 'ₓ',
        autre => autre,
    }
}

#[cfg(test)]
mod tests_unicode {
    use super::*;

    #[test]
    fn grecs_et_operateurs() {
        assert_eq!(latex_to_unicode(r"\alpha + \beta"), "α + β");
        assert_eq!(latex_to_unicode(r"x \leq y \neq z"), "x ≤ y ≠ z");
        assert_eq!(latex_to_unicode(r"\sum_{i=1}^{n}"), "∑ᵢ₌₁ⁿ");
    }

    #[test]
    fn fractions_et_racines() {
        assert_eq!(latex_to_unicode(r"\frac{a}{b}"), "a⁄b");
        assert_eq!(latex_to_unicode(r"\frac{a+b}{c}"), "(a+b)/(c)");
        assert_eq!(latex_to_unicode(r"\sqrt{x+1}"), "√(x+1)");
    }

    #[test]
    fn exposants_et_indices() {
        assert_eq!(latex_to_unicode("E=mc^2"), "E=mc²");
        assert_eq!(latex_to_unicode("x^{n+1}"), "xⁿ⁺¹");
        assert_eq!(latex_to_unicode("a_i"), "aᵢ");
        assert_eq!(latex_to_unicode("v_{k+1}"), "vₖ₊₁");
    }

    #[test]
    fn ensembles_mathbb() {
        assert_eq!(latex_to_unicode(r"x \in \mathbb{R}"), "x ∈ ℝ");
        assert_eq!(latex_to_unicode(r"\mathbb{N}"), "ℕ");
    }

    #[test]
    fn commande_inconnue_sans_perte() {
        // Commande inconnue : le nom garde sa place, l'argument n'est pas
        // interprété (accolades brutes).
        assert_eq!(latex_to_unicode(r"\unknowncmd{x}"), "unknowncmd{x}");
    }

    #[test]
    fn texte_et_espaces() {
        assert_eq!(latex_to_unicode(r"\text{si} x > 0"), "si x > 0");
        assert_eq!(latex_to_unicode(r"a\,b"), "a b");
    }

    #[test]
    fn display_rendu_complet() {
        // Exemple type de note.
        let rendu = latex_to_unicode(r"\int_0^{\infty} e^{-x} \, dx = 1");
        assert!(rendu.contains('∫'));
        assert!(rendu.contains('∞'));
        assert!(rendu.contains("\u{207B}\u{02E3}"));
    }
}
