//! Registre de plugins : enregistrement, chargement, activation, notification.
//!
//! L'état activé de chaque plugin est géré par le registre (et non par le trait
//! [`Plugin`]) : un plugin n'est activé qu'après un `on_load` réussi, et reste
//! activé jusqu'à un `on_unload` réussi. Les descripteurs [`PluginInfo`]
//! reflètent cet état pour l'UI.

use super::api::{Plugin, PluginContext, PluginInfo};

/// Registre des plugins du binaire.
///
/// Chaque entrée associe le plugin à son état activé/désactivé. L'ordre
/// d'enregistrement est préservé partout (chargement, notification,
/// déchargement, descripteurs).
#[derive(Default)]
pub struct PluginRegistry {
    plugins: Vec<(Box<dyn Plugin>, bool)>,
}

impl PluginRegistry {
    /// Crée un registre vide.
    pub fn new() -> Self {
        Self::default()
    }

    /// Enregistre un plugin (id unique requis). Retourne `Err` si l'id est
    /// déjà enregistré ; le plugin existant n'est ni remplacé ni modifié.
    pub fn register(&mut self, plugin: Box<dyn Plugin>) -> Result<(), String> {
        let id = plugin.id();
        if self.plugins.iter().any(|(existing, _)| existing.id() == id) {
            return Err(format!("plugin déjà enregistré : {id}"));
        }
        self.plugins.push((plugin, false));
        Ok(())
    }

    /// Charge (`on_load`) tous les plugins enregistrés, un seul appel par
    /// plugin chargé. Propage l'erreur au premier échec : les plugins déjà
    /// chargés restent activés, le plugin en échec reste désactivé (jamais de
    /// plugin mi-chargé).
    pub fn load_all(&mut self, ctx: &PluginContext) -> Result<(), String> {
        for (plugin, enabled) in &mut self.plugins {
            if *enabled {
                continue;
            }
            plugin.on_load(ctx)?;
            *enabled = true;
        }
        Ok(())
    }

    /// Notifie `on_change` à chaque plugin actif. Propage l'erreur au premier
    /// échec.
    pub fn notify_change(&mut self, ctx: &PluginContext, changed: &[String]) -> Result<(), String> {
        for (plugin, enabled) in &mut self.plugins {
            if *enabled {
                plugin.on_change(ctx, changed)?;
            }
        }
        Ok(())
    }

    /// Décharge (`on_unload`) tous les plugins activés. Propage l'erreur au
    /// premier échec : un plugin dont le déchargement échoue reste activé
    /// (jamais de plugin mi-déchargé).
    pub fn unload_all(&mut self) -> Result<(), String> {
        for (plugin, enabled) in &mut self.plugins {
            if *enabled {
                plugin.on_unload()?;
                *enabled = false;
            }
        }
        Ok(())
    }

