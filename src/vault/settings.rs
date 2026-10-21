//! Paramètres d'application (`.obsidian/app.json`) et raccourcis clavier
//! personnalisés (`.obsidian/hotkeys.json`).
//!
//! Format JSON simple aligné sur les conventions Obsidian : les champs
//! inconnus sont ignorés, les champs absents prennent leur valeur par défaut,
//! et chaque écriture est atomique (fichier temporaire + rename).

use std::fs;
use std::path::Path;

/// Nom du fichier des paramètres dans `.obsidian/`.
const APP_SETTINGS_FILE: &str = "app.json";

/// Nom du fichier des raccourcis clavier dans `.obsidian/`.
const HOTKEYS_FILE: &str = "hotkeys.json";

/// Paramètres globaux de l'application.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AppSettings {
    /// Thème actif (`"dark"` ou `"light"`).
    #[serde(default)]
    pub theme: String,
    /// Mode Vim de l'éditeur.
    #[serde(default)]
    pub vim_mode: bool,
    /// Correcteur orthographique.
    #[serde(default)]
    pub spellcheck: bool,
    /// Dossier des gabarits (relatif au vault).
    #[serde(default)]
    pub templates_dir: String,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            theme: "dark".to_string(),
            vim_mode: false,
            spellcheck: false,
            templates_dir: "Templates".to_string(),
        }
    }
}

/// Un raccourci clavier personnalisé.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Hotkey {
    /// Identifiant de la commande déclenchée.
    pub command: String,
    /// Combinaison de touches (modificateurs puis touche).
    pub keys: Vec<String>,
}

/// Liste des raccourcis clavier personnalisés.
pub type Hotkeys = Vec<Hotkey>;

/// Lit `.obsidian/app.json` et retourne les paramètres.
///
/// Fichier absent ou vide → [`AppSettings::default`]. JSON invalide → erreur.
/// Les champs inconnus sont ignorés ; les champs absents prennent leur valeur
/// par défaut.
pub fn load_app_settings(vault_root: &Path) -> anyhow::Result<AppSettings> {
    let file = vault_root.join(".obsidian").join(APP_SETTINGS_FILE);
    match read_optional_file(&file)? {
        Some(raw) => Ok(serde_json::from_str(&raw)?),
        None => Ok(AppSettings::default()),
    }
}

/// Écrit `.obsidian/app.json` de façon atomique (temporaire + rename), en
/// créant `.obsidian/` si nécessaire.
pub fn save_app_settings(vault_root: &Path, settings: &AppSettings) -> anyhow::Result<()> {
    let file = vault_root.join(".obsidian").join(APP_SETTINGS_FILE);
    let data = serde_json::to_string_pretty(settings)?;
    write_atomic(&file, &data)
}

/// Lit `.obsidian/hotkeys.json` et retourne la liste des raccourcis.
///
/// Fichier absent ou vide → liste vide. JSON invalide → erreur.
pub fn load_hotkeys(vault_root: &Path) -> anyhow::Result<Hotkeys> {
    let file = vault_root.join(".obsidian").join(HOTKEYS_FILE);
    match read_optional_file(&file)? {
        Some(raw) => Ok(serde_json::from_str(&raw)?),
        None => Ok(Vec::new()),
    }
}

/// Écrit `.obsidian/hotkeys.json` de façon atomique (temporaire + rename), en
/// créant `.obsidian/` si nécessaire.
pub fn save_hotkeys(vault_root: &Path, hotkeys: &Hotkeys) -> anyhow::Result<()> {
    let file = vault_root.join(".obsidian").join(HOTKEYS_FILE);
    let data = serde_json::to_string_pretty(hotkeys)?;
    write_atomic(&file, &data)
}

