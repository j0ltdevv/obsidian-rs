use obsidian_rs::app;

fn main() -> iced::Result {
    iced::application(app::boot, app::update, app::view)
        .title("obsidian-rs")
        .font(std::borrow::Cow::Borrowed(
            &include_bytes!("../assets/fonts/Inter-Regular.ttf")[..],
        ))
        .default_font(obsidian_rs::ui::theme::font())
        .window_size((720, 480))
        .centered()
        .subscription(app::subscription)
        .theme(|app: &obsidian_rs::app::App| {
            use obsidian_rs::vault::themes::{DEFAULT_DARK, DEFAULT_LIGHT};
            let (bg, fg, accent) = if app.current_theme.eq_ignore_ascii_case("light") {
                (DEFAULT_LIGHT.bg, DEFAULT_LIGHT.fg, DEFAULT_LIGHT.accent)
            } else {
                (DEFAULT_DARK.bg, DEFAULT_DARK.fg, DEFAULT_DARK.accent)
            };
            let rgb = |c: (u8, u8, u8)| iced::Color::from_rgb8(c.0, c.1, c.2);
            iced::Theme::custom(
                "obsidian".to_string(),
                iced::theme::Palette {
                    background: rgb(bg),
                    text: rgb(fg),
                    primary: rgb(accent),
                    success: iced::Color::from_rgb8(46, 160, 67),
                    warning: iced::Color::from_rgb8(230, 150, 30),
                    danger: iced::Color::from_rgb8(214, 69, 69),
                },
            )
        })
        .run()
}
