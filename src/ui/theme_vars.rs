//! Variables CSS d'Obsidian traduites en tokens utilisables par les vues.
//!
//! Obsidian décrit un thème par des propriétés CSS personnalisées
//! (`--background-primary`, `--text-normal`, `--interactive-accent`, …) que
//! l'utilisateur peut surcharger via `.obsidian/snippets/*.css`. Ce module
//! traduit ce sous-ensemble en tokens directement exploitables par iced.
//!
//! # Ordre de chargement
//!
//! 1. les **valeurs par défaut** du mode actif (sombre ou clair) ;
//! 2. le **fichier CSS du thème actif** du vault (`.obsidian/themes/<nom>.css`,
//!    ou le premier `*.css` trouvé dans ce dossier) ;
//! 3. les **snippets de l'utilisateur** (`.obsidian/snippets/*.css`, dans
//!    l'ordre alphabétique des noms de fichiers).
//!
//! Chaque étape écrase la précédente : la dernière déclaration lue l'emporte.
//! L'ordre est appliqué par [`load_theme_vars`].
//!
//! # Portée du parsing
//!
//! Le parsing CSS est volontairement minimal : un scanner de déclarations
//! `--nom: valeur;` suffit. Il ignore le contenu des commentaires `/* … */`,
//! suit les accolades pour connaître le bloc englobant (un bloc
//! `.theme-dark` ou `.theme-light` fixe le mode) et résout les références
//! `var(--nom)`. En revanche, il ne gère ni les sélecteurs complexes ni les
//! règles `@media` : ce n'est pas un parseur CSS.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use iced::Color;

use crate::vault::css_snippets;
use crate::vault::themes;

/// Mode de thème (sombre ou clair).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThemeMode {
    /// Mode sombre, correspondant au bloc `.theme-dark` d'Obsidian.
    Dark,
    /// Mode clair, correspondant au bloc `.theme-light` d'Obsidian.
    Light,
}

impl ThemeMode {
    /// Mode associé à un nom de thème : seul « light » (insensible à la
    /// casse) bascule en mode clair, tout autre nom reste sombre.
    pub fn for_theme_name(name: &str) -> ThemeMode {
        if name.trim().eq_ignore_ascii_case("light") {
            ThemeMode::Light
        } else {
            ThemeMode::Dark
        }
    }
}

/// Variables de thème issues des propriétés CSS personnalisées d'Obsidian.
///
/// Chaque champ correspond à une propriété CSS connue ; une propriété absente
/// du CSS du vault conserve la valeur par défaut du mode.
#[derive(Clone, Debug, PartialEq)]
pub struct ThemeVars {
    /// Fond principal de l'interface (`--background-primary`).
    pub background_primary: Color,
    /// Fond des surfaces secondaires (panneaux, champs) (`--background-secondary`).
    pub background_secondary: Color,
    /// Couleur des bordures et séparateurs (`--background-modifier-border`).
    pub background_modifier_border: Color,
    /// Fond de survol (`--background-modifier-hover`).
    pub background_modifier_hover: Color,
    /// Couleur du texte principal (`--text-normal`).
    pub text_normal: Color,
    /// Couleur du texte secondaire (`--text-muted`).
    pub text_muted: Color,
    /// Couleur du texte très discret (`--text-faint`).
    pub text_faint: Color,
    /// Couleur du texte accentué (liens, titres actifs) (`--text-accent`).
    pub text_accent: Color,
    /// Couleur du texte posé sur un aplat d'accent (`--text-on-accent`).
    pub text_on_accent: Color,
    /// Couleur d'accent du thème (`--accent`).
    pub accent: Color,
    /// Couleur d'accent au survol (`--accent-hover`).
    pub accent_hover: Color,
    /// Accent interactif (sélections, éléments actifs) (`--interactive-accent`).
    pub interactive_accent: Color,
    /// Police de l'interface, telle que déclarée (`--font-interface`).
    pub font_interface: String,
    /// Police du texte des notes (`--font-text`).
    pub font_text: String,
    /// Police à chasse fixe (`--font-monospace`).
    pub font_monospace: String,
    /// Petit rayon d'arrondi (`--radius-s`).
    pub radius_s: f32,
    /// Rayon d'arrondi moyen (`--radius-m`).
    pub radius_m: f32,
    /// Grand rayon d'arrondi (`--radius-l`).
    pub radius_l: f32,
    /// Hauteur des champs de saisie (`--input-height`).
    pub input_height: f32,
}

