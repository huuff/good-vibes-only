#[cfg(target_os = "android")]
mod android;
mod clock;
mod i18n;
mod persist;
mod preferences;
mod reminders;
mod rewards;
mod store;
mod todos;
mod ui;

fn main() {
    dioxus::launch(ui::app);
}
