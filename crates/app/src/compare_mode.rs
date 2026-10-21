//! Versions and history › Compare (live ui2/950–954, behaviours "Versions
//! and history › Compare"): no dialog — the document enters a compare mode
//! between a base (the workspace or version shown, "Main") and a target
//! (the row compared, "V3"):
//! - the toolbar row shows a centred bar "Comparing **Main** and **V3**
//!   [Reverse compare] (?)" ([`render_bar`]);
//! - every tab gets a status dot ([`tab_status`], [`tab_dot`]); drawings
//!   and tabs only in the base can't be opened;
//! - the history panel highlights the base row (light blue, blue bar) and
//!   the target row (light pink, red bar) (history_panel.rs);
//! - a Part Studio shows the compare panel in place of the feature list
//!   ([`render_panel`]: "Main (units)" / "V3 (units)", Features, "Only show
//!   differences", the selected feature's parameters) and grey graphics
//!   with a Main ○—— V3 blend slider ([`render_slider`], [`blend`]).
//!
//! Picking another history row leaves compare mode ([`stop`]).
//!
//! The state is per document window (a global keyed by the DocModel).

use crate::doc_model::DocModel;
use crate::theme::p;
use gpui_kit::component::{ActiveTheme as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_model::compare::{ChangeKind, CodeRow, FeatureCompare};
use noshape_model::document::{Document, TabContent, TabId};
use noshape_model::reference::FeatureId;
use noshape_store::{BranchId, MicroversionId};
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::ops::Range;
use std::rc::Rc;
use std::sync::Arc;

/// Base (workspace) blue and target (version) red (live 953).
pub const BASE_BLUE: u32 = 0x1651b0;
pub const TARGET_RED: u32 = 0xbd3039;
/// The bar's names (live 953: both dark blue).
const BAR_NAME: u32 = 0x164b7b;
/// Tab dots (live 952).
const DOT_YELLOW: u32 = 0xfab601;
/// Compare list markers (live cp-920): changed gold, only in the target green.
const MARK_GOLD: u32 = 0xdaa300;
const MARK_GREEN: u32 = 0x74ce3a;
/// A moved feature's marker (live mv-901: glyphicons circle_arrow_down /
/// _top, `#daa300`): our own drawing, a filled circle with the arrow cut out.
const GLYPH_MOVED_DOWN: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16"><mask id="mvd"><rect width="16" height="16" fill="#fff"/><path d="M8 3.6v7.2M4.9 7.8 8 10.9l3.1-3.1" stroke="#000" stroke-width="1.9" fill="none" stroke-linecap="round" stroke-linejoin="round"/></mask><circle cx="8" cy="8" r="7.6" fill="currentColor" mask="url(#mvd)"/></svg>"##;
const GLYPH_MOVED_UP: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16"><mask id="mvu"><rect width="16" height="16" fill="#fff"/><path d="M8 12.4V5.2M4.9 8.2 8 5.1l3.1 3.1" stroke="#000" stroke-width="1.9" fill="none" stroke-linecap="round" stroke-linejoin="round"/></mask><circle cx="8" cy="8" r="7.6" fill="currentColor" mask="url(#mvu)"/></svg>"##;
/// A changed field's outline in the compare dialogs (live cp-921).
const CHANGED: u32 = 0xfab601;

/// History rows (live 952): base light blue, target light pink.
pub const BASE_ROW: u32 = 0xb2ddf6;
pub const TARGET_ROW: u32 = 0xf6e0e1;

/// One side of a compare: a workspace (its head) or a history entry (a
/// version, or a change inside a workspace or version).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Workspace(BranchId),
    Entry(MicroversionId),
}

pub struct CompareSession {
    pub base_side: Side,
    pub target_side: Side,
    pub base_label: String,
    pub target_label: String,
    pub base: Arc<Document>,
    pub target: Arc<Document>,
    /// Graphics blend: 0 = base, 1 = target.
    pub blend: f32,
    pub only_differences: bool,
    pub selected: Option<FeatureId>,
    /// Sketches shown in the graphics (the list's eye column, live dom
    /// ui2-compare-features: sketches start hidden, their names grey).
    pub shown_sketches: HashSet<FeatureId>,
    /// Each side's configuration (the panel's Configurations section, help
    /// compare-configurations; defaults until picked).
    pub base_config: noshape_model::configuration::ConfigValues,
    pub target_config: noshape_model::configuration::ConfigValues,
    /// Feature Studio compares, made on first view.
    code: RefCell<HashMap<TabId, Rc<CodeCompare>>>,
    code_scroll: UniformListScrollHandle,
    /// Each code pane's horizontal scroll (base, target).
    code_x: Rc<Cell<[f32; 2]>>,
    /// The workspace and history point the window showed at the start:
    /// opening anything else leaves compare mode.
    origin: (BranchId, Option<MicroversionId>),
}

#[derive(Default)]
struct Sessions(HashMap<EntityId, CompareSession>);
impl Global for Sessions {}

/// "Compare history entries" (the panel header's compare button; help
/// "Comparing"): on while picking two entries; the first pick (blue).
#[derive(Default)]
struct Picks(HashMap<EntityId, Option<(Side, String)>>);
impl Global for Picks {}

/// A tab's state in compare mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TabStatus {
    /// Yellow: "Compare has found differences."
    Differs,
    /// No differences (no dot).
    Same,
    /// White, tab disabled: "Compare not supported for this tab." (drawings).
    NotSupported,
    /// Red, tab disabled: "Tab only exists in 'Main'."
    OnlyInBase,
}

impl TabStatus {
    /// The tab can't be opened in compare mode.
    pub fn disabled(self) -> bool {
        matches!(self, TabStatus::NotSupported | TabStatus::OnlyInBase)
    }
}

fn changed(doc: &Entity<DocModel>, cx: &mut App) {
    doc.update(cx, |_, cx| cx.notify());
    cx.refresh_windows();
}

/// A side's document state.
fn load(doc: &Entity<DocModel>, side: Side, cx: &App) -> anyhow::Result<Document> {
    let store = &doc.read(cx).session.store;
    let mv = match side {
        Side::Workspace(b) => store.head(b)?,
        Side::Entry(mv) => mv,
    };
    Ok(Document::from_blobs(&store.read_all(mv)?)?)
}