/// Une déclaration CSS personnalisée extraite d'un fragment de style.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CssDeclaration {
    /// Nom de la propriété, préfixé de `--`.
    pub name: String,
    /// Valeur brute, espaces de début et de fin retirés.
    pub value: String,
    /// Mode imposé par le bloc englobant, lorsque le sélecteur fixe
    /// `.theme-dark` ou `.theme-light`.
    pub mode: Option<ThemeMode>,
}

impl ThemeVars {
    /// Valeurs par défaut du mode sombre (palette proche d'Obsidian).
    pub fn defaults_dark() -> ThemeVars {
        ThemeVars {
            background_primary: Color::from_rgb8(0x1c, 0x1c, 0x1c),
            background_secondary: Color::from_rgb8(0x28, 0x28, 0x28),
            background_modifier_border: Color::from_rgb8(0x33, 0x33, 0x33),
            background_modifier_hover: Color::from_rgba(1.0, 1.0, 1.0, 0.067),
            text_normal: Color::from_rgb8(0xda, 0xda, 0xda),
            text_muted: Color::from_rgb8(0xb3, 0xb3, 0xb3),
            text_faint: Color::from_rgb8(0x66, 0x66, 0x66),
            text_accent: Color::from_rgb8(0xa6, 0x8a, 0xf9),
            text_on_accent: Color::from_rgb8(0xff, 0xff, 0xff),
            accent: Color::from_rgb8(0x8a, 0x5c, 0xf5),
            accent_hover: Color::from_rgb8(0xa6, 0x8a, 0xf9),
            interactive_accent: Color::from_rgb8(0x8a, 0x5c, 0xf5),
            font_interface: "Inter".to_string(),
            font_text: "Inter".to_string(),
            font_monospace: "monospace".to_string(),
            radius_s: 4.0,
            radius_m: 8.0,
            radius_l: 12.0,
            input_height: 30.0,
        }
    }

    /// Valeurs par défaut du mode clair.
    pub fn defaults_light() -> ThemeVars {
        ThemeVars {
            background_primary: Color::from_rgb8(0xff, 0xff, 0xff),
            background_secondary: Color::from_rgb8(0xf6, 0xf6, 0xf6),
            background_modifier_border: Color::from_rgb8(0xe4, 0xe4, 0xe4),
            background_modifier_hover: Color::from_rgb8(0xec, 0xec, 0xec),
            text_normal: Color::from_rgb8(0x22, 0x22, 0x22),
            text_muted: Color::from_rgb8(0x6b, 0x6b, 0x6b),
            text_faint: Color::from_rgb8(0x9a, 0x9a, 0x9a),
            text_accent: Color::from_rgb8(0x78, 0x52, 0xee),
            text_on_accent: Color::from_rgb8(0xff, 0xff, 0xff),
            accent: Color::from_rgb8(0x8a, 0x5c, 0xf5),
            accent_hover: Color::from_rgb8(0x6d, 0x28, 0xd9),
            interactive_accent: Color::from_rgb8(0x8a, 0x5c, 0xf5),
            font_interface: "Inter".to_string(),
            font_text: "Inter".to_string(),
            font_monospace: "monospace".to_string(),
            radius_s: 4.0,
            radius_m: 8.0,
            radius_l: 12.0,
            input_height: 30.0,
        }
    }

