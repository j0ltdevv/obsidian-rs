//! Plugin de démo « stats » : enregistre le nombre de notes du vault au
//! chargement et compte les notifications `on_change` reçues.

use crate::plugins::api::{Plugin, PluginContext};

/// Trait auxiliaire interne : expose l'état d'un [`DemoStats`].
///
/// Non public : réservé aux tests et à un usage interne à la crate.
#[cfg(test)]
pub(crate) trait PluginStats {
    /// Nombre de notifications `on_change` reçues depuis la création (remis à
    /// zéro par `on_unload`).
    fn count(&self) -> usize;

    /// Nombre de notes enregistré lors du dernier `on_load`.
    fn loaded_note_count(&self) -> usize;
}

/// Crée le plugin de démo « stats » (id "demo-stats", nom "Statistiques").
pub fn demo_stats_plugin() -> Box<dyn Plugin> {
    Box::new(DemoStats::new())
}

/// Plugin de démo « stats » : statistiques simples sur le vault courant.
#[derive(Debug)]
struct DemoStats {
    id: String,
    name: String,
    /// Nombre de notifications `on_change` reçues.
    count: usize,
    /// Nombre de notes du vault au dernier `on_load`.
    loaded_note_count: usize,
}

impl DemoStats {
    fn new() -> Self {
        Self {
            id: String::from("demo-stats"),
            name: String::from("Statistiques"),
            count: 0,
            loaded_note_count: 0,
        }
    }
}

#[cfg(test)]
impl PluginStats for DemoStats {
    fn count(&self) -> usize {
        self.count
    }

    fn loaded_note_count(&self) -> usize {
        self.loaded_note_count
    }
}

impl Plugin for DemoStats {
    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn on_load(&mut self, ctx: &PluginContext) -> Result<(), String> {
        self.loaded_note_count = ctx.note_count;
        Ok(())
    }

    fn on_change(&mut self, _ctx: &PluginContext, _changed: &[String]) -> Result<(), String> {
        self.count += 1;
        Ok(())
    }

    fn on_unload(&mut self) -> Result<(), String> {
        self.count = 0;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VAULT_NAME: &str = "demo-vault";
    const NOTE_COUNT: usize = 42;

    fn sample_context() -> PluginContext<'static> {
        PluginContext {
            vault_name: VAULT_NAME,
            note_count: NOTE_COUNT,
        }
    }

    fn empty_context() -> PluginContext<'static> {
        PluginContext {
            vault_name: "",
            note_count: 0,
        }
    }

    fn sample_plugin() -> DemoStats {
        DemoStats::new()
    }

    #[test]
    fn demo_stats_exposes_id_and_name() {
        let plugin = sample_plugin();
        assert_eq!(plugin.id(), "demo-stats");
        assert_eq!(plugin.name(), "Statistiques");
    }

    #[test]
    fn demo_stats_starts_at_zero() {
        let plugin = sample_plugin();
        assert_eq!(plugin.count(), 0);
        assert_eq!(plugin.loaded_note_count(), 0);
    }

    #[test]
    fn demo_stats_plugin_constructs_usable_trait_object() {
        let mut plugin = demo_stats_plugin();
        assert_eq!(plugin.id(), "demo-stats");
        assert_eq!(plugin.name(), "Statistiques");
        assert!(plugin.on_load(&sample_context()).is_ok());
        assert!(plugin.on_change(&sample_context(), &[]).is_ok());
        assert!(plugin.on_unload().is_ok());
    }

    #[test]

    fn demo_stats_load_stores_note_count_without_touching_counter() {
        let mut plugin = sample_plugin();
        plugin.on_load(&sample_context()).unwrap();
        assert_eq!(plugin.loaded_note_count(), NOTE_COUNT);
        assert_eq!(plugin.count(), 0);
    }

    #[test]
    fn demo_stats_load_replaces_previous_note_count() {
        let mut plugin = sample_plugin();
        plugin.on_load(&sample_context()).unwrap();
        plugin.on_load(&empty_context()).unwrap();
        assert_eq!(plugin.loaded_note_count(), 0);
    }

    #[test]
    fn demo_stats_change_increments_counter() {
        let mut plugin = sample_plugin();
        plugin.on_load(&sample_context()).unwrap();
        plugin.on_change(&sample_context(), &[]).unwrap();
        assert_eq!(plugin.count(), 1);
    }

    #[test]
    fn demo_stats_counts_each_notification() {
        let mut plugin = sample_plugin();
        for i in 0..25 {
            plugin
                .on_change(&sample_context(), &[format!("note-{i}")])
                .unwrap();
        }
        assert_eq!(plugin.count(), 25);
    }

    #[test]
    fn demo_stats_unload_resets_counter_and_preserves_loaded_count() {
        let mut plugin = sample_plugin();
        plugin.on_load(&sample_context()).unwrap();
        plugin
            .on_change(&sample_context(), &["a".to_string()])
            .unwrap();
        plugin
            .on_change(&sample_context(), &["b".to_string()])
            .unwrap();
        assert_eq!(plugin.count(), 2);

        plugin.on_unload().unwrap();

        assert_eq!(plugin.count(), 0);
        assert_eq!(plugin.loaded_note_count(), NOTE_COUNT);
    }

    #[test]
    fn demo_stats_accepts_empty_context() {
        let mut plugin = sample_plugin();
        let ctx = empty_context();

        plugin.on_load(&ctx).unwrap();
        assert_eq!(plugin.loaded_note_count(), 0);

        plugin.on_change(&ctx, &[]).unwrap();
        assert_eq!(plugin.count(), 1);

        plugin.on_unload().unwrap();
        assert_eq!(plugin.count(), 0);
    }

    #[test]
    fn demo_stats_getters_reachable_via_auxiliary_trait_object() {
        let mut plugin = sample_plugin();
        plugin.on_load(&sample_context()).unwrap();
        plugin.on_change(&sample_context(), &[]).unwrap();

        let view: &dyn PluginStats = &plugin;
        assert_eq!(view.count(), 1);
        assert_eq!(view.loaded_note_count(), NOTE_COUNT);
    }

    #[test]
    fn demo_stats_errors_propagate_through_trait_object() {
        struct RefusingLoad(DemoStats);

        impl Plugin for RefusingLoad {
            fn id(&self) -> &str {
                self.0.id()
            }

            fn name(&self) -> &str {
                self.0.name()
            }

            fn on_load(&mut self, _ctx: &PluginContext) -> Result<(), String> {
                Err("échec chargement".to_string())
            }

            fn on_change(&mut self, ctx: &PluginContext, changed: &[String]) -> Result<(), String> {
                self.0.on_change(ctx, changed)
            }

            fn on_unload(&mut self) -> Result<(), String> {
                self.0.on_unload()
            }
        }

        let mut plugin: Box<dyn Plugin> = Box::new(RefusingLoad(DemoStats::new()));
        assert!(matches!(
            plugin.on_load(&sample_context()),
            Err(msg) if msg == "échec chargement"
        ));
    }
}
