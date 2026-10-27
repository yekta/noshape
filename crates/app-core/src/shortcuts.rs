//! Onshape's customizable keyboard shortcuts ("My account › Preferences ›
//! Keyboard shortcuts", live: shortcuts/30-47, dom/customize-keyboard-
//! shortcuts.txt and dom/keyboard-shortcuts.txt).
//!
//! The registry lists every customizable action of the six tabs (General,
//! Part Studio, Assembly, 3D view, Sketch, Drawing) with Onshape's default
//! key, plus the Onshape-controlled (🔒) rows the Shift+/ sheet shows.
//! Customizations are stored in `<config>/noshape/keyboard-shortcuts.json`
//! and applied to GPUI's keymap at startup and right after every edit:
//! the default bindings of `actions::default_key_bindings` stay exactly as
//! they are until an action is customized; then its default bindings are
//! replaced by the new key (same contexts), or dropped when it is disabled.
//! Actions without a default key are bound to [`RunShortcut`], which the
//! Part Studio (features, sketch tools, 3D view and analysis commands) and
//! the drawing editor (through the workspace) carry out. Drawing keys are
//! handled by the drawing editor's own key handler, so for them a keystroke
//! interceptor maps a customized key to the action's default key (or runs
//! it) and swallows a default key that was moved or disabled.

use gpui_kit::*;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::OnceLock;

/// Runs a registry action that has no GPUI action of its own (a user key
/// for an action Onshape leaves unassigned): `id` is the entry id.
#[derive(Clone, PartialEq, Debug, Action)]
#[action(namespace = noshape, no_json)]
pub struct RunShortcut {
    pub id: &'static str,
}

/// The registry's data (shared with the agent API's ui_shortcuts).
pub use noshape_ops::shortcuts::{ENTRIES, Entry, LOCKED, Tab, entries, entry};

/// The entry whose GPUI action is `action_name` ("noshape::SketchLine").
fn entry_for_action(action_name: &str) -> Option<&'static Entry> {
    let short = action_name.rsplit("::").next().unwrap_or(action_name);
    ENTRIES.iter().find(|e| e.action == Some(short))
}

// ------------------------------------------------------------------ keys

/// Keys produced with Shift on a US layout, and the key they are on.
const SHIFTED: &[(&str, &str)] = &[
    ("!", "1"), ("@", "2"), ("#", "3"), ("$", "4"), ("%", "5"), ("^", "6"), ("&", "7"), ("*", "8"), ("(", "9"), (")", "0"),
    ("_", "-"), ("+", "="), ("{", "["), ("}", "]"), ("|", "\\"), (":", ";"), ("\"", "'"), ("<", ","), (">", "."), ("?", "/"), ("~", "`"),
];

/// A keystroke in the registry's canonical form: `ctrl-alt-shift-cmd-key`,
/// lowercase, shifted symbols as Shift + the key they are on ("shift-/").
pub fn canonical(k: &Keystroke) -> String {
    let mut key = k.key.to_lowercase();
    let mut shift = k.modifiers.shift;
    if let Some((_, base)) = SHIFTED.iter().find(|(sym, _)| *sym == key) {
        key = base.to_string();
        shift = true;
    }
    let m = &k.modifiers;
    let mut s = String::new();
    for (on, name) in [(m.control, "ctrl-"), (m.alt, "alt-"), (shift, "shift-"), (m.platform, "cmd-"), (m.function, "fn-")] {
        if on {
            s.push_str(name);
        }
    }
    s.push_str(&key);
    s
}

/// Parses a stored key (GPUI keystroke syntax) into canonical form.
pub fn canonical_str(key: &str) -> Option<String> {
    let key = key.trim();
    if key.is_empty() {
        return None;
    }
    Keystroke::parse(&key.to_lowercase()).ok().map(|k| canonical(&k))
}

/// The keystrokes a canonical key is bound as: the key, and for Shift +
/// a symbol key also the symbol itself (some platforms report the shifted
/// character), as the default bindings do ("shift-1" and "!").
fn binding_variants(key: &str) -> Vec<String> {
    let mut out = vec![key.to_string()];
    if let Some((mods, base)) = key.rsplit_once("shift-")
        && let Some((sym, _)) = SHIFTED.iter().find(|(_, b)| *b == base)
    {
        out.push(format!("{mods}{sym}"));
        out.push(format!("{mods}shift-{sym}"));
    }
    out
}

