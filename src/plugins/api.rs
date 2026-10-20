//! API publique des plugins.
//!
//! Les plugins vivent dans le binaire (pas d'exécution de code externe) et
//! implémentent le trait [`Plugin`] pour recevoir des événements du cœur.

/// Contexte fourni à un plugin au chargement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PluginContext<'a> {
    /// Nom du vault courant (affichage).
    pub vault_name: &'a str,
    /// Nombre de notes actuellement indexées.
    pub note_count: usize,
}

/// Points d'extension qu'un plugin peut implémenter.
pub trait Plugin {
    /// Identifiant unique du plugin (ex. "my-plugin").
    fn id(&self) -> &str;
    /// Nom lisible.
    fn name(&self) -> &str;
    /// Appelé une fois au chargement (enable).
    fn on_load(&mut self, _ctx: &PluginContext) -> Result<(), String> {
        Ok(())
    }
    /// Appelé à chaque mutation des notes (on_change), avec les ids notés.
    fn on_change(&mut self, _ctx: &PluginContext, _changed: &[String]) -> Result<(), String> {
        Ok(())
    }
    /// Appelé à l'arrêt (disable).
    fn on_unload(&mut self) -> Result<(), String> {
        Ok(())
    }
}

/// Descripteur statique d'un plugin (pour l'affichage dans un panneau).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PluginInfo<'a> {
    /// Identifiant unique du plugin.
    pub id: &'a str,
    /// Nom lisible.
    pub name: &'a str,
    /// Plugin activé ou non.
    pub enabled: bool,
}

#[cfg(test)]
mod tests {
    use super::{Plugin, PluginContext, PluginInfo};

    const VAULT_NAME: &str = "demo-vault";
    const NOTE_COUNT: usize = 42;

    fn sample_context() -> PluginContext<'static> {
        PluginContext {
            vault_name: VAULT_NAME,
            note_count: NOTE_COUNT,
        }
    }

    /// Implémentation minimale : seules `id`/`name` sont surchargées.
    struct MinimalPlugin {
        id: &'static str,
        name: &'static str,
    }

    impl Plugin for MinimalPlugin {
        fn id(&self) -> &str {
            self.id
        }
        fn name(&self) -> &str {
            self.name
        }
    }

    #[test]
    fn default_methods_callable_without_panic() {
        let mut plugin = MinimalPlugin {
            id: "minimal",
            name: "Minimal",
        };

        assert!(plugin.on_load(&sample_context()).is_ok());

        let changed = vec!["note-a".to_string(), "note-b".to_string()];
        assert!(plugin.on_change(&sample_context(), &changed).is_ok());

        assert!(plugin.on_unload().is_ok());
    }

    #[test]
    fn id_and_name_reflect_construction() {
        let plugin = MinimalPlugin {
            id: "my-plugin",
            name: "My Plugin",
        };

        assert_eq!(plugin.id(), "my-plugin");
        assert_eq!(plugin.name(), "My Plugin");
    }

    #[test]
    fn context_exposes_all_fields() {
        let ctx = sample_context();
        assert_eq!(ctx.vault_name, VAULT_NAME);
        assert_eq!(ctx.note_count, NOTE_COUNT);

        let other = PluginContext {
            vault_name: "autre-vault",
            note_count: 0,
        };
        assert_eq!(other.vault_name, "autre-vault");
        assert_eq!(other.note_count, 0);
    }

    #[test]
    fn info_exposes_all_fields() {
        let enabled = PluginInfo {
            id: "my-plugin",
            name: "My Plugin",
            enabled: true,
        };
        assert_eq!(enabled.id, "my-plugin");
        assert_eq!(enabled.name, "My Plugin");
        assert!(enabled.enabled);

        let disabled = PluginInfo {
            id: "silent",
            name: "Silent",
            enabled: false,
        };
        assert_eq!(disabled.id, "silent");
        assert_eq!(disabled.name, "Silent");
        assert!(!disabled.enabled);
    }

    #[test]
    fn trait_object_usable() {
        let mut plugin: Box<dyn Plugin> = Box::new(MinimalPlugin {
            id: "via-box",
            name: "Via Box",
        });

        assert_eq!(plugin.id(), "via-box");
        assert_eq!(plugin.name(), "Via Box");
        assert!(plugin.on_load(&sample_context()).is_ok());

        let changed = vec!["n1".to_string()];
        assert!(plugin.on_change(&sample_context(), &changed).is_ok());

        assert!(plugin.on_unload().is_ok());
    }

    #[test]
    fn trait_objects_in_collection() {
        let plugins: Vec<Box<dyn Plugin>> = vec![
            Box::new(MinimalPlugin {
                id: "one",
                name: "Un",
            }),
            Box::new(MinimalPlugin {
                id: "two",
                name: "Deux",
            }),
        ];

        let ids: Vec<&str> = plugins.iter().map(|p| p.id()).collect();
        assert_eq!(ids, ["one", "two"]);

        let names: Vec<&str> = plugins.iter().map(|p| p.name()).collect();
        assert_eq!(names, ["Un", "Deux"]);
    }

    #[test]
    fn custom_error_propagated_through_trait_object() {
        struct RefusingPlugin;

        impl Plugin for RefusingPlugin {
            fn id(&self) -> &str {
                "refusing"
            }
            fn name(&self) -> &str {
                "Refusing"
            }
            fn on_load(&mut self, _ctx: &PluginContext) -> Result<(), String> {
                Err("refusé".to_string())
            }
        }

        let mut plugin: Box<dyn Plugin> = Box::new(RefusingPlugin);
        assert!(matches!(plugin.on_load(&sample_context()), Err(msg) if msg == "refusé"));
    }
}
