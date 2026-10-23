//! Palettes de thèmes (données pures, sans dépendance UI).
//!
//! Deux thèmes sont fournis : `DEFAULT_DARK` et `DEFAULT_LIGHT`. Le reste du
//! code les sélectionne par nom via `by_name`, de façon insensible à la casse
//! et sans allocation.
//!
//! Un vault peut aussi fournir ses propres thèmes CSS (`.obsidian/themes/`).
//! [`theme_css_path`] et [`theme_css_source`] résolvent alors le nom du thème
//! actif vers le fichier correspondant ; l'interface-native les traduit en
//! variables (voir `crate::ui::theme_vars`).

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Palette complète d'un thème.
///
/// Les composantes sont des octets RGB (`0..=255`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Theme {
    /// Nom du thème, par exemple `"dark"` ou `"light"`.
    pub name: &'static str,
    /// Couleur d'accent (actions, liens, sélections).
    pub accent: (u8, u8, u8),
    /// Variante de l'accent lisible **en texte** sur `bg` (WCAG AA ≥ 4.5).
    pub accent_text: (u8, u8, u8),
    /// Couleur de fond principal.
    pub bg: (u8, u8, u8),
    /// Couleur du texte principal.
    pub fg: (u8, u8, u8),
    /// Couleur des surfaces (panneaux, champs, cartes).
    pub surface: (u8, u8, u8),
    /// Couleur des séparateurs et bordures.
    pub border: (u8, u8, u8),
}

/// Thème sombre par défaut.
///
/// Valeurs des tokens documentés d'Obsidian (`--color-base-*` et l'accent
/// par défaut HSL 258/88 %/66 % ≈ `#8A5CF5`).
pub const DEFAULT_DARK: Theme = Theme {
    name: "dark",
    accent: (138, 92, 245),
    accent_text: (169, 139, 255),
    bg: (28, 28, 28),
    fg: (218, 218, 218),
    surface: (40, 40, 40),
    border: (51, 51, 51),
};

/// Thème clair par défaut (mêmes tokens, mode clair).
pub const DEFAULT_LIGHT: Theme = Theme {
    name: "light",
    accent: (138, 92, 245),
    accent_text: (120, 82, 238),
    bg: (255, 255, 255),
    fg: (34, 34, 34),
    surface: (246, 246, 246),
    border: (228, 228, 228),
};

/// Liste des thèmes connus, dans l'ordre de préférence d'affichage.
const THEMES: &[&Theme] = &[&DEFAULT_DARK, &DEFAULT_LIGHT];

/// Recherche un thème par son nom, de façon insensible à la casse.
///
/// Seuls `"dark"` et `"light"` sont reconnus (et leurs variantes en casse).
/// Un nom inconnu retourne `None` plutôt qu'un thème par défaut, afin que le
/// choix de l'utilisateur ne soit pas silencieusement ignoré.
pub fn by_name(name: &str) -> Option<&'static Theme> {
    THEMES
        .iter()
        .find(|theme| theme.name.eq_ignore_ascii_case(name))
        .copied()
}

/// Résout le nom du thème actif vers un fichier CSS du vault.
///
/// Recherche, dans `.obsidian/themes/` :
/// 1. le fichier `<nom du thème>.css` ;
/// 2. un fichier dont le nom correspond sans tenir compte de la casse ;
/// 3. le premier fichier `*.css` du dossier, par ordre alphabétique.
///
/// Retourne `None` quand le vault ne fournit aucun fichier de thème : les
/// valeurs par défaut restent alors en vigueur. Un nom de thème contenant un
/// séparateur de chemin est refusé, pour ne jamais sortir du vault.
pub fn theme_css_path(vault_root: &Path, theme_name: &str) -> Option<PathBuf> {
    let name = theme_name.trim();
    if name.is_empty() || name.contains('/') || name.contains('\\') || name.contains("..") {
        return None;
    }
    let dir = vault_root.join(".obsidian").join("themes");
    let exact = dir.join(format!("{name}.css"));
    if exact.is_file() {
        return Some(exact);
    }
    let entries = fs::read_dir(&dir).ok()?;
    let mut candidates: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("css"))
        .collect();
    candidates.sort();
    let wanted = name.to_ascii_lowercase();
    candidates
        .iter()
        .find(|path| {
            path.file_stem()
                .and_then(|stem| stem.to_str())
                .map(|stem| stem.to_ascii_lowercase() == wanted)
                .unwrap_or(false)
        })
        .cloned()
        .or_else(|| candidates.into_iter().next())
}

