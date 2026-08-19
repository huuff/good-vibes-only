mod clock;
mod persist;
mod preferences;
mod store;
mod todos;
mod ui;

fn main() {
    dioxus::launch(ui::app);
}
