//! Vue Plugins : liste les plugins enregistrés (id, nom, état activé).
//!
//! C'est un panneau de diagnostic : il lit [`PluginRegistry::info`] via
//! l'état de l'application et n'émet aucun message.

use iced::widget::{column, container, scrollable, text, Column};
use iced::{Element, Length};

use crate::app::Message;
use crate::plugins::api::PluginInfo;

/// Panneau listant les plugins enregistrés.
pub fn plugins_view(plugins: &[PluginInfo<'_>]) -> Element<'static, Message> {
    let mut list: Column<'static, Message> = column![]
        .spacing(6)
        .padding([12, 16])
        .width(Length::Fill)
        .height(Length::Fill);

    list = list.push(text("Plugins").size(20));
    if plugins.is_empty() {
        list = list.push(text("Aucun plugin enregistré."));
    } else {
        for info in plugins {
            let state = if info.enabled {
                "✓ activé"
            } else {
                "— désactivé"
            };
            list = list.push(text(format!("{} ({}) — {}", info.name, info.id, state)));
        }
    }

    container(scrollable(list))
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}