/// Enter compare mode between two history entries (base: the first pick,
/// or what the window shows; target: the entry compared).
pub fn compare(doc: &Entity<DocModel>, base: (Side, String), target: (Side, String), cx: &mut App) -> anyhow::Result<()> {
    let (b, t) = (load(doc, base.0, cx)?, load(doc, target.0, cx)?);
    let origin = {
        let s = &doc.read(cx).session;
        (s.branch, s.viewing)
    };
    cx.default_global::<Picks>().0.remove(&doc.entity_id());
    let s = CompareSession {
        base_side: base.0,
        target_side: target.0,
        base_label: base.1,
        target_label: target.1,
        base: Arc::new(b),
        target: Arc::new(t),
        blend: 0.,
        only_differences: true,
        selected: None,
        shown_sketches: HashSet::new(),
        base_config: Default::default(),
        target_config: Default::default(),
        code: Default::default(),
        code_scroll: UniformListScrollHandle::new(),
        code_x: Default::default(),
        origin,
    };
    // Live cmp2-903: the open tab missing from a side.
    let active = doc.read(cx).session.active_tab;
    let missing = active.is_some_and(|t| s.base.tab(t).is_none() || s.target.tab(t).is_none());
    cx.default_global::<Sessions>().0.insert(doc.entity_id(), s);
    changed(doc, cx);
    if missing {
        doc.update(cx, |_, cx| cx.emit(crate::doc_model::DocEvent::Info("Can't open tab. The tab does not exist in this document comparison.".into())));
    }
    Ok(())
}

/// Leave compare mode once the window opens another workspace or history
/// point (creating a workspace, Open, restore…); workspace.rs calls it.
pub fn follow(doc: &Entity<DocModel>, cx: &mut App) {
    let Some(origin) = session(doc, cx).map(|s| s.origin) else { return };
    let now = {
        let s = &doc.read(cx).session;
        (s.branch, s.viewing)
    };
    if now != origin {
        stop(doc, cx);
    }
}

/// Leave compare mode.
pub fn stop(doc: &Entity<DocModel>, cx: &mut App) {
    if cx.default_global::<Sessions>().0.remove(&doc.entity_id()).is_some() {
        changed(doc, cx);
    }
}

/// Reverse compare: swap base and target.
pub fn reverse(doc: &Entity<DocModel>, cx: &mut App) {
    update(doc, cx, |s| {
        std::mem::swap(&mut s.base_side, &mut s.target_side);
        std::mem::swap(&mut s.base_label, &mut s.target_label);
        std::mem::swap(&mut s.base, &mut s.target);
        std::mem::swap(&mut s.base_config, &mut s.target_config);
        s.selected = None;
        s.code.borrow_mut().clear();
        s.code_x.set([0.; 2]);
    });
}

/// The bar's "Main" / "V3" links ("Go to 'Main'"): leave compare mode and
/// open that side.
fn go_to(doc: &Entity<DocModel>, side: Side, cx: &mut App) {
    stop(doc, cx);
    let r = doc.update(cx, |d, cx| -> anyhow::Result<()> {
        match side {
            Side::Workspace(b) if b == d.session.branch => d.session.view_microversion(None)?,
            Side::Workspace(b) => d.session.switch_branch(b)?,
            Side::Entry(mv) => d.session.view_microversion(Some(mv))?,
        }
        d.reloaded(cx);
        Ok(())
    });
    if let Err(e) = r {
        doc.update(cx, |_, cx| cx.emit(crate::doc_model::DocEvent::Error(format!("{e:#}"))));
    }
}

fn update(doc: &Entity<DocModel>, cx: &mut App, f: impl FnOnce(&mut CompareSession)) {
    if let Some(s) = cx.default_global::<Sessions>().0.get_mut(&doc.entity_id()) {
        f(s);
        changed(doc, cx);
    }
}

fn session<'a>(doc: &Entity<DocModel>, cx: &'a App) -> Option<&'a CompareSession> {
    cx.try_global::<Sessions>().and_then(|g| g.0.get(&doc.entity_id()))
}

/// The (base, target) history entries while comparing.
pub fn rows(doc: &Entity<DocModel>, cx: &App) -> Option<(Side, Side)> {
    session(doc, cx).map(|s| (s.base_side, s.target_side))
}

/// "Compare history entries" is on (picking two entries).
pub fn picking(doc: &Entity<DocModel>, cx: &App) -> bool {
    cx.try_global::<Picks>().is_some_and(|p| p.0.contains_key(&doc.entity_id()))
}

/// The first entry picked (highlighted blue until the second is picked).
pub fn first_pick(doc: &Entity<DocModel>, cx: &App) -> Option<Side> {
    cx.try_global::<Picks>().and_then(|p| p.0.get(&doc.entity_id())).and_then(|p| p.as_ref().map(|p| p.0))
}

/// The panel header's compare button: start / stop picking.
pub fn toggle_picking(doc: &Entity<DocModel>, cx: &mut App) {
    let picks = &mut cx.default_global::<Picks>().0;
    if picks.remove(&doc.entity_id()).is_none() {
        picks.insert(doc.entity_id(), None);
        stop(doc, cx);
    }
    changed(doc, cx);
}

/// A history entry clicked while picking: the first is the base, the
/// second starts the compare.
pub fn pick(doc: &Entity<DocModel>, side: Side, label: String, cx: &mut App) -> anyhow::Result<()> {
    let first = cx.default_global::<Picks>().0.get_mut(&doc.entity_id()).and_then(|p| p.take());
    match first {
        Some(base) if base.0 != side => compare(doc, base, (side, label), cx)?,
        _ => {
            cx.default_global::<Picks>().0.insert(doc.entity_id(), Some((side, label)));
            changed(doc, cx);
        }
    }
    Ok(())
}

/// The graphics blend (0 = base, 1 = target) while comparing.
pub fn blend(doc: &Entity<DocModel>, cx: &App) -> Option<f32> {
    session(doc, cx).map(|s| s.blend)
}

/// Both sides for the graphics: (base, base configuration, target, target
/// configuration).
pub fn documents(doc: &Entity<DocModel>, cx: &App) -> Option<(Arc<Document>, noshape_model::configuration::ConfigValues, Arc<Document>, noshape_model::configuration::ConfigValues)> {
    session(doc, cx).map(|s| (s.base.clone(), s.base_config.clone(), s.target.clone(), s.target_config.clone()))
}

/// The target side of a tab (for the graphics).
pub fn target_document(doc: &Entity<DocModel>, cx: &App) -> Option<Arc<Document>> {
    session(doc, cx).map(|s| s.target.clone())
}

