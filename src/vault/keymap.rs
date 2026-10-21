//! Raccourcis clavier (données pures, sans dépendance UI).
//!
//! Le module ne dépend ni d'iced ni de l'état de l'application : il expose
//! le catalogue des actions, la table de raccourcis par défaut et la
//! résolution des personnalisations du vault (`.obsidian/hotkeys.json`).
//! La couche UI se charge de traduire ces données en événements clavier.
//!
//! Format de personnalisation (`.obsidian/hotkeys.json`, format Obsidian) :
//!
//! ```json
//! [
//!   { "command": "new-note", "keys": ["Mod+Alt+N"] },
//!   { "command": "quick-switcher:open", "keys": ["Mod", "O"] }
//! ]
//! ```
//!
//! Les deux écritures sont acceptées : une combinaison par chaîne (`Mod+O`)
//! ou modificateurs puis touche (`["Mod", "O"]`). Un identifiant de
//! commande inconnu ou une combinaison illisible est ignoré : le fichier
//! n'est jamais fatal.

use std::fmt;
use std::path::Path;

use crate::vault::settings::{load_hotkeys, Hotkey};

/// Actions déclenchées par les raccourcis clavier.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum KeyAction {
    /// Ouvre la palette de commandes (`Ctrl+P`).
    OpenCommandPalette,
    /// Ouvre le sélecteur de notes (`Ctrl+O`).
    OpenSwitcher,
    /// Crée une nouvelle note (`Ctrl+N`).
    NewNote,
    /// Enregistre la note en cours (`Ctrl+S`).
    SaveNote,
    /// Enregistre la note en cours sous un autre nom (`Ctrl+Maj+S`).
    SaveAs,
    /// Recherche dans tout le vault (`Ctrl+Maj+F`).
    SearchInAllFiles,
    /// Bascule entre lecture et édition (`Ctrl+E`).
    ToggleEditPreview,
    /// Onglet de note suivant (`Ctrl+Tab`).
    NextTab,
    /// Onglet de note précédent (`Ctrl+Maj+Tab`).
    PrevTab,
    /// Ferme l'onglet de note actif (`Ctrl+W`).
    CloseTab,
    /// Ouvre les réglages (`Ctrl+,`).
    OpenSettings,
    /// Revient en arrière dans l'historique de navigation (`Ctrl+Alt+←`).
    NavigateBack,
    /// Avance dans l'historique de navigation (`Ctrl+Alt+→`).
    NavigateForward,
    /// Bascule la barre latérale gauche (`Ctrl+Maj+D`).
    ToggleLeftSidebar,
    /// Bascule la barre latérale droite (`Ctrl+Maj+R`).
    ToggleRightSidebar,
    /// Affiche l'aide des raccourcis clavier (`F1`).
    ShortcutHelp,
    /// Annule ou ferme la vue courante (`Échap`).
    Escape,
    /// Bascule la vue du graphe (`Ctrl+G`).
    ToggleGraph,
    /// Alias historique de [`KeyAction::ToggleLeftSidebar`] (`Ctrl+B`).
    ///
    /// Conservé pour la compatibilité avec le registre de commandes
    /// (`vault::commands`), qui le référence encore. Le raccourci `Ctrl+B`
    /// est traité comme une liaison historique de la barre latérale
    /// gauche : il n'apparaît donc pas une seconde fois dans la palette.
    ToggleSidebar,
}

/// Un raccourci clavier : une combinaison de modificateurs et une touche
/// mappés à une `KeyAction`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Shortcut {
    /// Modificateurs, triés par ordre lexicographique pour une comparaison
    /// déterministe (`["Alt", "Ctrl", "Shift"]`).
    pub modifiers: Vec<String>,
    /// Touche canonique, par exemple `"P"`, `"Tab"` ou `"Escape"`.
    pub key: String,
    /// Action déclenchée par ce raccourci.
    pub action: KeyAction,
}

