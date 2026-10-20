//! Plugin de démo « todo » : compte les notes dont le contenu contient une
//! case à cocher non cochée ("- [ ]").
//!
//! Le cœur ne transmet pas le contenu des notes (le [`PluginContext`] ne porte
//! que le nom du vault et le nombre de notes) : le plugin maintient donc son
//! propre index simulé via [`DemoTodoPlugin::set_content`]. `on_load`
//! (re)compte toutes les notes connues, tandis que `on_change` ne recompte
//! que les ids notifiés, en mettant à jour un mapping id → bool stocké
//! (`pending`) UNIQUEMENT pour ces ids.

use crate::plugins::api::{Plugin, PluginContext};
use std::collections::HashMap;

/// Marqueur d'une case à cocher non cochée dans le contenu d'une note.
const PENDING_MARKER: &str = "- [ ]";

/// Trait auxiliaire interne : expose le nombre de tâches en attente d'un
/// [`DemoTodoPlugin`].
///
/// Non public : réservé à un usage interne à la crate.
#[cfg(test)]
pub(crate) trait TodoRemaining {
    /// Nombre de notes contenant au moins une case à cocher non cochée.
    fn remaining(&self) -> usize;
}

/// Plugin de démo qui compte les notes dont le contenu contient "- [ ]".
///
/// L'état est un mapping id → bool (`pending`) qui mémorise, pour chaque note
/// connue, si elle contient une case à cocher non cochée. `on_load` reconstruit
/// ce mapping depuis l'index simulé ; `on_change` ne recompte que les ids
/// fournis, les autres notes conservant leur statut mémorisé.
pub struct DemoTodoPlugin {
    id: String,
    name: String,
    /// Index simulé : id de note → contenu connu.
    contents: HashMap<String, String>,
    /// Mapping id → bool : la note contient-elle "- [ ]" ?
    pending: HashMap<String, bool>,
}

impl DemoTodoPlugin {
    /// Crée le plugin de démo « todo » (id `demo-todo`).
    pub fn new() -> Self {
        Self {
            id: "demo-todo".to_owned(),
            name: "Tâches en attente".to_owned(),
            contents: HashMap::new(),
            pending: HashMap::new(),
        }
    }

    /// Remplace le contenu connu d'une note (simulation de l'index du vault).
    /// Retourne `Err` si l'id est vide. Le mapping id → bool n'est actualisé
    /// que par `on_load` (toutes les notes) ou `on_change` (ids fournis).
    #[cfg(test)]
    pub(crate) fn set_content(&mut self, id: &str, content: &str) -> Result<(), String> {
        if id.is_empty() {
            return Err("id de note vide refusé".to_owned());
        }
        self.contents.insert(id.to_owned(), content.to_owned());
        Ok(())
    }

    /// Recompte toutes les notes connues à partir de leur contenu stocké.
    fn recount_all(&mut self) {
        self.pending.clear();
        for (id, content) in &self.contents {
            self.pending
                .insert(id.clone(), content.contains(PENDING_MARKER));
        }
    }

    /// Nombre de notes contenant au moins une case à cocher non cochée.
    #[cfg(test)]
    pub(crate) fn remaining(&self) -> usize {
        self.pending
            .values()
            .filter(|&&has_pending| has_pending)
            .count()
    }
}

impl Default for DemoTodoPlugin {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
impl TodoRemaining for DemoTodoPlugin {
    fn remaining(&self) -> usize {
        DemoTodoPlugin::remaining(self)
    }
}

/// Construit le plugin de démo « todo » en trait object.
pub fn demo_todo_plugin() -> Box<dyn Plugin> {
    Box::new(DemoTodoPlugin::new())
}

impl Plugin for DemoTodoPlugin {
    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn on_load(&mut self, _ctx: &PluginContext) -> Result<(), String> {
        // (Re)initialisation : le mapping entier est reconstruit depuis l'index
        // connu — comptage des notes dont le contenu contient "- [ ]".
        self.recount_all();
        Ok(())
    }

    fn on_change(&mut self, _ctx: &PluginContext, changed: &[String]) -> Result<(), String> {
        // Recompte UNIQUEMENT les ids fournis : les autres notes gardent leur
        // statut mémorisé. Un id vide est une entrée invalide : l'erreur est
        // propagée à l'appelant.
        for id in changed {
            if id.is_empty() {
                return Err("id de note vide refusé".to_owned());
            }
            if let Some(content) = self.contents.get(id) {
                self.pending
                    .insert(id.clone(), content.contains(PENDING_MARKER));
            }
        }
        Ok(())
    }

    fn on_unload(&mut self) -> Result<(), String> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_context() -> PluginContext<'static> {
        PluginContext {
            vault_name: "demo-vault",
            note_count: 3,
        }
    }

    fn sample_plugin() -> DemoTodoPlugin {
        DemoTodoPlugin::new()
    }

    #[test]
    fn demo_todo_plugin_exposes_id_and_name() {
        let plugin = sample_plugin();
        assert_eq!(plugin.id(), "demo-todo");
        assert_eq!(plugin.name(), "Tâches en attente");
    }

