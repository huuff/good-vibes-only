//! The TALLY UI. State is a single [`Data`] signal, persisted to
//! localStorage after every mutation. One DOM serves both form factors:
//! a 900px CSS breakpoint switches between the mobile bottom bar (design
//! option 1a) and the desktop rail + sidebar (2a).

mod ledger;
mod nav;
mod sheet;
mod sidebar;

use chrono::{Local, NaiveDate};
use dioxus::prelude::*;

use crate::store::Data;

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
    /// Calendar month shown in the detail sheet.
    pub month: Signal<NaiveDate>,
    /// Name/note edit mode inside the detail sheet.
    pub editing: Signal<bool>,
    pub name_draft: Signal<String>,
    pub note_draft: Signal<String>,
    /// Delete confirm armed.
    pub confirm: Signal<bool>,
}

impl Overlays {
    pub fn open_detail(&mut self, id: u64) {
        self.month.set(Local::now().date_naive());
        self.editing.set(false);
        self.confirm.set(false);
        self.detail.set(Some(id));
        push_history_entry();
    }

    pub fn open_add(&mut self) {
        self.name_draft.set(String::new());
        self.note_draft.set(String::new());
        self.adding.set(true);
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
    let overlays = Overlays {
        detail: use_signal(|| None),
        adding: use_signal(|| false),
        month: use_signal(|| Local::now().date_naive()),
        editing: use_signal(|| false),
        name_draft: use_signal(String::new),
        note_draft: use_signal(String::new),
        confirm: use_signal(|| false),
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
            }
        });
    });

    rsx! {
        document::Stylesheet { href: CSS }
        document::Style { {font_faces()} }
        div { class: "shell",
            {nav::rail(overlays)}
            main { class: "main",
                {ledger::ledger(data, overlays)}
                {nav::bottom_bar(overlays)}
            }
            {sidebar::sidebar(data)}
        }
        {sheet::detail_sheet(data, overlays)}
        {sheet::add_sheet(data, overlays)}
    }
}
