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
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_model::compare::{ChangeKind, FeatureCompare};
use noshape_model::document::{Document, TabContent, TabId};
use noshape_model::reference::FeatureId;
use noshape_store::MicroversionId;
use std::collections::HashMap;
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
/// A changed field's outline in the compare dialogs (live cp-921).
const CHANGED: u32 = 0xfab601;

/// History rows (live 952): base light blue, target light pink.
pub const BASE_ROW: u32 = 0xb2ddf6;
pub const TARGET_ROW: u32 = 0xf6e0e1;

pub struct CompareSession {
    /// The history rows compared (None: the current workspace).
    pub base_mv: Option<MicroversionId>,
    pub target_mv: Option<MicroversionId>,
    pub base_label: String,
    pub target_label: String,
    pub base: Arc<Document>,
    pub target: Arc<Document>,
    /// Graphics blend: 0 = base, 1 = target.
    pub blend: f32,
    pub only_differences: bool,
    pub selected: Option<FeatureId>,
}

#[derive(Default)]
struct Sessions(HashMap<EntityId, CompareSession>);
impl Global for Sessions {}

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

/// Enter compare mode: `base` is what the window shows, `target` the row compared.
pub fn start(doc: &Entity<DocModel>, base: (Option<MicroversionId>, String, Document), target: (Option<MicroversionId>, String, Document), cx: &mut App) {
    let s = CompareSession { base_mv: base.0, target_mv: target.0, base_label: base.1, target_label: target.1, base: Arc::new(base.2), target: Arc::new(target.2), blend: 0., only_differences: true, selected: None };
    cx.default_global::<Sessions>().0.insert(doc.entity_id(), s);
    changed(doc, cx);
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
        std::mem::swap(&mut s.base_mv, &mut s.target_mv);
        std::mem::swap(&mut s.base_label, &mut s.target_label);
        std::mem::swap(&mut s.base, &mut s.target);
        s.selected = None;
    });
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

/// The (base, target) history rows (None: the current workspace).
pub fn rows(doc: &Entity<DocModel>, cx: &App) -> Option<(Option<MicroversionId>, Option<MicroversionId>)> {
    session(doc, cx).map(|s| (s.base_mv, s.target_mv))
}

/// The graphics blend (0 = base, 1 = target) while comparing.
pub fn blend(doc: &Entity<DocModel>, cx: &App) -> Option<f32> {
    session(doc, cx).map(|s| s.blend)
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

/// The dot after a tab's name, with its tooltip (live 952: 10 px).
pub fn tab_dot(id: impl Into<ElementId>, status: TabStatus, base_label: &str) -> Option<AnyElement> {
    let dot = div().id(id).size(px(10.)).flex_none().rounded_full();
    let (dot, tip) = match status {
        TabStatus::Same => return None,
        TabStatus::Differs => (dot.bg(rgb(DOT_YELLOW)), "Compare has found differences.".to_string()),
        TabStatus::NotSupported => (dot.bg(rgb(0xffffff)).border_1().border_color(rgb(0xc0c0c0)), "Compare not supported for this tab.".to_string()),
        TabStatus::OnlyInBase => (dot.bg(rgb(TARGET_RED)), format!("Tab only exists in '{base_label}'.")),
    };
    Some(dot.tooltip(crate::ui::tooltip(tip)).into_any_element())
}

/// The base label (for tab tooltips).
pub fn base_label(doc: &Entity<DocModel>, cx: &App) -> Option<String> {
    session(doc, cx).map(|s| s.base_label.clone())
}

/// "NOT COMPARABLE / Tab only exists in 'Main'." in a tab's graphics area.
pub fn render_not_comparable(base_label: &str) -> AnyElement {
    v_flex()
        .size_full()
        .items_center()
        .justify_center()
        .gap(px(8.))
        .text_color(rgb(0xb0b0b0))
        .font_weight(FontWeight::SEMIBOLD)
        .child(div().text_size(px(16.)).child("NOT COMPARABLE"))
        .child(div().text_size(px(16.)).child(format!("Tab only exists in '{base_label}'.")))
        .into_any_element()
}

/// The centred "Comparing Main and V3 [Reverse compare] (?)" bar in the
/// toolbar row (live 953: 372 px, white, `#ccc` border, rounded below).
pub fn render_bar(doc: &Entity<DocModel>, cx: &App) -> Option<AnyElement> {
    let s = session(doc, cx)?;
    let pal = p();
    let word = |t: String, c: u32| div().text_size(px(14.)).font_weight(FontWeight::BOLD).text_color(rgb(c)).child(t);
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
            .child(word(s.base_label.clone(), BAR_NAME))
            .child(word("and".into(), 0x333333))
            .child(word(s.target_label.clone(), BAR_NAME))
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
    let rows: Vec<FeatureCompare> = noshape_model::compare::compare_features(a, b).into_iter().filter(|r| !s.only_differences || r.status.is_some()).collect();
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
            let (in_base, in_target) = (r.status != Some(ChangeKind::Added), r.status != Some(ChangeKind::Removed));
            let mark = match r.status {
                Some(ChangeKind::Modified) => Some(("≠", if on { 0xffffff } else { MARK_GOLD })),
                Some(ChangeKind::Removed) => Some((">", TARGET_RED)),
                Some(ChangeKind::Added) => Some(("<", MARK_GREEN)),
                None => None,
            };
            let name = |show: bool| {
                h_flex()
                    .w(px(170.))
                    .flex_none()
                    .gap(px(4.))
                    .when(show, |c| {
                        c.child(svg().path(crate::part_studio::feature_icon(&r.type_id)).size(px(16.)).flex_none().text_color(pal.text))
                            .child(div().min_w_0().truncate().text_color(if sketch { rgb(0xaaaaaa) } else { pal.text }).child(r.name.clone()))
                    })
            };
            h_flex()
                .id(SharedString::from(format!("compare-feature-{}", id.0)))
                .h(px(29.))
                .flex_none()
                .px(px(6.))
                .border_b_1()
                .border_color(rgb(0xdee2e6))
                .text_size(px(13.))
                .cursor_pointer()
                .when(on, |d| d.bg(rgb(BASE_ROW)))
                .when(!on, |d| d.hover(|d| d.bg(pal.panel_hover)))
                .child(name(in_base))
                .child(div().w(px(26.)).flex_none().flex().justify_center().font_weight(FontWeight::BOLD).text_size(px(15.)).children(mark.map(|(m, c)| div().text_color(rgb(c)).child(m))))
                .child(name(in_target))
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