/// The chips Onshape shows for a key: modifiers and the key separately
/// ("shift", "q"; "alt/⌥", "c"; "page down").
pub fn key_chips(key: &str) -> Vec<String> {
    let Ok(k) = Keystroke::parse(key) else { return vec![key.to_string()] };
    let m = &k.modifiers;
    let mut out = Vec::new();
    for (on, name) in [(m.control, "ctrl"), (m.alt, "alt/⌥"), (m.shift, "shift"), (m.platform, "cmd"), (m.function, "fn")] {
        if on {
            out.push(name.to_string());
        }
    }
    let name = match k.key.as_str() {
        "pagedown" => "page down".to_string(),
        "pageup" => "page up".to_string(),
        other => other.to_string(),
    };
    out.push(name);
    out
}

/// A key as Onshape writes it in the edit box ("shift+q").
pub fn key_text(key: &str) -> String {
    key_chips(key).join("+")
}

/// What a key press in the "Edit shortcut" box does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Recorded {
    /// A combination to assign (canonical).
    Key(String),
    /// Enter: confirm (not recorded).
    Confirm,
    /// Esc: close without saving.
    Cancel,
    /// Not a valid entry (Win, num lock, backspace, arrows, caps lock,
    /// scroll lock, F1-F12, the context menu key) or a lone modifier.
    Ignored,
}

/// Classifies a key press in the edit box.
pub fn record(k: &Keystroke) -> Recorded {
    let key = k.key.to_lowercase();
    match key.as_str() {
        "enter" => return Recorded::Confirm,
        "escape" => return Recorded::Cancel,
        "shift" | "control" | "ctrl" | "alt" | "platform" | "cmd" | "super" | "win" | "function" | "fn" | "backspace" | "left" | "right" | "up" | "down" | "capslock" | "numlock" | "scrolllock" | "menu" | "contextmenu" | "" => {
            return Recorded::Ignored;
        }
        _ => {}
    }
    if key.len() > 1 && key.starts_with('f') && key[1..].parse::<u32>().is_ok_and(|n| (1..=12).contains(&n)) {
        return Recorded::Ignored;
    }
    // The Windows key is not a valid modifier (Onshape); Cmd is on macOS.
    if k.modifiers.platform && !cfg!(target_os = "macos") {
        return Recorded::Ignored;
    }
    Recorded::Key(canonical(k))
}

// ------------------------------------------------------------------ state

/// A user change to an action's shortcut.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Custom {
    /// The key (canonical); `None` = Onshape's default key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    /// Disabled (shown struck through, the key does nothing).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub disabled: bool,
}

/// How a row's key is shown.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyStyle {
    Default,
    Customized,
    Disabled,
}

/// The user's shortcut customizations, by entry id.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Shortcuts {
    custom: BTreeMap<String, Custom>,
}

impl Shortcuts {
    pub fn from_json(text: &str) -> Self {
        let custom: BTreeMap<String, Custom> = serde_json::from_str(text).unwrap_or_default();
        let custom = custom
            .into_iter()
            .filter(|(id, _)| entry(id).is_some())
            .map(|(id, c)| (id, Custom { key: c.key.and_then(|k| canonical_str(&k)), disabled: c.disabled }))
            .collect();
        Self { custom }
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(&self.custom).unwrap_or_default()
    }

    pub fn is_empty(&self) -> bool {
        self.custom.is_empty()
    }

    /// The key the action has (customized or default), even if disabled.
    pub fn key(&self, e: &Entry) -> Option<String> {
        match self.custom.get(e.id).and_then(|c| c.key.clone()) {
            Some(k) => Some(k),
            None => e.default.and_then(canonical_str),
        }
    }

    pub fn is_disabled(&self, e: &Entry) -> bool {
        self.custom.get(e.id).is_some_and(|c| c.disabled)
    }

    pub fn is_customized(&self, e: &Entry) -> bool {
        self.custom.contains_key(e.id)
    }

    /// The key that runs the action (`None`: unassigned or disabled).
    pub fn active_key(&self, e: &Entry) -> Option<String> {
        if self.is_disabled(e) { None } else { self.key(e) }
    }

