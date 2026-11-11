//! Variable Studio tab: a document-level table of variables usable as
//! `#name` in every Part Studio of the document.

use crate::doc_model::{DocEvent, DocModel};
use crate::os_ui as os;
use crate::theme::p;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::button::{Button, ButtonCustomVariant, ButtonVariants as _};
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use crate::os_menu::OsContextMenuExt as _;
use gpui_kit::*;
use noshape_model::TabContent;
use noshape_model::document::TabId;
use noshape_model::variables::{VarKind, VariableStudio, VsField};
use std::collections::{HashMap, HashSet};

#[path = "variable_config.rs"]
mod variable_config;

pub struct VariableStudioView {
    tab: TabId,
    doc: Entity<DocModel>,
    inputs: HashMap<String, (Entity<InputState>, String)>,
    subs: Vec<Subscription>,
    focus: FocusHandle,
    /// Table collapsed with its title caret.
    collapsed: bool,
    /// Title being renamed (more menu, Rename).
    renaming: Option<Entity<InputState>>,
    /// Shown as a table in this Part Studio's / Assembly's Variable table
    /// panel (no toolbar, button or footer; reference menu).
    host: Option<TabId>,
    /// The footer's `?` popover is open, and the footer group's bounds.
    help_open: bool,
    footer_bounds: std::rc::Rc<std::cell::Cell<Bounds<Pixels>>>,
    /// The footer `?`: a press on it is its own toggle, not a press outside the popover.
    help_bounds: std::rc::Rc<std::cell::Cell<Bounds<Pixels>>>,
    /// Same-document studios inserted into this one: editable tables
    /// (Onshape edits the inserted studio itself, live document/50–51).
    inserted_views: HashMap<TabId, Entity<VariableStudioView>>,
    /// The last "Collapse all tables" seen ([collapse_generation]).
    collapse_seen: u64,
    /// A header edge being dragged: its column and the guide's x in the table (its own tab only).
    col_drag: Option<(usize, f32)>,
    /// Enter added a variable from the Name row: focus the next (empty)
    /// Name row once it's rebuilt, as Onshape does.
    focus_add_row: bool,
    /// Name, value and description cells by their input's key ("n3", "e3", "d3"; live input/variable-value-*,
    /// behaviours "Input behaviour" F, vs-rename-variable): a click focuses the cell (no text field, blue 2 px
    /// border); a typed key or a double-click opens the editor; Enter / a click away commit, Esc reverts.
    cell_focus: HashMap<String, FocusHandle>,
    cell_selected: Option<String>,
    /// After a Variable type pick, that row's type select keeps the keyboard in its search input (live
    /// vs-type-pick-then-type): the row, the input.
    type_search: Option<(usize, Entity<InputState>)>,
    cell_editing: Option<String>,
    /// A Value cell's `#` list (live vs-value-autocomplete), keyed by the cell.
    completion: Option<crate::var_complete::VarCompletion>,
    /// The rows last drawn, as the ui-parity state lists them (`ui_rows`).
    ui_drawn: Vec<serde_json::Value>,
    /// The right edge's Configuration panel (variable_config.rs).
    config_open: bool,
    cfg_cell: Option<variable_config::CfgEdit>,
    cfg_folded: HashSet<String>,
    cfg_filter: Option<Entity<InputState>>,
    /// The panel's type filters (bits: list, checkbox, variable).
    cfg_kinds: u8,
    /// Add configuration input › Checkbox / Configuration variable.
    cfg_dialog: Option<variable_config::InputDialog>,
    /// The left Configurations box folded away (its edge toggle, live vs-914).
    cfg_box_collapsed: bool,
}

thread_local! {
    /// "Collapse all tables" per table group (a Variable Studio tab, or the
    /// Part Studio / Assembly whose Variable table panel shows the tables).
    static COLLAPSE_ALL: std::cell::RefCell<HashMap<TabId, u64>> = Default::default();
}

/// How many times "Collapse all tables" ran in `group`'s tables.
pub fn collapse_generation(group: TabId) -> u64 {
    COLLAPSE_ALL.with(|c| c.borrow().get(&group).copied().unwrap_or(0))
}

/// The "…" menu's "Collapse all tables": every table of `group` collapses.
pub fn collapse_all_tables(group: TabId, cx: &mut App) {
    COLLAPSE_ALL.with(|c| *c.borrow_mut().entry(group).or_default() += 1);
    cx.refresh_windows();
}

impl Focusable for VariableStudioView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl VariableStudioView {
    /// The variables as last drawn (the ui-parity state's `variables`): `{"name", "type", "value", "description"}`,
    /// the inserted studios' tables first, then this one's (`"title"`: its tab's name).
    pub fn ui_state(&self, cx: &App) -> serde_json::Value {
        let title = |t: TabId| match crate::variable_insert::previewing(self.tab, t) {
            true => String::new(),
            false => self.doc.read(cx).session.doc.tab(t).map(|t| t.name.clone()).unwrap_or_default(),
        };
        // A collapsed table's rows aren't shown (live: none).
        let rows = |collapsed: bool, drawn: &[serde_json::Value]| if collapsed { serde_json::json!([]) } else { serde_json::json!(drawn) };
        let mut tables: Vec<serde_json::Value> = self.inserted_views.iter().map(|(t, v)| serde_json::json!({ "title": title(*t), "collapsed": v.read(cx).collapsed, "rows": rows(v.read(cx).collapsed, &v.read(cx).ui_drawn) })).collect();
        tables.sort_by(|a, b| a["title"].as_str().cmp(&b["title"].as_str()));
        // Being renamed, the title reads as its field (live vs-table-rename).
        let own = self.renaming.as_ref().map(|st| st.read(cx).value().to_string()).unwrap_or_else(|| title(self.tab));
        tables.push(serde_json::json!({ "title": own, "collapsed": self.collapsed, "rows": rows(self.collapsed, &self.ui_drawn) }));
        let insert_into_all = self.studio(cx).is_some_and(|s| s.insert_into_all);
        serde_json::json!({ "tables": tables, "insert_into_all": insert_into_all, "configuration_panel": self.config_open, "configuration": self.config_open.then(|| self.ui_configuration(cx)), "help": self.help_open })
    }

    /// The studio's configuration inputs: `{"name", "kind": list | checkbox | variable | text, "default", "shown",
    /// "rows"}` (a list's options; `shown` the configuration the studio shows).
    fn ui_configuration(&self, cx: &App) -> Vec<serde_json::Value> {
        use noshape_model::configuration::ConfigInputKind as K;
        let Some(cfg) = self.studio(cx).map(|s| &s.configuration) else { return Vec::new() };
        cfg.inputs
            .iter()
            .map(|i| {
                let shown = cfg.value_of(&cfg.active, &i.id);
                let (kind, default, rows, shown) = match &i.kind {
                    K::List { options, default } => {
                        let name = |id: &str| options.iter().find(|o| o.id == id).map(|o| o.name.clone());
                        let mut rows: Vec<String> = options.iter().map(|o| o.name.clone()).collect();
                        // The italic add row being typed in reads as a row (live vs-cfg-list-rows: its text is drawn there).
                        let typed = self.cfg_cell.as_ref().filter(|e| e.key == format!("add-{}", i.id)).map(|e| e.input.read(cx).value().to_string());
                        rows.extend(typed.filter(|t| !t.is_empty()));
                        ("list", serde_json::json!(name(default)), rows, serde_json::json!(shown.as_deref().and_then(name)))
                    }
                    K::Checkbox { default } => ("checkbox", serde_json::json!(default), Vec::new(), serde_json::json!(shown.map(|v| v == "true"))),
                    K::Variable { default, .. } => ("variable", serde_json::json!(default.text), Vec::new(), serde_json::json!(shown)),
                    K::Text { default } => ("text", serde_json::json!(default), Vec::new(), serde_json::json!(shown)),
                };
                serde_json::json!({ "name": i.name, "kind": kind, "default": default, "shown": shown, "rows": rows })
            })
            .collect()
    }

    pub fn new(doc: Entity<DocModel>, tab: TabId, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let subs = vec![cx.subscribe_in(&doc, window, |_: &mut Self, _, _: &DocEvent, _, cx| cx.notify())];
        Self {
            tab,
            doc,
            inputs: HashMap::new(),
            subs,
            focus: cx.focus_handle(),
            collapsed: false,
            renaming: None,
            host: None,
            help_open: false,
            footer_bounds: Default::default(),
            help_bounds: Default::default(),
            inserted_views: HashMap::new(),
            collapse_seen: collapse_generation(tab),
            col_drag: None,
            focus_add_row: false,
            cell_focus: HashMap::new(),
            cell_selected: None,
            type_search: None,
            cell_editing: None,
            completion: None,
            ui_drawn: Vec::new(),
            config_open: false,
            cfg_cell: None,
            cfg_folded: HashSet::new(),
            cfg_filter: None,
            cfg_kinds: 0,
            cfg_dialog: None,
            cfg_box_collapsed: false,
        }
    }

    /// The studio's table inside `host`'s Variable table panel.
    pub fn new_in_panel(doc: Entity<DocModel>, tab: TabId, host: TabId, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut v = Self::new(doc, tab, window, cx);
        v.host = Some(host);
        v.collapse_seen = collapse_generation(host);
        v
    }