    /// Valeurs par défaut du mode demandé.
    pub fn defaults_for(mode: ThemeMode) -> ThemeVars {
        match mode {
            ThemeMode::Dark => ThemeVars::defaults_dark(),
            ThemeMode::Light => ThemeVars::defaults_light(),
        }
    }

    /// Analyse un fragment CSS en partant des valeurs par défaut sombres.
    ///
    /// La dernière déclaration d'une propriété l'emporte ; une propriété
    /// inconnue est ignorée.
    pub fn from_css(css: &str) -> ThemeVars {
        ThemeVars::from_css_for_mode(css, ThemeMode::Dark)
    }

    /// Analyse un fragment CSS en partant des valeurs par défaut du mode.
    ///
    /// Les blocs `.theme-dark` et `.theme-light` ne sont appliqués que s'ils
    /// correspondent au mode demandé.
    pub fn from_css_for_mode(css: &str, mode: ThemeMode) -> ThemeVars {
        let mut vars = ThemeVars::defaults_for(mode);
        vars.merge_snippets_for_mode(css, mode);
        vars
    }

    /// Applique les déclarations d'un fragment CSS aux variables courantes.
    ///
    /// La dernière déclaration l'emporte : un snippet surcharge donc le thème
    /// qui a été chargé avant lui.
    pub fn merge_snippets(&mut self, css: &str) {
        self.apply_all(css, None);
    }

    /// Applique les déclarations d'un fragment CSS en ignorant les blocs du
    /// mode opposé à celui demandé.
    pub fn merge_snippets_for_mode(&mut self, css: &str, mode: ThemeMode) {
        self.apply_all(css, Some(mode));
    }

    /// Applique toutes les déclarations utiles d'un fragment CSS.
    fn apply_all(&mut self, css: &str, mode: Option<ThemeMode>) {
        let declarations = extract_declarations(css);
        let raw = raw_values(&declarations, mode);
        for declaration in &declarations {
            if !matches_mode(declaration.mode, mode) {
                continue;
            }
            let value = resolve_value(&declaration.value, &raw, 0);
            self.apply(&declaration.name, &value);
        }
    }

    /// Applique une déclaration aux variables connues.
    ///
    /// Une propriété inconnue ou une valeur illisible est ignorée : la
    /// variable conserve sa valeur courante.
    fn apply(&mut self, name: &str, value: &str) {
        match name {
            "--background-primary" => set_color(&mut self.background_primary, value),
            "--background-secondary" => set_color(&mut self.background_secondary, value),
            "--background-modifier-border" => {
                set_color(&mut self.background_modifier_border, value)
            }
            "--background-modifier-hover" => set_color(&mut self.background_modifier_hover, value),
            "--text-normal" => set_color(&mut self.text_normal, value),
            "--text-muted" => set_color(&mut self.text_muted, value),
            "--text-faint" => set_color(&mut self.text_faint, value),
            "--text-accent" => set_color(&mut self.text_accent, value),
            "--text-on-accent" => set_color(&mut self.text_on_accent, value),
            "--accent" => set_color(&mut self.accent, value),
            "--accent-hover" => set_color(&mut self.accent_hover, value),
            "--interactive-accent" => set_color(&mut self.interactive_accent, value),
            "--font-interface" => self.font_interface = value.to_string(),
            "--font-text" => self.font_text = value.to_string(),
            "--font-monospace" => self.font_monospace = value.to_string(),
            "--radius-s" => set_length(&mut self.radius_s, value),
            "--radius-m" => set_length(&mut self.radius_m, value),
            "--radius-l" => set_length(&mut self.radius_l, value),
            "--input-height" => set_length(&mut self.input_height, value),
            _ => {}
        }
    }
}

