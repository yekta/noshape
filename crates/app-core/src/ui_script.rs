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

fn bounds() -> &'static Mutex<HashMap<String, Bounds<Pixels>>> {
    static B: OnceLock<Mutex<HashMap<String, Bounds<Pixels>>>> = OnceLock::new();
    B.get_or_init(Default::default)
}

/// `el` recording its bounds under `name` each time it is painted.
pub fn mark<E: ParentElement>(el: E, name: impl FnOnce() -> String) -> E {
    if !active() {
        return el;
    }
    let name = name();
    el.child(
        canvas(
            move |b, _, _| {
                if let Ok(mut m) = bounds().lock() {
                    m.insert(name.clone(), b);
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

/// The modal drawn since the last [`clear_dialog`], if any.
pub fn open_dialog() -> Option<serde_json::Value> {
    dialog_slot().lock().ok()?.clone()
}

pub fn clear_dialog() {
    if let Ok(mut d) = dialog_slot().lock() {
        *d = None;
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
    bounds().lock().ok()?.get(name).copied()
}

/// Every recorded name starting with `prefix`, with its bounds.
pub fn named(prefix: &str) -> Vec<(String, Bounds<Pixels>)> {
    bounds().lock().map(|m| m.iter().filter(|(k, _)| k.starts_with(prefix)).map(|(k, b)| (k.clone(), *b)).collect()).unwrap_or_default()
}
