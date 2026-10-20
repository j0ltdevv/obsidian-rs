//! Système de plugins : API, registre et scaffold.
//!
//! Les plugins vivent dans le binaire (pas d'exécution de code externe) et
//! exposent des points d'extension définis par [`api::Plugin`].

pub mod api;
pub mod demo_stats;
pub mod demo_todo;
pub mod registry;
pub mod scaffold;
