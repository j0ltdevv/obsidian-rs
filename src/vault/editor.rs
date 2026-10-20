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
struct Fence {
    char: char,
    len: usize,
}

/// Reconnaît une fence ouvrante (au moins trois backticks ou tildes).
fn fence_open(line: &str) -> Option<Fence> {
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
fn fence_close(line: &str, open: &Fence) -> bool {
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

/// Reconnaît un titre ATX et retourne son niveau (1 à 6) et son texte.
fn heading(line: &str) -> Option<(u8, String)> {
    let body = unindent_upto3(line)?;
    let level = body.chars().take_while(|c| *c == '#').count();
    if level == 0 || level > 6 {
        return None;
    }
    // Les dièses ne forment un titre que s'ils sont suivis d'un espace ou de
    // la fin de ligne (`#Titre` reste du texte).
    let rest = &body[level..];
    if !rest.is_empty() && !rest.starts_with(' ') && !rest.starts_with('\t') {
        return None;
    }
    let text = strip_closing_hashes(rest.trim());
    Some((level as u8, text.to_string()))
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