/// Charge les variables de thème d'un vault.
///
/// L'ordre de chargement est celui documenté en tête de module : valeurs par
/// défaut du mode, puis fichier CSS du thème actif du vault, puis snippets de
/// l'utilisateur. Un vault sans fichier de thème ni snippet conserve les
/// valeurs par défaut.
pub fn load_theme_vars(vault_root: &Path, theme_name: &str) -> ThemeVars {
    let mode = ThemeMode::for_theme_name(theme_name);
    let mut vars = ThemeVars::defaults_for(mode);

    // 2. Fichier CSS du thème actif du vault.
    if let Some(css) = themes::theme_css_source(vault_root, theme_name) {
        vars.merge_snippets_for_mode(&css, mode);
    }

    // 3. Snippets de l'utilisateur, dans l'ordre des noms de fichiers.
    for (_path, css) in snippet_sources(vault_root) {
        vars.merge_snippets_for_mode(&css, mode);
    }

    vars
}

/// Sources CSS des snippets du vault, dans l'ordre des noms de fichiers.
///
/// La découverte et l'ordre proviennent de [`css_snippets::load_snippets`] ;
/// le contenu brut est relu ici pour couvrir les variables non colorimétriques
/// (polices, rayons, hauteurs de champs). Un fichier illisible est ignoré.
fn snippet_sources(vault_root: &Path) -> Vec<(PathBuf, String)> {
    css_snippets::load_snippets(vault_root)
        .into_iter()
        .filter_map(|(path, _vars)| {
            let css = fs::read_to_string(&path).ok()?;
            Some((path, css))
        })
        .collect()
}

/// Extrait les déclarations `--nom: valeur;` d'un fragment CSS.
///
/// Le scanner suit les accolades pour connaître le bloc englobant : une
/// déclaration placée dans un bloc `.theme-dark` (ou `.theme-light`) est
/// marquée du mode correspondant. Le contenu des commentaires `/* … */` est
/// ignoré. L'ordre des déclarations est conservé.
pub fn extract_declarations(css: &str) -> Vec<CssDeclaration> {
    let cleaned = strip_comments(css);
    let mut declarations = Vec::new();
    let mut selectors: Vec<String> = Vec::new();
    let mut buffer = String::new();

    for ch in cleaned.chars() {
        match ch {
            '{' => {
                let selector = buffer.trim().to_string();
                buffer.clear();
                selectors.push(selector);
            }
            '}' => {
                push_declaration(&mut buffer, &selectors, &mut declarations);
                selectors.pop();
            }
            ';' => {
                push_declaration(&mut buffer, &selectors, &mut declarations);
            }
            _ => buffer.push(ch),
        }
    }
    push_declaration(&mut buffer, &selectors, &mut declarations);
    declarations
}

/// Analyse le contenu accumulé et l'ajoute aux déclarations si c'en est une.
fn push_declaration(
    buffer: &mut String,
    selectors: &[String],
    declarations: &mut Vec<CssDeclaration>,
) {
    let text = std::mem::take(buffer);
    let text = text.trim();
    let Some(rest) = text.strip_prefix("--") else {
        return;
    };
    let Some(colon) = rest.find(':') else {
        return;
    };
    let name = format!("--{}", rest[..colon].trim());
    if name.len() <= 2 {
        return;
    }
    let value = rest[colon + 1..].trim();
    if value.is_empty() {
        return;
    }
    declarations.push(CssDeclaration {
        name,
        value: value.to_string(),
        mode: mode_of(selectors),
    });
}

/// Mode imposé par une pile de sélecteurs, du plus externe au plus interne.
fn mode_of(selectors: &[String]) -> Option<ThemeMode> {
    for selector in selectors.iter().rev() {
        let lower = selector.to_ascii_lowercase();
        if lower.contains("theme-dark") {
            return Some(ThemeMode::Dark);
        }
        if lower.contains("theme-light") {
            return Some(ThemeMode::Light);
        }
    }
    None
}