/// Construit un raccourci en triant ses modificateurs.
///
/// Le tri rend la représentation canonique : deux combinaisons identiques
/// écrites dans un ordre différent comparent et hachent de la même façon.
fn shortcut(modifiers: &[&str], key: &str, action: KeyAction) -> Shortcut {
    let mut modifiers: Vec<String> = modifiers.iter().map(|m| m.to_string()).collect();
    modifiers.sort();
    modifiers.dedup();
    Shortcut {
        modifiers,
        key: key.to_string(),
        action,
    }
}

/// Liaisons principales par défaut : une par action du catalogue.
const DEFAULT_BINDINGS: &[(KeyAction, &[&str], &str)] = &[
    (KeyAction::OpenCommandPalette, &["Ctrl"], "P"),
    (KeyAction::OpenSwitcher, &["Ctrl"], "O"),
    (KeyAction::NewNote, &["Ctrl"], "N"),
    (KeyAction::SaveNote, &["Ctrl"], "S"),
    (KeyAction::SaveAs, &["Ctrl", "Shift"], "S"),
    (KeyAction::SearchInAllFiles, &["Ctrl", "Shift"], "F"),
    (KeyAction::ToggleEditPreview, &["Ctrl"], "E"),
    (KeyAction::NextTab, &["Ctrl"], "Tab"),
    (KeyAction::PrevTab, &["Ctrl", "Shift"], "Tab"),
    (KeyAction::CloseTab, &["Ctrl"], "W"),
    (KeyAction::OpenSettings, &["Ctrl"], "Comma"),
    (KeyAction::NavigateBack, &["Ctrl", "Alt"], "Left"),
    (KeyAction::NavigateForward, &["Ctrl", "Alt"], "Right"),
    (KeyAction::ToggleLeftSidebar, &["Ctrl", "Shift"], "D"),
    (KeyAction::ToggleRightSidebar, &["Ctrl", "Shift"], "R"),
    (KeyAction::ShortcutHelp, &[], "F1"),
    (KeyAction::Escape, &[], "Escape"),
    (KeyAction::ToggleGraph, &["Ctrl"], "G"),
];

/// Liaisons historiques conservées en plus des liaisons principales.
///
/// Elles restent actives (rien ne casse) mais ne sont pas affichées : la
/// palette montre la liaison principale de chaque action.
const LEGACY_BINDINGS: &[(KeyAction, &[&str], &str)] = &[
    (KeyAction::OpenSwitcher, &["Ctrl"], "K"),
    (KeyAction::ToggleLeftSidebar, &["Ctrl"], "B"),
    (KeyAction::ToggleRightSidebar, &["Ctrl", "Shift"], "B"),
];

/// Actions du catalogue, dans l'ordre d'affichage de la palette.
const CATALOG: &[KeyAction] = &[
    KeyAction::OpenCommandPalette,
    KeyAction::OpenSwitcher,
    KeyAction::NewNote,
    KeyAction::SaveNote,
    KeyAction::SaveAs,
    KeyAction::SearchInAllFiles,
    KeyAction::ToggleEditPreview,
    KeyAction::NextTab,
    KeyAction::PrevTab,
    KeyAction::CloseTab,
    KeyAction::OpenSettings,
    KeyAction::NavigateBack,
    KeyAction::NavigateForward,
    KeyAction::ToggleLeftSidebar,
    KeyAction::ToggleRightSidebar,
    KeyAction::ShortcutHelp,
    KeyAction::Escape,
    KeyAction::ToggleGraph,
];

/// Liste des raccourcis par défaut (liaisons principales).
///
/// Le vecteur est trié par `KeyAction` afin que l'ordre soit déterministe
/// indépendamment de l'ordre d'écriture.
pub fn default_shortcuts() -> Vec<Shortcut> {
    let mut shortcuts: Vec<Shortcut> = DEFAULT_BINDINGS
        .iter()
        .map(|item| shortcut(item.1, item.2, item.0))
        .collect();
    // `sort_by_key` est stable ; l'ordre suit le discriminant de `KeyAction`.
    shortcuts.sort_by_key(|s| s.action);
    shortcuts
}

