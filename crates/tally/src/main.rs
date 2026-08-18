mod persist;
mod preferences;
mod store;
mod ui;

fn main() {
    dioxus::launch(ui::app);
}