fn status_of(s: &CompareSession, tab: TabId) -> Option<TabStatus> {
    let b = s.base.tab(tab)?;
    let Some(t) = s.target.tab(tab) else { return Some(TabStatus::OnlyInBase) };
    if matches!(b.content, TabContent::Drawing(_)) {
        return Some(TabStatus::NotSupported);
    }
    let same = b.name == t.name && serde_json::to_value(&b.content).ok() == serde_json::to_value(&t.content).ok();
    Some(if same { TabStatus::Same } else { TabStatus::Differs })
}

/// A tab's compare state (None: not comparing).
pub fn tab_status(doc: &Entity<DocModel>, tab: TabId, cx: &App) -> Option<TabStatus> {
    session(doc, cx).and_then(|s| status_of(s, tab))
}

/// The dot after a tab's name, with its tooltip (live 952: 10 px). An
/// unchanged tab gets the white dot too, still enabled (help
/// fs-compare-01: "Part Studio 1 ○").
pub fn tab_dot(id: impl Into<ElementId>, status: TabStatus, base_label: &str) -> AnyElement {
    let dot = div().id(id).size(px(10.)).flex_none().rounded_full();
    let white = |d: Stateful<Div>| d.bg(rgb(0xffffff)).border_1().border_color(rgb(0xc0c0c0));
    let (dot, tip) = match status {
        TabStatus::Same => (white(dot), "Compare has found no differences.".to_string()),
        TabStatus::Differs => (dot.bg(rgb(DOT_YELLOW)), "Compare has found differences.".to_string()),
        TabStatus::NotSupported => (white(dot), "Compare not supported for this tab.".to_string()),
        TabStatus::OnlyInBase => (dot.bg(rgb(TARGET_RED)), format!("Tab only exists in '{base_label}'.")),
    };
    dot.tooltip(crate::ui::tooltip(tip)).into_any_element()
}

/// The base label (for tab tooltips).
pub fn base_label(doc: &Entity<DocModel>, cx: &App) -> Option<String> {
    session(doc, cx).map(|s| s.base_label.clone())
}

/// "NOT COMPARABLE" over the reason, centred in the area under the
/// toolbar row (live cp-923: `#333`, bold 14 px, 31 px apart).
fn render_not_comparable(reason: String) -> AnyElement {
    v_flex()
        .size_full()
        .pt(px(crate::ui::TOOLBAR_H))
        .items_center()
        .justify_center()
        .gap(px(14.))
        .bg(rgb(0xffffff))
        .text_size(px(14.))
        .font_weight(FontWeight::BOLD)
        .text_color(rgb(0x333333))
        .child("NOT COMPARABLE")
        .child(reason)
        .into_any_element()
}

/// What a tab shows in compare mode in place of its own view (None: its
/// view, a Part Studio with the compare panel): "NOT COMPARABLE" for a tab
/// only in the base and for tabs compare doesn't cover (assemblies, live
/// cp-923; help "Available in: Part Studio, Feature Studio"), the code
/// compare for a Feature Studio.
pub fn content_override(doc: &Entity<DocModel>, tab: TabId, cx: &App) -> Option<AnyElement> {
    let s = session(doc, cx)?;
    match status_of(s, tab)? {
        TabStatus::OnlyInBase => return Some(render_not_comparable(format!("Tab only exists in '{}'.", s.base_label))),
        TabStatus::NotSupported => return Some(render_not_comparable("Compare not supported for this tab.".into())),
        _ => {}
    }
    match (&s.base.tab(tab)?.content, &s.target.tab(tab)?.content) {
        (TabContent::PartStudio(_), TabContent::PartStudio(_)) => None,
        (TabContent::FeatureStudio(a), TabContent::FeatureStudio(b)) => Some(render_code(s, tab, a, b, cx)),
        _ => Some(render_not_comparable("Compare not supported for this tab.".into())),
    }
}

/// The centred "Comparing Main and V3 [Reverse compare] (?)" bar in the
/// toolbar row (live 953: 372 px, white, `#ccc` border, rounded below).
pub fn render_bar(doc: &Entity<DocModel>, cx: &App) -> Option<AnyElement> {
    let s = session(doc, cx)?;
    let pal = p();
    let word = |t: String, c: u32| div().text_size(px(14.)).font_weight(FontWeight::BOLD).text_color(rgb(c)).child(t);
    // The names are links (live dom ui2-compare: "Go to 'Main'").
    let link = |id: &'static str, t: String, side: Side| {
        let d = doc.clone();
        div()
            .id(id)
            .cursor_pointer()
            .tooltip(crate::ui::tooltip(format!("Go to '{t}'")))
            .on_click(move |_, _, cx| go_to(&d, side, cx))
            .child(word(t, BAR_NAME))
    };
    let d = doc.clone();
    Some(
        h_flex()
            .id("compare-bar")
            .h(px(32.))
            .px(px(20.))
            .gap(px(12.))
            .bg(rgb(0xffffff))
            .border_1()
            .border_t_0()
            .border_color(rgb(0xcccccc))
            .rounded_b(px(6.))
            .child(word("Comparing".into(), 0x333333))
            .child(link("compare-go-base", s.base_label.clone(), s.base_side))
            .child(word("and".into(), 0x333333))
            .child(link("compare-go-target", s.target_label.clone(), s.target_side))
            .child(
                div()
                    .id("compare-reverse")
                    .h(px(27.))
                    .px(px(8.))
                    .flex()
                    .items_center()
                    .rounded(px(3.5))
                    .bg(rgb(BASE_BLUE))
                    .text_size(px(13.))
                    .text_color(rgb(0xffffff))
                    .cursor_pointer()
                    .hover(|d| d.bg(rgb(0x2b65c4)))
                    .child("Reverse compare")
                    .on_click(move |_, _, cx| reverse(&d, cx)),
            )
            .child(div().size(px(22.)).flex_none().text_color(pal.text_muted).child(crate::assets::icon("help-filled", 22., cx)))
            .into_any_element(),
    )
}