/// Source CSS du thème actif du vault, si celui-ci en fournit une.
///
/// Un fichier illisible est ignoré (retour `None`) : l'application conserve
/// alors les valeurs par défaut plutôt que d'échouer au démarrage.
pub fn theme_css_source(vault_root: &Path, theme_name: &str) -> Option<String> {
    let path = theme_css_path(vault_root, theme_name)?;
    fs::read_to_string(path).ok()
}

/// Mode d'affichage visé par la résolution d'un thème CSS.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThemeMode {
    /// Mode sombre (blocs `.theme-dark` du CSS).
    Dark,
    /// Mode clair (blocs `.theme-light` du CSS).
    Light,
}

/// Bloc de déclarations d'un thème CSS, dans l'ordre du fichier.
///
/// Seules les variables personnalisées (`--nom: valeur`) sont conservées :
/// les déclarations classiques (`color`, `padding`, …) n'ont pas de sens
/// pour un thème iced et sont ignorées.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CssRule {
    /// Sélecteur tel qu'écrit (`body`, `.theme-dark`, `body.theme-dark …`).
    pub selector: String,
    /// Variables du bloc, dans l'ordre d'écriture (les dernières gagnent).
    pub declarations: Vec<(String, String)>,
}

/// Thème CSS analysé : les blocs du fichier, dans l'ordre d'apparition.
///
/// L'analyse est tolérante : un fichier tronqué, un bloc non fermé ou une
/// déclaration mal formée ne fait jamais échouer la lecture, la partie
/// exploitable est simplement conservée.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CssTheme {
    /// Blocs reconnus, dans l'ordre du fichier.
    pub rules: Vec<CssRule>,
}

/// Variables d'Obsidian qui pilotent réellement l'interface.
///
/// Sert à signaler les variables qu'un thème tiers omet : l'appelant retombe
/// alors sur ses valeurs par défaut au lieu d'afficher un thème incomplet.
pub const KEY_VARIABLES: &[&str] = &[
    "--background-primary",
    "--background-primary-alt",
    "--background-secondary",
    "--background-secondary-alt",
    "--background-modifier-border",
    "--background-modifier-hover",
    "--text-normal",
    "--text-muted",
    "--text-faint",
    "--text-accent",
    "--text-accent-hover",
    "--text-on-accent",
    "--interactive-accent",
    "--interactive-accent-hover",
    "--font-text-size",
    "--font-interface",
    "--font-text",
    "--font-monospace",
];

/// Inventaire des variables clés couvertes (ou non) par un thème résolu.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VariableAudit {
    /// Variables clés présentes, dans l'ordre de [`KEY_VARIABLES`].
    pub present: Vec<&'static str>,
    /// Variables clés absentes, dans l'ordre de [`KEY_VARIABLES`].
    pub missing: Vec<&'static str>,
}

impl VariableAudit {
    /// Vrai quand toutes les variables clés sont définies.
    pub fn is_complete(&self) -> bool {
        self.missing.is_empty()
    }
}

/// Portée d'un sélecteur vis-à-vis de la cascade des thèmes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Scope {
    /// Base commune (`body`, `:root`, `html`).
    Base,
    /// Bloc propre au mode sombre.
    Dark,
    /// Bloc propre au mode clair.
    Light,
    /// Bloc sans rapport avec les variables de thème.
    Ignore,
}

impl CssTheme {
    /// Analyse une feuille de style de thème (contenu de `theme.css`).
    ///
    /// Les commentaires `/* … */` sont retirés (y compris un commentaire non
    /// terminé, qui consomme la fin du fichier), les blocs imbriqués (comme
    /// `@media { … }`) sont parcourus, et seules les déclarations `--nom`
    /// sont retenues. Un sélecteur peut recevoir plusieurs blocs : ils sont
    /// conservés dans l'ordre.
    pub fn parse(css: &str) -> Self {
        let source = strip_comments(css);
        let mut rules: Vec<CssRule> = Vec::new();
        let mut selectors: Vec<String> = Vec::new();
        let mut buffer = String::new();

        for c in source.chars() {
            match c {
                '{' => {
                    selectors.push(buffer.trim().to_string());
                    buffer.clear();
                }
                '}' => {
                    let declarations = parse_declarations(&buffer);
                    buffer.clear();
                    let Some(selector) = selectors.pop() else {
                        continue;
                    };
                    if declarations.is_empty() {
                        continue;
                    }
                    rules.push(CssRule {
                        selector,
                        declarations,
                    });
                }
                _ => buffer.push(c),
            }
        }

        CssTheme { rules }
    }

