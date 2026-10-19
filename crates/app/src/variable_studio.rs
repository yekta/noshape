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
use noshape_model::variables::{VarKind, VariableDef, VariableStudio};
use noshape_units::ExprValue;
use std::collections::HashMap;

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
    /// Same-document studios inserted into this one: editable tables
    /// (Onshape edits the inserted studio itself, live document/50–51).
    inserted_views: HashMap<TabId, Entity<VariableStudioView>>,
    /// The last "Collapse all tables" seen ([collapse_generation]).
    collapse_seen: u64,
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
            inserted_views: HashMap::new(),
            collapse_seen: collapse_generation(tab),
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

    fn edit(&mut self, desc: &str, cx: &mut Context<Self>, f: impl FnOnce(&mut VariableStudio)) {
        self.doc.update(cx, |doc, cx| {
            doc.edit_tab(self.tab, desc, cx, |c| {
                if let TabContent::VariableStudio(v) = c {
                    f(v)
                }
            });
        });
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
        let st = cx.new(|cx| InputState::new(window, cx).placeholder(placeholder).default_value(value.clone()));
        let sub = cx.subscribe_in(&st, window, move |this: &mut Self, st, ev: &InputEvent, _, cx| {
            if matches!(ev, InputEvent::PressEnter { .. } | InputEvent::Blur) {
                let v = st.read(cx).value().to_string();
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
const COL_W: f32 = 140.;
const ROW_H: f32 = 34.;
const RULE: u32 = 0xaaaaaa;
const HEAD_BG: u32 = 0xeaeaea;

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
    div().flex_1().ml(px(-4.)).child(Input::new(st).appearance(false).text_size(px(14.)))
}

fn cell_div() -> Div {
    div().w(px(COL_W)).h(px(ROW_H)).flex_none().flex().items_center().border_r_1().border_b_1().border_color(rgb(RULE))
}

impl VariableStudioView {
    fn move_variable(&mut self, i: usize, down: bool, cx: &mut Context<Self>) {
        self.inputs.clear();
        self.edit(if down { "Move variable down" } else { "Move variable up" }, cx, |s| {
            let j = if down { i + 1 } else { i.wrapping_sub(1) };
            if i < s.variables.len() && j < s.variables.len() {
                s.variables.swap(i, j);
            }
        })
    }

    fn delete_variable(&mut self, i: usize, cx: &mut Context<Self>) {
        self.inputs.clear();
        self.edit("Delete variable", cx, |s| {
            if i < s.variables.len() {
                s.variables.remove(i);
            }
        })
    }

    /// The more menu's Rename: edit the studio's name in its title row.
    fn start_rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let name = self.doc.read(cx).session.doc.tab(self.tab).map(|t| t.name.clone()).unwrap_or_default();
        let st = cx.new(|cx| InputState::new(window, cx).default_value(name));
        let sub = cx.subscribe_in(&st, window, |this: &mut Self, st, ev: &InputEvent, _, cx| {
            if matches!(ev, InputEvent::PressEnter { .. } | InputEvent::Blur) && this.renaming.is_some() {
                let name = st.read(cx).value().trim().to_string();
                let tab = this.tab;
                this.renaming = None;
                if !name.is_empty() {
                    this.doc.update(cx, |d, cx| {
                        d.edit_document(&format!("Rename tab : {name}"), cx, |doc| {
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
            out.extend(table.map(|t| div().w(px(COL_W * 4. + 1.)).mb(px(12.)).child(t).into_any_element()));
        }
        out
    }

    /// Onshape's footer (live DOM): a 40 px `#eee` bar, 1 px `#ccc` top rule,
    /// centered "Insert into all Part Studios and Assemblies" with its
    /// checkbox (307x35, `#aaa` outline, left corners 5.25) and a `?` menu
    /// button (49x35).
    fn render_footer(&self, on: bool, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let variant = ButtonCustomVariant::new(cx).color(rgb(0xffffff).into()).foreground(pal.text.into()).hover(pal.panel_hover.into()).active(pal.panel_active.into()).shadow(false);
        // Live: `svg-icon-comments-checked-checkbox` (green #009400, the tick overshoots the box).
        let check = if on {
            div().size(px(20.)).child(crate::assets::icon("checked-checkbox", 20., cx)).into_any_element()
        } else {
            div().size(px(20.)).flex().items_center().justify_center().child(div().size(px(13.)).rounded(px(2.)).border_1().border_color(rgb(0x888888))).into_any_element()
        };
        let probe = self.footer_bounds.clone();
        h_flex()
            .h(px(40.))
            .flex_none()
            .justify_center()
            .bg(rgb(0xeeeeee))
            .border_t_1()
            .border_color(rgb(0xcccccc))
            .child(
                Button::new("vs-insert-all")
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
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.edit(if on { "Do not insert into all Part Studios and Assemblies" } else { "Insert into all Part Studios and Assemblies" }, cx, |s| s.insert_into_all = !on)
                    })),
            )
            .child(
                Button::new("vs-insert-all-help")
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
                    .on_mouse_down_out(cx.listener(|this, _: &MouseDownEvent, _, cx| {
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
                            .child(
                                div()
                                    .id("vs-help-learn")
                                    .cursor_pointer()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .underline()
                                    .child("Learn more about Variable Studios")
                                    .on_click(|_, _, cx| cx.open_url("https://github.com/yekta/noshape#readme")),
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
        let mut out = String::from("Name\tVariable type\tValue\tDescription\n");
        for v in &studio.variables {
            out.push_str(&format!("{}\t{:?}\t{}\t{}\n", v.name, v.kind.unwrap_or(VarKind::Any), v.expr.text, v.description));
        }
        cx.write_to_clipboard(ClipboardItem::new_string(out));
    }
}

impl Render for VariableStudioView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pal = p();
        // "Collapse all tables" ran in this table's group.
        let generation = collapse_generation(self.group());
        if generation != self.collapse_seen {
            self.collapse_seen = generation;
            self.collapsed = true;
        }
        let studio = self.studio(cx).cloned().unwrap_or_default();
        let (units, title) = {
            let d = &self.doc.read(cx).session.doc;
            (d.settings.units, d.tab(self.tab).map(|t| t.name.clone()).unwrap_or_default())
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
        for (i, v) in studio.variables.iter().enumerate() {
            if only.as_ref().is_some_and(|o| !o.contains(&v.name)) {
                continue;
            }
            let name = self.cell(format!("n{i}"), v.name.clone(), window, cx, move |this, val, cx| {
                this.edit("Rename variable", cx, |s| {
                    if let Some(v) = s.variables.get_mut(i) {
                        v.name = val.trim().trim_start_matches('#').to_string();
                    }
                })
            });
            let expr = self.cell(format!("e{i}"), v.expr.text.clone(), window, cx, move |this, val, cx| {
                this.edit("Edit variable", cx, |s| {
                    if let Some(v) = s.variables.get_mut(i) {
                        v.expr = ExprValue::new(val);
                    }
                })
            });
            let desc = self.cell(format!("d{i}"), v.description.clone(), window, cx, move |this, val, cx| {
                this.edit("Edit variable description", cx, |s| {
                    if let Some(v) = s.variables.get_mut(i) {
                        v.description = val;
                    }
                })
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
            let from_import = v.kind == Some(VarKind::FromImport);
            let value_cell = match shown {
                // From import: the imported value and the Edit pencil (help:
                // "In the Value cell, click the Edit pencil icon").
                _ if from_import => {
                    let text = v.import.as_ref().map(|imp| noshape_model::variables::import_value(&self.doc.read(cx).session.doc, imp).map(|x| x.to_string()).unwrap_or_else(|e| e)).unwrap_or_else(|| "Select CSV file".into());
                    let (doc, tab) = (self.doc.clone(), self.tab);
                    cell_div()
                        .id(("vs-val", i))
                        .px(px(7.))
                        .gap(px(4.))
                        .child(div().flex_1().min_w_0().truncate().child(text))
                        .child(div().id(("vs-edit", i)).flex_none().cursor_pointer().tooltip(crate::ui::tooltip("Edit")).child(crate::ui::glyph("edit", 14., pal.text_muted)).on_click(move |_, window, cx| crate::variable_import::open(doc.clone(), tab, i, window, cx)))
                        .into_any_element()
                }
                Some(text) => {
                    let st = expr.clone();
                    cell_div()
                        .id(("vs-val", i))
                        .px(px(7.))
                        .cursor_text()
                        .child(text)
                        .on_click(cx.listener(move |_, _: &ClickEvent, window, cx| {
                            st.read(cx).focus_handle(cx).focus(window, cx);
                            cx.notify();
                        }))
                        .into_any_element()
                }
                None => cell_div()
                    .id(("vs-val", i))
                    .when_some(error, |d, e| d.text_color(pal.error).tooltip(crate::ui::tooltip(e)))
                    .child(cell_input(&expr))
                    .into_any_element(),
            };
            let kind = v.kind.unwrap_or(VarKind::Any);
            let cell_bounds: std::rc::Rc<std::cell::Cell<Bounds<Pixels>>> = Default::default();
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
                    .child(cell_div().child(cell_input(&name)))
                    .child(
                        cell_div()
                            .relative()
                            .child({
                                // The cell's painted bounds place the select list under it.
                                let b = cell_bounds.clone();
                                canvas(move |bounds, _, _| b.set(bounds), |_, _, _, _| {}).absolute().top_0().left_0().size_full()
                            })
                            .child(div().absolute().right(px(6.)).top(px(11.)).child(crate::ui::glyph("ui-caret-down", 12., pal.text)))
                            .child(
                            // A plain cell (a kit Button centres its label): text 7 px in, as live.
                            div()
                                .id(("vs-kind", i))
                                .size_full()
                                .flex()
                                .items_center()
                                .cursor_pointer()
                                .pl(px(7.))
                                .pt(px(2.))
                                .child(div().text_size(px(14.)).text_color(pal.text).child(kind_label(kind)))
                                .on_click(move |_, window, cx| {
                                    let kinds = kinds.clone();
                                    let b = cell_bounds.get();
                                    // Onshape's select list (live, document/16).
                                    let menu = crate::os_menu::OsMenu::build(window, cx, move |mut m, _, _| {
                                        m = m.select_list(COL_W);
                                        // Onshape's types (behaviours.md, Variable Studio).
                                        for k in [VarKind::Length, VarKind::Angle, VarKind::Number, VarKind::Any, VarKind::FromImport] {
                                            let v = kinds.clone();
                                            m = m.item(crate::os_menu::OsMenuItem::new(kind_label(k)).checked(k == kind).on_click(move |_, window, cx| {
                                                v.update(cx, |this, cx| {
                                                    this.edit("Change variable type", cx, |s| {
                                                        if let Some(var) = s.variables.get_mut(i) {
                                                            var.kind = Some(k);
                                                        }
                                                    });
                                                    // From import: its settings are in the Variable dialog.
                                                    if k == VarKind::FromImport {
                                                        crate::variable_import::open(this.doc.clone(), this.tab, i, window, cx);
                                                    }
                                                })
                                            }));
                                        }
                                        m
                                    });
                                    crate::os_menu::open_anchored(point(b.origin.x, b.origin.y + b.size.height), Anchor::TopLeft, menu, window, cx);
                                }),
                        ),
                    )
                    .child(value_cell)
                    .child(cell_div().child(cell_input(&desc)))
                    .os_context_menu(move |m, _, _| {
                        let (v1, v2, v3) = (menu_view.clone(), menu_view.clone(), menu_view.clone());
                        let m = m.item(crate::os_menu::OsMenuItem::new("Delete").on_click(move |_, _, cx| v1.update(cx, |t, cx| t.delete_variable(i, cx))));
                        let m = if i > 0 { m.item(crate::os_menu::OsMenuItem::new("Move up").on_click(move |_, _, cx| v2.update(cx, |t, cx| t.move_variable(i, false, cx)))) } else { m };
                        if i + 1 < n { m.item(crate::os_menu::OsMenuItem::new("Move down").on_click(move |_, _, cx| v3.update(cx, |t, cx| t.move_variable(i, true, cx)))) } else { m }
                    })
                    .into_any_element(),
            );
        }
        // The last row: an italic "Name" cell; typing a name adds a Length
        // variable set to 0 in the document's length unit.
        let unit = units.length.symbol();
        let new_name = self.cell("new".into(), String::new(), window, cx, move |this, val, cx| {
            let name = val.trim().trim_start_matches('#').to_string();
            if name.is_empty() {
                return;
            }
            this.inputs.remove("new");
            this.edit("Add variable", cx, |s| s.variables.push(VariableDef { name, expr: ExprValue::new(format!("0 {unit}")), kind: Some(VarKind::Length), description: String::new(), import: None }));
        });
        let collapsed = self.collapsed;
        let more_view = view.clone();
        let host_menu = host_refs.map(|(target, list)| {
            let k = list.iter().position(|r| r.studio == self.tab).unwrap_or(0);
            (self.doc.clone(), target, self.tab, title.clone(), k, list.len())
        });
        let group = self.group();
        // In a panel the title row spans the panel (live, part-studio/77).
        let in_panel = self.host.is_some();
        let table = v_flex()
            .when(!in_panel, |d| d.w(px(COL_W * 4. + 1.)))
            .when(in_panel, |d| d.w_full())
            .child(
                h_flex()
                    .h(px(35.))
                    .px(px(6.))
                    .bg(rgb(HEAD_BG))
                    .border_1()
                    .border_color(rgb(RULE))
                    .child(
                        h_flex()
                            .id("vs-collapse")
                            .gap(px(3.))
                            .cursor_pointer()
                            .child(crate::os_ui::glyph(if collapsed { crate::os_ui::GLYPH_COLLAPSED } else { crate::os_ui::GLYPH_EXPANDED }, 10., pal.text))
                            // An inserted studio's title starts with the studio icon (live part-studio/160).
                            .when(in_panel, |r| r.child(crate::assets::icon("variable-studio", 20., cx)))
                            .child(match &self.renaming {
                                Some(st) => div().w(px(260.)).text_size(px(16.)).child(Input::new(st).h(px(26.)).text_size(px(16.))).into_any_element(),
                                None => div().text_size(px(16.)).font_weight(FontWeight::MEDIUM).text_color(pal.text).child(title).into_any_element(),
                            })
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.collapsed = !this.collapsed;
                                cx.notify();
                            })),
                    )
                    .child(div().flex_1())
                    .child(
                        Button::new("vs-more")
                            .custom(plain)
                            .w(px(25.))
                            .h(px(22.))
                            .p_0()
                            // Live: `#333` dots 7 px apart (the svg ignores the link colour).
                            .child(div().mt(px(3.)).child(crate::os_ui::glyph(crate::os_ui::GLYPH_OVERFLOW, 20., pal.text)))
                            .on_click(move |e, window, cx| {
                                let (more_view, host_menu) = (more_view.clone(), host_menu.clone());
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
                                        let m = m.item(crate::os_menu::OsMenuItem::new(format!("Switch to {name}")).on_click(move |_, window, cx| crate::workspace::request_activate_tab(me, window, cx)));
                                        let m = if k > 0 { m.item(item("Move up").on_click(move |_, _, cx| crate::variable_insert::move_ref(&d1, target, me, false, cx))) } else { m };
                                        let m = if k + 1 < n { m.item(item("Move down").on_click(move |_, _, cx| crate::variable_insert::move_ref(&d2, target, me, true, cx))) } else { m };
                                        m.item(collapse)
                                            .item(item("Edit reference…").on_click(move |_, window, cx| crate::variable_insert::open(d4.clone(), target, window, cx)))
                                            .item(item("Remove reference").on_click(move |_, _, cx| crate::variable_insert::set_ref(&d3, target, me, None, cx)))
                                    }
                                    None => m.item(collapse),
                                }
                            });
                                crate::os_menu::open_anchored(point(e.position().x + px(12.), e.position().y + px(12.)), Anchor::TopRight, menu, window, cx);
                            }),
                    ),
            )
            .when(!collapsed, |t| {
                t.child(
                    h_flex()
                        .border_l_1()
                        .border_color(rgb(RULE))
                        .bg(rgb(HEAD_BG))
                        .text_size(px(14.))
                        .font_weight(FontWeight::BOLD)
                        .text_color(pal.text)
                        .children(["Name", "Variable type", "Value", "Description"].map(|h| cell_div().justify_center().child(h))),
                )
                .children(rows)
                .when(only.is_none(), |t| t.child(h_flex().child(cell_div().border_l_1().italic().child(cell_input(&new_name)))))
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
        let footer = self.render_footer(studio.insert_into_all, cx);
        // Onshape: the undo / redo toolbar, then the button and the table.
        v_flex()
            .size_full()
            .track_focus(&self.focus)
            .bg(rgb(0xffffff))
            .child(crate::ui::toolbar().child(crate::ui::undo_redo(&self.doc, cx)))
            .child(v_flex().id("vars").flex_1().min_h_0().pt(px(5.)).items_center().overflow_y_scroll().child(insert).child(div().h(px(2.))).children(inserted).child(table))
            .child(footer)
            .into_any_element()
    }
}
