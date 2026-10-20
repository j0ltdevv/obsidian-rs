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