/// Liste des liaisons historiques conservées (triées par action).
pub fn legacy_shortcuts() -> Vec<Shortcut> {
    let mut shortcuts: Vec<Shortcut> = LEGACY_BINDINGS
        .iter()
        .map(|item| shortcut(item.1, item.2, item.0))
        .collect();
    shortcuts.sort_by_key(|s| s.action);
    shortcuts
}

/// Actions du catalogue, dans l'ordre d'affichage de la palette.
pub fn all_actions() -> &'static [KeyAction] {
    CATALOG
}

/// Métadonnées d'une action : identifiant, libellé et catégorie.
fn action_meta(action: KeyAction) -> (&'static str, &'static str, &'static str) {
    match action {
        KeyAction::OpenCommandPalette => ("command-palette", "Palette de commandes", "Navigation"),
        KeyAction::OpenSwitcher => (
            "open-switcher",
            "Ouvrir le sélecteur de notes",
            "Navigation",
        ),
        KeyAction::NewNote => ("new-note", "Créer une nouvelle note", "Fichier"),
        KeyAction::SaveNote => ("save-note", "Sauvegarder la note", "Fichier"),
        KeyAction::SaveAs => (
            "save-as",
            "Enregistrer la note sous un autre nom",
            "Fichier",
        ),
        KeyAction::SearchInAllFiles => (
            "toggle-search",
            "Rechercher dans tout le vault",
            "Navigation",
        ),
        KeyAction::ToggleEditPreview => (
            "toggle-editor-mode",
            "Basculer le mode lecture/édition",
            "Vue",
        ),
        KeyAction::NextTab => ("next-tab", "Onglet suivant", "Navigation"),
        KeyAction::PrevTab => ("prev-tab", "Onglet précédent", "Navigation"),
        KeyAction::CloseTab => ("close-tab", "Fermer l'onglet actif", "Navigation"),
        KeyAction::OpenSettings => ("open-settings", "Ouvrir les réglages", "Navigation"),
        KeyAction::NavigateBack => ("navigate-back", "Revenir en arrière", "Navigation"),
        KeyAction::NavigateForward => ("navigate-forward", "Avancer", "Navigation"),
        KeyAction::ToggleLeftSidebar => (
            "toggle-left-sidebar",
            "Basculer la barre latérale gauche",
            "Vue",
        ),
        KeyAction::ToggleRightSidebar => (
            "toggle-right-sidebar",
            "Basculer la barre latérale droite",
            "Vue",
        ),
        KeyAction::ShortcutHelp => ("shortcut-help", "Afficher l'aide des raccourcis", "Aide"),
        KeyAction::Escape => ("escape", "Annuler ou fermer la vue courante", "Navigation"),
        KeyAction::ToggleGraph => ("toggle-graph", "Basculer le graphe", "Vue"),
        KeyAction::ToggleSidebar => ("toggle-sidebar", "Basculer la barre latérale", "Vue"),
    }
}

/// Identifiant canonique d'une action (celui de `hotkeys.json`).
pub fn action_id(action: KeyAction) -> &'static str {
    action_meta(action).0
}

/// Libellé affichable d'une action.
pub fn action_label(action: KeyAction) -> &'static str {
    action_meta(action).1
}

/// Catégorie affichable d'une action (« Fichier », « Vue », …).
pub fn action_category(action: KeyAction) -> &'static str {
    action_meta(action).2
}