    pub fn style(&self, e: &Entry) -> KeyStyle {
        if self.is_disabled(e) {
            KeyStyle::Disabled
        } else if self.is_customized(e) {
            KeyStyle::Customized
        } else {
            KeyStyle::Default
        }
    }

    /// The other action of the tab that `key` runs now (the conflict
    /// "Confirming the changes will disable the shortcut X").
    pub fn conflict_for(&self, e: &Entry, key: &str) -> Option<&'static Entry> {
        entries(e.tab).find(|o| o.id != e.id && self.active_key(o).as_deref() == Some(key))
    }

    /// Two actions of the tab run on this action's key (Onshape's ❗).
    pub fn has_conflict(&self, e: &Entry) -> bool {
        self.active_key(e).is_some_and(|k| self.conflict_for(e, &k).is_some())
    }

    /// Confirms the edit popup: `key` (canonical; `None` keeps the current
    /// key) and the "Disable shortcut" box. Other actions of the tab on the
    /// same key are disabled (they keep their key, struck through).
    /// Returns them.
    pub fn assign(&mut self, e: &Entry, key: Option<String>, disabled: bool) -> Vec<&'static Entry> {
        let key = key.or_else(|| self.key(e));
        let mut others = Vec::new();
        if !disabled && let Some(k) = &key {
            while let Some(o) = self.conflict_for(e, k) {
                let mut c = self.custom.get(o.id).cloned().unwrap_or_default();
                c.disabled = true;
                self.custom.insert(o.id.to_string(), c);
                others.push(o);
            }
        }
        let default = e.default.and_then(canonical_str);
        let custom = Custom { key: key.filter(|k| Some(k) != default.as_ref()), disabled };
        if custom == Custom::default() {
            self.custom.remove(e.id);
        } else {
            self.custom.insert(e.id.to_string(), custom);
        }
        others
    }

    /// The row's ⟲ / the popup's "Reset to default".
    pub fn reset(&mut self, e: &Entry) {
        self.custom.remove(e.id);
    }

    /// "Reset all".
    pub fn reset_all(&mut self) {
        self.custom.clear();
    }
}

// ------------------------------------------------------------------ global state

fn state() -> &'static RwLock<Shortcuts> {
    static STATE: OnceLock<RwLock<Shortcuts>> = OnceLock::new();
    STATE.get_or_init(|| RwLock::new(load()))
}

fn path() -> std::path::PathBuf {
    crate::paths::config_dir().join("keyboard-shortcuts.json")
}

fn load() -> Shortcuts {
    if let Ok(text) = std::fs::read_to_string(path()) {
        return Shortcuts::from_json(&text);
    }
    // Sketch shortcuts of the earlier sketch-only preferences
    // (`sketch-shortcuts.json`: {"<tool name>": "<key>"}).
    let old = crate::paths::config_dir().join("sketch-shortcuts.json");
    let migrated = std::fs::read_to_string(old).map(|t| migrate_sketch_shortcuts(&t)).unwrap_or_default();
    if !migrated.is_empty() {
        let _ = std::fs::write(path(), migrated.to_json());
    }
    migrated
}

/// Converts the earlier sketch shortcut file into customizations.
fn migrate_sketch_shortcuts(text: &str) -> Shortcuts {
    let mut s = Shortcuts::default();
    let Ok(map) = serde_json::from_str::<BTreeMap<String, String>>(text) else { return s };
    for (name, key) in map {
        let n = name.trim().to_lowercase();
        let alias = match n.as_str() {
            "use / project edges" => "use",
            "center rectangle" => "center point rectangle",
            "mirror" => "mirror sketch",
            "fillet" => "sketch fillet",
            "chamfer" => "sketch chamfer",
            other => other,
        };
        let Some(e) = entries(Tab::Sketch).find(|e| e.plain_name().to_lowercase() == alias) else { continue };
        if let Some(k) = canonical_str(&key) {
            s.assign(e, Some(k), false);
        }
    }
    s
}

/// The current customizations.
pub fn current() -> Shortcuts {
    state().read().clone()
}

