//! Registre de commandes pour la palette (données pures, sans dépendance UI).
//!
//! Le module expose une commande enregistrable (`Command`), les commandes par
//! défaut de l'application et une recherche floue par sous-séquence du
//! libellé. Il ne dépend ni d'iced ni de l'état de l'application ; la couche
//! UI traduit ces données en actions.

use crate::vault::keymap::KeyAction;

/// Une commande enregistrable.
#[derive(Debug, Clone, PartialEq)]
pub struct Command {
    /// Identifiant unique et stable (utilisé par les plugins et la persistance).
    pub id: &'static str,
    /// Libellé affichable.
    pub label: &'static str,
    /// Raccourci associé (optionnel).
    pub shortcut: Option<KeyAction>,
    /// Catégorie (ex. "Fichier", "Édition", "Vue", "Plugins").
    pub category: &'static str,
}

/// Construit une commande.
const fn command(
    id: &'static str,
    label: &'static str,
    shortcut: Option<KeyAction>,
    category: &'static str,
) -> Command {
    Command {
        id,
        label,
        shortcut,
        category,
    }
}

/// Commandes par défaut de l'application.
///
/// Les six premières portent un raccourci clavier ; les suivantes n'en ont
/// pas. L'ordre d'enregistrement détermine l'ordre renvoyé par une recherche
/// avec requête vide.
pub fn default_commands() -> Vec<Command> {
    vec![
        command(
            "open-switcher",
            "Ouvrir le sélecteur de notes",
            Some(KeyAction::OpenSwitcher),
            "Navigation",
        ),
        command(
            "save-note",
            "Sauvegarder la note",
            Some(KeyAction::SaveNote),
            "Fichier",
        ),
        command(
            "toggle-graph",
            "Basculer le graphe",
            Some(KeyAction::ToggleGraph),
            "Vue",
        ),
        command(
            "new-note",
            "Créer une nouvelle note",
            Some(KeyAction::NewNote),
            "Fichier",
        ),
        command(
            "toggle-sidebar",
            "Basculer la barre latérale",
            Some(KeyAction::ToggleSidebar),
            "Vue",
        ),
        command(
            "escape",
            "Annuler ou fermer la vue courante",
            Some(KeyAction::Escape),
            "Navigation",
        ),
        command(
            "search-vault",
            "Rechercher dans le vault",
            None,
            "Navigation",
        ),
        command(
            "command-palette",
            "Palette de commandes",
            None,
            "Navigation",
        ),
        command(
            "toggle-editor-mode",
            "Basculer le mode lecture/édition",
            None,
            "Vue",
        ),
        command("zoom-in", "Zoomer avant", None, "Vue"),
        command("zoom-out", "Zoomer arrière", None, "Vue"),
        command(
            "create-daily-note",
            "Créer une note journalière",
            None,
            "Fichier",
        ),
        command("insert-template", "Insérer un modèle", None, "Fichier"),
        command("toggle-dataview", "Basculer la vue Dataview", None, "Vue"),
        command("toggle-history", "Basculer l'historique", None, "Vue"),
        command("toggle-theme", "Basculer les thèmes", None, "Vue"),
        command("toggle-canvas", "Basculer le Canvas", None, "Vue"),
        command("toggle-plugins", "Basculer les plugins", None, "Plugins"),
        command("toggle-outline", "Basculer le plan", None, "Vue"),
        command("toggle-search", "Basculer la recherche", None, "Vue"),
        command("toggle-tasks", "Basculer les tâches", None, "Vue"),
    ]
}

/// Position de départ de la première sous-séquence de `query` dans `label`,
/// si elle existe. La position renvoyée est minimale : c'est l'indice du
/// premier caractère du libellé qui peut débuter une correspondance.
fn first_subsequence_start(label: &[char], query: &[char]) -> Option<usize> {
    if query.is_empty() {
        return Some(0);
    }
    let mut qi = 0;
    let mut start = 0;
    for (li, &c) in label.iter().enumerate() {
        if c == query[qi] {
            if qi == 0 {
                start = li;
            }
            qi += 1;
            if qi == query.len() {
                return Some(start);
            }
        }
    }
    None
}