    /// Variables effectives pour un mode, selon la cascade des thèmes.
    ///
    /// Les blocs de base (`body`, `:root`, `html`) sont appliqués d'abord,
    /// puis les blocs du mode demandé : une variable définie dans les deux
    /// est donc toujours gagnée par le bloc de mode, quelle que soit l'ordre
    /// des blocs dans le fichier. À l'intérieur d'une même portée, la
    /// dernière déclaration l'emporte.
    pub fn effective_variables(&self, mode: ThemeMode) -> BTreeMap<String, String> {
        let mut variables: BTreeMap<String, String> = BTreeMap::new();
        for wanted in [Scope::Base, scope_of_mode(mode)] {
            for rule in &self.rules {
                if selector_scope(&rule.selector) != wanted {
                    continue;
                }
                for (name, value) in &rule.declarations {
                    variables.insert(name.clone(), value.clone());
                }
            }
        }
        variables
    }
}

/// Inventaire des variables clés présentes dans un thème résolu.
pub fn audit_variables(variables: &BTreeMap<String, String>) -> VariableAudit {
    let mut present = Vec::new();
    let mut missing = Vec::new();
    for name in KEY_VARIABLES {
        if variables.contains_key(*name) {
            present.push(*name);
        } else {
            missing.push(*name);
        }
    }
    VariableAudit { present, missing }
}

/// Variables effectives du thème CSS d'un vault, si ce thème existe.
///
/// Compose [`theme_css_source`] et [`CssTheme::parse`] : un vault sans
/// fichier de thème retourne `None`, et l'appelant garde alors ses valeurs
/// par défaut.
pub fn css_theme_variables(
    vault_root: &Path,
    theme_name: &str,
    mode: ThemeMode,
) -> Option<BTreeMap<String, String>> {
    let source = theme_css_source(vault_root, theme_name)?;
    Some(CssTheme::parse(&source).effective_variables(mode))
}

/// Portée correspondant à un mode d'affichage.
fn scope_of_mode(mode: ThemeMode) -> Scope {
    match mode {
        ThemeMode::Dark => Scope::Dark,
        ThemeMode::Light => Scope::Light,
    }
}

/// Classe un sélecteur CSS selon la cascade des thèmes.
///
/// Un sélecteur mentionnant `theme-dark` / `theme-light` appartient au mode
/// correspondant (`body.theme-dark`, `.theme-dark .markdown-preview-view`, …) ;
/// sinon `body`, `:root` et `html` forment la base commune. Le reste est
/// ignoré : les thèmes Obsidian contiennent beaucoup de sélecteurs de mise en
/// page qui n'ont pas d'équivalent ici.
fn selector_scope(selector: &str) -> Scope {
    let lowered = selector.to_ascii_lowercase();
    if lowered.contains("theme-dark") {
        return Scope::Dark;
    }
    if lowered.contains("theme-light") {
        return Scope::Light;
    }
    if lowered
        .split(|c: char| c == ',' || c.is_whitespace())
        .any(|part| {
            let part = part.trim();
            part == "body" || part == ":root" || part == "html"
        })
    {
        return Scope::Base;
    }
    Scope::Ignore
}