/// The Main ○—— V3 slider over the graphics (live 953: labels 14 px bold,
/// a 300 px `#777` track, a 12 px white thumb). Dragging blends the
/// graphics from the base (left) to the target (right).
pub fn render_slider(doc: &Entity<DocModel>, cx: &App) -> Option<AnyElement> {
    let s = session(doc, cx)?;
    const TRACK: f32 = 300.;
    let at = blend(doc, cx).unwrap_or(0.).clamp(0., 1.) * TRACK;
    let d = doc.clone();
    let set = move |x: Pixels, bounds: Bounds<Pixels>, cx: &mut App| {
        let t = (f32::from(x - bounds.left()) / TRACK).clamp(0., 1.);
        update(&d, cx, |s| s.blend = t);
    };
    let set2 = set.clone();
    Some(
        v_flex()
            .w(px(TRACK + 12.))
            .child(
                h_flex()
                    .px(px(6.))
                    .justify_between()
                    .text_size(px(14.))
                    .font_weight(FontWeight::BOLD)
                    .child(div().text_color(rgb(BASE_BLUE)).child(s.base_label.clone()))
                    .child(div().text_color(rgb(TARGET_RED)).child(s.target_label.clone())),
            )
            .child(
                div()
                    .id("compare-blend")
                    .relative()
                    .mt(px(4.))
                    .mx(px(6.))
                    .w(px(TRACK))
                    .h(px(14.))
                    .cursor_pointer()
                    .child(div().absolute().top(px(6.)).left_0().w(px(TRACK)).h(px(2.)).bg(rgb(0x777777)))
                    .child(div().absolute().top(px(1.)).left(px(at - 6.)).size(px(12.)).rounded_full().bg(rgb(0xffffff)).border_1().border_color(rgb(0x888888)))
                    .child(
                        canvas(|_, _, _| {}, move |bounds, _, window, _| {
                            let set = set2.clone();
                            window.on_mouse_event(move |e: &MouseMoveEvent, phase, _, cx| {
                                if phase == DispatchPhase::Bubble && e.pressed_button == Some(MouseButton::Left) && bounds.contains(&e.position) {
                                    set(e.position.x, bounds, cx);
                                }
                            });
                            let set = set2.clone();
                            window.on_mouse_event(move |e: &MouseDownEvent, phase, _, cx| {
                                if phase == DispatchPhase::Bubble && e.button == MouseButton::Left && bounds.contains(&e.position) {
                                    set(e.position.x, bounds, cx);
                                }
                            });
                        })
                        .absolute()
                        .top_0()
                        .left_0()
                        .size_full(),
                    ),
            )
            .into_any_element(),
    )
}

/// "(millimeter, degree)" under a side's name.
fn units_label(d: &Document) -> String {
    let u = &d.settings.units;
    format!("({}, {})", u.length.name().to_lowercase(), u.angle.name().to_lowercase())
}

