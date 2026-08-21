mod clock;
mod i18n;
mod persist;
mod preferences;
mod rewards;
mod store;
mod todos;
mod ui;

fn main() {
    dioxus::launch(ui::app);
}