/// Recherche floue de commandes par libellé (sous-séquence insensible à la
/// casse), triée par pertinence : position du premier match puis longueur du
/// libellé. Le tri est stable : à pertinence égale, l'ordre d'enregistrement
/// est conservé. Une requête vide renvoie toutes les commandes dans l'ordre
/// d'enregistrement ; sans match, le vecteur renvoyé est vide.
pub fn search_commands(commands: &[Command], query: &str) -> Vec<usize> {
    if query.is_empty() {
        return (0..commands.len()).collect();
    }
    let query: Vec<char> = query.chars().flat_map(char::to_lowercase).collect();
    let mut matches: Vec<(usize, usize, usize)> = Vec::new();
    for (index, cmd) in commands.iter().enumerate() {
        let label: Vec<char> = cmd.label.chars().flat_map(char::to_lowercase).collect();
        if let Some(first) = first_subsequence_start(&label, &query) {
            matches.push((index, first, label.len()));
        }
    }
    matches.sort_by_key(|&(_, first, len)| (first, len));
    matches.into_iter().map(|(index, _, _)| index).collect()
}

#[cfg(test)]
mod tests {
    use super::{command, default_commands, search_commands, Command, KeyAction};

    use std::collections::HashSet;

    const ACTIONS: [KeyAction; 6] = [
        KeyAction::OpenSwitcher,
        KeyAction::SaveNote,
        KeyAction::ToggleGraph,
        KeyAction::NewNote,
        KeyAction::ToggleSidebar,
        KeyAction::Escape,
    ];

    /// Vérifie que la liste par défaut n'est pas vide et que les ids sont uniques.
    #[test]
    fn la_liste_par_defaut_n_est_pas_vide_et_les_ids_sont_uniques() {
        let commands = default_commands();
        assert!(!commands.is_empty());
        let ids: HashSet<&str> = commands.iter().map(|c| c.id).collect();
        assert_eq!(ids.len(), commands.len(), "ids dupliqués");
    }

    /// Vérifie que chaque commande possède un libellé non vide.
    #[test]
    fn chaque_commande_a_un_label_non_vide() {
        for c in default_commands() {
            assert!(!c.label.is_empty(), "label vide pour {}", c.id);
        }
    }

    /// Vérifie que chaque commande possède une catégorie non vide.
    #[test]
    fn chaque_commande_a_une_categorie_non_vide() {
        for c in default_commands() {
            assert!(!c.category.is_empty(), "catégorie vide pour {}", c.id);
        }
    }

    /// Vérifie que les raccourcis couvrent exactement les six KeyAction.
    #[test]
    fn les_raccourcis_couvrent_exactement_les_six_actions() {
        let commands = default_commands();
        let shortcuts: Vec<KeyAction> = commands.iter().filter_map(|c| c.shortcut).collect();
        assert_eq!(shortcuts.len(), ACTIONS.len());
        let actions: HashSet<KeyAction> = shortcuts.into_iter().collect();
        assert_eq!(actions.len(), ACTIONS.len());
        for action in ACTIONS {
            assert!(actions.contains(&action), "action absente : {action:?}");
        }
    }

    /// Vérifie le mapping exact entre chaque KeyAction et sa commande.
    #[test]
    fn les_raccourcis_defaut_sont_bien_associes_a_leurs_commandes() {
        let attendus: [(KeyAction, &str); 6] = [
            (KeyAction::OpenSwitcher, "open-switcher"),
            (KeyAction::SaveNote, "save-note"),
            (KeyAction::ToggleGraph, "toggle-graph"),
            (KeyAction::NewNote, "new-note"),
            (KeyAction::ToggleSidebar, "toggle-sidebar"),
            (KeyAction::Escape, "escape"),
        ];
        for (action, id) in attendus {
            let commandes = default_commands();
            let commandes: Vec<&Command> = commandes
                .iter()
                .filter(|c| c.shortcut == Some(action))
                .collect();
            assert_eq!(
                commandes.len(),
                1,
                "action {action:?} non couverte ou doublée"
            );
            assert_eq!(commandes[0].id, id);
        }
    }

