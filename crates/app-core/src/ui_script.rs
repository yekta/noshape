//! The interaction parity suite's hooks (tests/ui-parity/README.md): with
//! `NOSHAPE_UI_SCRIPT` set, named controls record their window bounds as they
//! are painted, so the app's driver clicks them by name ("field:Depth",
//! "feature:Extrude 1"). Off, `mark` returns the element untouched.

use gpui_kit::*;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

/// Whether the app runs a ui-parity scenario.
pub fn active() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("NOSHAPE_UI_SCRIPT").is_some())
}

/// Each named control's window and bounds when last painted.
type Marks = Mutex<HashMap<String, (WindowId, Bounds<Pixels>)>>;

fn bounds() -> &'static Marks {
    static B: OnceLock<Marks> = OnceLock::new();
    B.get_or_init(Default::default)
}

type PageState = std::rc::Rc<dyn Fn(&mut Window, &mut App) -> serde_json::Value>;

thread_local! {
    static PAGES: std::cell::RefCell<HashMap<WindowId, PageState>> = Default::default();
}

/// How the driver reads window `window`'s state (README "state"): each kind of window (a document, the documents
/// page, Preferences) registers its reader when it opens.
pub fn register_page(window: WindowId, state: impl Fn(&mut Window, &mut App) -> serde_json::Value + 'static) {
    if active() {
        PAGES.with(|p| p.borrow_mut().insert(window, std::rc::Rc::new(state)));
    }
}

/// The state of window `window`, read by its registered reader.
pub fn page_state(window: &mut Window, cx: &mut App) -> Option<serde_json::Value> {
    let f = PAGES.with(|p| p.borrow().get(&window.window_handle().window_id()).cloned())?;
    Some(f(window, cx))
}

/// `el` recording its bounds under `name` each time it is painted.
pub fn mark<E: ParentElement>(el: E, name: impl FnOnce() -> String) -> E {
    if !active() {
        return el;
    }
    let name = name();
    el.child(
        canvas(
            move |b, window, _| {
                if let Ok(mut m) = bounds().lock() {
                    m.insert(name.clone(), (window.window_handle().window_id(), b));
                }
            },
            |_, _, _, _| {},
        )
        .absolute()
        .inset_0(),
    )
}

/// `el` in a box recording its bounds under `name` (for elements that take no
/// children); `el` itself when no scenario runs.
pub fn wrap(el: AnyElement, name: impl FnOnce() -> String) -> AnyElement {
    if !active() {
        return el;
    }
    mark(div().relative().child(el), name).into_any_element()
}

fn open_list_slot() -> &'static Mutex<Option<serde_json::Value>> {
    static L: OnceLock<Mutex<Option<serde_json::Value>>> = OnceLock::new();
    L.get_or_init(Default::default)
}

/// A dropdown list drawn open (`Some`, as `state.list`) or closed.
pub fn note_list(list: Option<serde_json::Value>) {
    if let Ok(mut l) = open_list_slot().lock() {
        *l = list;
    }
}

/// The dropdown list open now, if any.
pub fn open_list() -> Option<serde_json::Value> {
    open_list_slot().lock().ok()?.clone()
}

fn tooltip_slot() -> &'static Mutex<Option<String>> {
    static T: OnceLock<Mutex<Option<String>>> = OnceLock::new();
    T.get_or_init(Default::default)
}

/// A tooltip drawn now (`state.tooltip`); the driver clears it and redraws before reading.
pub fn note_tooltip(text: &str) {
    if !active() {
        return;
    }
    if let Ok(mut t) = tooltip_slot().lock() {
        *t = Some(text.to_string());
    }
}

/// The tooltip drawn since the last [`clear_tooltip`], if any.
pub fn shown_tooltip() -> Option<String> {
    tooltip_slot().lock().ok()?.clone()
}

pub fn clear_tooltip() {
    if let Ok(mut t) = tooltip_slot().lock() {
        *t = None;
    }
}

fn dialog_slot() -> &'static Mutex<Option<serde_json::Value>> {
    static D: OnceLock<Mutex<Option<serde_json::Value>>> = OnceLock::new();
    D.get_or_init(Default::default)
}

