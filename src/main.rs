use obsy::app;

/// Recharge la taille de fenêtre persistée (sinon 1400×900, comme Obsidian).
fn saved_window_size() -> (u32, u32) {
    let path = app::window_size_path(std::path::Path::new(app::DEMO_VAULT_DIR));
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str::<(u32, u32)>(&s).ok())
        .unwrap_or((1400, 900))
}

fn main() -> iced::Result {
    iced::application(app::boot, app::update, app::view)
        .title("obsy")
        .font(std::borrow::Cow::Borrowed(
            &include_bytes!("../assets/fonts/Inter-Regular.ttf")[..],
        ))
        .default_font(obsy::ui::theme::font())
        .window_size(saved_window_size())
        .centered()
        .subscription(app::subscription)
        .theme(|app: &obsy::app::App| {
            // Palette du thème actif du vault : CSS du thème (+ snippets)
            // lu par theme_vars, avec repli sur les valeurs intégrées.
            let vars = obsy::ui::theme::load_theme_vars(&app.vault_root, &app.current_theme);
            obsy::ui::theme::theme_from_vars(&vars)
        })
        .run()
}