/// Retire le contenu des commentaires `/* … */` d'un fragment CSS.
///
/// Les commentaires sont remplacés par un espace afin de ne pas accolader les
/// déclarations voisines.
fn strip_comments(css: &str) -> String {
    let mut out = String::with_capacity(css.len());
    let mut chars = css.chars().peekable();
    let mut in_comment = false;
    while let Some(ch) = chars.next() {
        if in_comment {
            if ch == '*' && chars.peek() == Some(&'/') {
                chars.next();
                in_comment = false;
                out.push(' ');
            }
            continue;
        }
        if ch == '/' && chars.peek() == Some(&'*') {
            chars.next();
            in_comment = true;
            out.push(' ');
            continue;
        }
        out.push(ch);
    }
    out
}

/// Valeurs brutes retenues pour un mode donné : la dernière déclaration
/// d'une propriété l'emporte.
///
/// Les déclarations d'un bloc du mode opposé sont écartées, afin qu'une
/// référence `var(--nom)` ne soit pas résolue avec la valeur de l'autre mode.
fn raw_values(declarations: &[CssDeclaration], mode: Option<ThemeMode>) -> HashMap<String, String> {
    let mut raw: HashMap<String, String> = HashMap::with_capacity(declarations.len());
    for declaration in declarations {
        if !matches_mode(declaration.mode, mode) {
            continue;
        }
        raw.insert(declaration.name.clone(), declaration.value.clone());
    }
    raw
}

/// Indique si une déclaration s'applique au mode demandé.
///
/// Une déclaration sans mode (hors bloc `.theme-dark` / `.theme-light`)
/// s'applique toujours ; de même, une application sans mode filtre (usage de
/// [`ThemeVars::merge_snippets`]) retient toutes les déclarations.
fn matches_mode(found: Option<ThemeMode>, kept: Option<ThemeMode>) -> bool {
    match (found, kept) {
        (None, _) | (_, None) => true,
        (Some(found), Some(kept)) => found == kept,
    }
}

/// Profondeur maximale de résolution des références `var()`.
const MAX_VAR_DEPTH: usize = 8;

/// Remplace les références `var(--nom)` d'une valeur par leur définition.
fn resolve_value(value: &str, map: &HashMap<String, String>, depth: usize) -> String {
    if depth >= MAX_VAR_DEPTH {
        return value.to_string();
    }
    let Some(start) = value.find("var(") else {
        return value.to_string();
    };
    let bytes = value.as_bytes();
    let mut index = start + 4;
    let mut nested = 0usize;
    let mut end = None;
    while index < bytes.len() {
        match bytes[index] {
            b'(' => nested += 1,
            b')' => {
                if nested == 0 {
                    end = Some(index);
                    break;
                }
                nested -= 1;
            }
            _ => {}
        }
        index += 1;
    }
    let Some(end) = end else {
        return value.to_string();
    };
    let inner = value[start + 4..end].trim();
    let (name, fallback) = match inner.split_once(',') {
        Some((name, fallback)) => (name.trim(), Some(fallback.trim())),
        None => (inner, None),
    };
    let replacement = map
        .get(name)
        .map(|referenced| resolve_value(referenced, map, depth + 1))
        .or_else(|| fallback.map(|fallback| fallback.to_string()))
        .unwrap_or_default();
    let mut resolved = String::with_capacity(value.len());
    resolved.push_str(&value[..start]);
    resolved.push_str(&replacement);
    resolved.push_str(&value[end + 1..]);
    resolve_value(&resolved, map, depth + 1)
}

/// Retire le suffixe `!important` d'une valeur CSS.
fn strip_important(value: &str) -> &str {
    let text = value.trim();
    match text.to_ascii_lowercase().rfind("!important") {
        Some(index) if index + "!important".len() == text.len() => text[..index].trim_end(),
        _ => text,
    }
}

/// Applique une valeur de couleur à une variable, si elle est lisible.
fn set_color(slot: &mut Color, value: &str) {
    if let Some(color) = parse_css_color(value) {
        *slot = color;
    }
}

/// Applique une longueur à une variable, si elle est lisible.
fn set_length(slot: &mut f32, value: &str) {
    if let Some(length) = parse_css_length(value) {
        *slot = length;
    }
}

