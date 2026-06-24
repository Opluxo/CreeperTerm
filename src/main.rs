mod app;
mod config;
mod plugin;
mod ssh;
mod terminal;
mod ui;

fn main() -> iced::Result {
    env_logger::init();
    iced::application("CreeperTerm", app::App::update, app::App::view)
        .subscription(app::App::subscription)
        .run_with("creeper-term", app::App::new)
}
