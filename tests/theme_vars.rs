//! Tests du système de variables de thème (`src/ui/theme_vars.rs`).
//!
//! Chaque test vérifie un comportement : extraction des propriétés CSS
//! personnalisées, ordre de surcharge (thème puis snippet), valeurs par
//! défaut, et chargement depuis un vault temporaire.

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use iced::Color;

use obsy::ui::theme::theme_vars::{extract_declarations, load_theme_vars, ThemeMode, ThemeVars};
use obsy::ui::theme::{colors_from_vars, font, font_from_vars, palette_from_vars};
use obsy::vault::themes::theme_css_source;

/// Suffixe unique par vault temporaire (pid + compteur atomique).
fn unique_suffix() -> String {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    format!("{}-{n}", std::process::id())
}

/// Vault temporaire supprimé à la fin du test (y compris en cas de panique).
struct TempVault {
    root: PathBuf,
}

impl TempVault {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("obsy-theme-vars-{}", unique_suffix()));
        fs::create_dir_all(&root).expect("création du vault temporaire");
        Self { root }
    }

    /// Écrit un fichier du vault, en créant les dossiers intermédiaires.
    fn write(&self, relative: &str, contents: &str) -> PathBuf {
        let path = self.root.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("création du dossier parent");
        }
        fs::write(&path, contents).expect("écriture du fichier");
        path
    }
}

