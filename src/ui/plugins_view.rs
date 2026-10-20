//! Vue Plugins : liste les plugins enregistrés (id, nom, état activé).
//!
//! C'est un panneau de diagnostic : il lit [`PluginRegistry::info`] via
//! l'état de l'application et n'émet aucun message.

use iced::widget::{button, column, container, row, scrollable, text, Column};
use iced::{Alignment, Element, Length};

use crate::app::Message;
use crate::plugins::api::PluginInfo;

/// Panneau listant les plugins enregistrés, avec un bouton d'activation.
pub fn plugins_view<'a>(
    plugins: &[PluginInfo<'_>],
    on_toggle: impl Fn(&str) -> Message + 'a,
) -> Element<'a, Message> {
    let mut list: Column<'a, Message> = column![]
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
            let label = text(format!("{} ({}) — {}", info.name, info.id, state));
            let toggle = button(if info.enabled {
                "Désactiver"
            } else {
                "Activer"
            })
            .on_press(on_toggle(info.id))
            .style(crate::ui::ghost_button_style);
            list = list.push(row![label, toggle].spacing(8).align_y(Alignment::Center));
        }
    }

    container(scrollable(list))
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}