/// Action associée à un identifiant de commande.
///
/// Les identifiants canoniques de l'application sont acceptés, ainsi que
/// les identifiants style Obsidian (`file:save`, `workspace:next-tab`, …).
/// Un identifiant inconnu renvoie `None` : le raccourci est alors ignoré.
pub fn action_by_id(id: &str) -> Option<KeyAction> {
    let id = id.trim();
    Some(match id {
        "command-palette" | "command-palette:open" => KeyAction::OpenCommandPalette,
        "open-switcher" | "quick-switcher:open" => KeyAction::OpenSwitcher,
        "new-note" | "file:new" => KeyAction::NewNote,
        "save-note" | "file:save" => KeyAction::SaveNote,
        "save-as" | "file:save-as" => KeyAction::SaveAs,
        "toggle-search" | "search:open-all" => KeyAction::SearchInAllFiles,
        "toggle-editor-mode" | "editor:toggle-source" => KeyAction::ToggleEditPreview,
        "next-tab" | "workspace:next-tab" => KeyAction::NextTab,
        "prev-tab" | "workspace:previous-tab" => KeyAction::PrevTab,
        "close-tab" | "file:close" => KeyAction::CloseTab,
        "open-settings" | "app:open-settings" => KeyAction::OpenSettings,
        "navigate-back" | "workspace:file-navigation:back" => KeyAction::NavigateBack,
        "navigate-forward" | "workspace:file-navigation:forward" => KeyAction::NavigateForward,
        "toggle-left-sidebar" | "toggle-sidebar" | "workspace:toggle-left-sidebar" => {
            KeyAction::ToggleLeftSidebar
        }
        "toggle-right-sidebar" | "workspace:toggle-right-sidebar" => KeyAction::ToggleRightSidebar,
        "shortcut-help" => KeyAction::ShortcutHelp,
        "escape" => KeyAction::Escape,
        "toggle-graph" => KeyAction::ToggleGraph,
        _ => return None,
    })
}

/// Nom de l'action (utilisé quand aucune liaison n'est associée).
fn action_name(action: KeyAction) -> &'static str {
    match action {
        KeyAction::OpenCommandPalette => "OpenCommandPalette",
        KeyAction::OpenSwitcher => "OpenSwitcher",
        KeyAction::NewNote => "NewNote",
        KeyAction::SaveNote => "SaveNote",
        KeyAction::SaveAs => "SaveAs",
        KeyAction::SearchInAllFiles => "SearchInAllFiles",
        KeyAction::ToggleEditPreview => "ToggleEditPreview",
        KeyAction::NextTab => "NextTab",
        KeyAction::PrevTab => "PrevTab",
        KeyAction::CloseTab => "CloseTab",
        KeyAction::OpenSettings => "OpenSettings",
        KeyAction::NavigateBack => "NavigateBack",
        KeyAction::NavigateForward => "NavigateForward",
        KeyAction::ToggleLeftSidebar => "ToggleLeftSidebar",
        KeyAction::ToggleRightSidebar => "ToggleRightSidebar",
        KeyAction::ShortcutHelp => "ShortcutHelp",
        KeyAction::Escape => "Escape",
        KeyAction::ToggleGraph => "ToggleGraph",
        KeyAction::ToggleSidebar => "ToggleSidebar",
    }
}

/// Modificateur canonique associé à un nom de modificateur.
///
/// `Mod` vaut `Ctrl` hors macOS et `Super` (touche commande) sur macOS.
fn canonical_modifier(modifier: &str) -> Option<String> {
    let lower = modifier.trim().to_ascii_lowercase();
    Some(match lower.as_str() {
        "ctrl" | "control" => "Ctrl".to_string(),
        "shift" => "Shift".to_string(),
        "alt" | "option" => "Alt".to_string(),
        "super" | "cmd" | "command" | "meta" | "win" | "windows" => "Super".to_string(),
        "mod" => {
            if cfg!(target_os = "macos") {
                "Super".to_string()
            } else {
                "Ctrl".to_string()
            }
        }
        _ => return None,
    })
}