/// Analyse une valeur de couleur CSS.
///
/// Les formats `#rgb`, `#rrggbb` et `rgb()` sont confiés au parseur du vault
/// ([`css_snippets::parse_color`]) ; les formats complémentaires (`#rgba`,
/// `#rrggbbaa`, `rgba()`, `hsl()`, `hsla()`, `transparent`) sont complétés ici.
pub fn parse_css_color(value: &str) -> Option<Color> {
    let text = strip_important(value);
    if text.is_empty() {
        return None;
    }
    if text.eq_ignore_ascii_case("transparent") {
        return Some(Color::TRANSPARENT);
    }
    if let Some((r, g, b)) = css_snippets::parse_color(text) {
        return Some(Color::from_rgb8(r, g, b));
    }
    if let Some(hex) = text.strip_prefix('#') {
        if let Some(color) = parse_hex_color(hex) {
            return Some(color);
        }
    }
    if let Some(color) = parse_rgb_function(text) {
        return Some(color);
    }
    parse_hsl_function(text)
}

/// Analyse une couleur hexadécimale de 3, 4, 6 ou 8 chiffres.
fn parse_hex_color(hex: &str) -> Option<Color> {
    fn nibble(ch: char) -> Option<u8> {
        ch.to_digit(16).map(|value| value as u8)
    }
    let digits: Vec<char> = hex.chars().collect();
    let mut rgba = [255u8; 4];
    match digits.len() {
        3 | 4 => {
            for (index, ch) in digits.iter().enumerate() {
                rgba[index] = nibble(*ch)? * 17;
            }
        }
        6 | 8 => {
            for index in 0..digits.len() / 2 {
                rgba[index] = nibble(digits[index * 2])? * 16 + nibble(digits[index * 2 + 1])?;
            }
        }
        _ => return None,
    }
    Some(Color::from_rgba8(
        rgba[0],
        rgba[1],
        rgba[2],
        f32::from(rgba[3]) / 255.0,
    ))
}

/// Arguments d'une fonction CSS (`rgb()`, `hsl()`, …), séparés par virgules
/// ou par espaces.
fn function_args(text: &str, names: &[&str]) -> Option<Vec<String>> {
    let lower = text.to_ascii_lowercase();
    for name in names {
        let prefix = format!("{name}(");
        if !lower.starts_with(&prefix) {
            continue;
        }
        let inner = text[prefix.len()..].strip_suffix(')')?;
        let parts = if inner.contains(',') {
            inner
                .split(',')
                .map(|part| part.trim().to_string())
                .collect()
        } else {
            inner
                .split(['/', ' ', '\t'])
                .map(|part| part.trim().to_string())
                .filter(|part| !part.is_empty())
                .collect()
        };
        return Some(parts);
    }
    None
}

/// Analyse une couleur `rgb()` ou `rgba()`.
fn parse_rgb_function(text: &str) -> Option<Color> {
    let parts = function_args(text, &["rgba", "rgb"])?;
    if parts.len() < 3 {
        return None;
    }
    let r = parse_channel(&parts[0])?;
    let g = parse_channel(&parts[1])?;
    let b = parse_channel(&parts[2])?;
    let alpha = match parts.get(3) {
        Some(value) => parse_alpha(value)?,
        None => 1.0,
    };
    Some(Color::from_rgba8(r, g, b, alpha))
}

/// Composante RGB : nombre de `0` à `255` ou pourcentage.
fn parse_channel(value: &str) -> Option<u8> {
    let text = value.trim();
    if let Some(percent) = text.strip_suffix('%') {
        let number: f32 = percent.trim().parse().ok()?;
        return Some((number.clamp(0.0, 100.0) / 100.0 * 255.0).round() as u8);
    }
    let number: f32 = text.parse().ok()?;
    Some(number.clamp(0.0, 255.0).round() as u8)
}

