//! Raccourcis clavier par défaut (données pures, sans dépendance UI).
//!
//! Le module ne dépend ni d'iced ni de l'état de l'application : il expose les
//! actions, la description d'un raccourci et la table par défaut. La couche UI
//! se charge de traduire ces données en événements clavier.

/// Actions déclenchées par les raccourcis clavier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum KeyAction {
    /// Ouvrir le sélecteur de notes (`Ctrl+K`).
    OpenSwitcher,
    /// Enregistrer la note en cours (`Ctrl+S`).
    SaveNote,
    /// Basculer la vue du graphe (`Ctrl+G`).
    ToggleGraph,
    /// Créer une nouvelle note (`Ctrl+N`).
    NewNote,
    /// Basculer la barre latérale (`Ctrl+B`).
    ToggleSidebar,
    /// Annuler ou fermer la vue courante (`Échap`).
    Escape,
}

/// Un raccourci clavier : une combinaison de modificateurs et une touche
/// mappés à une `KeyAction`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Shortcut {
    /// Modificateurs, triés par ordre lexicographique pour une comparaison
    /// déterministe (`["Alt", "Ctrl", "Shift"]`).
    pub modifiers: Vec<&'static str>,
    /// Touche, par exemple `"K"`, `"S"` ou `"Escape"`.
    pub key: &'static str,
    /// Action déclenchée par ce raccourci.
    pub action: KeyAction,
}