/// Normalise une liste de modificateurs : noms canoniques, triés, sans
/// doublon. Les noms inconnus sont ignorés.
pub fn canonical_modifiers(modifiers: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for modifier in modifiers {
        if let Some(canonical) = canonical_modifier(modifier) {
            if !out.contains(&canonical) {
                out.push(canonical);
            }
        }
    }
    out.sort();
    out
}

/// Normalise un nom de touche vers sa forme canonique.
///
/// `"p"` et `"P"` donnent `"P"`, `"esc"` donne `"Escape"`, `"f1"` donne
/// `"F1"`. Renvoie `None` si le nom n'est pas une touche connue.
pub fn canonical_key(key: &str) -> Option<String> {
    let trimmed = key.trim();
    if trimmed.is_empty() {
        return None;
    }
    let lower = trimmed.to_ascii_lowercase();
    // Touches de fonction F1..F12.
    if let Some(rest) = lower.strip_prefix('f') {
        if let Ok(number) = rest.parse::<u8>() {
            if (1..=12).contains(&number) {
                return Some(format!("F{number}"));
            }
        }
    }
    Some(match lower.as_str() {
        "esc" | "escape" => "Escape".to_string(),
        "tab" => "Tab".to_string(),
        "enter" | "return" => "Enter".to_string(),
        "space" | "spacebar" => "Space".to_string(),
        "backspace" => "Backspace".to_string(),
        "delete" | "del" => "Delete".to_string(),
        "left" | "arrowleft" => "Left".to_string(),
        "right" | "arrowright" => "Right".to_string(),
        "up" | "arrowup" => "Up".to_string(),
        "down" | "arrowdown" => "Down".to_string(),
        "home" => "Home".to_string(),
        "end" => "End".to_string(),
        "pageup" | "pgup" => "PageUp".to_string(),
        "pagedown" | "pgdn" => "PageDown".to_string(),
        "comma" | "," => "Comma".to_string(),
        "period" | "." => "Period".to_string(),
        "slash" | "/" => "Slash".to_string(),
        _ => {
            // Caractère unique : lettre (mise en majuscule) ou chiffre.
            let mut chars = trimmed.chars();
            let first = chars.next()?;
            if chars.next().is_some() {
                return None;
            }
            let mut upper = first.to_uppercase();
            let upper = upper.next()?;
            upper.to_string()
        }
    })
}

/// Analyse une liste de touches de `hotkeys.json` en combinaison canonique.
///
/// Accepte `["Mod+O"]` (format Obsidian) comme `["Mod", "O"]` (modificateurs
/// puis touche). Renvoie `None` si la combinaison est illisible (liste vide,
/// modificateur sans touche, touche inconnue).
pub fn parse_chord(keys: &[String]) -> Option<(Vec<String>, String)> {
    let mut parts: Vec<String> = Vec::new();
    for entry in keys {
        for part in entry.split('+') {
            let part = part.trim();
            if !part.is_empty() {
                parts.push(part.to_string());
            }
        }
    }
    let (last, modifiers) = parts.split_last()?;
    // Un modificateur en dernière position signifie qu'aucune touche n'est
    // donnée : la combinaison est illisible.
    if canonical_modifier(last).is_some() {
        return None;
    }
    let key = canonical_key(last)?;
    Some((canonical_modifiers(modifiers), key))
}

/// Raccourci formaté pour l'affichage (« Ctrl + Maj + P »).
fn format_shortcut(shortcut: &Shortcut) -> String {
    let mut parts: Vec<String> = shortcut
        .modifiers
        .iter()
        .map(|modifier| match modifier.as_str() {
            "Shift" => "Maj".to_string(),
            other => other.to_string(),
        })
        .collect();
    parts.push(format_key(&shortcut.key));
    parts.join(" + ")
}

/// Touche formatée pour l'affichage (« Échap », « ← », « F1 »).
fn format_key(key: &str) -> String {
    match key {
        "Escape" => "Échap".to_string(),
        "Enter" => "Entrée".to_string(),
        "Space" => "Espace".to_string(),
        "Left" => "←".to_string(),
        "Right" => "→".to_string(),
        "Up" => "↑".to_string(),
        "Down" => "↓".to_string(),
        "Comma" => ",".to_string(),
        "Period" => ".".to_string(),
        "Slash" => "/".to_string(),
        other => other.to_string(),
    }
}