    /// The table group this table collapses with (its host, else itself).
    fn group(&self) -> TabId {
        self.host.unwrap_or(self.tab)
    }

    /// Where this table is inserted (its host), as an insert target, with
    /// the host's references: a Variable Studio's `inserted` list, or a
    /// Part Studio's / Assembly's shown studios.
    fn host_refs(&self, cx: &App) -> Option<(crate::variable_insert::InsertTarget, Vec<noshape_model::variables::StudioRef>)> {
        let host = self.host?;
        let d = &self.doc.read(cx).session.doc;
        Some(match d.tab(host).map(|t| &t.content) {
            Some(TabContent::VariableStudio(v)) => (crate::variable_insert::InsertTarget::Studio(host), v.inserted.clone()),
            _ => (crate::variable_insert::InsertTarget::Element(host), noshape_model::variables::element_studios(d, host)),
        })
    }

    fn studio<'a>(&self, cx: &'a App) -> Option<&'a VariableStudio> {
        match &self.doc.read(cx).session.doc.tab(self.tab)?.content {
            TabContent::VariableStudio(v) => Some(v),
            _ => None,
        }
    }

    /// An edit of the studio; made in a Part Studio's or Assembly's panel, Undo takes it back there (live
    /// vs-ps-inserted-edit).
    fn edit(&mut self, desc: &str, cx: &mut Context<Self>, f: impl FnOnce(&mut VariableStudio)) {
        let origin = self.host.unwrap_or(self.tab);
        self.doc.update(cx, |doc, cx| {
            doc.edit_tab_in(self.tab, origin, desc, cx, |c| {
                if let TabContent::VariableStudio(v) = c {
                    f(v)
                }
            });
        });
    }

    /// The Name cell's rename (op variable_set's): `#old` follows in every tab that sees the studio, a taken name
    /// becomes `<name>_0`; live's toast says which.
    fn rename_variable(&mut self, i: usize, name: String, cx: &mut Context<Self>) {
        if !noshape_model::variable_table::is_identifier(name.trim().trim_start_matches('#')) {
            cx.notify();
            return;
        }
        let tab = self.tab;
        let old = self.studio(cx).and_then(|s| s.variables.get(i)).map(|v| v.name.clone()).unwrap_or_default();
        let mut done = None;
        self.doc.update(cx, |d, cx| {
            let what = format!("{} :: Rename variable", d.session.doc.tab(tab).map(|t| t.name.clone()).unwrap_or_default());
            d.edit_document(&what, cx, |doc| done = noshape_ops::variable_studio::rename_everywhere(doc, tab, i, &name));
        });
        let Some((new, taken, tabs)) = done else { return };
        let message = if taken { noshape_ops::variable_studio::RENAMED_TAKEN_MESSAGE.to_string() } else { noshape_ops::variable_studio::renamed_message(&old, &new, tabs.len()) };
        self.doc.update(cx, |_, cx| cx.emit(DocEvent::Info(message)));
    }

    /// Open a cell's editor: with the typed key (type-to-edit), or the
    /// whole text selected (double-click).
    fn start_cell_edit(&mut self, key: &str, typed: Option<String>, window: &mut Window, cx: &mut Context<Self>) {
        let Some((st, _)) = self.inputs.get(key).cloned() else { return };
        self.cell_editing = Some(key.to_string());
        self.cell_selected = Some(key.to_string());
        // (Value fields don't select on focus: the caret stays after the typed key.)
        st.update(cx, |s, cx| {
            s.focus(window, cx);
            match typed {
                Some(t) => {
                    let n = t.len();
                    s.set_value(t, window, cx);
                    s.set_selected_range(n..n, cx);
                }
                None => s.select_all(window, cx),
            }
        });
        cx.notify();
    }

    /// Esc in a cell's editor: back to the committed text, the cell
    /// focused.
    fn revert_cell_edit(&mut self, key: &str, window: &mut Window, cx: &mut Context<Self>) {
        if let Some((st, last)) = self.inputs.get(key).cloned() {
            st.update(cx, |s, cx| s.set_value(last, window, cx));
        }
        self.cell_editing = None;
        self.cell_selected = Some(key.to_string());
        let fh = self.cell_focus_handle(key, cx);
        window.focus(&fh, cx);
        cx.notify();
    }

    /// What a Value cell's `#` list offers in row `row`: the variables above it (with the inserted tables'), and the
    /// units of its type.
    fn scope_above(&self, row: usize, cx: &App) -> (std::collections::BTreeMap<String, noshape_units::Quantity>, Option<noshape_units::Dim>) {
        let Some(studio) = self.studio(cx) else { return Default::default() };
        let below: Vec<&str> = studio.variables.iter().skip(row).map(|v| v.name.as_str()).collect();
        let eval = noshape_model::variables::studio_evaluation(&self.doc.read(cx).session.doc, self.tab);
        let scope = eval.values.iter().filter(|(n, _)| !below.contains(&n.as_str())).map(|(n, q)| (n.clone(), *q)).collect();
        (scope, studio.variables.get(row).and_then(|v| v.kind).and_then(|k| k.dim()))
    }

    /// The `#` list shown (or closed), as the ui-parity state's list.
    fn set_completion(&mut self, c: Option<crate::var_complete::VarCompletion>) {
        if c.is_none() && self.completion.is_some() {
            crate::ui_script::note_list(None);
        }
        self.completion = c;
    }

    /// The `#` list's entry `index` (the selected one when `None`) put in the cell; `commit`: Enter's, which then
    /// commits the cell as its Enter does.
    fn accept_completion(&mut self, index: Option<usize>, commit: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(c) = self.completion.take() else { return };
        crate::ui_script::note_list(None);
        let Some((st, _)) = self.inputs.get(&c.key).cloned() else { return };
        c.accept(index, &st, window, cx);
        if commit {
            window.defer(cx, |window, cx| window.dispatch_action(Box::new(gpui_kit::component::input::Enter { secondary: false, shift: false }), cx));
        }
        cx.notify();
    }

    /// The focus handle of a cell (one per cell, always tracked, so a click
    /// can focus it right away).
    /// A Variable type picked on row `i`: its select's search input takes the keyboard. Typing shows the search in
    /// the cell; Enter picks the first type the text starts (else keeps the type) and selects the next row's type
    /// cell; Esc closes the search (live vs-type-pick-then-type).
    fn start_type_search(&mut self, i: usize, window: &mut Window, cx: &mut Context<Self>) {
        let st = cx.new(|cx| InputState::new(window, cx).select_on_focus(false).revert_on_escape(false));
        let sub = cx.subscribe_in(&st, window, move |this: &mut Self, st, ev: &InputEvent, window, cx| match ev {
            InputEvent::Change => cx.notify(),
            InputEvent::PressEnter { .. } => {
                let q = st.read(cx).value().trim().to_lowercase();
                let pick = (!q.is_empty()).then(|| TYPE_KINDS.iter().copied().find(|k| kind_label(*k).to_lowercase().starts_with(&q))).flatten();
                if let Some(k) = pick {
                    let units = this.doc.read(cx).session.doc.settings.units;
                    this.edit("Change variable type", cx, |s| noshape_ops::variable_studio::set_kind(s, i, k, &units));
                }
                this.type_search = None;
                let next = format!("t{}", i + 1);
                this.cell_selected = Some(next.clone());
                let fh = this.cell_focus_handle(&next, cx);
                window.focus(&fh, cx);
                cx.notify();
            }
            InputEvent::Blur if this.type_search.as_ref().is_some_and(|(_, s)| s == st) => {
                this.type_search = None;
                cx.notify();
            }
            _ => {}
        });
        self.subs.push(sub);
        st.update(cx, |s, cx| s.focus(window, cx));
        self.type_search = Some((i, st));
        cx.notify();
    }

    /// A table cell (not one being typed in) has the keyboard.
    fn cell_has_keyboard(&self, window: &Window) -> bool {
        self.cell_focus.values().any(|f| f.is_focused(window))
    }

    fn cell_focus_handle(&mut self, key: &str, cx: &mut Context<Self>) -> FocusHandle {
        self.cell_focus.entry(key.to_string()).or_insert_with(|| cx.focus_handle()).clone()
    }

    /// A name or description cell: its text, selected on a click, edited on a
    /// key or a double-click (as a value cell).
    fn text_cell(&mut self, key: String, col: usize, input: &Entity<InputState>, shown: String, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let id = SharedString::from(format!("vs-cell-{key}"));
        if self.cell_editing.as_deref() == Some(key.as_str()) {
            let k = key.clone();
            return gpui_kit::base::input::input_frame(
                cell_div(col)
                    .id(id)
                    .border_2()
                    .border_color(rgb(0x1651b0))
                    .capture_key_down(cx.listener(move |this, e: &KeyDownEvent, window, cx| {
                        if e.keystroke.key == "escape" {
                            cx.stop_propagation();
                            this.revert_cell_edit(&k, window, cx);
                        }
                    }))
                    .child(cell_input(input)),
            )
            .into_any_element();
        }
        let fh = self.cell_focus_handle(&key, cx);
        let selected = self.cell_selected.as_deref() == Some(key.as_str()) && fh.is_focused(window);
        let (k1, k2, k3) = (key.clone(), key.clone(), key);
        cell_div(col)
            .id(id)
            .px(px(7.))
            .track_focus(&fh)
            .when(selected, |d| d.border_2().border_color(rgb(0x1651b0)).px(px(6.)))
            .child(div().min_w_0().truncate().child(crate::ui::user_text(shown)))
            .on_mouse_down(MouseButton::Left, cx.listener(move |this, _: &MouseDownEvent, window, cx| {
                this.cell_selected = Some(k1.clone());
                let fh = this.cell_focus_handle(&k1, cx);
                window.focus(&fh, cx);
                cx.notify();
            }))
            .on_click(cx.listener(move |this, e: &ClickEvent, window, cx| {
                if e.click_count() >= 2 {
                    this.start_cell_edit(&k2, None, window, cx);
                }
            }))
            .on_key_down(cx.listener(move |this, e: &KeyDownEvent, window, cx| {
                let m = &e.keystroke.modifiers;
                if m.control || m.alt || m.platform {
                    return;
                }
                if let Some(ch) = e.keystroke.key_char.clone().filter(|c| !c.chars().any(char::is_control)) {
                    cx.stop_propagation();
                    this.start_cell_edit(&k3, Some(ch), window, cx);
                }
            }))
            .into_any_element()
    }

    fn cell(&mut self, key: String, value: String, window: &mut Window, cx: &mut Context<Self>, commit: impl Fn(&mut Self, String, &mut Context<Self>) + 'static) -> Entity<InputState> {
        if let Some((st, last)) = self.inputs.get(&key).cloned() {
            if last != value {
                st.update(cx, |s, cx| s.set_value(value.clone(), window, cx));
                self.inputs.insert(key, (st.clone(), value));
            }
            return st;
        }
        // The add row's cell shows Onshape's "Name" placeholder.
        let placeholder = if key == "new" { "Name" } else { "" };
        // A cell's editor opens from the cell (type-to-edit keeps the caret
        // after the key; double-click selects all itself).
        let table_cell = key != "new" && key.get(1..).is_some_and(|n| n.parse::<usize>().is_ok());
        // (Esc in a cell's editor is `revert_cell_edit`'s: the cell stays selected.)
        let st = cx.new(|cx| InputState::new(window, cx).placeholder(placeholder).default_value(value.clone()).select_on_focus(!table_cell).revert_on_escape(!table_cell));
        let add_row = key == "new";
        let k = key.clone();
        let sub = cx.subscribe_in(&st, window, move |this: &mut Self, st, ev: &InputEvent, window, cx| {
            // A Value cell's `#` list: the variables above the row, as the Part Studio's table (no "New variable…").
            if let Some(row) = k.strip_prefix('e').and_then(|n| n.parse::<usize>().ok()) {
                match ev {
                    InputEvent::Change if this.cell_editing.as_deref() == Some(k.as_str()) => {
                        let (scope, dim) = this.scope_above(row, cx);
                        let c = crate::var_complete::VarCompletion::compute(&k, st.read(cx), &scope, dim).map(|mut c| {
                            c.new_variable = None;
                            c
                        });
                        this.set_completion(c);
                        cx.notify();
                    }
                    InputEvent::Blur | InputEvent::PressEnter { .. } => this.set_completion(None),
                    _ => {}
                }
            }
            if matches!(ev, InputEvent::PressEnter { .. } | InputEvent::Blur) {
                let v = st.read(cx).value().to_string();
                // A cell's editor closes: Enter keeps the cell focused, a
                // click away leaves it.
                if this.cell_editing.as_deref() == Some(k.as_str()) {
                    this.cell_editing = None;
                    this.cell_selected = matches!(ev, InputEvent::PressEnter { .. }).then(|| k.clone());
                    if this.cell_selected.is_some() {
                        let fh = this.cell_focus_handle(&k, cx);
                        window.focus(&fh, cx);
                    }
                    cx.notify();
                }
                // Unchanged text commits nothing (no empty undo step; Esc's
                // revert then blur stays a no-op).
                if !add_row && this.inputs.get(&k).is_some_and(|(_, last)| *last == v) {
                    return;
                }
                // The add row empties once used: Enter then the blur that
                // follows added the variable twice ("cd", "cd").
                if add_row && noshape_model::variable_table::is_identifier(v.trim().trim_start_matches('#')) {
                    st.update(cx, |s, cx| s.set_value("", window, cx));
                    if matches!(ev, InputEvent::PressEnter { .. }) {
                        this.focus_add_row = true;
                    }
                }
                // The blur after an Enter sees this text as saved.
                if !add_row && let Some(e) = this.inputs.get_mut(&k) {
                    e.1 = v.clone();
                }
                commit(this, v, cx);
            }
        });
        self.subs.push(sub);
        self.inputs.insert(key, (st.clone(), value));
        st
    }
}

