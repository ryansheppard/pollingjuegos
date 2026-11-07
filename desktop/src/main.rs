mod app;
mod loopback;
mod screen;
mod session;
mod store;
mod task;

use app::App;

fn main() -> iced::Result {
    iced::application(App::new, App::update, App::view)
        .title("pollingjuegos")
        .subscription(App::subscription)
        .run()
}
