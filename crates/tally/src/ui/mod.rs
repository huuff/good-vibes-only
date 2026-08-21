//! The TALLY UI. State is a single [`Data`] signal, persisted to
//! localStorage after every mutation. One DOM serves both form factors:
//! a 900px CSS breakpoint switches between the mobile bottom bar (design
//! option 1a) and the desktop rail + sidebar (2a).

mod ledger;
mod nav;
mod rewards;
mod schedule;
mod settings;
mod sheet;
mod sidebar;
mod todos;

use chrono::NaiveDate;
use dioxus::prelude::*;

use crate::clock;
use crate::preferences::Preferences;
use crate::rewards::{Reward, RewardData};
use crate::store::{DEFAULT_STICKING_TARGET, Data};
use crate::todos::{Difficulty, Todo, TodoData};
use schedule::ScheduleDraft;

/// Tracked through the dioxus asset system (not inlined in index.html) so
/// `dx serve` hot-reloads style edits without a rebuild.
static CSS: Asset = asset!("/assets/style.css");
static ARCHIVO_400: Asset = asset!("/assets/fonts/archivo-400.woff2");
static ARCHIVO_600: Asset = asset!("/assets/fonts/archivo-600.woff2");
static ARCHIVO_800: Asset = asset!("/assets/fonts/archivo-800.woff2");
static ARCHIVO_900: Asset = asset!("/assets/fonts/archivo-900.woff2");

/// @font-face rules live here rather than in the stylesheet: the woff2
/// files go through the asset system (hashed filenames), so only Rust
/// knows their URLs.
fn font_faces() -> String {
    [
        (400, ARCHIVO_400),
        (600, ARCHIVO_600),
        (800, ARCHIVO_800),
        (900, ARCHIVO_900),
    ]
    .into_iter()
    .map(|(weight, font)| {
        format!(
            "@font-face{{font-family:'Archivo';font-style:normal;\
             font-weight:{weight};font-display:swap;\
             src:url('{font}') format('woff2')}}"
        )
    })
    .collect()
}

/// Signals for the overlays (detail sheet and add form), created once in
/// [`app`] and passed down by copy.
#[derive(Clone, Copy)]
pub struct Overlays {
    /// Habit whose detail sheet is open.
    pub detail: Signal<Option<u64>>,
    pub adding: Signal<bool>,
    pub adding_todo: Signal<bool>,
    /// Calendar month shown in the detail sheet.
    pub month: Signal<NaiveDate>,
    /// Name/schedule edit mode inside the detail sheet.
    pub editing: Signal<bool>,
    pub name_draft: Signal<String>,
    /// Schedule picker state for the add form and the edit mode.
    pub sched_draft: Signal<ScheduleDraft>,
    /// Editable repetition milestone for the habit-building phase.
    pub target_draft: Signal<u32>,
    /// Delete confirm armed.
    pub confirm: Signal<bool>,
    pub todo_name: Signal<String>,
    pub todo_date: Signal<String>,
    pub todo_time: Signal<String>,
    pub todo_difficulty: Signal<Difficulty>,
    /// Todo whose edit sheet is open (the add form, prefilled).
    pub todo_edit: Signal<Option<u64>>,
    pub adding_reward: Signal<bool>,
    pub reward_name: Signal<String>,
    pub reward_cost: Signal<String>,
    /// Reward whose edit sheet is open (the add form, prefilled).
    pub reward_edit: Signal<Option<u64>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Page {
    #[default]
    Today,
    Todos,
    Rewards,
    Settings,
}

impl Overlays {
    pub fn open_detail(&mut self, id: u64) {
        self.month.set(clock::today());
        self.editing.set(false);
        self.confirm.set(false);
        self.detail.set(Some(id));
        push_history_entry();
    }

    pub fn open_add(&mut self) {
        self.name_draft.set(String::new());
        self.sched_draft.set(ScheduleDraft::default());
        self.target_draft.set(DEFAULT_STICKING_TARGET);
        self.adding.set(true);
        push_history_entry();
    }

    pub fn open_add_todo(&mut self) {
        self.todo_name.set(String::new());
        self.todo_date.set(String::new());
        self.todo_time.set(String::new());
        self.todo_difficulty.set(Difficulty::default());
        self.adding_todo.set(true);
        push_history_entry();
    }

    pub fn open_add_reward(&mut self) {
        self.reward_name.set(String::new());
        self.reward_cost.set(String::new());
        self.adding_reward.set(true);
        push_history_entry();
    }

    /// The add form, prefilled from an existing reward, in edit mode.
    pub fn open_edit_reward(&mut self, reward: &Reward) {
        self.reward_name.set(reward.name.clone());
        self.reward_cost.set(reward.cost.to_string());
        self.confirm.set(false);
        self.reward_edit.set(Some(reward.id));
        push_history_entry();
    }