/// Onshape's table metrics (live DOM): 140 px columns, 34 px rows, `#aaa`
/// rules, `#eaeaea` title and header rows.
const ROW_H: f32 = 34.;
const RULE: u32 = 0xaaaaaa;

/// Whether a value is an expression (references a variable or computes),
/// rather than a plain number with an optional unit ("4 in").
/// Onshape's type names (live document/16).
fn kind_label(k: VarKind) -> &'static str {
    match k {
        VarKind::Length => "Length",
        VarKind::Angle => "Angle",
        VarKind::Number => "Number",
        VarKind::Any => "Any",
        VarKind::FromImport => "From import",
    }
}

/// A length or angle literal with more decimals than the workspace's precision, rounded to it with its unit kept
/// (live vs-value-decimals: "1.23456 mm" shows "1.235 mm", the typed text its tooltip).
fn rounded_literal(text: &str, units: &noshape_units::UnitPrefs) -> Option<String> {
    let t = text.trim();
    let n = t.find(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-' || c == '+')).unwrap_or(t.len());
    let (number, rest) = t.split_at(n);
    let decimals = number.split_once('.').map_or(0, |(_, d)| d.len());
    let precision = match rest.trim() {
        "mm" | "cm" | "m" | "in" | "ft" | "yd" | "µm" | "um" => units.length_precision,
        "deg" | "rad" => units.angle_precision,
        _ => return None,
    } as usize;
    let value: f64 = number.parse().ok()?;
    (decimals > precision).then(|| format!("{value:.precision$}{rest}"))
}

fn is_expression(text: &str) -> bool {
    let t = text.trim();
    t.contains('#') || t.trim_start_matches(['-', '+']).contains(['+', '-', '*', '/', '(', '^'])
}

/// The length unit an expression is written in: the first unit symbol in
/// it, else the unit of the first variable it references (followed a few
/// levels deep).
fn expression_unit(studio: &VariableStudio, text: &str, depth: usize) -> Option<noshape_units::LengthUnit> {
    let words: Vec<&str> = text.split(|c: char| !(c.is_alphanumeric() || c == '_' || c == '#')).filter(|w| !w.is_empty()).collect();
    for w in &words {
        if let Some(u) = noshape_units::LengthUnit::ALL.iter().find(|u| u.symbol() == *w) {
            return Some(*u);
        }
    }
    if depth > 4 {
        return None;
    }
    words.iter().filter_map(|w| w.strip_prefix('#')).find_map(|name| studio.variables.iter().find(|v| v.name == name).and_then(|v| expression_unit(studio, &v.expr.text, depth + 1)))
}

/// A borderless input whose text starts 7 px into its cell (Onshape).
fn cell_input(st: &Entity<InputState>) -> Div {
    // Live (dom/caret-inputs.json): `os-param-text`, 13.33 px.
    div().flex_1().ml(px(-4.)).child(Input::new(st).appearance(false).text_size(px(13.33)))
}

/// Column `c`'s width: a Variable Studio tab's own (its document's, while it draws: live vs-column-resize), else the
/// Part Studio Variable table's, shared by every table there (live 11-02: dragging a header edge there resizes the
/// inserted Variable Studio tables too).
fn col_w(c: usize) -> f32 {
    STUDIO_COLS.with(|w| w.get()).unwrap_or_else(crate::ui::var_col_widths)[c]
}

thread_local! {
    /// The widths of the Variable Studio tab being drawn (set around its render).
    static STUDIO_COLS: std::cell::Cell<Option<[f32; 4]>> = const { std::cell::Cell::new(None) };
}

/// A Variable Studio tab's header edge being dragged (its column).
#[derive(Clone)]
struct VsColDrag(usize);

impl Render for VsColDrag {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

/// Onshape's variable types in its Variable type select's order (behaviours.md, Variable Studio).
const TYPE_KINDS: [VarKind; 5] = [VarKind::Length, VarKind::Angle, VarKind::Number, VarKind::Any, VarKind::FromImport];

fn cell_div(c: usize) -> Div {
    div().w(px(col_w(c))).h(px(ROW_H)).flex_none().flex().items_center().bg(p().bg_senary).border_r_1().border_b_1().border_color(rgb(RULE))
}

/// A variable table's "Variable type" cell: the type 7 px in, a ▾ in the text
/// colour at the right and Onshape's select list under the cell. Every variable table
/// draws it alike, a Variable Studio's or a Part Studio's own (live psb/t-807).
pub fn type_cell(id: impl Into<ElementId>, options: Vec<String>, current: usize, on_select: impl Fn(usize, &mut Window, &mut App) + 'static) -> Div {
    let pal = p();
    let bounds: std::rc::Rc<std::cell::Cell<Bounds<Pixels>>> = Default::default();
    let probe = bounds.clone();
    let on_select = std::rc::Rc::new(on_select);
    let label = options.get(current).cloned().unwrap_or_default();
    cell_div(1)
        .relative()
        // The cell's painted bounds place the select list under it.
        .child(canvas(move |b, _, _| probe.set(b), |_, _, _, _| {}).absolute().top_0().left_0().size_full())
        // Live (psb/t-807, dark/dk-961): an 8 x 4 solid ▾, 16 px under the
        // row's top rule, its right end 6 px from the cell's right rule.
        .child(div().absolute().right(px(5.)).top(px(13.)).child(crate::os_ui::glyph(crate::os_ui::GLYPH_CARET, 8., pal.text)))
        .child(
            // A plain cell (a kit Button centres its label): text 7 px in, as live.
            div()
                .id(id)
                .size_full()
                .flex()
                .items_center()
                .cursor_pointer()
                .pl(px(7.))
                .pt(px(2.))
                .child(div().text_size(px(14.)).text_color(pal.text).child(label))
                .on_click(move |_, window, cx| {
                    let (options, on_select, b) = (options.clone(), on_select.clone(), bounds.get());
                    // Onshape's select list (live, document/16).
                    let menu = crate::os_menu::OsMenu::build(window, cx, move |mut m, _, _| {
                        m = m.select_list(col_w(1));
                        for (k, o) in options.iter().enumerate() {
                            let f = on_select.clone();
                            m = m.item(crate::os_menu::OsMenuItem::new(o.clone()).checked(k == current).on_click(move |_, window, cx| f(k, window, cx)));
                        }
                        m
                    });
                    crate::os_menu::open_anchored(point(b.origin.x, b.origin.y + b.size.height), Anchor::TopLeft, menu, window, cx);
                }),
        )
}

impl VariableStudioView {
    fn move_variable(&mut self, i: usize, down: bool, cx: &mut Context<Self>) {
        self.inputs.clear();
        self.edit(noshape_ops::variable_studio::move_description(down), cx, |s| noshape_ops::variable_studio::move_variable(s, i, down))
    }

    fn delete_variable(&mut self, i: usize, cx: &mut Context<Self>) {
        self.inputs.clear();
        self.edit("Delete variable", cx, |s| noshape_ops::variable_studio::delete_variable(s, i))
    }

    /// The more menu's Rename or a click on the title: edit the studio's name in its title row.
    fn start_rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let name = self.doc.read(cx).session.doc.tab(self.tab).map(|t| t.name.clone()).unwrap_or_default();
        let st = cx.new(|cx| InputState::new(window, cx).default_value(name));
        let sub = cx.subscribe_in(&st, window, |this: &mut Self, st, ev: &InputEvent, window, cx| {
            if matches!(ev, InputEvent::PressEnter { .. } | InputEvent::Blur) && this.renaming.is_some() {
                let name = st.read(cx).value().trim().to_string();
                let tab = this.tab;
                this.renaming = None;
                // Enter leaves the keyboard with the studio (live vs-table-rename: focus none).
                if matches!(ev, InputEvent::PressEnter { .. }) {
                    window.focus(&this.focus, cx);
                }
                if !name.is_empty() {
                    this.doc.update(cx, |d, cx| {
                        d.edit_document_not_undoable(&format!("Rename tab : {name}"), cx, |doc| {
                            if let Some(t) = doc.tab_mut(tab) {
                                t.name = name.clone();
                            }
                        })
                    });
                }
                cx.notify();
            }
        });
        self.subs.push(sub);
        st.update(cx, |s, cx| {
            s.focus(window, cx);
            s.select_all(window, cx);
        });
        self.renaming = Some(st);
        cx.notify();
    }

    /// Tables of the Variable Studios inserted into this one, above its own
    /// (live document/48). Same-document ones are editable (edits change
    /// the inserted studio, live document/50–51); another document's are
    /// read-only.
    fn render_inserted_tables(&mut self, studio: &VariableStudio, window: &mut Window, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let n = studio.inserted.len();
        let target = crate::variable_insert::InsertTarget::Studio(self.tab);
        self.inserted_views.retain(|k, _| studio.inserted.iter().any(|r| r.external.is_none() && r.studio == *k));
        let mut out = Vec::new();
        for (k, r) in studio.inserted.iter().enumerate() {
            let table = if r.external.is_some() {
                crate::variable_insert::readonly_table(&self.doc, target, r, k, n, cx)
            } else {
                let (doc, s, host) = (self.doc.clone(), r.studio, self.tab);
                let view = self.inserted_views.entry(s).or_insert_with(|| cx.new(|cx| VariableStudioView::new_in_panel(doc, s, host, window, cx)));
                Some(view.clone().into_any_element())
            };
            out.extend(table.map(|t| div().w(px((0..4).map(col_w).sum::<f32>() + 1.)).mb(px(12.)).child(t).into_any_element()));
        }
        out
    }

    /// Onshape's footer (live DOM): a 40 px `#eee` bar, 1 px `#ccc` top rule,
    /// centered "Insert into all Part Studios and Assemblies" with its
    /// checkbox (307x35, `#aaa` outline, left corners 5.25) and a `?` menu
    /// button (49x35).
    fn render_footer(&self, on: bool, configured: bool, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let variant = ButtonCustomVariant::new(cx).color(rgb(0xffffff).into()).foreground(pal.text.into()).hover(pal.panel_hover.into()).active(pal.panel_active.into()).shadow(false);
        // Live: `svg-icon-comments-checked-checkbox` (green #009400, the tick overshoots the box).
        let check = if on {
            div().size(px(20.)).child(crate::assets::icon("checked-checkbox", 20., cx)).into_any_element()
        } else {
            div().size(px(20.)).flex().items_center().justify_center().child(div().size(px(13.)).rounded(px(2.)).border_1().border_color(rgb(0x888888))).into_any_element()
        };
        let probe = self.footer_bounds.clone();
        let help_probe = self.help_bounds.clone();
        h_flex()
            .h(px(40.))
            .flex_none()
            .justify_center()
            .bg(rgb(0xeeeeee))
            .border_t_1()
            .border_color(rgb(0xcccccc))
            .child(
                Button::new("vs-insert-all")
                    .map(|d| crate::ui_script::mark(d, || "checkbox:Insert into all Part Studios and Assemblies".into()))
                    .child(canvas(move |b, _, _| probe.set(b), |_, _, _, _| {}).absolute().top_0().left_0().size_full())
                    .custom(variant)
                    .h(px(35.))
                    .px(px(10.5))
                    .rounded_l(px(5.25))
                    .rounded_r(px(0.))
                    .bg(rgb(0xffffff))
                    .border_1()
                    .border_color(rgb(RULE))
                    .child(h_flex().gap(px(4.)).child(check).child(div().text_size(px(14.)).child("Insert into all Part Studios and Assemblies")))
                    // A configured studio can't be inserted everywhere (live vs-905: greyed).
                    .when(configured, |b| gpui_kit::component::Disableable::disabled(b, true).opacity(0.5))
                    // Not an undo step (live vs-cfg-*: Undo stays off after ticking it).
                    .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                        // The checkbox takes the keyboard from any field (live vs-cfg-insert-all-disabled: focus none).
                        window.focus(&this.focus, cx);
                        if configured {
                            return;
                        }
                        let tab = this.tab;
                        this.doc.update(cx, |d, cx| {
                            d.edit_tab_not_undoable(tab, noshape_ops::variable_studio::insert_into_all_description(!on), cx, |c| {
                                if let TabContent::VariableStudio(v) = c {
                                    v.insert_into_all = !on;
                                }
                            })
                        });
                        cx.notify();
                    })),
            )
            .child(
                Button::new("vs-insert-all-help")
                    .map(|d| crate::ui_script::mark(d, || "button:Insert into all help".into()))
                    .child(canvas(move |b, _, _| help_probe.set(b), |_, _, _, _| {}).absolute().top_0().left_0().size_full())
                    .custom(variant)
                    .h(px(35.))
                    .w(px(49.))
                    .rounded_r(px(5.25))
                    .rounded_l(px(0.))
                    .border_1()
                    .when(!self.help_open, |b| b.border_l_0())
                    .bg(rgb(0xffffff))
                    .border_color(if self.help_open { pal.accent } else { rgb(RULE) })
                    // ▴: the popover opens upward (live document/12, 42).
                    .child(h_flex().gap(px(4.)).child(crate::assets::icon("help", 20., cx)).child(os::glyph(os::GLYPH_CARET_UP, 7., pal.text)))
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.help_open = !this.help_open;
                        cx.notify();
                    })),
            )
            .children(self.help_open.then(|| self.render_help(on, cx)))
            .into_any_element()
    }

    /// The footer `?` popover (live document/42, 45; dom/variable-studio-help-
    /// dropdown): a 355 px Bootstrap dropdown holding an info alert (14 px pad,
    /// `#def1fb` / `#b2ddf6`, `#094174`): bold title, text, "Learn more about
    /// Variable Studios" (opens noshape's README; Onshape's help is cloud).
    fn render_help(&self, on: bool, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let (title, body) = if on {
            ("This Variable Studio is automatically referenced in this workspace", "New Part Studios and Assemblies in this workspace will automatically reference this Variable Studio.")
        } else {
            (
                "Automatically insert this Variable Studio into all Part Studios and Assemblies in this workspace",
                "Automatic references enable you to insert this Variable Studio into multiple tabs. References to this Variable Studio will be placed in every new and existing Part Studio and Assembly in this workspace.",
            )
        };
        let b = self.footer_bounds.get();
        deferred(
            anchored().anchor(Anchor::BottomLeft).position(point(b.left(), b.top() - px(1.))).snap_to_window_with_margin(px(4.)).child(
                div()
                    .occlude()
                    .w(px(355.))
                    .p(px(1.))
                    .bg(rgb(0xffffff))
                    .border_1()
                    .border_color(rgb(0xd7d7d7))
                    .rounded(px(5.25))
                    .on_mouse_down_out(cx.listener(|this, ev: &MouseDownEvent, _, cx| {
                        if this.help_bounds.get().contains(&ev.position) {
                            return;
                        }
                        this.help_open = false;
                        cx.notify();
                    }))
                    .child(
                        v_flex()
                            .p(px(14.))
                            .gap(px(21.))
                            .bg(pal.info_fill)
                            .border_1()
                            .border_color(pal.info_outline)
                            .rounded(px(5.25))
                            .text_size(px(14.))
                            .text_color(pal.info_text)
                            .child(div().font_weight(FontWeight::BOLD).child(title))
                            .child(div().child(body))
                            .map(|d| crate::ui_script::mark_message(d, || format!("{title} {body} Learn more about Variable Studios")))
                            .child(
                                div()
                                    .id("vs-help-learn")
                                    .map(|d| crate::ui_script::mark(d, || "button:Learn more about Variable Studios".into()))
                                    .cursor_pointer()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .underline()
                                    .child("Learn more about Variable Studios")
                                    .on_click(|_, _, cx| noshape_app_core::help::open("Variable Studios", cx)),
                            ),
                    ),
            ),
        )
        .with_priority(100)
        .into_any_element()
    }

    /// Tab-separated copy of the table (Onshape's "Copy table").
    fn copy_table(&self, cx: &mut Context<Self>) {
        let Some(studio) = self.studio(cx) else { return };
        cx.write_to_clipboard(ClipboardItem::new_string(noshape_ops::variable_studio::studio_table_text(studio)));
        let name = self.doc.read(cx).session.doc.tab(self.tab).map(|t| t.name.clone()).unwrap_or_default();
        self.doc.update(cx, |_, cx| cx.emit(DocEvent::Info(format!("{name} table copied to clipboard."))));
    }
}