/// A modal drawn now (`state.dialog`: its title and fields, as a feature dialog's); the driver clears it and
/// redraws before reading, so a closed one is gone.
pub fn note_dialog(dialog: impl FnOnce() -> serde_json::Value) {
    if !active() {
        return;
    }
    if let Ok(mut d) = dialog_slot().lock() {
        *d = Some(dialog());
    }
}

fn fields_slot() -> &'static Mutex<Vec<serde_json::Value>> {
    static F: OnceLock<Mutex<Vec<serde_json::Value>>> = OnceLock::new();
    F.get_or_init(Default::default)
}

/// A field of the panel being drawn (`{"label", "kind", "text"}`), taken by the panel's [`note_dialog_fields`].
pub fn note_field(field: impl FnOnce() -> serde_json::Value) {
    if !active() {
        return;
    }
    if let Ok(mut f) = fields_slot().lock() {
        f.push(field());
    }
}

/// The fields noted since the last take (a page's: Preferences).
pub fn take_fields() -> Vec<serde_json::Value> {
    let all = fields_slot().lock().map(|mut f| std::mem::take(&mut *f)).unwrap_or_default();
    // A page drawn more than once since: each field once, as last drawn, in the order first drawn.
    let mut out: Vec<serde_json::Value> = Vec::new();
    for f in all {
        match out.iter_mut().find(|o| o["label"] == f["label"]) {
            Some(o) => *o = f,
            None => out.push(f),
        }
    }
    out
}

/// A panel drawn now (☰ › Workspace units…) as `state.dialog`, with the fields noted since the last one.
pub fn note_dialog_fields(title: &str) {
    if !active() {
        return;
    }
    let fields = fields_slot().lock().map(|mut f| std::mem::take(&mut *f)).unwrap_or_default();
    note_dialog(|| serde_json::json!({ "title": title, "kind": "other", "tabs": [], "fields": fields, "error": null, "ok": "enabled" }));
}

/// A modal drawn now by its title alone, unless the frame noted it (or another) in full with [`note_dialog`].
pub fn note_modal(title: &str) {
    if !active() {
        return;
    }
    if let Ok(mut d) = dialog_slot().lock()
        && d.is_none()
    {
        *d = Some(serde_json::json!({ "kind": "modal", "title": title, "fields": [], "buttons": [] }));
    }
}

/// The modal drawn since the last [`clear_dialog`], if any.
pub fn open_dialog() -> Option<serde_json::Value> {
    dialog_slot().lock().ok()?.clone()
}

pub fn clear_dialog() {
    if let Ok(mut d) = dialog_slot().lock() {
        *d = None;
    }
    if let Ok(mut f) = fields_slot().lock() {
        f.clear();
    }
}

/// Forget every recorded control (before a frame that records what shows now).
pub fn clear() {
    if let Ok(mut m) = bounds().lock() {
        m.clear();
    }
}

/// The bounds `name` had when last painted.
pub fn bounds_of(name: &str) -> Option<Bounds<Pixels>> {
    bounds().lock().ok()?.get(name).map(|(_, b)| *b)
}

/// The window `name` was last painted in, with its bounds there.
pub fn window_bounds_of(name: &str) -> Option<(WindowId, Bounds<Pixels>)> {
    bounds().lock().ok()?.get(name).copied()
}

/// Every recorded name starting with `prefix`, with its bounds.
pub fn named(prefix: &str) -> Vec<(String, Bounds<Pixels>)> {
    bounds().lock().map(|m| m.iter().filter(|(k, _)| k.starts_with(prefix)).map(|(k, (_, b))| (k.clone(), *b)).collect()).unwrap_or_default()
}

/// `state.focus` for a text field with the keyboard: its text and selection in characters.
pub fn field_focus(label: &str, st: &gpui_kit::component::input::InputState) -> serde_json::Value {
    let text = st.value().to_string();
    let sel = st.selected_range();
    let chars = |b: usize| text.get(..b.min(text.len())).map_or(0, |t| t.chars().count());
    let (a, b) = (chars(sel.start), chars(sel.end));
    serde_json::json!({ "kind": "field", "label": label, "text": text, "selection": [a, b], "all": a == 0 && b == text.chars().count() && b > 0 })
}