/// Changes the customizations, saves them and applies them to the keymap.
pub fn update(cx: &mut App, f: impl FnOnce(&mut Shortcuts)) {
    let saved = {
        let mut s = state().write();
        f(&mut s);
        s.clone()
    };
    let p = path();
    let result = if saved.is_empty() { std::fs::remove_file(&p).or_else(|e| if e.kind() == std::io::ErrorKind::NotFound { Ok(()) } else { Err(e) }) } else { std::fs::write(&p, saved.to_json()) };
    if let Err(e) = result {
        log::error!("could not save keyboard shortcuts to {}: {e}", p.display());
    }
    // Not inside the key dispatch that may have confirmed the edit.
    cx.defer(apply);
    cx.refresh_windows();
}

// ------------------------------------------------------------------ keymap

/// Marks the bindings this module manages in the keymap.
const META: KeyBindingMetaIndex = KeyBindingMetaIndex(0x5_c0de);

/// The default bindings of `actions::default_key_bindings`.
struct DefaultBindings(Vec<KeyBinding>);
impl Global for DefaultBindings {}

/// Installs the default bindings with the user's customizations, and the
/// drawing key interceptor. Called once at startup.
pub fn init(cx: &mut App, defaults: Vec<KeyBinding>) {
    let managed = managed_bindings(&defaults, &state().read());
    cx.set_global(DefaultBindings(defaults));
    cx.bind_keys(managed);
    cx.intercept_keystrokes(drawing_interceptor).detach();
}

/// Re-applies the customizations to the keymap, keeping the managed
/// bindings where they were among the others (precedence unchanged).
fn apply(cx: &mut App) {
    let Some(defaults) = cx.try_global::<DefaultBindings>() else { return };
    let mut managed = managed_bindings(&defaults.0, &state().read());
    let keymap = cx.key_bindings();
    {
        let mut km = keymap.borrow_mut();
        let all: Vec<KeyBinding> = km.bindings().cloned().collect();
        let mut out = Vec::with_capacity(all.len() + managed.len());
        for b in all {
            if b.meta() == Some(META) {
                out.append(&mut managed);
            } else {
                out.push(b);
            }
        }
        out.append(&mut managed);
        km.clear();
        km.add_bindings(out);
    }
    cx.refresh_windows();
}

fn binding(key: &str, action: Box<dyn Action>, context: Option<Rc<KeyBindingContextPredicate>>) -> Option<KeyBinding> {
    KeyBinding::load(key, action, context, false, None, &DummyKeyboardMapper).ok().map(|b| b.with_meta(META))
}

/// The keymap bindings for the defaults and the customizations: the
/// defaults in order, minus those of customized actions, then each
/// customized action's key in the contexts its defaults had (actions
/// without a GPUI action: [`RunShortcut`] in the tab's context). Drawing
/// keys are left to the drawing interceptor.
fn managed_bindings(defaults: &[KeyBinding], s: &Shortcuts) -> Vec<KeyBinding> {
    let mut out: Vec<KeyBinding> = Vec::with_capacity(defaults.len() + 8);
    for b in defaults {
        if entry_for_action(b.action().name()).is_some_and(|e| e.tab != Tab::Drawing && s.is_customized(e)) {
            continue;
        }
        out.push(b.clone().with_meta(META));
    }
    for e in ENTRIES.iter().filter(|e| e.tab != Tab::Drawing && s.is_customized(e)) {
        let Some(key) = s.active_key(e) else { continue };
        let variants = binding_variants(&key);
        let defs: Vec<&KeyBinding> = match e.action {
            Some(name) => defaults.iter().filter(|b| b.action().name().rsplit("::").next() == Some(name)).collect(),
            None => Vec::new(),
        };
        if defs.is_empty() {
            let ctx = KeyBindingContextPredicate::parse(e.tab.run_context()).ok().map(Rc::new);
            for v in &variants {
                out.extend(binding(v, Box::new(RunShortcut { id: e.id }), ctx.clone()));
            }
            continue;
        }
        let mut contexts: Vec<Option<Rc<KeyBindingContextPredicate>>> = Vec::new();
        for d in defs {
            let ctx = d.predicate();
            if contexts.contains(&ctx) {
                continue;
            }
            contexts.push(ctx.clone());
            for v in &variants {
                out.extend(binding(v, d.action().boxed_clone(), ctx.clone()));
            }
        }
    }
    out
}

// ------------------------------------------------------------------ drawings