/// Retire les commentaires CSS, y compris un commentaire non terminé.
fn strip_comments(css: &str) -> String {
    let mut out = String::with_capacity(css.len());
    let mut chars = css.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '/' && chars.peek() == Some(&'*') {
            chars.next();
            let mut closed = false;
            loop {
                match chars.next() {
                    Some('*') if chars.peek() == Some(&'/') => {
                        chars.next();
                        closed = true;
                        break;
                    }
                    Some(_) => {}
                    None => break,
                }
            }
            if !closed {
                break;
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Extrait les variables `--nom: valeur` d'un corps de bloc CSS.
///
/// Le découpage se fait sur `;` en dehors des parenthèses et des chaînes,
/// pour préserver `url(data:image/svg+xml;base64,…)`. Une déclaration sans
/// `;` final (dernière du bloc) est acceptée ; une déclaration sans `:` ou
/// dont le nom n'est pas une variable est ignorée.
fn parse_declarations(block: &str) -> Vec<(String, String)> {
    let mut declarations: Vec<(String, String)> = Vec::new();
    for part in split_declarations(block) {
        let Some((name, value)) = part.split_once(':') else {
            continue;
        };
        let name = name.trim();
        if !name.starts_with("--") || name.len() <= 2 {
            continue;
        }
        let value = normalize_value(strip_important(value));
        if value.is_empty() {
            continue;
        }
        declarations.push((name.to_string(), value));
    }
    declarations
}

/// Découpe un corps de bloc sur les `;` de premier niveau.
fn split_declarations(block: &str) -> Vec<String> {
    let mut parts: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut depth: usize = 0;
    let mut quote: Option<char> = None;
    for c in block.chars() {
        if let Some(q) = quote {
            current.push(c);
            if c == q {
                quote = None;
            }
            continue;
        }
        match c {
            '\'' | '"' => {
                quote = Some(c);
                current.push(c);
            }
            '(' => {
                depth += 1;
                current.push(c);
            }
            ')' => {
                depth = depth.saturating_sub(1);
                current.push(c);
            }
            ';' if depth == 0 => {
                parts.push(std::mem::take(&mut current));
            }
            _ => current.push(c),
        }
    }
    if !current.trim().is_empty() {
        parts.push(current);
    }
    parts
}

/// Retire un suffixe `!important` (espaces tolérées) d'une valeur.
fn strip_important(value: &str) -> &str {
    if let Some(position) = value.rfind('!') {
        let suffix: String = value[position + 1..]
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect();
        if suffix.eq_ignore_ascii_case("important") {
            return value[..position].trim_end();
        }
    }
    value
}

/// Réduit les espaces (retours à la ligne compris) à un espace simple.
fn normalize_value(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for c in value.chars() {
        if c.is_whitespace() {
            if !out.is_empty() && !out.ends_with(' ') {
                out.push(' ');
            }
        } else {
            out.push(c);
        }
    }
    out.trim_end().to_string()
}

/// Luminance relative sRGB d'une couleur (WCAG 2.x).
#[cfg(test)]
fn luminance_of((r, g, b): (u8, u8, u8)) -> f64 {
    fn channel(c: u8) -> f64 {
        let v = f64::from(c) / 255.0;
        if v <= 0.03928 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    }
    0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b)
}

/// Ratio de contraste WCAG entre deux couleurs.
#[cfg(test)]
fn contrast(a: (u8, u8, u8), b: (u8, u8, u8)) -> f64 {
    let (l1, l2) = if luminance_of(a) > luminance_of(b) {
        (luminance_of(a), luminance_of(b))
    } else {
        (luminance_of(b), luminance_of(a))
    };
    (l1 + 0.05) / (l2 + 0.05)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_dark_est_present() {
        assert_eq!(DEFAULT_DARK.name, "dark");
    }

    #[test]
    fn default_light_est_present() {
        assert_eq!(DEFAULT_LIGHT.name, "light");
    }

    #[test]
    fn by_name_insensible_a_la_casse() {
        assert_eq!(by_name("dark"), Some(&DEFAULT_DARK));
        assert_eq!(by_name("DARK"), Some(&DEFAULT_DARK));
        assert_eq!(by_name("Dark"), Some(&DEFAULT_DARK));
        assert_eq!(by_name("light"), Some(&DEFAULT_LIGHT));
        assert_eq!(by_name("LIGHT"), Some(&DEFAULT_LIGHT));
        assert_eq!(by_name("LiGhT"), Some(&DEFAULT_LIGHT));
    }

    #[test]
    fn by_name_inconnu_retourne_none() {
        assert_eq!(by_name(""), None);
        assert_eq!(by_name("obsidian"), None);
        assert_eq!(by_name("darkmode"), None);
        assert_eq!(by_name("solarized"), None);
    }

    #[test]
    fn contraste_fg_bg_superieur_a_trois() {
        assert!(contrast(DEFAULT_DARK.fg, DEFAULT_DARK.bg) > 3.0);
        assert!(contrast(DEFAULT_LIGHT.fg, DEFAULT_LIGHT.bg) > 3.0);
    }

    /// WCAG AA pour texte normal exige un ratio ≥ 4.5.
    #[test]
    fn contraste_aa_texte_normal() {
        assert!(
            contrast(DEFAULT_DARK.fg, DEFAULT_DARK.bg) >= 4.5,
            "dark fg/bg doit respecter AA (≥ 4.5)"
        );
        assert!(
            contrast(DEFAULT_LIGHT.fg, DEFAULT_LIGHT.bg) >= 4.5,
            "light fg/bg doit respecter AA (≥ 4.5)"
        );
    }

    /// Le texte accentué doit rester lisible : AA exige ≥ 4.5 sur le fond.
    #[test]
    fn contraste_aa_texte_accent() {
        for theme in [DEFAULT_DARK, DEFAULT_LIGHT] {
            let ratio = contrast(theme.accent_text, theme.bg);
            assert!(
                ratio >= 4.5,
                "{} : accent_text/bg = {ratio:.2}, attendu ≥ 4.5",
                theme.name
            );
        }
    }

    /// L'accent brut sert de fond/aplat : WCAG 1.4.11 demande ≥ 3 sur le fond.
    #[test]
    fn contraste_accent_brut_sur_fond() {
        for theme in [DEFAULT_DARK, DEFAULT_LIGHT] {
            let ratio = contrast(theme.accent, theme.bg);
            assert!(
                ratio >= 3.0,
                "{} : accent/bg = {ratio:.2}, attendu ≥ 3.0",
                theme.name
            );
        }
    }

    #[test]
    fn toutes_composantes_rgb_dans_l_intervalle() {
        for theme in [DEFAULT_DARK, DEFAULT_LIGHT] {
            for &(r, g, b) in [
                &theme.accent,
                &theme.accent_text,
                &theme.bg,
                &theme.fg,
                &theme.surface,
                &theme.border,
            ] {
                assert!((0..=255).contains(&r), "r hors limite pour {}", theme.name);
                assert!((0..=255).contains(&g), "g hors limite pour {}", theme.name);
                assert!((0..=255).contains(&b), "b hors limite pour {}", theme.name);
            }
        }
    }

    #[test]
    fn by_name_retourne_des_themes_distincts() {
        assert_ne!(by_name("dark"), by_name("light"));
        assert_eq!(by_name("dark"), Some(&DEFAULT_DARK));
        assert_eq!(by_name("light"), Some(&DEFAULT_LIGHT));
    }

    #[test]
    fn parse_extrait_les_variables_et_ignore_le_reste() {
        let theme = CssTheme::parse(
            r#"
            .markdown-preview-view {
                color: red;
                padding: 4px;
                --custom-text: #abcdef;
            }
            "#,
        );
        assert_eq!(theme.rules.len(), 1);
        assert_eq!(theme.rules[0].declarations.len(), 1);
        assert_eq!(theme.rules[0].declarations[0].0, "--custom-text");
        assert_eq!(theme.rules[0].declarations[0].1, "#abcdef");
        assert_eq!(selector_scope(".markdown-preview-view"), Scope::Ignore);
        assert!(theme.effective_variables(ThemeMode::Dark).is_empty());
    }

    #[test]
    fn cascade_base_puis_mode() {
        let theme = CssTheme::parse(
            r#"
            body { --text-normal: #aaa; --background-primary: #111; }
            .theme-dark { --text-normal: #ddd; }
            .theme-light { --text-normal: #222; }
            "#,
        );
        let dark = theme.effective_variables(ThemeMode::Dark);
        assert_eq!(dark.get("--text-normal").map(String::as_str), Some("#ddd"));
        assert_eq!(
            dark.get("--background-primary").map(String::as_str),
            Some("#111")
        );
        let light = theme.effective_variables(ThemeMode::Light);
        assert_eq!(light.get("--text-normal").map(String::as_str), Some("#222"));
    }

    #[test]
    fn le_mode_gagne_meme_s_il_est_ecrit_avant_body() {
        let theme = CssTheme::parse(
            r#"
            .theme-dark { --text-normal: #ddd; }
            body { --text-normal: #aaa; }
            "#,
        );
        assert_eq!(
            theme
                .effective_variables(ThemeMode::Dark)
                .get("--text-normal")
                .map(String::as_str),
            Some("#ddd")
        );
    }

    #[test]
    fn derniere_declaration_gagne_dans_une_portee() {
        let theme = CssTheme::parse(
            r#"
            body { --a: 1; --a: 2; }
            body { --a: 3; }
            "#,
        );
        assert_eq!(
            theme
                .effective_variables(ThemeMode::Dark)
                .get("--a")
                .map(String::as_str),
            Some("3")
        );
    }

    #[test]
    fn commentaires_ignores() {
        let theme = CssTheme::parse("/* --a: 0; */\nbody { /* --b: 0; */ --a: 1; }");
        let variables = theme.effective_variables(ThemeMode::Light);
        assert_eq!(variables.get("--a").map(String::as_str), Some("1"));
        assert!(!variables.contains_key("--b"));
    }

    #[test]
    fn commentaire_non_termine_tronque_sans_paniquer() {
        let theme = CssTheme::parse("body { --a: 1; } /* jamais fermé --b: 2; }}");
        let variables = theme.effective_variables(ThemeMode::Dark);
        assert_eq!(variables.get("--a").map(String::as_str), Some("1"));
        assert!(!variables.contains_key("--b"));
    }

    #[test]
    fn important_retire_de_la_valeur() {
        let theme = CssTheme::parse("body { --a: 1px !important; --b: 2px ! important; }");
        let variables = theme.effective_variables(ThemeMode::Dark);
        assert_eq!(variables.get("--a").map(String::as_str), Some("1px"));
        assert_eq!(variables.get("--b").map(String::as_str), Some("2px"));
    }

    #[test]
    fn valeur_multiligne_normalisee() {
        let theme = CssTheme::parse("body {\n  --font-text:\n    \"Inter\",\n    sans-serif;\n}");
        assert_eq!(
            theme
                .effective_variables(ThemeMode::Dark)
                .get("--font-text")
                .map(String::as_str),
            Some("\"Inter\", sans-serif")
        );
    }

    #[test]
    fn point_virgule_dans_une_fonction_conserve() {
        let theme = CssTheme::parse(
            "body { --icon: url(data:image/svg+xml;base64,AAA); --after: 1; --font: \"a;b\"; }",
        );
        let variables = theme.effective_variables(ThemeMode::Light);
        assert_eq!(
            variables.get("--icon").map(String::as_str),
            Some("url(data:image/svg+xml;base64,AAA)")
        );
        assert_eq!(variables.get("--after").map(String::as_str), Some("1"));
        assert_eq!(variables.get("--font").map(String::as_str), Some("\"a;b\""));
    }

    #[test]
    fn declarations_mal_formees_ignorees() {
        let theme = CssTheme::parse("body { : 1; --: 2; --ok: ; --a 3; --b: 4 }");
        let variables = theme.effective_variables(ThemeMode::Dark);
        assert_eq!(variables.len(), 1);
        assert_eq!(variables.get("--b").map(String::as_str), Some("4"));
    }

    #[test]
    fn fichiers_vides_ou_absurdes_ne_paniquent_pas() {
        for source in [
            "",
            "   ",
            "{{{{",
            "}}}",
            "body {",
            "@media print {",
            "--a: 1;",
        ] {
            let theme = CssTheme::parse(source);
            assert!(theme.effective_variables(ThemeMode::Dark).is_empty());
        }
    }

    #[test]
    fn media_imbrique_et_scope_isolee() {
        let theme = CssTheme::parse("@media (min-width: 600px) { .theme-dark { --a: 1; } }");
        assert_eq!(
            theme
                .effective_variables(ThemeMode::Dark)
                .get("--a")
                .map(String::as_str),
            Some("1")
        );
        assert!(theme.effective_variables(ThemeMode::Light).is_empty());
    }

    #[test]
    fn portees_des_selecteurs() {
        assert_eq!(selector_scope("body"), Scope::Base);
        assert_eq!(selector_scope(":root"), Scope::Base);
        assert_eq!(selector_scope(".theme-dark"), Scope::Dark);
        assert_eq!(selector_scope("body.theme-light"), Scope::Light);
        assert_eq!(
            selector_scope(".theme-dark .markdown-preview-view"),
            Scope::Dark
        );
        assert_eq!(selector_scope(".workspace-leaf"), Scope::Ignore);
    }

    #[test]
    fn audit_signale_les_variables_manquantes() {
        let theme = CssTheme::parse("body { --text-normal: #ddd; --interactive-accent: #8a5cf5; }");
        let variables = theme.effective_variables(ThemeMode::Dark);
        let audit = audit_variables(&variables);
        assert_eq!(audit.present.len(), 2);
        assert_eq!(audit.missing.len(), KEY_VARIABLES.len() - 2);
        assert!(!audit.is_complete());
        assert!(audit.present.contains(&"--text-normal"));
        assert!(audit.missing.contains(&"--font-text"));

        let mut complet = BTreeMap::new();
        for name in KEY_VARIABLES {
            complet.insert((*name).to_string(), "x".to_string());
        }
        assert!(audit_variables(&complet).is_complete());
    }

    #[test]
    fn css_theme_variables_sans_fichier_retourne_none() {
        let dossier = std::env::temp_dir().join("obsidian-rs-theme-absent");
        assert_eq!(
            css_theme_variables(&dossier, "nonexistent", ThemeMode::Dark),
            None
        );
    }
}