/// Raccourci principal d'une action (celui affiché dans la palette).
fn default_shortcut(action: KeyAction) -> Option<Shortcut> {
    DEFAULT_BINDINGS
        .iter()
        .find(|item| item.0 == action)
        .map(|item| shortcut(item.1, item.2, item.0))
}

impl fmt::Debug for KeyAction {
    /// Affiche la combinaison principale de l'action (« Ctrl+P »).
    ///
    /// La palette de commandes affiche `{:?}` du raccourci de chaque
    /// commande : la combinaison est donc directement lisible.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match default_shortcut(*self) {
            Some(shortcut) => f.write_str(&format_shortcut(&shortcut)),
            None => f.write_str(action_name(*self)),
        }
    }
}

/// Table de raccourcis résolue.
///
/// Elle contient une liaison principale par action (celle affichée dans la
/// palette) et les liaisons historiques conservées.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyMap {
    /// Liaison principale de chaque action.
    primary: Vec<Shortcut>,
    /// Liaisons historiques conservées.
    legacy: Vec<Shortcut>,
}

impl KeyMap {
    /// Table par défaut, sans personnalisation.
    pub fn defaults() -> Self {
        Self {
            primary: default_shortcuts(),
            legacy: legacy_shortcuts(),
        }
    }

    /// Table résolue à partir des raccourcis du vault.
    ///
    /// Un raccourci personalize remplace la liaison principale de l'action
    /// visée (ainsi que ses liaisons historiques) : la personnalisation est
    /// donc prioritaire sur le défaut. Les identifiants de commande
    /// inconnus et les combinaisons illisibles sont ignorés.
    pub fn from_hotkeys(hotkeys: &[Hotkey]) -> Self {
        let mut map = Self::defaults();
        for hotkey in hotkeys {
            let Some(action) = action_by_id(&hotkey.command) else {
                continue;
            };
            let Some((modifiers, key)) = parse_chord(&hotkey.keys) else {
                continue;
            };
            map.primary.retain(|shortcut| shortcut.action != action);
            map.legacy.retain(|shortcut| shortcut.action != action);
            map.primary.push(Shortcut {
                modifiers,
                key,
                action,
            });
        }
        map.primary.sort_by_key(|shortcut| shortcut.action);
        map
    }

    /// Table résolue depuis `.obsidian/hotkeys.json` du vault.
    ///
    /// Fichier absent, vide ou illisible : la table par défaut est
    /// conservée (aucune erreur, aucune panique).
    pub fn from_vault(vault_root: &Path) -> Self {
        match load_hotkeys(vault_root) {
            Ok(hotkeys) => Self::from_hotkeys(&hotkeys),
            Err(_) => Self::defaults(),
        }
    }

    /// Action déclenchée par la combinaison donnée, si elle est liée.
    ///
    /// Les modificateurs et la touche sont normalisés avant la recherche :
    /// l'ordre des modificateurs et la casse de la touche n'ont pas
    /// d'importance.
    pub fn action_for(&self, modifiers: &[String], key: &str) -> Option<KeyAction> {
        let modifiers = canonical_modifiers(modifiers);
        let key = canonical_key(key)?;
        self.primary
            .iter()
            .find(|shortcut| shortcut.key == key && shortcut.modifiers == modifiers)
            .map(|shortcut| shortcut.action)
            .or_else(|| {
                self.legacy
                    .iter()
                    .find(|shortcut| shortcut.key == key && shortcut.modifiers == modifiers)
                    .map(|shortcut| shortcut.action)
            })
    }