/// The Part Studio compare panel (live 953): 400 px in place of the
/// feature list. `tab` must be a Part Studio on both sides.
pub fn render_panel(doc: &Entity<DocModel>, tab: TabId, cx: &App) -> Option<AnyElement> {
    let s = session(doc, cx)?;
    let pal = p();
    let target = target_document(doc, cx)?;
    let (Some(TabContent::PartStudio(a)), Some(TabContent::PartStudio(b))) = (s.base.tab(tab).map(|t| &t.content), target.tab(tab).map(|t| &t.content)) else { return None };
    // Each side in its chosen configuration.
    let configured = |ps: &noshape_model::partstudio::PartStudio, values: &noshape_model::configuration::ConfigValues| {
        let mut out = ps.clone();
        out.features = ps.features.iter().map(|f| ps.configuration.for_editing(f, values)).collect();
        out
    };
    let (a, b) = (&configured(a, &s.base_config), &configured(b, &s.target_config));
    let rows: Vec<FeatureCompare> = noshape_model::compare::compare_features(a, b).into_iter().filter(|r| !s.only_differences || r.status.is_some() || r.moved.is_some()).collect();
    let selected = s.selected.and_then(|id| rows.iter().find(|r| r.id == id)).cloned();
    let side = |name: String, color: u32, units: String| {
        v_flex()
            .w(px(197.))
            .child(div().text_size(px(18.)).font_weight(FontWeight::SEMIBOLD).text_color(rgb(color)).child(name))
            .child(div().text_size(px(12.)).text_color(rgb(0x595959)).child(units))
    };
    // Live cp-920/921: base | marker | target columns, 29 px rows. Changed:
    // a gold "≠" (white when selected); only in the base: a red ">"; only in
    // the target: a green "<". Sketches are greyed as in the feature list.
    let feature_rows: Vec<AnyElement> = rows
        .iter()
        .map(|r| {
            let id = r.id;
            let on = s.selected == Some(id);
            let d = doc.clone();
            let sketch = r.type_id == "newSketch" || r.type_id == "sketch";
            // Sketches start hidden in the graphics, their names grey; the
            // row's eye shows them (live dom ui2-compare-features).
            let shown = !sketch || s.shown_sketches.contains(&id);
            // A moved feature's two rows each show one side (live mv-900…903).
            let (in_base, in_target) = match r.moved {
                Some(m) => (m.at_base, !m.at_base),
                None => (r.status != Some(ChangeKind::Added), r.status != Some(ChangeKind::Removed)),
            };
            let mark = match r.status {
                _ if r.moved.is_some() => None,
                Some(ChangeKind::Modified) => Some(("≠", if on { 0xffffff } else { MARK_GOLD })),
                Some(ChangeKind::Removed) => Some((">", TARGET_RED)),
                Some(ChangeKind::Added) => Some(("<", MARK_GREEN)),
                None => None,
            };
            // The base row points the way it moved, the target row back.
            let moved_glyph = r.moved.map(|m| if m.down == m.at_base { GLYPH_MOVED_DOWN } else { GLYPH_MOVED_UP });
            // Live cp-922: base icon at x 6, "≠" centred at 181, target icon at 204.
            let name = |show: bool, w: Option<f32>| {
                let c = h_flex().gap(px(4.)).min_w_0();
                let c = match w {
                    Some(w) => c.w(px(w)).flex_none(),
                    None => c.flex_1(),
                };
                c.when(show, |c| {
                    c.child(svg().path(crate::part_studio::feature_icon(&r.type_id)).size(px(16.)).flex_none().text_color(pal.text))
                        .child(div().min_w_0().truncate().text_color(if shown { rgb(0x000000) } else { rgb(0xaaaaaa) }).child(r.name.clone()))
                })
            };
            // A moved feature has two rows: ids per row.
            let rk = if r.moved.is_some_and(|m| !m.at_base) { "t" } else { "b" };
            let group: SharedString = format!("compare-row-{}-{rk}", id.0).into();
            let d2 = doc.clone();
            h_flex()
                .id(SharedString::from(format!("compare-feature-{}-{rk}", id.0)))
                .group(group.clone())
                .h(px(29.))
                .flex_none()
                .px(px(6.))
                .border_b_1()
                .border_color(rgb(0xdee2e6))
                .text_size(px(13.))
                .cursor_pointer()
                .when(on, |d| d.bg(rgb(BASE_ROW)))
                .when(!on, |d| d.hover(|d| d.bg(pal.panel_hover)))
                .child(name(in_base, Some(158.)))
                .child(
                    div()
                        .w(px(40.))
                        .flex_none()
                        .flex()
                        .justify_center()
                        .font_weight(FontWeight::BOLD)
                        .text_size(px(15.))
                        .children(mark.map(|(m, c)| div().text_color(rgb(c)).child(m)))
                        .children(moved_glyph.map(|g| crate::os_ui::glyph(g, 14., rgb(if on { 0xffffff } else { MARK_GOLD })))),
                )
                .child(name(in_target, None))
                .when(sketch, |row| {
                    row.child(
                        div()
                            .id(SharedString::from(format!("compare-eye-{}-{rk}", id.0)))
                            .flex_none()
                            .invisible()
                            .group_hover(group.clone(), |d| d.visible())
                            .child(crate::assets::icon(if shown { "eye" } else { "eye-off" }, 16., cx))
                            .on_click(move |_, _, cx| {
                                cx.stop_propagation();
                                update(&d2, cx, |s| {
                                    if !s.shown_sketches.remove(&id) {
                                        s.shown_sketches.insert(id);
                                    }
                                })
                            }),
                    )
                })
                .on_click(move |_, _, cx| update(&d, cx, |s| s.selected = Some(id)))
                .into_any_element()
        })
        .collect();
    let params: AnyElement = match &selected {
        None => div().pt(px(12.)).w_full().flex().justify_center().text_size(px(13.)).font_weight(FontWeight::SEMIBOLD).text_color(pal.text).child("Select a feature to compare its parameters.").into_any_element(),
        // Two read-only feature dialogs side by side, base | target (live cp-921).
        Some(f) => h_flex()
            .items_start()
            .size_full()
            .child(dialog_column(f, true))
            .child(div().w(px(1.)).h_full().flex_none().bg(rgb(0xd7d7d7)))
            .child(dialog_column(f, false))
            .into_any_element(),
    };
    let d = doc.clone();
    let only = s.only_differences;
    Some(
        v_flex()
            .w(px(400.))
            .h_full()
            .flex_none()
            .bg(rgb(0xffffff))
            .border_r_1()
            .border_color(rgb(0xd7d7d7))
            .child(
                h_flex()
                    .h(px(48.))
                    .flex_none()
                    .items_start()
                    .pt(px(4.))
                    .px(px(10.))
                    .border_b_1()
                    .border_color(rgb(0xcccccc))
                    .child(side(s.base_label.clone(), BASE_BLUE, units_label(&s.base)))
                    .child(side(s.target_label.clone(), TARGET_RED, units_label(&s.target))),
            )
            .children(config_section(doc, a, b, s))
            .child(div().px(px(4.)).pt(px(10.)).pb(px(4.)).text_size(px(13.)).font_weight(FontWeight::SEMIBOLD).text_color(pal.text).child("Features"))
            .child(div().id("compare-features").flex_1().min_h_0().overflow_y_scroll().child(v_flex().children(feature_rows)))
            .child(
                h_flex()
                    .id("compare-only-diffs")
                    .h(px(30.))
                    .flex_none()
                    .px(px(10.))
                    .gap(px(4.))
                    .border_t_1()
                    .border_b_1()
                    .border_color(rgb(0xcccccc))
                    .cursor_pointer()
                    .text_size(px(13.33))
                    .text_color(pal.text)
                    .child(
                        div()
                            .size(px(13.))
                            .flex_none()
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(2.))
                            .border_1()
                            .border_color(if only { rgb(0x0075ff) } else { rgb(0x767676) })
                            .bg(if only { rgb(0x0075ff) } else { rgb(0xffffff) })
                            .when(only, |d| d.child(gpui_kit::component::Icon::new(gpui_kit::component::IconName::Check).size(px(11.)).text_color(rgb(0xffffff)))),
                    )
                    .child("Only show differences")
                    .on_click(move |_, _, cx| update(&d, cx, |s| s.only_differences = !s.only_differences)),
            )
            .child(div().id("compare-params").h(px(460.)).flex_none().overflow_y_scroll().child(params))
            .into_any_element(),
    )
}

/// "Configurations" with a "Configuration [option ▾]" per list input on
/// each side (help compare-configurations), when either side is configured.
fn config_section(doc: &Entity<DocModel>, a: &noshape_model::partstudio::PartStudio, b: &noshape_model::partstudio::PartStudio, s: &CompareSession) -> Option<AnyElement> {
    use noshape_model::configuration::ConfigInputKind;
    if a.configuration.inputs.is_empty() && b.configuration.inputs.is_empty() {
        return None;
    }
    let pal = p();
    let column = |ps: &noshape_model::partstudio::PartStudio, values: &noshape_model::configuration::ConfigValues, base: bool| {
        let mut col = v_flex().w(px(190.)).gap(px(2.));
        for input in &ps.configuration.inputs {
            let ConfigInputKind::List { options, .. } = &input.kind else { continue };
            let chosen = ps.configuration.value_of(values, &input.id).unwrap_or_default();
            let label = options.iter().find(|o| o.id == chosen).map(|o| o.name.clone()).unwrap_or_default();
            let (d, iid, opts) = (doc.clone(), input.id.clone(), options.clone());
            col = col.child(
                h_flex()
                    .id(SharedString::from(format!("cmp-cfg-{base}-{}", input.id)))
                    .h(px(28.))
                    .gap(px(6.))
                    .text_size(px(12.))
                    .child(div().flex_none().text_color(pal.text_muted).child(input.name.clone()))
                    .child(h_flex().w(px(110.)).h(px(24.)).border_b_1().border_color(rgb(0xcccccc)).cursor_pointer().text_size(px(13.)).text_color(pal.text).child(div().flex_1().truncate().child(label)).child(crate::os_ui::glyph(crate::os_ui::GLYPH_CARET, 7., rgb(0x666666))))
                    .on_click(move |e: &ClickEvent, window, cx| {
                        let (d, iid, opts, chosen) = (d.clone(), iid.clone(), opts.clone(), chosen.clone());
                        let menu = crate::os_menu::OsMenu::build(window, cx, move |mut m, _, _| {
                            m = m.select_list(150.);
                            for o in &opts {
                                let (d, iid, oid) = (d.clone(), iid.clone(), o.id.clone());
                                m = m.item(crate::os_menu::OsMenuItem::new(o.name.clone()).checked(o.id == chosen).on_click(move |_, _, cx| {
                                    update(&d, cx, |s| {
                                        let v = if base { &mut s.base_config } else { &mut s.target_config };
                                        v.0.insert(iid.clone(), oid.clone());
                                        s.selected = None;
                                    })
                                }));
                            }
                            m
                        });
                        crate::os_menu::open(e.position(), menu, window, cx);
                    }),
            );
        }
        col
    };
    Some(
        v_flex()
            .px(px(10.))
            .pb(px(6.))
            .border_b_1()
            .border_color(rgb(0xcccccc))
            .child(div().pt(px(6.)).pb(px(2.)).text_size(px(13.)).font_weight(FontWeight::SEMIBOLD).text_color(pal.text).child("Configurations"))
            .child(h_flex().items_start().gap(px(7.)).child(column(a, &s.base_config, true)).child(column(b, &s.target_config, false)))
            .into_any_element(),
    )
}