/// Construit un raccourci en triant ses modificateurs.
///
/// Le tri rend la représentation canonique : deux combinaisons identiques
/// écrites dans un ordre différent comparent et hachent de la même façon.
fn shortcut(modifiers: &[&'static str], key: &'static str, action: KeyAction) -> Shortcut {
    let mut modifiers = modifiers.to_vec();
    modifiers.sort_unstable();
    Shortcut {
        modifiers,
        key,
        action,
    }
}

/// Liste des raccourcis par défaut.
///
/// Le vecteur est trié par `KeyAction` afin que l'ordre soit déterministe
/// indépendamment de l'ordre d'écriture.
pub fn default_shortcuts() -> Vec<Shortcut> {
    let mut shortcuts = vec![
        shortcut(&["Ctrl"], "K", KeyAction::OpenSwitcher),
        shortcut(&["Ctrl"], "S", KeyAction::SaveNote),
        shortcut(&["Ctrl"], "G", KeyAction::ToggleGraph),
        shortcut(&["Ctrl"], "N", KeyAction::NewNote),
        shortcut(&["Ctrl"], "B", KeyAction::ToggleSidebar),
        shortcut(&[], "Escape", KeyAction::Escape),
    ];
    // `sort_by_key` est stable ; l'ordre suit le discriminant de `KeyAction`.
    shortcuts.sort_by_key(|s| s.action);
    shortcuts
}

#[cfg(test)]
mod tests {
    use super::{default_shortcuts, shortcut, KeyAction, Shortcut};

    use std::collections::HashSet;

    const ACTIONS: [KeyAction; 6] = [
        KeyAction::OpenSwitcher,
        KeyAction::SaveNote,
        KeyAction::ToggleGraph,
        KeyAction::NewNote,
        KeyAction::ToggleSidebar,
        KeyAction::Escape,
    ];

    /// Vérifie que les six actions par défaut sont présentes, une seule fois.
    #[test]
    fn les_six_actions_sont_presentes() {
        let shortcuts = default_shortcuts();
        assert_eq!(shortcuts.len(), ACTIONS.len());
        let actions: HashSet<KeyAction> = shortcuts.iter().map(|s| s.action).collect();
        assert_eq!(actions.len(), ACTIONS.len());
        for action in ACTIONS {
            assert!(actions.contains(&action), "action absente : {action:?}");
        }
    }

    /// Vérifie que chaque combinaison (modificateurs, touche) est unique.
    #[test]
    fn les_combinaisons_sont_uniques() {
        let shortcuts = default_shortcuts();
        let mut seen: HashSet<(Vec<&'static str>, &'static str)> = HashSet::new();
        for s in &shortcuts {
            assert!(
                seen.insert((s.modifiers.clone(), s.key)),
                "combinaison dupliquée : {:?} + {}",
                s.modifiers,
                s.key
            );
        }
        assert_eq!(seen.len(), shortcuts.len());
    }

    /// Vérifie qu'il n'y a pas de doublon d'action : chaque action est mappée
    /// à une et une seule combinaison.
    #[test]
    fn chaque_action_a_exactement_une_combinaison() {
        let shortcuts = default_shortcuts();
        for action in ACTIONS {
            let count = shortcuts.iter().filter(|s| s.action == action).count();
            assert_eq!(count, 1, "l'action {action:?} a {count} combinaisons");
        }
        // Chaque action possède une combinaison distincte des autres.
        let combinaisons: HashSet<(KeyAction, Vec<&'static str>, &'static str)> = shortcuts
            .iter()
            .map(|s| (s.action, s.modifiers.clone(), s.key))
            .collect();
        assert_eq!(combinaisons.len(), ACTIONS.len());
    }

    /// Vérifie que les modificateurs de chaque raccourci par défaut sont triés.
    #[test]
    fn les_modificateurs_sont_tries() {
        for s in default_shortcuts() {
            assert!(
                s.modifiers.windows(2).all(|w| w[0] < w[1]),
                "modificateurs non triés : {:?}",
                s.modifiers
            );
        }
    }

    /// Vérifie que le constructeur trie une combinaison écrite en désordre.
    #[test]
    fn le_constructeur_trie_les_modificateurs() {
        let s = shortcut(&["Shift", "Ctrl", "Alt"], "P", KeyAction::SaveNote);
        assert_eq!(s.modifiers.as_slice(), ["Alt", "Ctrl", "Shift"]);
        let t = shortcut(&["Ctrl", "Shift"], "P", KeyAction::SaveNote);
        let u = shortcut(&["Shift", "Ctrl"], "P", KeyAction::SaveNote);
        assert_eq!(t, u);
    }

    /// Vérifie le mapping exact des raccourcis par défaut.
    #[test]
    fn mapping_des_raccourcis_par_defaut() {
        let shortcuts = default_shortcuts();
        let attendus: [(KeyAction, &[&str], &str); 6] = [
            (KeyAction::OpenSwitcher, &["Ctrl"], "K"),
            (KeyAction::SaveNote, &["Ctrl"], "S"),
            (KeyAction::ToggleGraph, &["Ctrl"], "G"),
            (KeyAction::NewNote, &["Ctrl"], "N"),
            (KeyAction::ToggleSidebar, &["Ctrl"], "B"),
            (KeyAction::Escape, &[], "Escape"),
        ];
        for (action, modifiers, key) in attendus {
            assert!(
                shortcuts.iter().any(|s| s.action == action
                    && s.key == key
                    && s.modifiers.as_slice() == modifiers),
                "mapping inattendu pour {action:?}"
            );
        }
    }

    /// Vérifie que le résultat est trié par action (ordre déterministe).
    #[test]
    fn le_resultat_est_trie_par_action() {
        let shortcuts = default_shortcuts();
        assert!(
            shortcuts.windows(2).all(|w| w[0].action < w[1].action),
            "raccourcis non triés par action"
        );
        assert_eq!(default_shortcuts(), shortcuts);
    }

    /// Vérifie qu'un raccourci sans modificateur est bien construit.
    #[test]
    fn raccourci_sans_modificateur() {
        let s: Shortcut = shortcut(&[], "Escape", KeyAction::Escape);
        assert!(s.modifiers.is_empty());
        assert_eq!(s.key, "Escape");
        assert_eq!(s.action, KeyAction::Escape);
    }
}