    /// Descripteurs (id/name/enabled) pour l'UI, dans l'ordre d'enregistrement.
    pub fn info(&self) -> Vec<PluginInfo<'_>> {
        self.plugins
            .iter()
            .map(|(plugin, enabled)| PluginInfo {
                id: plugin.id(),
                name: plugin.name(),
                enabled: *enabled,
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugins::api::{Plugin, PluginContext};
    use std::cell::RefCell;
    use std::rc::Rc;

    /// Un appel `on_change` reçu par un plugin de test.
    #[derive(Clone, Debug, PartialEq)]
    struct Change {
        plugin_id: String,
        changed: Vec<String>,
    }

    /// Journal partagé des appels reçus par les plugins de test.
    #[derive(Clone, Default)]
    struct Calls {
        loads: Rc<RefCell<Vec<String>>>,
        changes: Rc<RefCell<Vec<Change>>>,
        unloads: Rc<RefCell<Vec<String>>>,
    }

    /// Plugin de test : enregistre ses appels dans un [`Calls`] partagé et
    /// peut échouer sur commande.
    struct TestPlugin {
        id: String,
        name: String,
        calls: Calls,
        fail_load: bool,
        fail_change: bool,
        fail_unload: bool,
    }

    impl TestPlugin {
        fn new(id: &str, calls: &Calls) -> Self {
            Self {
                id: id.to_string(),
                name: format!("Plugin {id}"),
                calls: calls.clone(),
                fail_load: false,
                fail_change: false,
                fail_unload: false,
            }
        }
    }

    impl Plugin for TestPlugin {
        fn id(&self) -> &str {
            &self.id
        }

        fn name(&self) -> &str {
            &self.name
        }

        fn on_load(&mut self, _ctx: &PluginContext) -> Result<(), String> {
            self.calls.loads.borrow_mut().push(self.id.clone());
            if self.fail_load {
                return Err(format!("échec on_load : {}", self.id));
            }
            Ok(())
        }

        fn on_change(&mut self, _ctx: &PluginContext, changed: &[String]) -> Result<(), String> {
            self.calls.changes.borrow_mut().push(Change {
                plugin_id: self.id.clone(),
                changed: changed.to_vec(),
            });
            if self.fail_change {
                return Err(format!("échec on_change : {}", self.id));
            }
            Ok(())
        }

        fn on_unload(&mut self) -> Result<(), String> {
            self.calls.unloads.borrow_mut().push(self.id.clone());
            if self.fail_unload {
                return Err(format!("échec on_unload : {}", self.id));
            }
            Ok(())
        }
    }

    fn ctx() -> PluginContext<'static> {
        PluginContext {
            vault_name: "demo",
            note_count: 3,
        }
    }

    #[test]
    fn plugins_registre_vide_tolere_tout_le_cycle() {
        let mut registry = PluginRegistry::new();
        let c = ctx();

        assert!(registry.load_all(&c).is_ok());
        assert!(registry.notify_change(&c, &["a".to_string()]).is_ok());
        assert!(registry.unload_all().is_ok());
        assert!(registry.info().is_empty());
    }

    #[test]
    fn plugins_load_all_appelle_on_load_dans_l_ordre() {
        let calls = Calls::default();
        let mut registry = PluginRegistry::new();
        registry
            .register(Box::new(TestPlugin::new("a", &calls)))
            .unwrap();
        registry
            .register(Box::new(TestPlugin::new("b", &calls)))
            .unwrap();

        registry.load_all(&ctx()).unwrap();

        assert_eq!(
            *calls.loads.borrow(),
            vec!["a".to_string(), "b".to_string()]
        );
    }

    #[test]
    fn plugins_load_all_reentrant_n_appelle_pas_deux_fois_on_load() {
        let calls = Calls::default();
        let mut registry = PluginRegistry::new();
        registry
            .register(Box::new(TestPlugin::new("a", &calls)))
            .unwrap();

        registry.load_all(&ctx()).unwrap();
        registry.load_all(&ctx()).unwrap();

        assert_eq!(*calls.loads.borrow(), vec!["a".to_string()]);
    }

    #[test]
    fn plugins_register_refuse_un_id_doublon() {
        let calls = Calls::default();
        let mut registry = PluginRegistry::new();
        registry
            .register(Box::new(TestPlugin::new("a", &calls)))
            .unwrap();

        let err = registry
            .register(Box::new(TestPlugin::new("a", &calls)))
            .unwrap_err();
        assert!(err.contains("a"));

        // Le plugin existant est intact : toujours un seul exemplaire.
        assert_eq!(registry.info().len(), 1);
        assert_eq!(registry.info()[0].id, "a");
    }

    #[test]
    fn plugins_notify_change_n_atteint_que_les_plugins_actifs() {
        let calls = Calls::default();
        let mut registry = PluginRegistry::new();
        registry
            .register(Box::new(TestPlugin::new("a", &calls)))
            .unwrap();
        registry
            .register(Box::new(TestPlugin::new("b", &calls)))
            .unwrap();
        let changed = vec!["note.md".to_string()];

        // Avant chargement : aucun plugin actif, aucune notification.
        registry.notify_change(&ctx(), &changed).unwrap();
        assert!(calls.changes.borrow().is_empty());

        registry.load_all(&ctx()).unwrap();
        registry.notify_change(&ctx(), &changed).unwrap();

        assert_eq!(
            *calls.changes.borrow(),
            vec![
                Change {
                    plugin_id: "a".to_string(),
                    changed: changed.clone(),
                },
                Change {
                    plugin_id: "b".to_string(),
                    changed: changed.clone(),
                },
            ]
        );
    }

    #[test]
    fn plugins_unload_all_appelle_on_unload_des_plugins_charges() {
        let calls = Calls::default();
        let mut registry = PluginRegistry::new();
        registry
            .register(Box::new(TestPlugin::new("a", &calls)))
            .unwrap();
        registry
            .register(Box::new(TestPlugin::new("b", &calls)))
            .unwrap();
        registry.load_all(&ctx()).unwrap();

        registry.unload_all().unwrap();

        assert_eq!(
            *calls.unloads.borrow(),
            vec!["a".to_string(), "b".to_string()]
        );
        // Un second déchargement ne rappelle personne.
        registry.unload_all().unwrap();
        assert_eq!(calls.unloads.borrow().len(), 2);
    }

    #[test]
    fn plugins_info_reflete_l_etat_actif() {
        let calls = Calls::default();
        let mut registry = PluginRegistry::new();
        registry
            .register(Box::new(TestPlugin::new("a", &calls)))
            .unwrap();

        let before = registry.info();
        assert_eq!(before.len(), 1);
        assert_eq!(before[0].id, "a");
        assert_eq!(before[0].name, "Plugin a");
        assert!(!before[0].enabled);

        registry.load_all(&ctx()).unwrap();
        assert!(registry.info()[0].enabled);

        registry.unload_all().unwrap();
        assert!(!registry.info()[0].enabled);
    }

    #[test]
    fn plugins_load_all_propage_l_erreur_au_premier_echec() {
        let calls = Calls::default();
        let mut registry = PluginRegistry::new();
        registry
            .register(Box::new(TestPlugin::new("a", &calls)))
            .unwrap();
        let mut b = TestPlugin::new("b", &calls);
        b.fail_load = true;
        registry.register(Box::new(b)).unwrap();

        let err = registry.load_all(&ctx()).unwrap_err();
        assert!(err.contains("b"));

        // « a » est chargé ; « b » (en échec) reste désactivé.
        let info = registry.info();
        assert!(info[0].enabled);
        assert!(!info[1].enabled);
    }

    #[test]
    fn plugins_notify_change_propage_l_erreur() {
        let calls = Calls::default();
        let mut registry = PluginRegistry::new();
        let mut b = TestPlugin::new("b", &calls);
        b.fail_change = true;
        registry.register(Box::new(b)).unwrap();
        registry.load_all(&ctx()).unwrap();

        let err = registry
            .notify_change(&ctx(), &["x".to_string()])
            .unwrap_err();
        assert!(err.contains("b"));
    }

    #[test]
    fn plugins_unload_all_propage_l_erreur_sans_mi_dechargement() {
        let calls = Calls::default();
        let mut registry = PluginRegistry::new();
        registry
            .register(Box::new(TestPlugin::new("a", &calls)))
            .unwrap();
        let mut b = TestPlugin::new("b", &calls);
        b.fail_unload = true;
        registry.register(Box::new(b)).unwrap();
        registry.load_all(&ctx()).unwrap();

        let err = registry.unload_all().unwrap_err();
        assert!(err.contains("b"));

        // « a » est déchargé ; « b » (échec) reste activé : état cohérent.
        let info = registry.info();
        assert!(!info[0].enabled);
        assert!(info[1].enabled);
    }

    #[test]
    fn plugins_ordre_d_appel_stable_selon_enregistrement() {
        let calls = Calls::default();
        let mut registry = PluginRegistry::new();
        for id in ["alpha", "beta", "gamma"] {
            registry
                .register(Box::new(TestPlugin::new(id, &calls)))
                .unwrap();
        }
        registry.load_all(&ctx()).unwrap();
        registry.notify_change(&ctx(), &["z".to_string()]).unwrap();

        assert_eq!(
            *calls.loads.borrow(),
            vec!["alpha".to_string(), "beta".to_string(), "gamma".to_string()]
        );
        assert_eq!(
            *calls.changes.borrow(),
            vec![
                Change {
                    plugin_id: "alpha".to_string(),
                    changed: vec!["z".to_string()],
                },
                Change {
                    plugin_id: "beta".to_string(),
                    changed: vec!["z".to_string()],
                },
                Change {
                    plugin_id: "gamma".to_string(),
                    changed: vec!["z".to_string()],
                },
            ]
        );
        let ids: Vec<&str> = registry.info().iter().map(|i| i.id).collect();
        assert_eq!(ids, vec!["alpha", "beta", "gamma"]);
    }
}