impl Render for VariableStudioView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Its own tab draws at the widths its document keeps; in a panel the panel's.
        let widths = if self.host.is_none() { self.studio(cx).and_then(|s| <[f32; 4]>::try_from(s.column_widths.clone()).ok()) } else { None };
        STUDIO_COLS.with(|w| w.set(widths.or_else(|| self.host.is_none().then(crate::ui::var_col_widths))));
        let el = self.render_view(window, cx);
        STUDIO_COLS.with(|w| w.set(None));
        el
    }
}

impl VariableStudioView {
    fn render_view(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        // "Collapse all tables" ran in this table's group.
        let generation = collapse_generation(self.group());
        if generation != self.collapse_seen {
            self.collapse_seen = generation;
            self.collapsed = true;
        }
        // Shown in its active configuration (the Configuration panel's rows).
        let studio = self.studio(cx).cloned().unwrap_or_default();
        let studio = studio.configured(&studio.configuration.active).into_owned();
        let (units, title) = {
            let d = &self.doc.read(cx).session.doc;
            // A table only previewed by the Insert dialog has no title yet (live).
            let previewed = self.host.is_some_and(|h| crate::variable_insert::previewing(h, self.tab));
            (d.settings.units, if previewed { String::new() } else { d.tab(self.tab).map(|t| t.name.clone()).unwrap_or_default() })
        };
        // Results with the tables inserted into this studio in scope.
        let eval = noshape_model::variables::studio_evaluation(&self.doc.read(cx).session.doc, self.tab);
        let view = cx.entity();
        let n = studio.variables.len();
        let plain = ButtonCustomVariant::new(cx).color(gpui_kit::transparent_white()).foreground(pal.text.into()).hover(gpui_kit::transparent_white()).active(gpui_kit::transparent_white()).shadow(false);
        let mut rows: Vec<AnyElement> = Vec::new();
        // In a panel, a reference to single variables shows only those.
        let host_refs = self.host_refs(cx);
        let only: Option<Vec<String>> = host_refs.as_ref().and_then(|(_, refs)| refs.iter().find(|r| r.studio == self.tab).and_then(|r| r.variables.clone()));
        // In a Part Studio's or Assembly's Variable table, a variable a later
        // table (or the element's own Variable feature) redefines gets an ⓘ
        // in its Name cell (live psb t-806).
        let auto_ref = match host_refs.as_ref().map(|(t, _)| t) {
            Some(crate::variable_insert::InsertTarget::Element(host)) => noshape_model::variables::is_auto_referenced(&self.doc.read(cx).session.doc, *host, self.tab),
            _ => false,
        };
        let overwritten = match host_refs.as_ref().map(|(t, _)| t) {
            Some(crate::variable_insert::InsertTarget::Element(host)) => noshape_model::variables::overwritten(&self.doc.read(cx).session.doc, *host, self.tab),
            _ => Default::default(),
        };
        let mut drawn: Vec<serde_json::Value> = Vec::new();
        for (i, v) in studio.variables.iter().enumerate() {
            if only.as_ref().is_some_and(|o| !o.contains(&v.name)) {
                continue;
            }
            let name = self.cell(format!("n{i}"), v.name.clone(), window, cx, move |this, val, cx| this.rename_variable(i, val, cx));
            // Live (qa4 10-20): a bare number gets the workspace unit of a Length
            // / Angle variable ("5" → "5 mm"), as in the Part Studio table.
            let cell_units = units;
            // Shown in the user's decimal separator (stored canonically).
            let expr = self.cell(format!("e{i}"), noshape_units::localize(&v.expr.text), window, cx, move |this, val, cx| {
                // A configured value: the shown configuration's row.
                this.edit("Edit variable", cx, |s| noshape_ops::variable_studio::set_value(s, i, &val, &cell_units))
            });
            // Convert to expression (live 652–656, as the Part Studio table): the
            // literal shows without its quotes; a failing expression is empty
            // with "Error regenerating." as the tooltip.
            let literal = noshape_model::variable_table::string_literal(&v.description).filter(|_| v.description_expr);
            let desc_error = v.description_expr && literal.is_none() && !v.description.trim().is_empty() && noshape_units::evaluate(&v.description, &eval).is_err();
            let desc_shown = if desc_error { String::new() } else { literal.map(str::to_string).unwrap_or_else(|| v.description.clone()) };
            let desc_tip = if desc_error { Some("Error regenerating.".to_string()) } else { v.description_expr.then(|| v.description.clone()).filter(|d| !d.is_empty()) };
            let (desc_text, desc_is_expr) = (v.description.clone(), v.description_expr);
            let desc = self.cell(format!("d{i}"), desc_shown.clone(), window, cx, move |this, val, cx| {
                this.edit("Edit variable description", cx, |s| noshape_ops::variable_studio::set_description(s, i, &val))
            });
            // Onshape (behaviours.md, Variable Studio): a plain value shows as
            // typed; an expression (`#width * 2`) shows its result ("4 in")
            // until the cell is clicked; a failing one is red with the error
            // as its tooltip.
            let (error, result) = match eval.results.get(i) {
                Some(Err(e)) => (Some(e.to_string()), None),
                Some(Ok(q)) => (None, Some(*q)),
                None => (None, None),
            };
            let editing = expr.read(cx).focus_handle(cx).is_focused(window);
            let shown = result.filter(|_| !editing && is_expression(&v.expr.text)).map(|q| {
                let unit = expression_unit(&studio, &v.expr.text, 0).unwrap_or(units.length);
                noshape_units::format_quantity(q, &noshape_units::UnitPrefs { length: unit, ..units })
            });
            let shown = shown.or_else(|| (!editing).then(|| rounded_literal(&v.expr.text, &units)).flatten());
            // From import: the imported value, "Not set" before a file is picked (live vs-type-from-import).
            let import_text = (v.kind == Some(VarKind::FromImport)).then(|| v.import.as_ref().map(|imp| noshape_model::variables::import_value(&self.doc.read(cx).session.doc, imp).map(|x| x.to_string()).unwrap_or_else(|e| e)).unwrap_or_else(|| "Not set".into()));
            if crate::ui_script::active() {
                // A cell being typed in shows what's typed (live vs-edit-value: "50" before Enter).
                let value = if let Some(t) = &import_text {
                    t.clone()
                } else if editing { expr.read(cx).value().to_string() } else { shown.clone().unwrap_or_else(|| noshape_units::localize(&v.expr.text)) };
                let typed = |st: &Entity<InputState>, saved: &str| if st.read(cx).focus_handle(cx).is_focused(window) { st.read(cx).value().to_string() } else { saved.to_string() };
                drawn.push(serde_json::json!({ "name": typed(&name, &v.name), "type": kind_label(v.kind.unwrap_or(VarKind::Any)), "value": value, "description": typed(&desc, &desc_text) }));
            }
            let from_import = v.kind == Some(VarKind::FromImport);
            let cfg_value = studio.configuration.configured_cell(&v.name, VsField::Value).is_some();
            let cfg_desc = studio.configuration.configured_cell(&v.name, VsField::Description).is_some();
            let value_cell = match shown {
                // From import: the imported value and the Edit pencil (help:
                // "In the Value cell, click the Edit pencil icon").
                _ if from_import => {
                    let text = import_text.clone().unwrap_or_default();
                    let (doc, tab) = (self.doc.clone(), self.tab);
                    cell_div(2)
                        .id(("vs-val", i))
                        .px(px(7.))
                        .gap(px(4.))
                        .child(div().flex_1().min_w_0().truncate().child(text))
                        .child(div().id(("vs-edit", i)).flex_none().cursor_pointer().tooltip(crate::ui::tooltip("Edit")).child(crate::ui::glyph("edit", 14., pal.text_muted)).on_click(move |_, window, cx| crate::variable_import::open(doc.clone(), tab, i, window, cx)))
                        .into_any_element()
                }
                // Editing: the field inside the focused cell's blue border.
                _ if self.cell_editing.as_deref() == Some(format!("e{i}").as_str()) => {
                    let key = format!("e{i}");
                    let open = self.completion.as_ref().is_some_and(|c| c.key == key);
                    let list = self.completion.as_ref().filter(|_| open).map(|c| {
                        let (scope, _) = self.scope_above(i, cx);
                        let units = self.doc.read(cx).session.doc.settings.units;
                        crate::ui_script::note_list(Some(c.ui_state(Some((&scope, &units)))));
                        let view = cx.entity();
                        c.render(&scope, &units, move |k, window, cx| view.update(cx, |this, cx| this.accept_completion(Some(k), false, window, cx)))
                    });
                    use gpui_kit::component::input::{Enter, MoveDown, MoveUp};
                    gpui_kit::base::input::input_frame(
                        cell_div(2)
                            .id(("vs-val", i))
                            .relative()
                            .border_2()
                            .border_color(rgb(0x1651b0))
                            // Esc closes an open `#` list first (live: the cell stays in edit with its text).
                            .capture_key_down(cx.listener(move |this, e: &KeyDownEvent, window, cx| {
                                if e.keystroke.key == "escape" {
                                    cx.stop_propagation();
                                    if this.completion.is_some() {
                                        this.set_completion(None);
                                        cx.notify();
                                    } else {
                                        this.revert_cell_edit(&format!("e{i}"), window, cx);
                                    }
                                }
                            }))
                            // The list open: ↑ / ↓ move, Enter inserts and commits, Tab inserts.
                            .when(open, |d| {
                                d.capture_action(cx.listener(|this, _: &MoveUp, _, cx| {
                                    cx.stop_propagation();
                                    if let Some(c) = &mut this.completion {
                                        c.move_selection(-1);
                                    }
                                    cx.notify();
                                }))
                                .capture_action(cx.listener(|this, _: &MoveDown, _, cx| {
                                    cx.stop_propagation();
                                    if let Some(c) = &mut this.completion {
                                        c.move_selection(1);
                                    }
                                    cx.notify();
                                }))
                                .capture_action(cx.listener(|this, _: &Enter, window, cx| {
                                    // An entry that changes nothing ("1.23456 mm" with mm highlighted): Enter commits
                                    // (live vs-value-decimals); so does the Enter an insertion sends after itself.
                                    let Some(c) = this.completion.as_ref() else { return };
                                    let text = this.inputs.get(&c.key).map(|(st, _)| st.read(cx).value().to_string()).unwrap_or_default();
                                    if !c.enter_inserts(&text) {
                                        this.set_completion(None);
                                        return;
                                    }
                                    cx.stop_propagation();
                                    this.accept_completion(None, true, window, cx);
                                }))
                                .capture_action(cx.listener(|this, _: &crate::actions::DialogTab, window, cx| {
                                    cx.stop_propagation();
                                    this.accept_completion(None, false, window, cx);
                                }))
                            })
                            .child(cell_input(&expr))
                            .children(list),
                    )
                    .into_any_element()
                }
                // The cell: its value (an expression's result), the stored text
                // as the tooltip, red with the error when it fails.
                shown => {
                    let fh = self.cell_focus_handle(&format!("e{i}"), cx);
                    // The blue border while the cell has the focus (clicking elsewhere drops it).
                    let selected = self.cell_selected.as_deref() == Some(format!("e{i}").as_str()) && fh.is_focused(window);
                    let text = shown.unwrap_or_else(|| noshape_units::localize(&v.expr.text));
                    let tip = error.clone().unwrap_or_else(|| noshape_units::localize(&v.expr.text));
                    cell_div(2)
                        .id(("vs-val", i))
                        .px(px(7.))
                        .track_focus(&fh)
                        .when(selected, |d| d.border_2().border_color(rgb(0x1651b0)).px(px(6.)))
                        .when(error.is_some(), |d| d.text_color(pal.error))
                        .tooltip(crate::ui::tooltip(tip))
                        .child(div().min_w_0().truncate().child(text))
                        .on_mouse_down(MouseButton::Left, cx.listener(move |this, _: &MouseDownEvent, window, cx| {
                            this.cell_selected = Some(format!("e{i}"));
                            let fh = this.cell_focus_handle(&format!("e{i}"), cx);
                            window.focus(&fh, cx);
                            cx.notify();
                        }))
                        .on_click(cx.listener(move |this, e: &ClickEvent, window, cx| {
                            if e.click_count() >= 2 {
                                this.start_cell_edit(&format!("e{i}"), None, window, cx);
                            }
                        }))
                        // Type-to-edit: the first printable key opens the editor with it.
                        .on_key_down(cx.listener(move |this, e: &KeyDownEvent, window, cx| {
                            let m = &e.keystroke.modifiers;
                            if m.control || m.alt || m.platform {
                                return;
                            }
                            if let Some(ch) = e.keystroke.key_char.clone().filter(|c| !c.chars().any(char::is_control)) {
                                cx.stop_propagation();
                                this.start_cell_edit(&format!("e{i}"), Some(ch), window, cx);
                            }
                        }))
                        .into_any_element()
                }
            };
            let kind = v.kind.unwrap_or(VarKind::Any);
            let name_cell = self.text_cell(format!("n{i}"), 0, &name, v.name.clone(), window, cx);
            let desc_cell = self.text_cell(format!("d{i}"), 3, &desc, desc_shown, window, cx);
            let kinds = view.clone();
            let menu_view = view.clone();
            rows.push(
                h_flex()
                    .id(("vs-row", i))
                    .border_l_1()
                    .border_color(rgb(RULE))
                    .bg(rgb(0xffffff))
                    .text_size(px(14.))
                    .text_color(pal.text)
                    // Name, Variable type and Value: the row menu. (Menus open in
                    // the capture phase, so the Description cell's own menu
                    // must not sit inside this one.)
                    .child(
                        h_flex()
                            .flex_none()
                            .os_context_menu_over_inputs({
                                let view = menu_view.clone();
                                move |m, _, _| row_menu(m, view.clone(), i, n)
                            })
                            .child(gpui_kit::base::input::input_frame(
                                div()
                                    .relative()
                                    .map(|d| crate::ui_script::mark(d, || format!("var_cell:{}:Name", v.name)))
                                    .child(name_cell)
                                    .when(overwritten.contains(&v.name), |d| d.child(div().id(("vs-over", i)).absolute().right(px(8.)).top(px(12.)).tooltip(crate::ui::tooltip("Overwritten in another variable table.")).child(crate::ui::glyph("info", 10., rgb(0x666666))))),
                            ))
                            .child({
                                let current = TYPE_KINDS.iter().position(|k| *k == kind).unwrap_or(0);
                                let var_name = v.name.clone();
                                let cell = type_cell(("vs-kind", i), TYPE_KINDS.iter().map(|k| kind_label(*k).to_string()).collect(), current, move |k, window, cx| {
                                    let k = TYPE_KINDS[k];
                                    kinds.update(cx, |this, cx| {
                                        let units = this.doc.read(cx).session.doc.settings.units;
                                        this.edit("Change variable type", cx, |s| noshape_ops::variable_studio::set_kind(s, i, k, &units));
                                        // The select keeps the keyboard; From import's file is picked with the Value cell's pencil
                                        // (live vs-type-from-import: no dialog, "Not set").
                                        this.start_type_search(i, window, cx);
                                    })
                                });
                                let tkey = format!("t{i}");
                                let tfh = self.cell_focus_handle(&tkey, cx);
                                let selected = self.cell_selected.as_deref() == Some(tkey.as_str()) && tfh.is_focused(window);
                                // The search typed after a pick, over the cell (empty: unseen, still holding the keyboard).
                                let search = self.type_search.as_ref().filter(|(r, _)| *r == i).map(|(_, st)| {
                                    let shown = !st.read(cx).value().is_empty();
                                    div().absolute().inset_0().bg(pal.panel).when(!shown, |d| d.opacity(0.)).border_b_2().border_color(rgb(0x1651b0)).flex().items_center().child(cell_input(st))
                                });
                                let cell = div()
                                    .id(("vs-kind-cell", i))
                                    .relative()
                                    .track_focus(&tfh)
                                    .child(cell)
                                    .children(search)
                                    .when(selected, |d| d.child(div().absolute().inset_0().border_2().border_color(rgb(0x1651b0))));
                                crate::ui_script::mark(cell, || format!("var_cell:{var_name}:Variable type"))
                            }),
                    )
                    // Value: the row menu plus Configure… (help config-rmb-configure-01),
                    // a configured value dotted orange.
                    .child({
                        let (view, cfg, var, current, kind_now) = (menu_view.clone(), studio.configuration.clone(), v.name.clone(), v.expr.text.clone(), v.kind);
                        div()
                            .id(("vs-valcell", i))
                            .relative()
                            .map(|d| crate::ui_script::mark(d, || format!("var_cell:{var}:Value")))
                            .child(value_cell)
                            .when(cfg_value, |d| d.child(variable_config::configured_outline()))
                            .os_context_menu_over_inputs(move |m, _, _| {
                                let m = row_menu(m, view.clone(), i, n);
                                variable_config::cell_menu(m, view.clone(), &cfg, (var.clone(), VsField::Value, current.clone(), kind_now))
                            })
                    })
                    // The Description cell adds Copy | Convert to expression (or
                    // text) to the row menu (live variable-studio/01–07).
                    .child({
                        let desc_view = menu_view.clone();
                        let (cfg, var) = (studio.configuration.clone(), v.name.clone());
                        div()
                            .id(("vs-desc", i))
                            .relative()
                            .map(|d| crate::ui_script::mark(d, || format!("var_cell:{var}:Description")))
                            .child(desc_cell)
                            .when(cfg_desc, |d| d.child(variable_config::configured_outline()))
                            .when_some(desc_tip, |d, t| d.tooltip(crate::ui::tooltip(t)))
                            .os_context_menu_over_inputs(move |m, _, _| {
                                let (text, v) = (desc_text.clone(), desc_view.clone());
                                let m = row_menu(m, desc_view.clone(), i, n);
                                variable_config::cell_menu(m, desc_view.clone(), &cfg, (var.clone(), VsField::Description, text.clone(), None))
                                    // Live vs-desc-menu-convert: Copy only on a text description.
                                    .when(!desc_is_expr, |m| m.item(crate::os_menu::OsMenuItem::new("Copy").on_click(move |_, _, cx| cx.write_to_clipboard(ClipboardItem::new_string(text.clone())))))
                                    .item(crate::os_menu::OsMenuItem::new(if desc_is_expr { "Convert to text" } else { "Convert to expression" }).on_click(move |_, _, cx| {
                                        v.update(cx, |t, cx| {
                                            t.edit("Edit variable description", cx, |s| {
                                                if let Some(v) = s.variables.get_mut(i) {
                                                    v.toggle_description_expr();
                                                }
                                            })
                                        })
                                    }))
                            })
                    })
                    .into_any_element(),
            );
        }
        // The last row: an italic "Name" cell; typing a name adds a Length
        // variable set to 0 in the document's length unit.
        let add_units = units;
        let new_name = self.cell("new".into(), String::new(), window, cx, move |this, val, cx| {
            let name = val.trim().trim_start_matches('#').to_string();
            // Live: an invalid name isn't added and stays in the cell (vs-add-variable-invalid).
            if !noshape_model::variable_table::is_identifier(&name) {
                return;
            }
            this.inputs.remove("new");
            let Some(studio) = this.studio(cx) else { return };
            let free = noshape_ops::variable_studio::free_name(studio, &name);
            this.edit("Add variable", cx, |s| noshape_ops::variable_studio::add_variable(s, &free, &add_units));
            if free != name {
                this.doc.update(cx, |_, cx| cx.emit(DocEvent::Info(noshape_ops::variable_studio::RENAMED_MESSAGE.into())));
            }
        });
        if std::mem::take(&mut self.focus_add_row) {
            // After this frame, once the new row's input is laid out.
            let st = new_name.clone();
            window.defer(cx, move |window, cx| st.update(cx, |s, cx| s.focus(window, cx)));
        }
        self.ui_drawn = drawn;
        let collapsed = self.collapsed;
        let more_view = view.clone();
        let host_menu = host_refs.map(|(target, list)| {
            let k = list.iter().position(|r| r.studio == self.tab).unwrap_or(0);
            (self.doc.clone(), target, self.tab, title.clone(), k, list.len())
        });
        let group = self.group();
        let (collapse_mark, menu_mark) = (format!("button:Collapse:{title}"), format!("button:Table menu:{title}"));
        // In a panel the title row spans the panel (live, part-studio/77).
        let in_panel = self.host.is_some();
        let own_tab = !in_panel;
        let table = v_flex()
            .relative()
            .when(!in_panel, |d| d.w(px((0..4).map(col_w).sum::<f32>() + 1.)))
            .when(in_panel, |d| d.w_full())
            .child(
                h_flex()
                    .h(px(35.))
                    .px(px(6.))
                    .bg(pal.table_header)
                    .border_1()
                    .border_color(rgb(RULE))
                    .child(
                        h_flex()
                            .id("vs-collapse")
                            .gap(px(3.))
                            .cursor_pointer()
                            // The fold glyph (the title beside it is its name's field).
                            .child(div().map(|d| crate::ui_script::mark(d, || collapse_mark)).child(crate::os_ui::glyph(if collapsed { crate::os_ui::GLYPH_COLLAPSED } else { crate::os_ui::GLYPH_EXPANDED }, 10., pal.text)))
                            // An inserted studio's title starts with the studio icon (live
                            // part-studio/160); one there only through "Insert into all…" with
                            // the automatic-reference icon (live dom assembly-panel-variable-table).
                            .when(in_panel && !auto_ref, |r| r.child(crate::assets::icon("variable-studio", 20., cx)))
                            .when(auto_ref, |r| r.child(div().id("vs-auto-ref").map(|d| crate::ui_script::mark(d, || "button:Automatically referenced Variable Studio".into())).pl(px(4.)).tooltip(crate::ui::tooltip("Automatically referenced Variable Studio")).child(crate::assets::icon("auto-variable-reference", 20., cx))))
                            .child(match &self.renaming {
                                Some(st) => div()
                                    .w(px(260.))
                                    .text_size(px(16.))
                                    .border_b_1()
                                    .border_color(pal.accent)
                                    // Esc keeps the name: the field goes before its blur would rename.
                                    .on_key_down(cx.listener(|this, ev: &KeyDownEvent, window, cx| {
                                        if ev.keystroke.key == "escape" {
                                            cx.stop_propagation();
                                            this.renaming = None;
                                            window.focus(&this.focus, cx);
                                            cx.notify();
                                        }
                                    }))
                                    .child(Input::new(st).h(px(26.)).appearance(false).text_size(px(16.)).font_weight(FontWeight::MEDIUM).text_color(pal.text).pl(px(3.))).into_any_element(),
                                // The title is its name's field: a click edits it (live vs-table-title-rename, vs-ps-inserted-drag).
                                None => div()
                                    .id("vs-title")
                                    .map(|d| crate::ui_script::mark(d, || format!("table_title:{title}")))
                                    .text_size(px(16.))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(pal.text)
                                    .cursor_text()
                                    .child(crate::ui::user_text(title))
                                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                        cx.stop_propagation();
                                        this.start_rename(window, cx);
                                    }))
                                    .into_any_element(),
                            })
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.collapsed = !this.collapsed;
                                cx.notify();
                            })),
                    )
                    .child(div().flex_1())
                    .child(
                        Button::new("vs-more")
                            .map(|d| crate::ui_script::mark(d, || menu_mark))
                            .when(in_panel, |d| crate::ui_script::mark_first(d, || "panel_more:Variable table".into()))
                            .custom(plain)
                            .w(px(25.))
                            .h(px(22.))
                            .p_0()
                            // Live: `#333` dots 7 px apart (the svg ignores the link colour).
                            .child(div().mt(px(3.)).child(crate::os_ui::glyph(crate::os_ui::GLYPH_OVERFLOW, 20., pal.text)))
                            .on_click(move |e, window, cx| {
                                let (more_view, host_menu) = (more_view.clone(), host_menu.clone());
                                let has_inserted = !more_view.read(cx).inserted_views.is_empty();
                                // Live document/15 (own: Copy table, Rename, Collapse all tables) and
                                // part-studio/161, 165 (inserted: + Switch to, Move up/down, Edit
                                // reference…, Remove reference). "Create task…" is cloud.
                                let menu = crate::os_menu::OsMenu::build(window, cx, move |m, _, _| {
                                let (v, v2) = (more_view.clone(), more_view.clone());
                                let item = crate::os_menu::OsMenuItem::new;
                                let m = m
                                    .dropdown()
                                    .item(item("Copy table").on_click(move |_, _, cx| v.update(cx, |t, cx| t.copy_table(cx))))
                                    .item(item("Rename").on_click(move |_, window, cx| v2.update(cx, |t, cx| t.start_rename(window, cx))));
                                let collapse = item("Collapse all tables").on_click(move |_, _, cx| collapse_all_tables(group, cx));
                                match host_menu.clone() {
                                    // An inserted table: the reference's items.
                                    Some((doc, target, me, name, k, n)) => {
                                        let (d1, d2, d3, d4) = (doc.clone(), doc.clone(), doc.clone(), doc.clone());
                                        let m = m.item(crate::os_menu::OsMenuItem::new(format!("Switch to {name}")).on_click(move |_, window, cx| crate::actions::request_activate_tab(me, window, cx)));
                                        let m = if k > 0 { m.item(item("Move up").on_click(move |_, _, cx| crate::variable_insert::move_ref(&d1, target, me, false, cx))) } else { m };
                                        let m = if k + 1 < n { m.item(item("Move down").on_click(move |_, _, cx| crate::variable_insert::move_ref(&d2, target, me, true, cx))) } else { m };
                                        // One there only through "Insert into all…" has no reference to edit or remove
                                        // (live vs-ps-own-table-menu).
                                        m.item(collapse).when(!auto_ref, |m| {
                                            m.item(item("Edit reference…").on_click(move |_, window, cx| crate::variable_insert::open_update(d4.clone(), target, me, window, cx)))
                                                .item(item("Remove reference").on_click(move |_, _, cx| crate::variable_insert::set_ref(&d3, target, me, None, cx)))
                                        })
                                    }
                                    // Own table: only with inserted tables to collapse
                                    // (live document/15 vs 49).
                                    None if has_inserted => m.item(collapse),
                                    None => m,
                                }
                            });
                                // A page's dropdown: Escape leaves it open (live vs-table-menu, as the configuration panel's ⋯).
                                crate::os_menu::mark_page_dropdown(&menu, cx);
                                crate::os_menu::open_anchored(point(e.position().x + px(12.), e.position().y + px(12.)), Anchor::TopRight, menu, window, cx);
                            }),
                    ),
            )
            .when(!collapsed, |t| {
                t.child(
                    h_flex()
                        .border_l_1()
                        .border_color(rgb(RULE))
                        .bg(pal.table_header)
                        .text_size(px(14.))
                        .font_weight(FontWeight::BOLD)
                        .text_color(pal.text)
                        .children(["Name", "Variable type", "Value", "Description"].into_iter().enumerate().map(|(c, h)| {
                            let dragging = self.col_drag.is_some_and(|d| d.0 == c);
                            cell_div(c)
                                .map(|d| crate::ui_script::mark(d, move || format!("var_header:{h}")))
                                .justify_center()
                                .relative()
                                .child(h)
                                // Its own tab: a 5 px col-resize handle inside the right edge (as the Part Studio's table).
                                .when(own_tab, |d| {
                                    d.child(
                                        div()
                                            .id(("vs-col", c))
                                            .absolute()
                                            .top_0()
                                            .bottom_0()
                                            .right_0()
                                            .w(px(5.))
                                            .cursor(CursorStyle::ResizeColumn)
                                            .when(dragging, |d| d.bg(rgb(0xaaaaaa)))
                                            .hover(|d| d.bg(rgb(0xaaaaaa)))
                                            .on_drag(VsColDrag(c), |d, _, _, cx| cx.new(|_| d.clone())),
                                    )
                                })
                        }))
                        // The guide follows the pointer (at least 40 px); the width applies on release as an edit.
                        .on_drag_move(cx.listener(|this, e: &DragMoveEvent<VsColDrag>, _, cx| {
                            let c = e.drag(cx).0;
                            let left: f32 = 1. + (0..c).map(col_w).sum::<f32>();
                            let x = f32::from(e.event.position.x - e.bounds.left()).max(left + crate::ui::VAR_COL_MIN);
                            if this.col_drag != Some((c, x)) {
                                this.col_drag = Some((c, x));
                                cx.notify();
                            }
                        }))
                        .on_drop(cx.listener(|this, d: &VsColDrag, _, cx| {
                            if let Some((c, x)) = this.col_drag.take().filter(|x| x.0 == d.0) {
                                let mut ws: [f32; 4] = std::array::from_fn(col_w);
                                ws[c] = (x - 1. - ws[..c].iter().sum::<f32>()).round().max(crate::ui::VAR_COL_MIN);
                                this.edit(noshape_ops::variable_studio::RESIZE_COLUMN, cx, |s| s.column_widths = ws.to_vec());
                            }
                            cx.notify();
                        })),
                )
                .children(self.col_drag.map(|(_, x)| div().absolute().top(px(35.)).bottom_0().left(px(x)).w(px(1.)).bg(rgb(0x000000))))
                .children(rows)
                .when(only.is_none(), |t| {
                    // The add row's other columns are one borderless cell band
                    // (live dark/dk-961: `#1c1c1c` across them).
                    t.child(h_flex().child(gpui_kit::base::input::input_frame(cell_div(0).border_l_1().map(|d| crate::ui_script::mark(d, || "var_cell:Name".into())).child(cell_input(&new_name)))).child(div().w(px(col_w(1) + col_w(2) + col_w(3))).h(px(ROW_H)).flex_none().bg(p().bg_senary)))
                })
            });
        if self.host.is_some() {
            return v_flex().track_focus(&self.focus).child(table).into_any_element();
        }
        // Onshape: "Insert Variable Studio" (178x35, `#aaa` outline, radius
        // 5) centered above the tables; it opens the Insert Variable Studio
        // dialog for this studio.
        let doc = self.doc.clone();
        let me = self.tab;
        let insert = Button::new("vs-insert")
            .map(|d| crate::ui_script::mark(d, || "button:Insert Variable Studio".into()))
            .custom(ButtonCustomVariant::new(cx).color(rgb(0xffffff).into()).foreground(pal.text.into()).hover(pal.panel_hover.into()).active(pal.panel_active.into()).shadow(false))
            .h(px(35.))
            .px(px(10.5))
            .rounded(px(5.25))
            .border_1()
            .border_color(rgb(RULE))
            .child(h_flex().gap(px(3.)).child(crate::assets::icon("insert-variable", 20., cx)).child(div().text_size(px(14.)).child("Insert Variable Studio")))
            .tooltip("Insert Variable Studio")
            .on_click(move |_, window, cx| crate::variable_insert::open(doc.clone(), crate::variable_insert::InsertTarget::Studio(me), window, cx));
        let inserted = self.render_inserted_tables(&studio, window, cx);
        let footer = self.render_footer(studio.insert_into_all, !studio.configuration.is_empty(), cx);
        // The Configuration panel's button on the right edge, centred (live
        // document/10); the panel to its right when open (help config-panel-02).
        let tab = self.render_config_tab(cx);
        let raw = self.studio(cx).cloned().unwrap_or_default();
        let panel = if self.config_open { Some(self.render_config_panel(&raw, window, cx)) } else { None };
        let configured = !raw.configuration.is_empty();
        let config_box = if self.cfg_box_collapsed { None } else { self.render_config_box(&raw, cx) };
        // The box's edge toggle (live vs-914 x 627–659, centred).
        let box_toggle = configured.then(|| {
            div()
                .id("vs-cfg-box-toggle")
                .occlude()
                .w(px(33.))
                .h(px(34.))
                .ml(px(-1.))
                .flex()
                .items_center()
                .justify_center()
                .bg(pal.panel)
                .border_1()
                .border_color(pal.border_strong)
                .cursor_pointer()
                .hover(|d| d.bg(pal.panel_hover))
                .child(crate::assets::icon("treelist", 20., cx))
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.cfg_box_collapsed = !this.cfg_box_collapsed;
                    cx.notify();
                }))
        });
        let input_dialog = if self.config_open { self.render_input_dialog(cx) } else { None };
        // Onshape: the undo / redo toolbar, then the button and the table.
        v_flex()
            .size_full()
            .track_focus(&self.focus)
            // Live 2476: Ctrl+Z does nothing while a table cell has the keyboard (the toolbar's Undo still undoes).
            .on_action(cx.listener(|this, _: &crate::actions::Undo, window, cx| {
                if !this.cell_has_keyboard(window) {
                    cx.propagate();
                }
            }))
            .bg(rgb(0xffffff))
            .child(crate::ui::toolbar().child(crate::ui::undo_redo(&self.doc, cx)))
            .child(
                h_flex()
                    .flex_1()
                    .min_h_0()
                    .items_start()
                    .children(config_box)
                    .child(
                        v_flex()
                            .relative()
                            .flex_1()
                            .min_w_0()
                            .h_full()
                            .child(v_flex().id("vars").flex_1().min_h_0().pt(px(5.)).items_center().overflow_y_scroll().child(insert).child(div().h(px(2.))).children(inserted).child(table))
                            .child(footer)
                            .child(div().absolute().right_0().top_0().bottom_0().flex().items_center().child(tab))
                            .children(box_toggle.map(|t| div().absolute().left_0().top_0().bottom_0().flex().items_center().child(t)))
                            .children(input_dialog),
                    )
                    .children(panel),
            )
            .into_any_element()
    }
}

/// A variable row's right-click menu: Delete, then Move up / Move down where
/// the row has neighbours (live variable-studio/01–07).
fn row_menu(m: crate::os_menu::OsMenu, view: Entity<VariableStudioView>, i: usize, n: usize) -> crate::os_menu::OsMenu {
    let (v1, v2, v3) = (view.clone(), view.clone(), view);
    let m = m.item(crate::os_menu::OsMenuItem::new("Delete").on_click(move |_, _, cx| v1.update(cx, |t, cx| t.delete_variable(i, cx))));
    let m = if i > 0 { m.item(crate::os_menu::OsMenuItem::new("Move up").on_click(move |_, _, cx| v2.update(cx, |t, cx| t.move_variable(i, false, cx)))) } else { m };
    if i + 1 < n { m.item(crate::os_menu::OsMenuItem::new("Move down").on_click(move |_, _, cx| v3.update(cx, |t, cx| t.move_variable(i, true, cx)))) } else { m }
}