    #[test]
    fn on_load_initializes_state_from_known_notes() {
        let mut plugin = sample_plugin();
        plugin.set_content("a.md", "projet : - [ ] finir").unwrap();
        plugin.set_content("b.md", "rien à faire").unwrap();
        plugin.set_content("c.md", "- [x] corrigé").unwrap();

        assert!(plugin.on_load(&sample_context()).is_ok());

        // Seule "a.md" contient une case à cocher non cochée.
        assert_eq!(plugin.remaining(), 1);
    }

    #[test]
    fn on_load_resets_previous_state() {
        let mut plugin = sample_plugin();
        plugin.set_content("a.md", "- [ ] à faire").unwrap();
        plugin.on_load(&sample_context()).unwrap();
        assert_eq!(plugin.remaining(), 1);

        // Contenu mis à jour sans case non cochée, puis rechargement : le
        // mapping est reconstruit et la note n'est plus comptée.
        plugin.set_content("a.md", "terminé").unwrap();
        plugin.on_load(&sample_context()).unwrap();
        assert_eq!(plugin.remaining(), 0);
    }

    #[test]
    fn on_change_with_pending_note_counts_it() {
        let mut plugin = sample_plugin();
        plugin.set_content("a.md", "- [ ] à faire").unwrap();
        assert_eq!(plugin.remaining(), 0);

        let changed = vec!["a.md".to_string()];
        plugin.on_change(&sample_context(), &changed).unwrap();

        assert_eq!(plugin.remaining(), 1);
    }

    #[test]
    fn on_change_without_ids_changes_nothing() {
        let mut plugin = sample_plugin();
        plugin.set_content("a.md", "- [ ] à faire").unwrap();
        plugin.on_load(&sample_context()).unwrap();
        assert_eq!(plugin.remaining(), 1);

        plugin.on_change(&sample_context(), &[]).unwrap();

        assert_eq!(plugin.remaining(), 1);
    }

    #[test]
    fn on_change_only_recounts_provided_ids() {
        let mut plugin = sample_plugin();
        plugin.set_content("a.md", "- [ ] à faire").unwrap();
        plugin.set_content("b.md", "brouillon").unwrap();
        plugin.on_load(&sample_context()).unwrap();
        assert_eq!(plugin.remaining(), 1);

        // Seul "b.md" est notifié : "a.md" garde son statut mémorisé.
        let changed = vec!["b.md".to_string()];
        plugin.on_change(&sample_context(), &changed).unwrap();
        assert_eq!(plugin.remaining(), 1);

        // "b.md" contient désormais une case non cochée : il est recompté.
        plugin.set_content("b.md", "- [ ] nouvelle tâche").unwrap();
        plugin.on_change(&sample_context(), &changed).unwrap();
        assert_eq!(plugin.remaining(), 2);
    }

    #[test]
    fn on_change_with_unknown_note_is_ignored() {
        let mut plugin = sample_plugin();
        plugin.set_content("a.md", "- [ ] à faire").unwrap();
        plugin.on_load(&sample_context()).unwrap();

        let changed = vec!["inconnue.md".to_string()];
        plugin.on_change(&sample_context(), &changed).unwrap();

        assert_eq!(plugin.remaining(), 1);
    }

    #[test]
    fn remaining_getter_reachable_via_auxiliary_trait_object() {
        let mut plugin = sample_plugin();
        plugin.set_content("a.md", "- [ ] à faire").unwrap();
        plugin.on_load(&sample_context()).unwrap();

        let view: &dyn TodoRemaining = &plugin;
        assert_eq!(view.remaining(), 1);
    }

    #[test]
    fn on_change_error_propagated_through_trait_object() {
        let mut plugin: Box<dyn Plugin> = Box::new(sample_plugin());

        let changed = vec![String::new()];
        let err = plugin.on_change(&sample_context(), &changed).unwrap_err();
        assert!(err.contains("vide"));
    }

    #[test]
    fn behavior_is_deterministic() {
        let run = || {
            let mut plugin = sample_plugin();
            plugin.set_content("a.md", "- [ ] à faire").unwrap();
            plugin.set_content("b.md", "- [x] fait").unwrap();
            let mut steps = Vec::new();
            plugin.on_load(&sample_context()).unwrap();
            steps.push(plugin.remaining());
            plugin
                .on_change(&sample_context(), &["b.md".to_string()])
                .unwrap();
            steps.push(plugin.remaining());
            plugin.set_content("b.md", "- [ ] à refaire").unwrap();
            plugin
                .on_change(&sample_context(), &["b.md".to_string()])
                .unwrap();
            steps.push(plugin.remaining());
            plugin.on_change(&sample_context(), &[]).unwrap();
            steps.push(plugin.remaining());
            steps
        };

        let first = run();
        assert_eq!(first, vec![1, 1, 2, 2]);
        assert_eq!(run(), first);
    }
}
