//! Snippets CSS : chargement d'un sous-ensemble de variables de couleurs
//! depuis `.obsidian/snippets/*.css`.
//!
//! Le format Obsidian repose sur des variables CSS (`--interactive-accent`,
//! `--background-primary`, …). Ce module extrait ces variables (déclarations
//! `--nom: #rrggbb;`) et les expose sous forme de paires nom → couleur RGB,
//! sans interpréter d'autre syntaxe CSS. C'est un sous-ensemble volontaire :
//! la mise en page CSS complète est hors périmètre d'une app native iced.

use std::fs;
use std::path::{Path, PathBuf};

/// Clés CSS reconnues vers les composantes d'un thème.
pub const KNOWN_KEYS: &[(&str, &str)] = &[
    ("--interactive-accent", "accent"),
    ("--background-primary", "bg"),
    ("--text-normal", "fg"),
    ("--background-secondary", "surface"),
    ("--background-modifier-border", "border"),
];

/// Une déclaration de variable de couleur extraite d'un snippet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CssVar {
    /// Nom de la variable CSS (`--…`).
    pub name: String,
    /// Valeur RGB (r, g, b), de `0..=255`.
    pub rgb: (u8, u8, u8),
}

/// Parse une valeur de couleur CSS (`#rgb`, `#rrggbb`, `rgb(r,g,b)`).
/// Retourne `None` si le format n'est pas reconnu.
pub fn parse_color(value: &str) -> Option<(u8, u8, u8)> {
    let v = value.trim();
    if let Some(hex) = v.strip_prefix('#') {
        let digits: String = hex.chars().filter(|c| c.is_ascii_hexdigit()).collect();
        match digits.len() {
            3 => {
                let r = u8::from_str_radix(&digits[0..1].repeat(2), 16).ok()?;
                let g = u8::from_str_radix(&digits[1..2].repeat(2), 16).ok()?;
                let b = u8::from_str_radix(&digits[2..3].repeat(2), 16).ok()?;
                Some((r, g, b))
            }
            6 => {
                let r = u8::from_str_radix(&digits[0..2], 16).ok()?;
                let g = u8::from_str_radix(&digits[2..4], 16).ok()?;
                let b = u8::from_str_radix(&digits[4..6], 16).ok()?;
                Some((r, g, b))
            }
            _ => None,
        }
    } else if v.starts_with("rgb(") && v.ends_with(')') {
        let inner = v.trim_start_matches("rgb(").trim_end_matches(')');
        let parts: Vec<&str> = inner.split(',').collect();
        if parts.len() == 3 {
            let parse = |s: &str| s.trim().parse::<u8>().ok();
            Some((parse(parts[0])?, parse(parts[1])?, parse(parts[2])?))
        } else {
            None
        }
    } else {
        None
    }
}

/// Extrait toutes les déclarations `--nom: couleur;` d'un fichier CSS.
pub fn extract_css_vars(css: &str) -> Vec<CssVar> {
    let mut out = Vec::new();
    for line in css.lines() {
        let trimmed = line.trim();
        let Some(rest) = trimmed.strip_prefix("--") else {
            continue;
        };
        let Some(semi) = rest.find(';') else {
            continue;
        };
        let declaration = &rest[..semi];
        let Some(colon) = declaration.find(':') else {
            continue;
        };
        let name = format!("--{}", declaration[..colon].trim());
        let value = declaration[colon + 1..].trim();
        if let Some(rgb) = parse_color(value) {
            out.push(CssVar { name, rgb });
        }
    }
    out
}

/// Scanne `.obsidian/snippets/*.css` et retourne, pour chaque fichier (nom
/// du fichier, variables extraites). Dossier absent → liste vide ; un
/// fichier illisible est ignoré.
pub fn load_snippets(vault_root: &Path) -> Vec<(PathBuf, Vec<CssVar>)> {
    let dir = vault_root.join(".obsidian").join("snippets");
    let Ok(entries) = fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("css") {
            continue;
        }
        let Ok(css) = fs::read_to_string(&path) else {
            continue;
        };
        let vars = extract_css_vars(&css);
        out.push((path, vars));
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_hex_3_chiffres() {
        assert_eq!(parse_color("#abc"), Some((0xaa, 0xbb, 0xcc)));
    }

    #[test]
    fn parse_hex_6_chiffres() {
        assert_eq!(parse_color("#1a2b3c"), Some((0x1a, 0x2b, 0x3c)));
    }

    #[test]
    fn parse_rgb() {
        assert_eq!(parse_color("rgb(1, 2, 3)"), Some((1, 2, 3)));
    }

    #[test]
    fn parse_invalide_retourne_none() {
        assert_eq!(parse_color("red"), None);
        assert_eq!(parse_color("#12"), None);
        assert_eq!(parse_color("rgb(1, 2)"), None);
    }

    #[test]
    fn extraction_ignore_les_declarations_non_couleur() {
        let css = "--interactive-accent: #3366ff;\n--marge: 8px;\n--bg: rgb(10,20,30);\n";
        let vars = extract_css_vars(css);
        assert_eq!(vars.len(), 2);
        assert_eq!(vars[0].name, "--interactive-accent");
        assert_eq!(vars[0].rgb, (0x33, 0x66, 0xff));
        assert_eq!(vars[1].name, "--bg");
    }

    #[test]
    fn extraction_ignore_la_syntaxe_inconnue() {
        let vars = extract_css_vars("body { color: red; }");
        assert!(vars.is_empty());
    }
}