/// One side's read-only feature dialog in the compare panel (live cp-921):
/// the feature name over a rule, then its controls; a value that differs
/// from the other side has a `#fab601` outline.
fn dialog_column(f: &FeatureCompare, base: bool) -> AnyElement {
    use noshape_model::compare::ParamShape;
    let pal = p();
    let side = |p: &noshape_model::compare::ParamCompare| if base { p.base.clone() } else { p.target.clone() };
    let name = f.params.iter().find(|p| p.shape == ParamShape::Name).and_then(side);
    let Some(name) = name else { return div().w(px(197.)).flex_none().into_any_element() };
    let boxed = |text: String, w: Option<f32>, changed: bool| {
        let d = h_flex()
            .h(px(30.))
            .px(px(5.))
            .border_1()
            .border_color(if changed { rgb(CHANGED) } else { rgb(0xcccccc) })
            .rounded(px(2.))
            .text_size(px(13.))
            .text_color(rgb(0x393939))
            .child(div().min_w_0().truncate().child(text));
        match w {
            Some(w) => d.w(px(w)),
            None => d.w_full(),
        }
    };
    let rows = f.params.iter().filter(|p| p.shape != ParamShape::Name).filter_map(|p| {
        let v = side(p)?;
        let changed = p.differs();
        Some(match p.shape {
            ParamShape::Query => div().px(px(3.)).child(boxed(v, None, changed)).into_any_element(),
            ParamShape::Enum => div().px(px(3.)).child(boxed(v, Some(162.), changed).justify_between().child(crate::os_ui::glyph(crate::os_ui::GLYPH_CARET, 7., rgb(0x999999)))).into_any_element(),
            ParamShape::Bool => {
                let on = v == "true";
                h_flex()
                    .h(px(26.))
                    .px(px(6.))
                    .gap(px(6.))
                    .text_size(px(13.33))
                    .text_color(pal.text)
                    .child(div().size(px(13.)).flex_none().rounded(px(2.)).border_1().border_color(if changed { rgb(CHANGED) } else { rgb(0xcccccc) }).bg(if on { rgb(0xcccccc) } else { rgb(0xffffff) }))
                    .child(p.label.clone())
                    .into_any_element()
            }
            _ => h_flex()
                .h(px(32.))
                .px(px(6.))
                .justify_between()
                .text_size(px(14.))
                .text_color(pal.text)
                .child(div().min_w_0().truncate().child(p.label.clone()))
                .child(boxed(noshape_units::localize(&v), Some(100.), changed).justify_end())
                .into_any_element(),
        })
    });
    v_flex()
        .w(px(197.))
        .flex_none()
        .gap(px(3.))
        .child(div().h(px(35.)).px(px(6.)).flex().items_center().border_b_1().border_color(rgb(0xd7d7d7)).text_size(px(15.)).text_color(pal.text).child(name))
        .children(rows)
        .into_any_element()
}

// ---------------------------------------------------------------- Feature Studio compare

/// Help fs-compare-01, live cmp2-906…910 (Ace's diff): 14 px code on 16 px
/// lines, a 50 px `#f0f0f0` gutter ("12 -"), deleted lines pink with a red
/// bar on the left, added lines green with a green bar on the right, the
/// changed characters darker, missing lines hatched; each pane scrolls
/// sideways on its own; a 12 px scrollbar after each pane (no change marks).
const CODE_LINE_H: f32 = 16.;
const CODE_GUTTER_W: f32 = 50.;
const CODE_RULER_W: f32 = 12.;
const CODE_HEADER_H: f32 = 30.;
const CODE_GUTTER: u32 = 0xf0f0f0;
const CODE_DEL_ROW: u32 = 0xfff1f1;
const CODE_DEL_BAR: u32 = 0xea7158;
const CODE_ADD_ROW: u32 = 0xeffff1;
const CODE_ADD_BAR: u32 = 0x2bb534;
/// The changed characters (live dom ui2-fs-compare-styles: rgba over the row).
const CODE_DEL_MARK: (u32, f32) = (0xfb4a4a, 0.15);
const CODE_ADD_MARK: (u32, f32) = (0x4afb4a, 0.18);
const CODE_FONT: f32 = 14.;
const CODE_HATCH: u32 = 0xdddad9;

type LineStyles = Vec<Vec<(Range<usize>, HighlightStyle)>>;

/// One Feature Studio's compare: the aligned rows and each side's colours.
struct CodeCompare {
    rows: Vec<CodeRow>,
    styles: [LineStyles; 2],
    /// The longest line of each side, in characters.
    widest: [usize; 2],
}

impl CodeCompare {
    fn new(a: &str, b: &str) -> Self {
        let widest = [a, b].map(|t| t.lines().map(|l| l.chars().count()).max().unwrap_or(0));
        Self { rows: noshape_model::compare::compare_code(a, b), styles: [crate::feature_studio::line_styles(a), crate::feature_studio::line_styles(b)], widest }
    }
}

/// The text a Feature Studio shows (saved edits included).
fn fs_text(f: &noshape_model::custom::FeatureStudio) -> &str {
    f.draft.as_deref().unwrap_or(&f.source)
}