/// What the drawing interceptor does with a key.
#[derive(Clone, Debug, PartialEq, Eq)]
enum DrawingRoute {
    /// Not customized: the drawing editor's own handling.
    Pass,
    /// A default key that was moved or disabled: nothing.
    Swallow,
    /// A customized key: press the action's default key instead.
    Replay(String),
    /// A customized key of an action without a default key.
    Run(&'static str),
}

/// Actions that apply in a drawing (the Drawing tab and the sketch
/// actions marked `*`), Drawing tab first.
fn drawing_entries() -> impl Iterator<Item = &'static Entry> {
    entries(Tab::Drawing).chain(entries(Tab::Sketch).filter(|e| e.in_drawing_sketch()))
}

fn drawing_route(s: &Shortcuts, key: &str) -> DrawingRoute {
    if let Some(e) = drawing_entries().find(|e| s.active_key(e).as_deref() == Some(key)) {
        let default = e.default.and_then(canonical_str);
        return match default {
            Some(d) if d == key => DrawingRoute::Pass,
            Some(d) => DrawingRoute::Replay(d),
            None => DrawingRoute::Run(e.id),
        };
    }
    if drawing_entries().any(|e| s.is_customized(e) && e.default.and_then(canonical_str).as_deref() == Some(key)) {
        return DrawingRoute::Swallow;
    }
    DrawingRoute::Pass
}

thread_local! {
    /// Set while a replayed default key is dispatched.
    static REPLAYING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

fn drawing_interceptor(ev: &KeystrokeEvent, window: &mut Window, cx: &mut App) {
    if REPLAYING.with(|r| r.get()) {
        return;
    }
    let in_drawing = ev.context_stack.iter().any(|c| c.contains("Drawing"));
    if !in_drawing || ev.context_stack.iter().any(|c| c.contains("Input")) {
        return;
    }
    let route = {
        let s = state().read();
        if s.is_empty() {
            return;
        }
        drawing_route(&s, &canonical(&ev.keystroke))
    };
    match route {
        DrawingRoute::Pass => {}
        DrawingRoute::Swallow => cx.stop_propagation(),
        DrawingRoute::Replay(default) => {
            cx.stop_propagation();
            let Ok(k) = Keystroke::parse(&default) else { return };
            window.defer(cx, move |window, cx| {
                REPLAYING.with(|r| r.set(true));
                window.dispatch_keystroke(k, cx);
                REPLAYING.with(|r| r.set(false));
            });
        }
        DrawingRoute::Run(id) => {
            cx.stop_propagation();
            window.dispatch_action(Box::new(RunShortcut { id }), cx);
        }
    }
}

// ------------------------------------------------------------------ Shift+/ sheet

/// A row of the Shift+/ sheet.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SheetRow {
    /// Key groups (alternatives), each a list of chips.
    pub keys: Vec<Vec<String>>,
    /// Text before the chips ("select rollback bar").
    pub prefix: Option<&'static str>,
    pub name: &'static str,
    pub style: KeyStyle,
    pub locked: bool,
    pub conflict: bool,
}

fn locked_row(keys: &'static str, name: &'static str) -> SheetRow {
    let (prefix, rest) = match keys.strip_prefix("select rollback bar ") {
        Some(r) => (Some("select rollback bar"), r),
        None => (None, keys),
    };
    let keys = rest.split(" / ").map(|g| g.split(' ').filter(|c| !c.is_empty()).map(str::to_string).collect()).collect();
    SheetRow { keys, prefix, name, style: KeyStyle::Default, locked: true, conflict: false }
}

/// The Shift+/ sheet's rows of a tab: the Onshape-controlled ones and every
/// action with a key (customized blue, disabled struck through), by name.
pub fn sheet_rows(s: &Shortcuts, tab: Tab) -> Vec<SheetRow> {
    let mut rows: Vec<SheetRow> = LOCKED.iter().filter(|(t, _, _)| *t == tab).map(|(_, k, n)| locked_row(k, n)).collect();
    for e in entries(tab) {
        let Some(key) = s.key(e) else { continue };
        rows.push(SheetRow { keys: vec![key_chips(&key)], prefix: None, name: e.name, style: s.style(e), locked: false, conflict: s.has_conflict(e) });
    }
    rows.sort_by_key(|r| r.name.to_lowercase());
    rows
}