/// Lit un fichier optionnel : absent ou vide → `None`, sinon contenu brut.
fn read_optional_file(file: &Path) -> anyhow::Result<Option<String>> {
    match fs::read_to_string(file) {
        Ok(raw) => {
            if raw.trim().is_empty() {
                Ok(None)
            } else {
                Ok(Some(raw))
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}

/// Écrit `content` dans `target` de façon atomique : fichier temporaire dans le
/// même dossier, puis `rename`. Crée le dossier parent si nécessaire.
fn write_atomic(target: &Path, content: &str) -> anyhow::Result<()> {
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = target.with_extension("json.tmp");
    fs::write(&tmp, content)?;
    fs::rename(&tmp, target)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    struct Sandbox {
        dir: PathBuf,
    }

    impl Sandbox {
        fn new() -> Self {
            let mut dir = std::env::temp_dir();
            let unique = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            dir.push(format!("obsidian-rs-settings-{unique}"));
            fs::create_dir_all(&dir).unwrap();
            Self { dir }
        }

        fn obsidian(&self) -> PathBuf {
            self.dir.join(".obsidian")
        }

        fn ecrit(&self, nom: &str, contenu: &str) {
            fs::create_dir_all(self.obsidian()).unwrap();
            fs::write(self.obsidian().join(nom), contenu).unwrap();
        }
    }

    impl Drop for Sandbox {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.dir);
        }
    }

    #[test]
    fn app_settings_fichier_absent_retourne_defaut() {
        let sb = Sandbox::new();
        assert_eq!(load_app_settings(&sb.dir).unwrap(), AppSettings::default());
    }

    #[test]
    fn app_settings_fichier_vide_retourne_defaut() {
        let sb = Sandbox::new();
        sb.ecrit(APP_SETTINGS_FILE, "");
        assert_eq!(load_app_settings(&sb.dir).unwrap(), AppSettings::default());
    }

    #[test]
    fn app_settings_roundtrip_defaut() {
        let sb = Sandbox::new();
        save_app_settings(&sb.dir, &AppSettings::default()).unwrap();
        assert_eq!(load_app_settings(&sb.dir).unwrap(), AppSettings::default());
    }

    #[test]
    fn app_settings_roundtrip_personnalises() {
        let sb = Sandbox::new();
        let settings = AppSettings {
            theme: "light".to_string(),
            vim_mode: true,
            spellcheck: true,
            templates_dir: "Gabarits".to_string(),
        };
        save_app_settings(&sb.dir, &settings).unwrap();
        assert_eq!(load_app_settings(&sb.dir).unwrap(), settings);
    }

    #[test]
    fn app_settings_json_invalide_erreur() {
        let sb = Sandbox::new();
        sb.ecrit(APP_SETTINGS_FILE, "{invalide");
        assert!(load_app_settings(&sb.dir).is_err());
    }

    #[test]
    fn app_settings_champs_inconnus_ignores() {
        let sb = Sandbox::new();
        sb.ecrit(
            APP_SETTINGS_FILE,
            r#"{"theme": "light", "champInconnu": 42}"#,
        );
        let settings = load_app_settings(&sb.dir).unwrap();
        assert_eq!(settings.theme, "light");
        assert!(!settings.vim_mode);
        assert!(!settings.spellcheck);
    }

    #[test]
    fn app_settings_champs_absents_utilisent_valeurs_serde() {
        let sb = Sandbox::new();
        sb.ecrit(APP_SETTINGS_FILE, "{}");
        let settings = load_app_settings(&sb.dir).unwrap();
        assert_eq!(settings.theme, "");
        assert!(!settings.vim_mode);
        assert!(!settings.spellcheck);
    }

    #[test]
    fn hotkeys_fichier_absent_retourne_vide() {
        let sb = Sandbox::new();
        assert!(load_hotkeys(&sb.dir).unwrap().is_empty());
    }

    #[test]
    fn hotkeys_fichier_vide_retourne_vide() {
        let sb = Sandbox::new();
        sb.ecrit(HOTKEYS_FILE, "");
        assert!(load_hotkeys(&sb.dir).unwrap().is_empty());
    }

    #[test]
    fn hotkeys_roundtrip_vide() {
        let sb = Sandbox::new();
        let hotkeys: Hotkeys = Vec::new();
        save_hotkeys(&sb.dir, &hotkeys).unwrap();
        assert!(load_hotkeys(&sb.dir).unwrap().is_empty());
    }

    #[test]
    fn hotkeys_roundtrip_avec_entrees() {
        let sb = Sandbox::new();
        let hotkeys: Hotkeys = vec![
            Hotkey {
                command: "note:open".to_string(),
                keys: vec!["Mod".to_string(), "O".to_string()],
            },
            Hotkey {
                command: "graph:toggle".to_string(),
                keys: vec!["Mod".to_string(), "G".to_string()],
            },
        ];
        save_hotkeys(&sb.dir, &hotkeys).unwrap();
        assert_eq!(load_hotkeys(&sb.dir).unwrap(), hotkeys);
    }

    #[test]
    fn hotkeys_json_invalide_erreur() {
        let sb = Sandbox::new();
        sb.ecrit(HOTKEYS_FILE, "[{invalide");
        assert!(load_hotkeys(&sb.dir).is_err());
    }

    #[test]
    fn ecriture_atomique_sans_temporaire_residuel() {
        let sb = Sandbox::new();
        save_app_settings(&sb.dir, &AppSettings::default()).unwrap();
        save_hotkeys(&sb.dir, &Vec::<Hotkey>::new()).unwrap();

        let nom_fichiers: Vec<_> = fs::read_dir(sb.obsidian())
            .unwrap()
            .map(|entree| entree.unwrap().file_name())
            .collect();
        assert_eq!(nom_fichiers.len(), 2);
        assert!(nom_fichiers
            .iter()
            .all(|nom| !nom.to_string_lossy().ends_with(".tmp")));
    }
}
