//! Raccourcis clavier : valeurs par défaut, personnalisations
//! `.obsidian/hotkeys.json` et cohérence avec la palette de commandes.

use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

use obsy::app::palette_commands;
use obsy::vault::keymap::{self, KeyAction, KeyMap};
use obsy::vault::settings::Hotkey;

/// Vault temporaire, supprimé à la fin du test.
struct Sandbox {
    dir: PathBuf,
}

impl Sandbox {
    fn new() -> Self {
        let mut dir = std::env::temp_dir();
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        dir.push(format!("obsy-keymap-{unique}"));
        fs::create_dir_all(&dir).unwrap();
        Self { dir }
    }

    /// Écrit `.obsidian/hotkeys.json` avec le contenu donné.
    fn write_hotkeys(&self, content: &str) {
        let obsidian = self.dir.join(".obsidian");
        fs::create_dir_all(&obsidian).unwrap();
        fs::write(obsidian.join("hotkeys.json"), content).unwrap();
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

/// Liste de modificateurs canoniques.
fn modifiers(names: &[&str]) -> Vec<String> {
    names.iter().map(|name| name.to_string()).collect()
}

/// Vérifie que les combinaisons par défaut résolvent vers les actions
/// attendues (le jeu de raccourcis façon Obsidian).
#[test]
fn les_raccourcis_par_defaut_resolvent_les_actions_attendus() {
    let keymap = KeyMap::defaults();
    let ctrl = modifiers(&["Ctrl"]);
    let ctrl_maj = modifiers(&["Ctrl", "Shift"]);
    let ctrl_alt = modifiers(&["Ctrl", "Alt"]);
    assert_eq!(
        keymap.action_for(&ctrl, "P"),
        Some(KeyAction::OpenCommandPalette)
    );
    assert_eq!(keymap.action_for(&ctrl, "O"), Some(KeyAction::OpenSwitcher));
    assert_eq!(keymap.action_for(&ctrl, "N"), Some(KeyAction::NewNote));
    assert_eq!(keymap.action_for(&ctrl, "S"), Some(KeyAction::SaveNote));
    assert_eq!(keymap.action_for(&ctrl_maj, "S"), Some(KeyAction::SaveAs));
    assert_eq!(
        keymap.action_for(&ctrl_maj, "F"),
        Some(KeyAction::SearchInAllFiles)
    );
    assert_eq!(
        keymap.action_for(&ctrl, "E"),
        Some(KeyAction::ToggleEditPreview)
    );
    assert_eq!(keymap.action_for(&ctrl, "Tab"), Some(KeyAction::NextTab));
    assert_eq!(
        keymap.action_for(&ctrl_maj, "Tab"),
        Some(KeyAction::PrevTab)
    );
    assert_eq!(keymap.action_for(&ctrl, "W"), Some(KeyAction::CloseTab));
    assert_eq!(
        keymap.action_for(&ctrl, "Comma"),
        Some(KeyAction::OpenSettings)
    );
    assert_eq!(
        keymap.action_for(&ctrl_alt, "Left"),
        Some(KeyAction::NavigateBack)
    );
    assert_eq!(
        keymap.action_for(&ctrl_alt, "Right"),
        Some(KeyAction::NavigateForward)
    );
    assert_eq!(
        keymap.action_for(&ctrl_maj, "D"),
        Some(KeyAction::ToggleLeftSidebar)
    );
    assert_eq!(
        keymap.action_for(&ctrl_maj, "R"),
        Some(KeyAction::ToggleRightSidebar)
    );
    assert_eq!(keymap.action_for(&[], "F1"), Some(KeyAction::ShortcutHelp));
    assert_eq!(keymap.action_for(&[], "Escape"), Some(KeyAction::Escape));
    assert_eq!(keymap.action_for(&ctrl, "G"), Some(KeyAction::ToggleGraph));
}

/// Vérifie que chaque action du catalogue a exactement une liaison principale.
#[test]
fn chaque_action_du_catalogue_a_une_liaison_principale() {
    let keymap = KeyMap::defaults();
    assert_eq!(keymap.bindings().len(), keymap::all_actions().len());
    for action in keymap::all_actions() {
        let count = keymap
            .bindings()
            .iter()
            .filter(|shortcut| shortcut.action == *action)
            .count();
        assert_eq!(count, 1, "l'action {action:?} a {count} liaisons");
        assert!(keymap.shortcut_for(*action).is_some());
    }
}

/// Vérifie qu'un raccourci de `hotkeys.json` remplace la liaison par défaut.
#[test]
fn un_override_de_hotkeys_json_remplace_le_raccourci_par_defaut() {
    let hotkeys = vec![Hotkey {
        command: "new-note".to_string(),
        keys: vec!["Mod+Alt+N".to_string()],
    }];
    let keymap = KeyMap::from_hotkeys(&hotkeys);
    // La nouvelle combinaison déclenche l'action.
    assert_eq!(
        keymap.action_for(&modifiers(&["Ctrl", "Alt"]), "N"),
        Some(KeyAction::NewNote)
    );
    // La combinaison par défaut ne déclenche plus rien.
    assert_eq!(keymap.action_for(&modifiers(&["Ctrl"]), "N"), None);
    // Les autres actions gardent leur liaison par défaut.
    assert_eq!(
        keymap.action_for(&modifiers(&["Ctrl"]), "S"),
        Some(KeyAction::SaveNote)
    );
}

/// Vérifie qu'un `hotkeys.json` valide est appliqué par le vault.
#[test]
fn un_hotkeys_json_valide_est_applique_par_le_vault() {
    let sandbox = Sandbox::new();
    sandbox.write_hotkeys(r#"[{"command": "open-switcher", "keys": ["Mod+Shift+O"]}]"#);
    let keymap = KeyMap::from_vault(&sandbox.dir);
    assert_eq!(
        keymap.action_for(&modifiers(&["Ctrl", "Shift"]), "O"),
        Some(KeyAction::OpenSwitcher)
    );
    assert_eq!(keymap.action_for(&modifiers(&["Ctrl"]), "O"), None);
}

/// Vérifie qu'un `hotkeys.json` illisible est ignoré, sans panique.
#[test]
fn un_hotkeys_json_invalide_est_ignore_sans_panique() {
    let sandbox = Sandbox::new();
    sandbox.write_hotkeys("{ ceci n'est pas du json");
    let keymap = KeyMap::from_vault(&sandbox.dir);
    // Les valeurs par défaut restent en place.
    assert_eq!(
        keymap.action_for(&modifiers(&["Ctrl"]), "N"),
        Some(KeyAction::NewNote)
    );
    assert_eq!(
        keymap.action_for(&modifiers(&["Ctrl"]), "P"),
        Some(KeyAction::OpenCommandPalette)
    );
    assert_eq!(keymap.bindings().len(), keymap::all_actions().len());
}

/// Vérifie qu'un identifiant de commande inconnu est ignoré.
#[test]
fn les_identifiants_de_commandes_inconnus_sont_ignores() {
    let hotkeys = vec![Hotkey {
        command: "commande-inexistante".to_string(),
        keys: vec!["Mod+Z".to_string()],
    }];
    let keymap = KeyMap::from_hotkeys(&hotkeys);
    // Le raccourci inconnu n'est lié à rien.
    assert_eq!(keymap.action_for(&modifiers(&["Ctrl"]), "Z"), None);
    // Les valeurs par défaut sont intactes.
    assert_eq!(
        keymap.action_for(&modifiers(&["Ctrl"]), "N"),
        Some(KeyAction::NewNote)
    );
}

/// Vérifie qu'aucune combinaison n'est liée à deux actions.
#[test]
fn aucun_raccourci_n_est_lie_deux_fois() {
    let keymap = KeyMap::defaults();
    let mut seen: HashSet<(Vec<String>, String)> = HashSet::new();
    for shortcut in keymap.bindings().iter().chain(keymap.legacy_bindings()) {
        let chord = (shortcut.modifiers.clone(), shortcut.key.clone());
        assert!(
            seen.insert(chord.clone()),
            "combinaison liée deux fois : {chord:?}"
        );
    }
    // Après personnalisation, l'ancienne combinaison est libérée.
    let hotkeys = vec![Hotkey {
        command: "new-note".to_string(),
        keys: vec!["Mod+Alt+N".to_string()],
    }];
    let personalized = KeyMap::from_hotkeys(&hotkeys);
    let mut seen: HashSet<(Vec<String>, String)> = HashSet::new();
    for shortcut in personalized
        .bindings()
        .iter()
        .chain(personalized.legacy_bindings())
    {
        let chord = (shortcut.modifiers.clone(), shortcut.key.clone());
        assert!(
            seen.insert(chord.clone()),
            "combinaison liée deux fois : {chord:?}"
        );
    }
}

/// Vérifie que l'action d'aide des raccourcis existe et est liée à F1.
#[test]
fn l_aide_des_raccourcis_est_liee_a_f1() {
    let keymap = KeyMap::defaults();
    assert_eq!(keymap.action_for(&[], "F1"), Some(KeyAction::ShortcutHelp));
    let shortcut = keymap
        .shortcut_for(KeyAction::ShortcutHelp)
        .expect("liaison de l'aide");
    assert_eq!(shortcut.key, "F1");
    assert!(shortcut.modifiers.is_empty());
    assert_eq!(keymap.display_for(KeyAction::ShortcutHelp), "F1");
}

/// Vérifie que la palette liste chaque action du catalogue une seule fois.
#[test]
fn la_palette_liste_chaque_action_une_seule_fois() {
    let keymap = KeyMap::defaults();
    let commands = palette_commands(&keymap);
    for action in keymap::all_actions() {
        let count = commands
            .iter()
            .filter(|command| command.shortcut == Some(*action))
            .count();
        assert_eq!(count, 1, "l'action {action:?} apparaît {count} fois");
    }
    // Les identifiants de la palette sont uniques.
    let ids: HashSet<&str> = commands.iter().map(|command| command.id).collect();
    assert_eq!(ids.len(), commands.len());
}

/// Vérifie qu'un modificateur supplémentaire ne déclenche pas l'action.
#[test]
fn un_modificateur_supplementaire_ne_declenche_pas_l_action() {
    let keymap = KeyMap::defaults();
    assert_eq!(keymap.action_for(&modifiers(&["Ctrl", "Shift"]), "P"), None);
    assert_eq!(keymap.action_for(&modifiers(&["Ctrl", "Alt"]), "P"), None);
}

/// Vérifie qu'une combinaison sans touche est ignorée.
#[test]
fn un_raccourci_sans_touche_est_ignore() {
    let hotkeys = vec![Hotkey {
        command: "new-note".to_string(),
        keys: vec!["Ctrl".to_string(), "Shift".to_string()],
    }];
    let keymap = KeyMap::from_hotkeys(&hotkeys);
    // La personnalisation illisible est ignorée : le défaut reste actif.
    assert_eq!(
        keymap.action_for(&modifiers(&["Ctrl"]), "N"),
        Some(KeyAction::NewNote)
    );
}

/// Vérifie que l'affichage du raccourci est lisible.
#[test]
fn l_affichage_du_raccourci_est_lisible() {
    let keymap = KeyMap::defaults();
    assert_eq!(keymap.display_for(KeyAction::SaveNote), "Ctrl + S");
    assert_eq!(keymap.display_for(KeyAction::SaveAs), "Ctrl + Maj + S");
    assert_eq!(keymap.display_for(KeyAction::Escape), "Échap");
    assert_eq!(
        keymap.display_for(KeyAction::OpenSettings),
        "Ctrl + ,",
        "la virgule doit rester lisible"
    );
}
