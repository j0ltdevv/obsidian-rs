use obsidian_rs::app;

fn main() -> iced::Result {
    iced::application(app::boot, app::update, app::view)
        .title("obsidian-rs")
        .window_size((720, 480))
        .centered()
        .run()
}