/// `text` with tabs as 4 spaces (Ace), and `range` moved along.
fn expand_tabs(text: &str, ranges: &mut [Range<usize>]) -> String {
    if !text.contains('\t') {
        return text.to_string();
    }
    let mut map = Vec::with_capacity(text.len() + 1);
    let mut out = String::with_capacity(text.len() + 8);
    for (i, ch) in text.char_indices() {
        map.resize(i + 1, out.len());
        if ch == '\t' {
            out.push_str("    ");
        } else {
            out.push(ch);
        }
    }
    map.resize(text.len() + 1, out.len());
    for r in ranges {
        *r = map[r.start.min(text.len())]..map[r.end.min(text.len())];
    }
    out
}

/// The colour runs of one line plus the changed characters' background.
fn code_runs(len: usize, styles: &[(Range<usize>, HighlightStyle)], mark: Option<(Range<usize>, Hsla)>) -> Vec<(Range<usize>, HighlightStyle)> {
    let mut cuts = vec![0, len];
    for (r, _) in styles {
        cuts.extend([r.start.min(len), r.end.min(len)]);
    }
    if let Some((r, _)) = &mark {
        cuts.extend([r.start.min(len), r.end.min(len)]);
    }
    cuts.sort_unstable();
    cuts.dedup();
    cuts.windows(2)
        .filter_map(|w| {
            let (a, b) = (w[0], w[1]);
            let mut st = styles.iter().find(|(r, _)| r.start <= a && b <= r.end).map(|(_, s)| *s).unwrap_or_default();
            if let Some((r, c)) = &mark
                && r.start <= a
                && b <= r.end
            {
                st.background_color = Some(*c);
            }
            (st != HighlightStyle::default()).then_some((a..b, st))
        })
        .collect()
}

/// Diagonal stripes ("\", 5 px bands every 10 px) over `b`, phased on the
/// window so they run on across rows.
fn paint_hatch(b: Bounds<Pixels>, window: &mut Window) {
    let period = 10. * std::f32::consts::SQRT_2;
    let band = period / 2.;
    let (l, t, r, h) = (f32::from(b.left()), f32::from(b.top()), f32::from(b.right()), f32::from(b.size.height));
    window.with_content_mask(Some(ContentMask { bounds: b }), |window| {
        let mut k = ((l - h - t) / period).floor();
        loop {
            let x0 = t + k * period;
            if x0 > r {
                break;
            }
            let mut pb = PathBuilder::fill();
            pb.move_to(point(px(x0), px(t)));
            pb.line_to(point(px(x0 + band), px(t)));
            pb.line_to(point(px(x0 + band + h), px(t + h)));
            pb.line_to(point(px(x0 + h), px(t + h)));
            pb.close();
            if let Ok(path) = pb.build() {
                window.paint_path(path, rgb(CODE_HATCH));
            }
            k += 1.;
        }
    });
}

/// One side of a code row.
fn code_half(row: &CodeRow, base: bool, cc: &CodeCompare, x: f32, mono: &SharedString) -> AnyElement {
    let line = if base { row.base.as_ref() } else { row.target.as_ref() };
    let Some((n, text)) = line else {
        return div().flex_1().min_w_0().h(px(CODE_LINE_H)).child(canvas(|_, _, _| {}, |b, _, window, _| paint_hatch(b, window)).size_full()).into_any_element();
    };
    let (row_bg, mark_bg, bar, sign) = if base { (CODE_DEL_ROW, CODE_DEL_MARK, CODE_DEL_BAR, "-") } else { (CODE_ADD_ROW, CODE_ADD_MARK, CODE_ADD_BAR, "+") };
    let changed = row.changed.as_ref().map(|(a, b)| if base { a.clone() } else { b.clone() });
    let side = &cc.styles[usize::from(!base)];
    let mut ranges: Vec<Range<usize>> = side.get(n - 1).map(|v| v.iter().map(|(r, _)| r.clone()).collect()).unwrap_or_default();
    ranges.extend(changed.clone());
    let shown = expand_tabs(text, &mut ranges);
    let mark_bg: Hsla = Rgba { a: mark_bg.1, ..rgb(mark_bg.0) }.into();
    let mark = changed.as_ref().map(|_| ranges.pop().unwrap_or_default()).filter(|r| !r.is_empty()).map(|r| (r, mark_bg));
    let styles: Vec<(Range<usize>, HighlightStyle)> = side.get(n - 1).map(|v| v.iter().zip(&ranges).map(|((_, s), r)| (r.clone(), *s)).collect()).unwrap_or_default();
    let runs = code_runs(shown.len(), &styles, mark);
    h_flex()
        .flex_1()
        .min_w_0()
        .h(px(CODE_LINE_H))
        .overflow_hidden()
        .when(changed.is_some(), |d| d.bg(rgb(row_bg)))
        .child(
            h_flex()
                .relative()
                .w(px(CODE_GUTTER_W))
                .h_full()
                .flex_none()
                .bg(rgb(if changed.is_some() { row_bg } else { CODE_GUTTER }))
                .text_color(rgb(0x333333))
                .when(changed.is_some(), |d| d.child(div().absolute().left_0().top_0().bottom_0().w(px(3.)).bg(rgb(bar))))
                .child(div().flex_1().flex().justify_end().child(n.to_string()))
                .child(div().w(px(16.)).flex_none().pl(px(4.)).when(changed.is_some(), |d| d.child(sign))),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .h_full()
                .overflow_hidden()
                .pl(px(4.))
                .child(div().ml(px(-x)).whitespace_nowrap().font_family(mono.clone()).child(StyledText::new(shown).with_highlights(runs))),
        )
        .into_any_element()
}

