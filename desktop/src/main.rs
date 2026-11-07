#![cfg_attr(windows, windows_subsystem = "windows")]

mod app;
mod loopback;
mod screen;
mod session;
mod store;
mod style;
mod task;

use app::App;

fn main() -> iced::Result {
    let application = iced::application(App::new, App::update, App::view)
        .title("pollingjuegos")
        .theme(App::theme)
        .subscription(App::subscription)
        .default_font(style::BODY);
    style::FONTS
        .into_iter()
        .fold(application, |application, font| application.font(font))
        .run()
}