    /// The add form, prefilled from an existing todo, in edit mode.
    pub fn open_edit_todo(&mut self, todo: &Todo) {
        self.todo_name.set(todo.name.clone());
        self.todo_difficulty.set(todo.difficulty);
        self.todo_date.set(
            todo.target_date
                .map(|d| d.format("%Y-%m-%d").to_string())
                .unwrap_or_default(),
        );
        self.todo_time.set(
            todo.target_time
                .map(|t| t.format("%H:%M").to_string())
                .unwrap_or_default(),
        );
        self.confirm.set(false);
        self.todo_edit.set(Some(todo.id));
        push_history_entry();
    }

    /// Close whichever sheet is open. Goes through the history entry
    /// pushed on open — the popstate listener in [`app`] clears the
    /// signals — so UI closes (tap outside, Escape, save) and the
    /// platform back gesture are the same code path and the back stack
    /// never desyncs from what's on screen.
    pub fn dismiss(&self) {
        document::eval("history.back()");
    }
}

/// Every sheet open adds one history entry. That entry is what makes the
/// Android back button close the sheet instead of quitting: wry's
/// activity forwards back to `webview.goBack()` whenever the WebView has
/// history, and only lets the app exit when it doesn't.
fn push_history_entry() {
    document::eval("history.pushState({ sheet: true }, '')");
}

pub fn app() -> Element {
    let data = use_signal(Data::load);
    let todo_data = use_signal(TodoData::load);
    let reward_data = use_signal(RewardData::load);
    let preferences = use_signal(Preferences::load);
    let mut system_dark = use_signal(|| false);
    let page = use_signal(Page::default);
    let overlays = Overlays {
        detail: use_signal(|| None),
        adding: use_signal(|| false),
        adding_todo: use_signal(|| false),
        month: use_signal(clock::today),
        editing: use_signal(|| false),
        name_draft: use_signal(String::new),
        sched_draft: use_signal(ScheduleDraft::default),
        target_draft: use_signal(|| DEFAULT_STICKING_TARGET),
        confirm: use_signal(|| false),
        todo_name: use_signal(String::new),
        todo_date: use_signal(String::new),
        todo_time: use_signal(String::new),
        todo_difficulty: use_signal(Difficulty::default),
        todo_edit: use_signal(|| None),
        adding_reward: use_signal(|| false),
        reward_name: use_signal(String::new),
        reward_cost: use_signal(String::new),
        reward_edit: use_signal(|| None),
    };

    // Back-gesture handling: every history pop closes the open sheet
    // (clearing an already-clear signal is a no-op). The non-resolving
    // await keeps the eval's JS side alive so the listener can keep
    // sending for the app's lifetime.
    use_effect(move || {
        spawn(async move {
            let mut overlays = overlays;
            let mut pops = document::eval(
                "window.addEventListener('popstate', () => dioxus.send(true));
                 await new Promise(() => {});",
            );
            while pops.recv::<bool>().await.is_ok() {
                overlays.detail.set(None);
                overlays.adding.set(false);
                overlays.adding_todo.set(false);
                overlays.todo_edit.set(None);
                overlays.adding_reward.set(false);
                overlays.reward_edit.set(None);
            }
        });
    });

    // Keep the unsaved appearance choice in sync with the OS. CSS handles
    // first paint; this signal gives the toggle an accurate accessible state
    // and lets its first click create the opposite explicit override.
    use_effect(move || {
        spawn(async move {
            let mut changes = document::eval(
                "const media = window.matchMedia('(prefers-color-scheme: dark)');
                 dioxus.send(media.matches);
                 media.addEventListener('change', event => dioxus.send(event.matches));
                 await new Promise(() => {});",
            );
            while let Ok(dark) = changes.recv::<bool>().await {
                system_dark.set(dark);
            }
        });
    });

    let theme_class = match preferences().dark_mode {
        Some(true) => "app theme-dark",
        Some(false) => "app theme-light",
        None => "app theme-system",
    };

    let lang = preferences().language;
    let rewards_on = preferences().rewards_enabled;
    rsx! {
        document::Stylesheet { href: CSS }
        document::Style { {font_faces()} }
        div { class: theme_class,
            div { class: "shell",
                {nav::rail(page, overlays, lang, rewards_on)}
                main { class: "main",
                    match page() {
                        Page::Today => rsx! { {ledger::ledger(data, overlays, preferences)} },
                        Page::Todos => rsx! { {todos::todos(todo_data, overlays, lang)} },
                        Page::Rewards => rsx! { {rewards::rewards(reward_data, data, todo_data, overlays, lang)} },
                        Page::Settings => rsx! { {settings::settings(preferences, system_dark)} },
                    }
                    {nav::bottom_bar(page, overlays, lang, rewards_on)}
                }
                if page() == Page::Today {
                    {sidebar::sidebar(data, preferences)}
                }
            }
            {sheet::detail_sheet(data, overlays, preferences)}
            {sheet::add_sheet(data, overlays, preferences)}
            {todos::add_sheet(todo_data, overlays, lang, rewards_on)}
            {rewards::add_sheet(reward_data, overlays, lang)}
        }
    }
}