impl Drop for TempVault {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

/// 1. Les propriétés `--nom: valeur;` sont extraites, y compris dans `:root`.
#[test]
fn extraction_des_proprietes_custom() {
    let css = ":root {\n  --background-primary: #202020;\n  --text-normal: #e0e0e0;\n  --font-interface: \"Inter\", sans-serif;\n}\n";
    let vars = ThemeVars::from_css(css);

    assert_eq!(vars.background_primary, Color::from_rgb8(0x20, 0x20, 0x20));
    assert_eq!(vars.text_normal, Color::from_rgb8(0xe0, 0xe0, 0xe0));
    assert_eq!(vars.font_interface, "\"Inter\", sans-serif");
}

/// 2. La dernière déclaration d'une propriété l'emporte.
#[test]
fn derniere_declaration_gagne() {
    let css = "--background-primary: #111111;\n\
               .theme-dark { --background-primary: #222222; }\n\
               :root { --background-primary: #333333; }";
    let vars = ThemeVars::from_css(css);

    assert_eq!(vars.background_primary, Color::from_rgb8(0x33, 0x33, 0x33));
}

/// 3. Un snippet chargé après le thème écrase la valeur du thème.
#[test]
fn snippet_ecrase_le_theme() {
    let mut vars = ThemeVars::from_css("--background-primary: #101010;");
    vars.merge_snippets("--background-primary: #202020;");

    assert_eq!(vars.background_primary, Color::from_rgb8(0x20, 0x20, 0x20));
}

/// 4. Les valeurs par défaut existent pour le mode sombre et le mode clair.
#[test]
fn defauts_sombre_et_clair() {
    let dark = ThemeVars::defaults_dark();
    let light = ThemeVars::defaults_light();

    assert_ne!(dark.background_primary, light.background_primary);
    assert_ne!(dark.text_normal, light.text_normal);
    // Le mode sombre est réellement sombre, le mode clair réellement clair.
    assert!(dark.background_primary.r < 0.2, "fond sombre attendu");
    assert!(light.background_primary.r > 0.9, "fond clair attendu");
    assert!(
        dark.text_normal.r > 0.6,
        "texte clair attendu sur fond sombre"
    );
    assert!(
        light.text_normal.r < 0.2,
        "texte foncé attendu sur fond clair"
    );
}

// Les valeurs par défaut du mode sombre correspondent aux mesures de la spec
// (UI-SPEC §1.1 / §5.1).
#[test]
fn defauts_sombre_mesures_spec() {
    let dark = ThemeVars::defaults_dark();
    assert_eq!(dark.background_primary, Color::from_rgb8(0x1c, 0x1c, 0x1c));
    assert_eq!(
        dark.background_secondary,
        Color::from_rgb8(0x28, 0x28, 0x28)
    );
    assert_eq!(dark.text_muted, Color::from_rgb8(0xb3, 0xb3, 0xb3));
    assert_eq!(dark.text_accent, Color::from_rgb8(0xa6, 0x8a, 0xf9));
    assert_eq!(dark.accent_hover, Color::from_rgb8(0xa6, 0x8a, 0xf9));
    assert_eq!(
        dark.background_modifier_hover,
        Color::from_rgba(1.0, 1.0, 1.0, 0.067)
    );
}

/// 5. Les déclarations situées dans un commentaire sont ignorées.
#[test]
fn commentaires_ignores() {
    let css = "--background-primary: #ff0000;\n\
               /* --background-primary: #00ff00; */\n\
               /*\n\
               .theme-dark { --text-normal: #0000ff; }\n\
               */";
    let vars = ThemeVars::from_css(css);

    assert_eq!(vars.background_primary, Color::from_rgb8(0xff, 0x00, 0x00));
    assert_eq!(vars.text_normal, ThemeVars::defaults_dark().text_normal);
}

/// 6. Une propriété inconnue est ignorée et ne modifie aucune variable.
#[test]
fn propriete_inconnue_ignoree() {
    let defaults = ThemeVars::defaults_dark();
    let vars =
        ThemeVars::from_css("--propriete-inconnue: #ff0000; --background-primaire: #ff0000;");

    assert_eq!(vars, defaults);
}

/// 7. Un bloc du mode opposé n'est pas appliqué.
#[test]
fn bloc_du_mode_oppose_ignore() {
    let css = ":root { --background-primary: #111111; }\n\
               .theme-light { --background-primary: #eeeeee; }";

    let dark = ThemeVars::from_css_for_mode(css, ThemeMode::Dark);
    let light = ThemeVars::from_css_for_mode(css, ThemeMode::Light);

    assert_eq!(dark.background_primary, Color::from_rgb8(0x11, 0x11, 0x11));
    assert_eq!(light.background_primary, Color::from_rgb8(0xee, 0xee, 0xee));
}

/// 8. Les références `var(--nom)` sont résolues.
#[test]
fn reference_var_resolue() {
    let css = ":root {\n  --base: #123456;\n  --background-primary: var(--base);\n}";
    let vars = ThemeVars::from_css(css);

    assert_eq!(vars.background_primary, Color::from_rgb8(0x12, 0x34, 0x56));
}

/// 9. La palette iced reflète les variables de thème.
#[test]
fn palette_reflete_le_theme() {
    let vars = ThemeVars::from_css(
        "--background-primary: #101010;\n\
         --text-normal: #f0f0f0;\n\
         --interactive-accent: #00ff00;",
    );
    let palette = palette_from_vars(&vars);

    assert_eq!(palette.background, Color::from_rgb8(0x10, 0x10, 0x10));
    assert_eq!(palette.text, Color::from_rgb8(0xf0, 0xf0, 0xf0));
    assert_eq!(palette.primary, Color::from_rgb8(0x00, 0xff, 0x00));
}

/// 10. Les surfaces et bordures sont exposées à partir des variables.
#[test]
fn couleurs_dinterface_refletent_le_theme() {
    let vars = ThemeVars::from_css(
        "--background-secondary: #202020;\n\
         --background-modifier-border: #303030;",
    );
    let colors = colors_from_vars(&vars);

    assert_eq!(colors.surface, Color::from_rgb8(0x20, 0x20, 0x20));
    assert_eq!(colors.border, Color::from_rgb8(0x30, 0x30, 0x30));
}

/// 11. Une police non embarquée retombe sur la police par défaut.
#[test]
fn police_inconnue_retombe_sur_inter() {
    let vars = ThemeVars::from_css("--font-interface: \"Comic Sans MS\", cursive;");

    assert_eq!(font_from_vars(&vars), font());
}

/// 12. Le scanner expose les déclarations avec leur mode de bloc.
#[test]
fn scanneur_expose_le_mode_des_blocs() {
    let declarations = extract_declarations(
        ":root { --a: 1px; }\n\
         .theme-dark { --b: 2px; }\n\
         .theme-light { --c: 3px; }",
    );

    assert_eq!(declarations.len(), 3);
    assert_eq!(declarations[0].mode, None);
    assert_eq!(declarations[1].mode, Some(ThemeMode::Dark));
    assert_eq!(declarations[2].mode, Some(ThemeMode::Light));
    assert_eq!(declarations[0].value, "1px");
}

/// 13. Le fichier CSS du thème actif du vault est résolu et chargé.
#[test]
fn theme_du_vault_est_resolu() {
    let vault = TempVault::new();
    vault.write(
        ".obsidian/themes/mon-theme.css",
        ":root { --background-primary: #010203; }",
    );

    let source = theme_css_source(&vault.root, "mon-theme");
    assert!(source.is_some(), "le fichier de thème doit être trouvé");

    let vars = load_theme_vars(&vault.root, "mon-theme");
    assert_eq!(vars.background_primary, Color::from_rgb8(0x01, 0x02, 0x03));
}

/// 14. Les snippets du vault sont chargés après le thème et l'écrasent.
#[test]
fn snippets_du_vault_ecrasent_le_theme_du_vault() {
    let vault = TempVault::new();
    vault.write(
        ".obsidian/themes/mon-theme.css",
        ":root { --background-primary: #010203; }",
    );
    vault.write(
        ".obsidian/snippets/a.css",
        ":root { --background-primary: #040506; }",
    );

    let vars = load_theme_vars(&vault.root, "mon-theme");
    assert_eq!(vars.background_primary, Color::from_rgb8(0x04, 0x05, 0x06));
}

/// 15. Un vault sans thème ni snippet conserve les valeurs par défaut.
#[test]
fn vault_sans_theme_conserve_les_defauts() {
    let vault = TempVault::new();
    vault.write("note.md", "# Note");

    let vars = load_theme_vars(&vault.root, "dark");
    assert_eq!(vars, ThemeVars::defaults_dark());

    let light = load_theme_vars(&vault.root, "light");
    assert_eq!(light, ThemeVars::defaults_light());
}

/// 16. Un nom de thème contenant un séparateur de chemin est refusé.
#[test]
fn nom_de_theme_avec_chemin_refuse() {
    let vault = TempVault::new();
    vault.write(".obsidian/themes/mon-theme.css", ":root { --a: 1px; }");

    let path = obsy::vault::themes::theme_css_path(&vault.root, "../themes/mon-theme.css");
    assert!(
        path.is_none(),
        "un nom de thème ne doit pas sortir du vault"
    );
}

/// 17. Le dossier des variables du vault est bien celui du vault.
#[test]
fn racine_du_vault_est_utilisee() {
    let vault = TempVault::new();
    vault.write(
        ".obsidian/snippets/z.css",
        ":root { --text-normal: #abcdef; }",
    );

    let vars = load_theme_vars(&vault.root, "dark");
    assert_eq!(vars.text_normal, Color::from_rgb8(0xab, 0xcd, 0xef));
}
