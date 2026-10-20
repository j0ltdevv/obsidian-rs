//! Scaffold : helpers pour créer des plugins minimaux sans boilerplate.
//!
//! Les constructeurs [`noop_plugin`] et [`counting_plugin`] produisent des
//! plugins compilés dans le binaire (aucune exécution de code externe).

use crate::plugins::api::{Plugin, PluginContext};

/// Crée un plugin sans comportement (no-op) à partir d'un id et d'un nom.
pub fn noop_plugin(id: &str, name: &str) -> Box<dyn Plugin> {
    Box::new(NoopPlugin {
        id: id.to_owned(),
        name: name.to_owned(),
    })
}

/// Génère un plugin qui compte les notifications on_change reçues.
pub fn counting_plugin(id: &str, name: &str) -> Box<dyn Plugin> {
    Box::new(CountingPlugin::new(id, name))
}

/// Plugin sans comportement : ne réagit à aucun événement du cycle de vie.
struct NoopPlugin {
    id: String,
    name: String,
}

impl Plugin for NoopPlugin {
    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn on_load(&mut self, _ctx: &PluginContext) -> Result<(), String> {
        Ok(())
    }

    fn on_change(&mut self, _ctx: &PluginContext, _changed: &[String]) -> Result<(), String> {
        Ok(())
    }

    fn on_unload(&mut self) -> Result<(), String> {
        Ok(())
    }
}

/// Trait auxiliaire interne : expose le compteur d'un [`CountingPlugin`].
///
/// Non public : réservé aux tests et à un usage interne à la crate.
#[cfg(test)]
pub(crate) trait PluginCounter {
    /// Nombre de notifications `on_change` reçues depuis la création.
    fn count(&self) -> usize;
}

/// Plugin qui compte les notifications `on_change` reçues.
struct CountingPlugin {
    id: String,
    name: String,
    count: usize,
}

impl CountingPlugin {
    fn new(id: &str, name: &str) -> Self {
        Self {
            id: id.to_owned(),
            name: name.to_owned(),
            count: 0,
        }
    }
}

#[cfg(test)]
impl PluginCounter for CountingPlugin {
    fn count(&self) -> usize {
        self.count
    }
}

impl Plugin for CountingPlugin {
    fn id(&self) -> &str {
        &self.id
    }
    fn name(&self) -> &str {
        &self.name
    }
    fn on_load(&mut self, _ctx: &PluginContext) -> Result<(), String> {
        Ok(())
    }

    fn on_change(&mut self, _ctx: &PluginContext, _changed: &[String]) -> Result<(), String> {
        self.count += 1;
        Ok(())
    }

    fn on_unload(&mut self) -> Result<(), String> {
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

    fn sample_counting_plugin() -> CountingPlugin {
        CountingPlugin::new("counter", "Compteur")
    }

    #[test]
    fn noop_plugin_exposes_id_and_name() {
        let plugin = noop_plugin("my-noop", "Noop");
        assert_eq!(plugin.id(), "my-noop");
        assert_eq!(plugin.name(), "Noop");
    }

    #[test]
    fn noop_plugin_lifecycle_without_panic() {
        let mut plugin = noop_plugin("my-noop", "Noop");
        assert!(plugin.on_load(&sample_context()).is_ok());
        assert!(plugin.on_change(&sample_context(), &[]).is_ok());
        assert!(plugin.on_unload().is_ok());
    }

    #[test]
    fn noop_plugin_accepts_repeated_changes() {
        let mut plugin = noop_plugin("my-noop", "Noop");
        for i in 0..10 {
            let changed = vec![format!("note-{i}")];
            assert!(plugin.on_change(&sample_context(), &changed).is_ok());
        }
        assert!(plugin.on_unload().is_ok());
    }

    #[test]
    fn counting_plugin_exposes_id_and_name() {
        let plugin = counting_plugin("counter", "Compteur");
        assert_eq!(plugin.id(), "counter");
        assert_eq!(plugin.name(), "Compteur");
    }

    #[test]
    fn counting_plugin_starts_at_zero_and_load_does_not_increment() {
        let mut plugin = sample_counting_plugin();
        assert_eq!(plugin.count(), 0);
        assert!(plugin.on_load(&sample_context()).is_ok());
        assert_eq!(plugin.count(), 0);
    }

    #[test]
    fn counting_plugin_increments_per_change() {
        let mut plugin = sample_counting_plugin();
        let changed = vec![String::from("note-a")];
        plugin.on_change(&sample_context(), &changed).unwrap();
        assert_eq!(plugin.count(), 1);
        plugin.on_change(&sample_context(), &changed).unwrap();
        assert_eq!(plugin.count(), 2);
    }

    #[test]
    fn counting_plugin_counts_each_notification() {
        let mut plugin = sample_counting_plugin();
        for i in 0..25 {
            plugin
                .on_change(&sample_context(), &[format!("note-{i}")])
                .unwrap();
        }
        assert_eq!(plugin.count(), 25);
    }

    #[test]
    fn counting_plugin_unload_does_not_increment() {
        let mut plugin = sample_counting_plugin();
        plugin.on_change(&sample_context(), &[]).unwrap();
        plugin.on_unload().unwrap();
        assert_eq!(plugin.count(), 1);
    }

    #[test]
    fn counting_getter_reachable_via_auxiliary_trait_object() {
        let mut plugin = sample_counting_plugin();
        plugin.on_change(&sample_context(), &[]).unwrap();
        let view: &dyn PluginCounter = &plugin;
        assert_eq!(view.count(), 1);
    }
}