/// Alpha CSS : nombre de `0` à `1` ou pourcentage.
fn parse_alpha(value: &str) -> Option<f32> {
    let text = value.trim();
    if let Some(percent) = text.strip_suffix('%') {
        let number: f32 = percent.trim().parse().ok()?;
        return Some((number / 100.0).clamp(0.0, 1.0));
    }
    let number: f32 = text.parse().ok()?;
    Some(number.clamp(0.0, 1.0))
}

/// Analyse une couleur `hsl()` ou `hsla()`.
fn parse_hsl_function(text: &str) -> Option<Color> {
    let parts = function_args(text, &["hsla", "hsl"])?;
    if parts.len() < 3 {
        return None;
    }
    let hue = parse_angle(&parts[0])?;
    let saturation = parse_percent(&parts[1])?;
    let lightness = parse_percent(&parts[2])?;
    let alpha = match parts.get(3) {
        Some(value) => parse_alpha(value)?,
        None => 1.0,
    };
    let (r, g, b) = hsl_to_rgb(hue, saturation, lightness);
    Some(Color::from_rgba8(r, g, b, alpha))
}

/// Angle CSS en degrés (`240`, `240deg`, `0.5turn`).
fn parse_angle(value: &str) -> Option<f32> {
    let text = value.trim().to_ascii_lowercase();
    if let Some(number) = text.strip_suffix("deg") {
        return number.trim().parse().ok();
    }
    if let Some(number) = text.strip_suffix("turn") {
        return number.trim().parse::<f32>().ok().map(|value| value * 360.0);
    }
    if let Some(number) = text.strip_suffix("rad") {
        return number
            .trim()
            .parse::<f32>()
            .ok()
            .map(|value| value.to_degrees());
    }
    text.parse().ok()
}

/// Pourcentage CSS (`66%`).
fn parse_percent(value: &str) -> Option<f32> {
    let number: f32 = value.trim().strip_suffix('%')?.trim().parse().ok()?;
    Some(number.clamp(0.0, 100.0) / 100.0)
}

/// Convertit une couleur HSL en composantes RGB.
fn hsl_to_rgb(hue: f32, saturation: f32, lightness: f32) -> (u8, u8, u8) {
    let hue = hue.rem_euclid(360.0) / 30.0;
    let chroma = (1.0 - (2.0 * lightness - 1.0).abs()) * saturation;
    let second = chroma * (1.0 - (hue % 2.0 - 1.0).abs());
    let (r, g, b) = match hue as u32 {
        0 => (chroma, second, 0.0),
        1 => (second, chroma, 0.0),
        2 => (0.0, chroma, second),
        3 => (0.0, second, chroma),
        4 => (second, 0.0, chroma),
        5 => (chroma, 0.0, second),
        _ => (chroma, 0.0, second),
    };
    let offset = lightness - chroma / 2.0;
    let to_byte = |value: f32| ((value + offset).clamp(0.0, 1.0) * 255.0).round() as u8;
    (to_byte(r), to_byte(g), to_byte(b))
}

/// Analyse une longueur CSS en pixels.
///
/// Les unités `px`, `rem`, `em`, `pt`, `in`, `cm` et `mm` sont comprises ; une
/// valeur sans unité est lue comme un nombre de pixels.
pub fn parse_css_length(value: &str) -> Option<f32> {
    let text = strip_important(value).to_ascii_lowercase();
    if text.is_empty() {
        return None;
    }
    let split = text
        .find(|ch: char| !(ch.is_ascii_digit() || ch == '.' || ch == '-' || ch == '+'))
        .unwrap_or(text.len());
    let number: f32 = text[..split].trim().parse().ok()?;
    let factor = match text[split..].trim() {
        "" | "px" => 1.0,
        "rem" | "em" => 16.0,
        "pt" => 4.0 / 3.0,
        "in" => 96.0,
        "cm" => 96.0 / 2.54,
        "mm" => 96.0 / 25.4,
        _ => return None,
    };
    Some(number * factor)
}