/// A Feature Studio in compare mode (help fs-compare-01): the two sources
/// side by side under their entries' names, scrolling together.
fn render_code(s: &CompareSession, tab: TabId, a: &noshape_model::custom::FeatureStudio, b: &noshape_model::custom::FeatureStudio, cx: &App) -> AnyElement {
    let cc = s.code.borrow_mut().entry(tab).or_insert_with(|| Rc::new(CodeCompare::new(fs_text(a), fs_text(b)))).clone();
    let mono: SharedString = cx.theme().mono_font_family.clone();
    let scroll = s.code_scroll.clone();
    let xs = s.code_x.clone();
    let area: Rc<Cell<Bounds<Pixels>>> = Default::default();
    let header = |t: String| div().flex_1().min_w_0().flex().justify_center().truncate().child(t);
    let list = {
        let (cc, xs, mono) = (cc.clone(), xs.clone(), mono.clone());
        uniform_list("fs-compare-rows", cc.rows.len(), move |range, _, _| {
            let [xb, xt] = xs.get();
            range
                .map(|i| {
                    let row = &cc.rows[i];
                    h_flex()
                        .w_full()
                        .h(px(CODE_LINE_H))
                        .child(code_half(row, true, &cc, xb, &mono))
                        .child(div().w(px(CODE_RULER_W)).flex_none())
                        .child(code_half(row, false, &cc, xt, &mono))
                        .child(div().w(px(CODE_RULER_W)).flex_none())
                })
                .collect()
        })
        .track_scroll(&scroll)
        .size_full()
    };
    // Rulers and horizontal thumbs, drawn over the list; clicks / drags on a
    // ruler scroll there.
    let overlay = {
        let (cc, scroll, xs, mono, area) = (cc.clone(), scroll.clone(), xs.clone(), mono.clone(), area.clone());
        canvas(
            |_, _, _| {},
            move |b, _, window, _| {
                area.set(b);
                let n = cc.rows.len().max(1) as f32;
                let (h, w) = (f32::from(b.size.height), f32::from(b.size.width));
                let state = scroll.0.borrow();
                let (off, max) = (f32::from(-state.base_handle.offset().y), f32::from(state.base_handle.max_offset().y));
                drop(state);
                let content = n * CODE_LINE_H;
                let pane = (w - 2. * CODE_RULER_W) / 2.;
                for x in [b.left() + px(pane), b.right() - px(CODE_RULER_W)] {
                    let rb = Bounds::new(point(x, b.top()), size(px(CODE_RULER_W), b.size.height));
                    window.paint_quad(fill(rb, rgb(0xffffff)));
                    if content > h {
                        let th = (h * h / content).max(20.);
                        let ty = if max > 0. { off / max * (h - th) } else { 0. };
                        window.paint_quad(fill(Bounds::new(point(x + px(1.), b.top() + px(ty)), size(px(CODE_RULER_W - 2.), px(th))), hsla(0., 0., 0.5, 0.6)).corner_radii(px(3.)));
                    }
                }
                // Horizontal thumbs at each pane's bottom when its lines are wider.
                let fid = window.text_system().resolve_font(&font(mono.clone()));
                let cw = window.text_system().advance(fid, px(CODE_FONT), 'm').map(|s| f32::from(s.width)).unwrap_or(7.8);
                let text_w = pane - CODE_GUTTER_W - 4.;
                for (k, left) in [b.left(), b.left() + px(pane + CODE_RULER_W)].into_iter().enumerate() {
                    let full = cc.widest[k] as f32 * cw + 8.;
                    if full > text_w {
                        let tw = (text_w * text_w / full).max(20.);
                        let tx = xs.get()[k] / (full - text_w) * (text_w - tw);
                        window.paint_quad(fill(Bounds::new(point(left + px(CODE_GUTTER_W + 4. + tx), b.bottom() - px(9.)), size(px(tw), px(8.))), hsla(0., 0., 0.5, 0.6)).corner_radii(px(3.)));
                    }
                }
                let ruler_at = move |p: Point<Pixels>| {
                    let x = f32::from(p.x - b.left());
                    b.contains(&p) && ((x >= pane && x < pane + CODE_RULER_W) || x >= w - CODE_RULER_W)
                };
                let jump = {
                    let scroll = scroll.clone();
                    move |p: Point<Pixels>| {
                        let state = scroll.0.borrow();
                        let max = state.base_handle.max_offset().y;
                        let frac = (f32::from(p.y - b.top()) / h).clamp(0., 1.);
                        state.base_handle.set_offset(point(px(0.), -(max * frac)));
                    }
                };
                let j2 = jump.clone();
                window.on_mouse_event(move |e: &MouseDownEvent, phase, window, _| {
                    if phase == DispatchPhase::Bubble && e.button == MouseButton::Left && ruler_at(e.position) {
                        j2(e.position);
                        window.refresh();
                    }
                });
                window.on_mouse_event(move |e: &MouseMoveEvent, phase, window, _| {
                    if phase == DispatchPhase::Bubble && e.pressed_button == Some(MouseButton::Left) && ruler_at(e.position) {
                        jump(e.position);
                        window.refresh();
                    }
                });
            },
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full()
    };
    // Sideways wheel (or Shift + wheel) scrolls the pane under the pointer.
    let on_wheel = {
        let (cc, area, mono) = (cc.clone(), area.clone(), mono.clone());
        move |e: &ScrollWheelEvent, window: &mut Window, _: &mut App| {
            let d = e.delta.pixel_delta(px(CODE_LINE_H));
            let dx = f32::from(if e.modifiers.shift && d.x == px(0.) { d.y } else { d.x });
            if dx == 0. {
                return;
            }
            let b = area.get();
            let pane = (f32::from(b.size.width) - 2. * CODE_RULER_W) / 2.;
            let k = usize::from(f32::from(e.position.x - b.left()) > pane + CODE_RULER_W);
            let fid = window.text_system().resolve_font(&font(mono.clone()));
            let cw = window.text_system().advance(fid, px(CODE_FONT), 'm').map(|s| f32::from(s.width)).unwrap_or(7.8);
            let max = (cc.widest[k] as f32 * cw + 8. - (pane - CODE_GUTTER_W - 4.)).max(0.);
            let mut x = xs.get();
            x[k] = (x[k] - dx).clamp(0., max);
            xs.set(x);
            window.refresh();
        }
    };
    v_flex()
        .size_full()
        .pt(px(crate::ui::TOOLBAR_H))
        .bg(rgb(0xffffff))
        .child(
            h_flex()
                .h(px(CODE_HEADER_H))
                .flex_none()
                .text_size(px(14.))
                .line_height(px(21.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(rgb(0x333333))
                .child(header(s.base_label.clone()))
                .child(div().w(px(CODE_RULER_W)).flex_none())
                .child(header(s.target_label.clone()))
                .child(div().w(px(CODE_RULER_W)).flex_none()),
        )
        .child(div().id("fs-compare").flex_1().min_h_0().relative().text_size(px(CODE_FONT)).line_height(px(CODE_LINE_H)).font_family(mono).on_scroll_wheel(on_wheel).child(list).child(overlay))
        .into_any_element()
}