    /// Liaison principale d'une action (celle affichée dans la palette).
    pub fn shortcut_for(&self, action: KeyAction) -> Option<&Shortcut> {
        self.primary
            .iter()
            .find(|shortcut| shortcut.action == action)
    }

    /// Toutes les liaisons principales (une par action).
    pub fn bindings(&self) -> &[Shortcut] {
        &self.primary
    }

    /// Toutes les liaisons historiques conservées.
    pub fn legacy_bindings(&self) -> &[Shortcut] {
        &self.legacy
    }

    /// Raccourci d'une action, formaté pour l'affichage (« Ctrl + Maj + P »).
    ///
    /// Renvoie une chaîne vide si l'action n'a pas de liaison.
    pub fn display_for(&self, action: KeyAction) -> String {
        match self.shortcut_for(action) {
            Some(shortcut) => format_shortcut(shortcut),
            None => String::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        action_by_id, all_actions, canonical_key, canonical_modifiers, default_shortcuts,
        legacy_shortcuts, parse_chord, KeyAction, KeyMap,
    };

    use std::collections::HashSet;

    /// Vérifie que chaque action du catalogue a exactement une liaison
    /// principale et qu'aucune combinaison n'est partagée.
    #[test]
    fn chaque_action_a_exactement_une_liaison_principale() {
        let shortcuts = default_shortcuts();
        assert_eq!(shortcuts.len(), all_actions().len());
        let actions: HashSet<KeyAction> = shortcuts.iter().map(|s| s.action).collect();
        assert_eq!(actions.len(), all_actions().len());
        for action in all_actions() {
            let count = shortcuts.iter().filter(|s| s.action == *action).count();
            assert_eq!(count, 1, "l'action {action:?} a {count} liaisons");
        }
    }

    /// Vérifie qu'aucune combinaison n'est liée à deux actions, compte tenu
    /// des liaisons historiques.
    #[test]
    fn aucune_combinaison_n_est_partagee() {
        let keymap = KeyMap::defaults();
        let mut seen: HashSet<(Vec<String>, String)> = HashSet::new();
        for shortcut in keymap.bindings().iter().chain(keymap.legacy_bindings()) {
            let chord = (shortcut.modifiers.clone(), shortcut.key.clone());
            assert!(
                seen.insert(chord.clone()),
                "combinaison dupliquée : {chord:?}"
            );
        }
    }

    /// Vérifie que les combinaisons attendues résolvent vers la bonne action.
    #[test]
    fn les_combinaisons_par_defaut_resolvent_les_actions() {
        let keymap = KeyMap::defaults();
        let ctrl = vec!["Ctrl".to_string()];
        let ctrl_shift = vec!["Ctrl".to_string(), "Shift".to_string()];
        let ctrl_alt = vec!["Alt".to_string(), "Ctrl".to_string()];
        assert_eq!(
            keymap.action_for(&ctrl, "P"),
            Some(KeyAction::OpenCommandPalette)
        );
        assert_eq!(keymap.action_for(&ctrl, "O"), Some(KeyAction::OpenSwitcher));
        assert_eq!(keymap.action_for(&ctrl, "N"), Some(KeyAction::NewNote));
        assert_eq!(keymap.action_for(&ctrl, "S"), Some(KeyAction::SaveNote));
        assert_eq!(keymap.action_for(&ctrl_shift, "S"), Some(KeyAction::SaveAs));
        assert_eq!(
            keymap.action_for(&ctrl_shift, "F"),
            Some(KeyAction::SearchInAllFiles)
        );
        assert_eq!(
            keymap.action_for(&ctrl, "E"),
            Some(KeyAction::ToggleEditPreview)
        );
        assert_eq!(keymap.action_for(&ctrl, "Tab"), Some(KeyAction::NextTab));
        assert_eq!(
            keymap.action_for(&ctrl_shift, "Tab"),
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
            keymap.action_for(&ctrl_shift, "D"),
            Some(KeyAction::ToggleLeftSidebar)
        );
        assert_eq!(
            keymap.action_for(&ctrl_shift, "R"),
            Some(KeyAction::ToggleRightSidebar)
        );
        assert_eq!(keymap.action_for(&[], "F1"), Some(KeyAction::ShortcutHelp));
        assert_eq!(keymap.action_for(&[], "Escape"), Some(KeyAction::Escape));
        assert_eq!(keymap.action_for(&ctrl, "G"), Some(KeyAction::ToggleGraph));
    }

    /// Vérifie qu'un modificateur supplémentaire ne déclenche pas l'action.
    #[test]
    fn un_modificateur_supplementaire_ne_declenche_pas_l_action() {
        let keymap = KeyMap::defaults();
        let ctrl_shift = vec!["Ctrl".to_string(), "Shift".to_string()];
        assert_eq!(keymap.action_for(&ctrl_shift, "P"), None);
        assert_eq!(keymap.action_for(&ctrl_shift, "N"), None);
    }

    /// Vérifie qu'un identifiant inconnu est ignoré.
    #[test]
    fn un_identifiant_inconnu_est_ignore() {
        assert_eq!(action_by_id("commande-inexistante"), None);
        assert_eq!(action_by_id(""), None);
    }

    /// Vérifie que le format Obsidian (`Mod+O`) est compris.
    #[test]
    fn le_format_obsidian_est_compris() {
        let (modifiers, key) = parse_chord(&["Mod+O".to_string()]).expect("combinaison valide");
        assert_eq!(modifiers, vec!["Ctrl".to_string()]);
        assert_eq!(key, "O");
    }

    /// Vérifie que le format « modificateurs puis touche » est compris.
    #[test]
    fn le_format_modificateurs_puis_touche_est_compris() {
        let (modifiers, key) =
            parse_chord(&["Mod".to_string(), "O".to_string()]).expect("combinaison valide");
        assert_eq!(modifiers, vec!["Ctrl".to_string()]);
        assert_eq!(key, "O");
    }

    /// Vérifie qu'une combinaison sans touche est refusée.
    #[test]
    fn une_combinaison_sans_touche_est_refusee() {
        assert!(parse_chord(&[]).is_none());
        assert!(parse_chord(&["Ctrl".to_string(), "Shift".to_string()]).is_none());
        assert!(parse_chord(&["Ctrl+".to_string()]).is_none());
    }

    /// Vérifie la normalisation des touches et des modificateurs.
    #[test]
    fn la_normalisation_canonique_est_stable() {
        assert_eq!(canonical_key("p").as_deref(), Some("P"));
        assert_eq!(canonical_key("ESC").as_deref(), Some("Escape"));
        assert_eq!(canonical_key("f1").as_deref(), Some("F1"));
        assert_eq!(canonical_key("F12").as_deref(), Some("F12"));
        assert_eq!(canonical_key(",").as_deref(), Some("Comma"));
        assert!(canonical_key("").is_none());
        assert!(canonical_key("F13").is_none());
        assert_eq!(
            canonical_modifiers(&["Shift".to_string(), "Ctrl".to_string()]),
            vec!["Ctrl".to_string(), "Shift".to_string()]
        );
    }

    /// Vérifie que les liaisons historiques restent actives.
    #[test]
    fn les_liaisons_historiques_restent_actives() {
        let keymap = KeyMap::defaults();
        let ctrl = vec!["Ctrl".to_string()];
        let ctrl_shift = vec!["Ctrl".to_string(), "Shift".to_string()];
        assert_eq!(keymap.action_for(&ctrl, "K"), Some(KeyAction::OpenSwitcher));
        assert_eq!(
            keymap.action_for(&ctrl, "B"),
            Some(KeyAction::ToggleLeftSidebar)
        );
        assert_eq!(
            keymap.action_for(&ctrl_shift, "B"),
            Some(KeyAction::ToggleRightSidebar)
        );
        assert_eq!(legacy_shortcuts().len(), 3);
    }
}