    /// Vérifie que les commandes sans raccourci ont bien `shortcut == None`.
    #[test]
    fn les_commandes_sans_raccourci_ont_shortcut_a_none() {
        let ids_sans_raccourci = [
            "search-vault",
            "command-palette",
            "toggle-editor-mode",
            "zoom-in",
            "zoom-out",
            "create-daily-note",
            "insert-template",
            "toggle-dataview",
            "toggle-history",
            "toggle-theme",
            "toggle-canvas",
            "toggle-plugins",
        ];
        for id in ids_sans_raccourci {
            let commandes = default_commands();
            let c = commandes
                .iter()
                .find(|c| c.id == id)
                .expect("commande absente");
            assert_eq!(c.shortcut, None);
        }
    }

    /// Vérifie que les ids sont stables et dans l'ordre d'enregistrement.
    #[test]
    fn les_ids_sont_stables() {
        let attendus = [
            "open-switcher",
            "save-note",
            "toggle-graph",
            "new-note",
            "toggle-sidebar",
            "escape",
            "search-vault",
            "command-palette",
            "toggle-editor-mode",
            "zoom-in",
            "zoom-out",
            "create-daily-note",
            "insert-template",
            "toggle-dataview",
            "toggle-history",
            "toggle-theme",
            "toggle-canvas",
            "toggle-plugins",
            "toggle-outline",
            "toggle-search",
            "toggle-tasks",
        ];
        let ids: Vec<&str> = default_commands().iter().map(|c| c.id).collect();
        assert_eq!(ids, attendus);
    }

    /// Vérifie la recherche par sous-séquence : "sauv" trouve SaveNote.
    #[test]
    fn la_recherche_trouve_une_sous_sequence() {
        let commands = default_commands();
        let save_index = commands
            .iter()
            .position(|c| c.id == "save-note")
            .expect("la commande save-note doit exister");
        let results = search_commands(&commands, "sauv");
        assert_eq!(results.first(), Some(&save_index));
    }

    /// Vérifie que la recherche est insensible à la casse.
    #[test]
    fn la_recherche_est_insensible_a_la_casse() {
        let commands = default_commands();
        assert_eq!(
            search_commands(&commands, "sauv"),
            search_commands(&commands, "SAUV")
        );
        assert_eq!(
            search_commands(&commands, "NOUVELLE"),
            search_commands(&commands, "nouvelle")
        );
    }

    /// Vérifie que la pertinence priorise la position du premier match.
    #[test]
    fn le_tri_priorise_la_position_du_premier_match() {
        let commands = vec![
            command("tard", "zoo sauvage", None, "Test"),
            command("tot", "sauvage", None, "Test"),
        ];
        let results = search_commands(&commands, "sauv");
        assert_eq!(results, vec![1, 0]);
    }

    /// Vérifie qu'à position égale, le libellé le plus court passe en premier.
    #[test]
    fn le_tri_favorise_les_libelles_plus_courts_a_position_egale() {
        let commands = vec![
            command("long", "sauvegarder", None, "Test"),
            command("court", "sauvG", None, "Test"),
        ];
        let results = search_commands(&commands, "sauv");
        assert_eq!(results, vec![1, 0]);
    }

    /// Vérifie qu'une requête vide renvoie toutes les commandes en ordre.
    #[test]
    fn requete_vide_renvoie_toutes_les_commandes_dans_l_ordre() {
        let commands = default_commands();
        let expected: Vec<usize> = (0..commands.len()).collect();
        assert_eq!(search_commands(&commands, ""), expected);
    }

    /// Vérifie qu'aucun match ne renvoie un vecteur vide.
    #[test]
    fn aucun_match_renvoie_un_vecteur_vide() {
        let commands = default_commands();
        assert_eq!(search_commands(&commands, "zzzzqqqq"), Vec::<usize>::new());
    }

    /// Vérifie que le tri est stable à pertinence égale.
    #[test]
    fn le_tri_est_stable_a_egalite() {
        let commands = vec![
            command("premier", "Sauvegarder", None, "Test"),
            command("second", "Sauvegarder", None, "Test"),
            command("troisieme", "Sauvegarder", None, "Test"),
        ];
        let results = search_commands(&commands, "sauv");
        assert_eq!(results, vec![0, 1, 2]);
    }
}