/// Shift+/ search: rows of every tab whose keys or name match.
pub fn sheet_search(s: &Shortcuts, query: &str) -> Vec<SheetRow> {
    let q = query.trim().to_lowercase();
    Tab::SHEET
        .iter()
        .flat_map(|t| sheet_rows(s, *t))
        .filter(|r| {
            let keys: Vec<String> = r.keys.iter().map(|g| g.join(" ")).collect();
            let plus: Vec<String> = r.keys.iter().map(|g| g.join("+")).collect();
            q.is_empty() || r.name.to_lowercase().contains(&q) || keys.iter().chain(&plus).any(|k| k.to_lowercase().contains(&q))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::prelude::v1::test;

    fn e(id: &str) -> &'static Entry {
        entry(id).unwrap_or_else(|| panic!("no entry {id}"))
    }

    #[test]
    fn registry_matches_onshape_lists() {
        let count = |t| entries(t).count();
        assert_eq!((count(Tab::General), count(Tab::PartStudio), count(Tab::Assembly), count(Tab::View3d), count(Tab::Sketch), count(Tab::Drawing)), (24, 77, 16, 35, 54, 57));
        let mut ids = std::collections::HashSet::new();
        for en in ENTRIES {
            assert!(ids.insert(en.id), "{} twice", en.id);
            if let Some(k) = en.default {
                assert_eq!(canonical_str(k).as_deref(), Some(k), "{} default not canonical", en.id);
            }
        }
        // The Shift+/ sheet: 114 rows with the defaults (dom/keyboard-shortcuts.txt).
        let s = Shortcuts::default();
        let n: usize = Tab::SHEET.iter().map(|t| sheet_rows(&s, *t).len()).sum();
        assert_eq!(n, 114);
        let general: Vec<&str> = sheet_rows(&s, Tab::General).iter().map(|r| r.name).collect();
        assert_eq!(&general[..6], &["Accept & repeat command", "Accept command", "Cancel", "Clear selection", "Copy", "Curve/surface analysis"]);
    }

    #[test]
    fn a_fresh_graphics_area_is_orthographic() {
        // Onshape (live 10-23 a2): a new Part Studio, a reloaded tab and
        // another document all open orthographic; nothing stores it.
        assert_eq!(noshape_render::Camera::default().projection, noshape_render::Projection::Orthographic);
    }

    #[test]
    fn o_does_not_toggle_perspective() {
        // Onshape has no key for Perspective view (A-partstudio-ui 6.3): a
        // stray O must not flip the projection. O is the sketch's Offset.
        let defaults = crate::actions::default_key_bindings();
        for (keys, action, ctx) in summary(&managed_bindings(&defaults, &Shortcuts::default())) {
            assert!(!action.ends_with("ToggleProjection"), "{keys} ({ctx}) toggles the projection");
            if keys == "o" {
                assert!(action.ends_with("SketchOffset") && ctx.contains("Sketch"), "o runs {action} in {ctx}");
            }
        }
    }

    #[test]
    fn every_default_action_is_bound_by_default() {
        let defaults = crate::actions::default_key_bindings();
        for en in ENTRIES.iter().filter(|e| e.action.is_some()) {
            let name = en.action.unwrap_or_default();
            assert!(defaults.iter().any(|b| b.action().name().rsplit("::").next() == Some(name)), "{} has no default binding", en.id);
        }
    }

    #[test]
    fn keys_are_canonical_and_shown_as_chips() {
        let k = Keystroke::parse("shift-q").unwrap();
        assert_eq!(canonical(&k), "shift-q");
        assert_eq!(canonical(&Keystroke::parse("shift-?").unwrap()), "shift-/");
        assert_eq!(canonical(&Keystroke::parse("!").unwrap()), "shift-1");
        assert_eq!(canonical_str("Shift-Ctrl-E").as_deref(), Some("ctrl-shift-e"));
        assert_eq!(key_chips("ctrl-shift-e"), ["ctrl", "shift", "e"]);
        assert_eq!(key_chips("alt-c"), ["alt/⌥", "c"]);
        assert_eq!(key_chips("pagedown"), ["page down"]);
        assert_eq!(key_text("shift-q"), "shift+q");
        assert_eq!(binding_variants("shift-1"), ["shift-1", "!", "shift-!"]);
        assert_eq!(binding_variants("ctrl-shift-/"), ["ctrl-shift-/", "ctrl-?", "ctrl-shift-?"]);
        assert_eq!(binding_variants("l"), ["l"]);
    }

    #[test]
    fn recording_ignores_invalid_keys() {
        let r = |s: &str| record(&Keystroke::parse(s).unwrap());
        assert_eq!(r("shift-q"), Recorded::Key("shift-q".into()));
        assert_eq!(r("ctrl-shift-e"), Recorded::Key("ctrl-shift-e".into()));
        assert_eq!(r("enter"), Recorded::Confirm);
        assert_eq!(r("escape"), Recorded::Cancel);
        for k in ["backspace", "left", "up", "f1", "f12"] {
            assert_eq!(r(k), Recorded::Ignored, "{k}");
        }
        assert_eq!(r("f13"), Recorded::Key("f13".into()));
    }

    #[test]
    fn assigning_a_taken_key_disables_the_other_action() {
        let mut s = Shortcuts::default();
        let (ellipse, line, symmetric) = (e("sketch:Ellipse"), e("sketch:Line*"), e("sketch:Symmetric"));
        assert_eq!(s.key(ellipse), None);
        assert_eq!(s.conflict_for(ellipse, "shift-q"), Some(symmetric));
        assert_eq!(s.conflict_for(ellipse, "l"), Some(line));
        // Other tabs are separate: shift-s is Sketch (Part Studio) and Point (Sketch).
        assert_eq!(s.conflict_for(e("partStudio:Sketch"), "shift-s"), None);
        let off = s.assign(ellipse, Some("l".into()), false);
        assert_eq!(off, vec![line]);
        assert_eq!(s.style(ellipse), KeyStyle::Customized);
        assert_eq!(s.active_key(ellipse).as_deref(), Some("l"));
        assert_eq!(s.style(line), KeyStyle::Disabled);
        assert_eq!(s.key(line).as_deref(), Some("l"));
        assert_eq!(s.active_key(line), None);
        assert!(!s.has_conflict(ellipse));
        // Resetting Line brings back its key: both now run on l.
        s.reset(line);
        assert!(s.has_conflict(ellipse) && s.has_conflict(line));
        // Assigning the default key back removes the customization.
        s.reset(ellipse);
        s.assign(line, Some("l".into()), false);
        assert!(s.is_empty());
        // Disabling keeps the key.
        s.assign(line, None, true);
        assert_eq!((s.key(line).as_deref(), s.style(line)), (Some("l"), KeyStyle::Disabled));
        s.reset_all();
        assert!(s.is_empty());
    }

    #[test]
    fn customizations_round_trip_through_json() {
        let mut s = Shortcuts::default();
        s.assign(e("sketch:Ellipse"), Some("ctrl-shift-e".into()), false);
        s.assign(e("sketch:Bezier"), Some("l".into()), false);
        let text = s.to_json();
        assert_eq!(Shortcuts::from_json(&text), s);
        assert!(text.contains("\"sketch:Line*\"") && text.contains("\"disabled\": true"));
        // Unknown ids and bad keys are dropped; keys are canonicalized.
        let t = Shortcuts::from_json(r#"{"nope:x": {"key": "a"}, "sketch:Slot": {"key": "Shift-T"}}"#);
        assert_eq!(t.key(e("sketch:Slot")).as_deref(), Some("shift-t"));
        assert_eq!(Shortcuts::from_json("not json"), Shortcuts::default());
    }

    #[test]
    fn old_sketch_shortcuts_are_migrated() {
        let s = migrate_sketch_shortcuts(r#"{"Midpoint line": "alt-l", "slot": "Shift-T", "nonsense": "x", "Use / project edges": "ctrl-u"}"#);
        assert_eq!(s.active_key(e("sketch:Midpoint line")).as_deref(), Some("alt-l"));
        assert_eq!(s.active_key(e("sketch:Slot")).as_deref(), Some("shift-t"));
        assert_eq!(s.active_key(e("sketch:Use")).as_deref(), Some("ctrl-u"));
        assert!(migrate_sketch_shortcuts("not json").is_empty());
    }

    fn summary(bindings: &[KeyBinding]) -> Vec<(String, String, String)> {
        bindings
            .iter()
            .map(|b| {
                // The parse form ("l", "shift-8"): `to_string` is the display
                // form, which capitalizes letters ("L").
                let keys = b.keystrokes().iter().map(|k| k.unparse()).collect::<Vec<_>>().join(" ");
                let ctx = b.predicate().map(|p| format!("{p:?}")).unwrap_or_default();
                (keys, b.action().name().to_string(), ctx)
            })
            .collect()
    }

    #[test]
    fn keymap_is_unchanged_without_customizations() {
        let defaults = crate::actions::default_key_bindings();
        let managed = managed_bindings(&defaults, &Shortcuts::default());
        assert_eq!(summary(&managed), summary(&defaults));
        assert!(managed.iter().all(|b| b.meta() == Some(META)));
    }

    #[test]
    fn customized_keys_replace_the_defaults() {
        let defaults = crate::actions::default_key_bindings();
        let mut s = Shortcuts::default();
        s.assign(e("sketch:Ellipse"), Some("l".into()), false);
        s.assign(e("view3d:Front view"), Some("shift-8".into()), false);
        s.assign(e("general:Keyboard shortcuts"), None, true);
        let m = summary(&managed_bindings(&defaults, &s));
        let has = |keys: &str, action: &str| m.iter().any(|(k, a, _)| k == keys && a.ends_with(action));
        assert!(!has("l", "SketchLine"), "disabled Line keeps its binding");
        assert!(has("l", "RunShortcut"));
        assert!(!has("shift-1", "ViewFront") && !has("!", "ViewFront"));
        assert!(has("shift-8", "ViewFront") && has("*", "ViewFront"));
        assert!(!m.iter().any(|(_, a, _)| a.ends_with("ToggleShortcutHelp")));
        // The new key keeps the default's context.
        let ctx = |action: &str| m.iter().find(|(_, a, _)| a.ends_with(action)).map(|(_, _, c)| c.clone());
        assert_eq!(ctx("ViewFront"), summary(&defaults).iter().find(|(_, a, _)| a.ends_with("ViewFront")).map(|(_, _, c)| c.clone()));
        // Untouched actions keep theirs.
        assert!(has("shift-q", "ConstraintSymmetric") && has("f", "ZoomToFit"));
    }

    #[test]
    fn drawing_keys_are_routed() {
        let mut s = Shortcuts::default();
        assert_eq!(drawing_route(&s, "n"), DrawingRoute::Pass);
        s.assign(e("drawing:Note"), Some("shift-n".into()), false);
        assert_eq!(drawing_route(&s, "shift-n"), DrawingRoute::Replay("n".into()));
        assert_eq!(drawing_route(&s, "n"), DrawingRoute::Swallow);
        s.assign(e("drawing:Section view"), Some("x".into()), false);
        assert_eq!(drawing_route(&s, "x"), DrawingRoute::Run("drawing:Section view"));
        // Sketch actions marked * apply in drawings; others do not.
        s.assign(e("sketch:Ellipse"), Some("l".into()), false);
        assert_eq!(drawing_route(&s, "l"), DrawingRoute::Swallow);
        s.assign(e("sketch:Tangent arc*"), Some("ctrl-t".into()), false);
        assert_eq!(drawing_route(&s, "ctrl-t"), DrawingRoute::Run("sketch:Tangent arc*"));
    }

    #[test]
    fn sheet_shows_customized_and_disabled_keys() {
        let mut s = Shortcuts::default();
        s.assign(e("sketch:Ellipse"), Some("l".into()), false);
        let rows = sheet_rows(&s, Tab::Sketch);
        let row = |n: &str| rows.iter().find(|r| r.name == n).cloned();
        assert_eq!(row("Ellipse").map(|r| (r.keys, r.style)), Some((vec![vec!["l".to_string()]], KeyStyle::Customized)));
        assert_eq!(row("Line*").map(|r| r.style), Some(KeyStyle::Disabled));
        assert!(row("Suppress inferencing").is_some_and(|r| r.locked));
        assert_eq!(locked_row("ctrl y / shift cmd z", "Redo").keys, vec![vec!["ctrl".to_string(), "y".into()], vec!["shift".into(), "cmd".into(), "z".into()]]);
        assert!(sheet_search(&s, "shift+q").iter().any(|r| r.name == "Symmetric"));
        assert!(sheet_search(&s, "ellip").iter().any(|r| r.name == "Ellipse"));
    }
}
