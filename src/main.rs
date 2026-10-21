use obsidian_rs::app;

fn main() -> iced::Result {
    iced::application(app::boot, app::update, app::view)
        .title("obsidian-rs")
        .window_size((720, 480))
        .centered()
        .subscription(app::subscription)
        .theme(|app: &obsidian_rs::app::App| {
            if app.current_theme.eq_ignore_ascii_case("light") {
                iced::Theme::Light
            } else {
                iced::Theme::Dark
            }
        })
        .run()
}
