//! Versions and history › Merge into current workspace… (help "Merging",
//! images merge-dialog-*, protections-4.1-no-merge-01): Onshape's Merge
//! dialog. "Merging changes into [Main] from [B1]"; the Overall merge
//! strategy cards (Keep [target] · Merge changes · Replace with [source],
//! "N tabs have changes", "Compare with base ↗"); the Tabs table (each
//! changed tab's change in the target and in the source, with ✓ kept / ✗
//! discarded / ! gone after the merge, and its own Tab merge strategy,
//! yellow when it differs from the overall one); the collapsible "N tabs
//! have no changes" group; notices for an up-to-date target and for a
//! protected one; ? · Merge · Cancel.
//!
//! The merge itself is `Session::merge_picked`: Keep = the target's tab,
//! Replace = the source's, Merge changes = the three-way merge.

use crate::compare_mode::Side;
use crate::doc_model::{DocEvent, DocModel};
use crate::os_ui as os;
use crate::theme::p;
use gpui_kit::component::Disableable as _;
use gpui_kit::component::WindowExt as _;
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_model::document::{Tab, TabContent};
use noshape_store::{BranchId, MergePick, MicroversionId};
use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::rc::Rc;

const W: f32 = 948.;
/// Table columns (help merge-dialog-full-01): name at 26, the two change
/// columns at 207 and 363, the strategy select from 519.
const COL_NAME: f32 = 205.;
const COL_CHANGE: f32 = 176.;
const ROW_H: f32 = 30.;
const STRIPE: u32 = 0xf5f5f5;
const RULE: u32 = 0xdddddd;
const GREEN: u32 = 0x2e9f37;
const RED: u32 = 0xbd3039;
const BLUE: u32 = 0x1651b0;
/// A tab whose strategy differs from the overall one.
const DIFFERS: u32 = 0xfab601;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Strategy {
    Keep,
    Merge,
    Replace,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Change {
    None,
    Edited,
    New,
    Deleted,
}

impl Change {
    fn of(base: Option<&Vec<u8>>, side: Option<&Vec<u8>>) -> Self {
        match (base, side) {
            (None, None) => Change::None,
            (None, Some(_)) => Change::New,
            (Some(_), None) => Change::Deleted,
            (Some(a), Some(b)) if a == b => Change::None,
            _ => Change::Edited,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Change::None => "No changes",
            Change::Edited => "Edited",
            Change::New => "New tab",
            Change::Deleted => "Deleted",
        }
    }
}

/// One tab in the table.
struct Row {
    /// Its blob name ("tab/<id>").
    blob: String,
    name: String,
    icon: &'static str,
    ours: Change,
    theirs: Change,
    /// "Merge changes from both" is offered: an edited Part Studio or
    /// Assembly (help: not drawings, not created / deleted tabs).
    mergeable: bool,
    /// Changes in separate branches can't be merged for this tab type (ⓘ).
    unmergeable_type: bool,
}

impl Row {
    fn options(&self) -> &'static [Strategy] {
        if self.mergeable { &[Strategy::Keep, Strategy::Merge, Strategy::Replace] } else { &[Strategy::Keep, Strategy::Replace] }
    }

    /// The tab's strategy under the overall one (help merge-dialog-full-01:
    /// with "Merge changes", a tab changed only in the source is replaced,
    /// one changed only in the target kept).
    fn default_for(&self, overall: Strategy) -> Strategy {
        match overall {
            // Help merge-dialog-full-01: merged when both sides edited it.
            Strategy::Merge if self.mergeable && self.ours == Change::Edited && self.theirs == Change::Edited => Strategy::Merge,
            Strategy::Merge if self.theirs != Change::None => Strategy::Replace,
            Strategy::Merge => Strategy::Keep,
            s => s,
        }
    }
}

struct Plan {
    target: String,
    /// The workspaces' lanes in the history graph (chip colours).
    target_lane: usize,
    source: String,
    source_lane: usize,
    /// The merge base's name ("V1", else its change's description).
    base_label: String,
    from: MicroversionId,
    from_branch: Option<BranchId>,
    target_side: Side,
    source_side: Side,
    base: MicroversionId,
    changed: Vec<Row>,
    unchanged: Vec<(String, &'static str)>,
    /// The source is already in the target.
    up_to_date: bool,
    /// The target is protected and changed since the base (help
    /// "Protected workspaces": merges need an unchanged protected
    /// workspace).
    protected_with_changes: bool,
}

struct State {
    overall: Strategy,
    per_tab: HashMap<String, Strategy>,
    unchanged_open: bool,
}

fn tab_icon(c: &TabContent) -> &'static str {
    match c {
        TabContent::PartStudio(_) => "part-studio",
        TabContent::Assembly(_) => "assembly",
        TabContent::Drawing(_) => "drawing",
        TabContent::FeatureStudio(_) => "feature-studio",
        TabContent::MaterialLibrary(_) => "material-library",
        TabContent::File(f) => crate::file_tab_view::file_icon(f),
        TabContent::VariableStudio(_) => "variable-studio",
    }
}

fn plan(doc: &Entity<DocModel>, source: Side, source_name: String, cx: &App) -> anyhow::Result<Plan> {
    let s = &doc.read(cx).session;
    let store = &s.store;
    let ours = s.head()?;
    let (from, from_branch) = match source {
        Side::Workspace(b) => (store.head(b)?, Some(b)),
        Side::Entry(mv) => (mv, None),
    };
    let base = store.merge_base(ours, from)?.ok_or_else(|| anyhow::anyhow!("These workspaces share no history."))?;
    let (bb, ob, tb) = (store.read_all(base)?, store.read_all(ours)?, store.read_all(from)?);
    let tab_of = |m: &BTreeMap<String, Vec<u8>>, n: &str| m.get(n).and_then(|b| serde_json::from_slice::<Tab>(b).ok());
    // The target's tab order, then tabs only in the source.
    let current = &s.doc;
    let mut names: Vec<String> = current.tabs.iter().map(|t| noshape_model::document::Document::blob_name(t.id)).collect();
    for n in bb.keys().chain(ob.keys()).chain(tb.keys()).filter(|n| n.starts_with("tab/")) {
        if !names.contains(n) {
            names.push(n.clone());
        }
    }
    let (mut changed, mut unchanged) = (Vec::new(), Vec::new());
    for n in names {
        let Some(tab) = tab_of(&ob, &n).or_else(|| tab_of(&tb, &n)).or_else(|| tab_of(&bb, &n)) else { continue };
        let (oc, tc) = (Change::of(bb.get(&n), ob.get(&n)), Change::of(bb.get(&n), tb.get(&n)));
        let icon = tab_icon(&tab.content);
        if oc == Change::None && tc == Change::None {
            unchanged.push((tab.name, icon));
            continue;
        }
        let drawing = matches!(tab.content, TabContent::Drawing(_));
        let kind_merges = matches!(tab.content, TabContent::PartStudio(_) | TabContent::Assembly(_));
        let both = oc == Change::Edited && tc == Change::Edited;
        // Live (qa5 DOC2, mg-903): an edited Part Studio / Assembly offers
        // "Merge changes from both" (not a created or deleted tab).
        let edited_only = [oc, tc].iter().all(|c| matches!(c, Change::None | Change::Edited));
        changed.push(Row { blob: n, name: tab.name, icon, ours: oc, theirs: tc, mergeable: kind_merges && edited_only, unmergeable_type: drawing && both });
    }
    let branch_name = |b: BranchId| store.branch(b).map(|x| x.name).unwrap_or_default();
    // Lanes as the graph orders branches: the default first, then by creation.
    let branches = store.list_branches()?;
    let lane = |b: BranchId| branches.iter().position(|x| x.id == b).unwrap_or(0);
    let source_branch = from_branch.or_else(|| store.microversion(from).ok().map(|m| m.branch)).unwrap_or(s.branch);
    let base_label = store.list_versions()?.into_iter().find(|v| v.microversion == base).map(|v| v.name).unwrap_or_else(|| store.microversion(base).map(|m| m.description).unwrap_or_default());
    Ok(Plan {
        target: branch_name(s.branch),
        target_lane: lane(s.branch),
        source: source_name,
        source_lane: lane(source_branch),
        base_label,
        from,
        from_branch,
        target_side: Side::Workspace(s.branch),
        source_side: source,
        base,
        up_to_date: base == from,
        protected_with_changes: s.is_protected() && changed.iter().any(|r| r.ours != Change::None),
        changed,
        unchanged,
    })
}

/// A workspace name in a chip, in its history lane's colours (live dom
/// ui2-merge-dialog: Main rgb(208,219,229) / #1651b0, B1 rgb(195,242,228)
/// / #3ad4a7, i.e. the lane colour 30 % over white).
fn chip(name: &str, lane: usize, size: f32) -> Div {
    let border = crate::history_panel::lane_color(lane);
    let fill = if lane == 0 {
        rgb(0xd0dbe5)
    } else {
        let c = rgb(border);
        let mix = |v: f32| v + (1. - v) * 0.7;
        Rgba { r: mix(c.r), g: mix(c.g), b: mix(c.b), a: 1. }
    };
    div().px(px(3.)).rounded(px(3.)).border_1().border_color(rgb(border)).bg(fill).text_size(px(size)).font_weight(FontWeight::MEDIUM).text_color(rgb(0x333333)).child(name.to_string())
}

/// A strategy's icon for a row (icons2 from live: Keep shows the discard ×
/// only when the source changed the tab, Replace only when the target did).
fn strategy_icon(s: Strategy, row: &Row) -> &'static str {
    match s {
        Strategy::Keep if row.theirs != Change::None => "merge-keep",
        Strategy::Keep => "merge-keep-plain",
        Strategy::Merge => "merge-both",
        Strategy::Replace if row.ours != Change::None => "merge-replace",
        Strategy::Replace => "merge-replace-plain",
    }
}

fn strategy_label(s: Strategy, plan: &Plan) -> String {
    match s {
        Strategy::Keep => format!("Keep {}", plan.target),
        Strategy::Merge => "Merge changes from both".into(),
        Strategy::Replace => format!("Replace with {}", plan.source),
    }
}

fn tabs_have_changes(n: usize) -> String {
    if n == 1 { "1 tab has changes".into() } else { format!("{n} tabs have changes") }
}

/// Open the Merge dialog: merge `source` (another workspace, or a version)
/// into the current workspace.
pub fn open(doc: Entity<DocModel>, source: Side, source_name: String, window: &mut Window, cx: &mut App) {
    let plan = match plan(&doc, source, source_name, cx) {
        Ok(p) => Rc::new(p),
        Err(e) => {
            doc.update(cx, |_, cx| cx.emit(DocEvent::Error(format!("{e:#}"))));
            return;
        }
    };
    let merge_offered = plan.changed.iter().any(|r| r.mergeable);
    let state = Rc::new(RefCell::new(State { overall: if merge_offered { Strategy::Merge } else { Strategy::Keep }, per_tab: HashMap::new(), unchanged_open: true }));
    window.open_dialog(cx, move |dialog, _, cx| {
        let pal = p();
        let (plan, state) = (plan.clone(), state.clone());
        let st = state.borrow();
        let effective = |r: &Row| st.per_tab.get(&r.blob).copied().unwrap_or_else(|| r.default_for(st.overall));
        // Something would change: a source change taken, unless the target
        // already has it.
        let updates = plan.changed.iter().any(|r| match effective(r) {
            Strategy::Keep => false,
            Strategy::Merge => true,
            Strategy::Replace => r.theirs != Change::None || r.ours != Change::None,
        });
        let can_merge = updates && !plan.protected_with_changes && (!plan.up_to_date || plan.changed.iter().any(|r| effective(r) == Strategy::Replace));
        let title = h_flex()
            .w(px(W - 2.))
            .h(px(47.))
            .pl(px(10.))
            .pr(px(12.))
            .gap(px(6.))
            .border_b_1()
            .border_color(rgb(os::MODAL_DIVIDER))
            // Live: 400 14px `#333`, the chips 500 18px.
            .text_size(px(14.))
            .text_color(pal.text)
            .child("Merging changes into")
            .child(chip(&plan.target, plan.target_lane, 18.))
            .child("from")
            .child(chip(&plan.source, plan.source_lane, 18.))
            .child(div().flex_1())
            .child(div().flex_none().child(gpui_kit::component::dialog::DialogClose::new().child(div().id("modal-close").size(px(22.)).flex().items_center().justify_center().cursor_pointer().text_color(pal.text_faint).child(gpui_kit::component::Icon::empty().data(os::GLYPH_CLOSE).size(px(16.))))));
        // Overall merge strategy: Keep · Merge changes (when a tab can merge) · Replace.
        let mut cards: Vec<Strategy> = vec![Strategy::Keep];
        if merge_offered {
            cards.push(Strategy::Merge);
        }
        cards.push(Strategy::Replace);
        let card_w = 656. / cards.len() as f32;
        let card = |s: Strategy| {
            let on = st.overall == s;
            let color = if on { rgb(BLUE) } else { rgb(0x333333) };
            let icon = match s {
                Strategy::Keep if merge_offered => "icons/merge-card-keep-x.svg",
                Strategy::Keep => "icons/merge-card-keep.svg",
                Strategy::Merge => "icons/merge-card-merge.svg",
                Strategy::Replace => "icons/merge-card-replace.svg",
            };
            let first = match s {
                Strategy::Keep => h_flex().gap(px(4.)).child("Keep").child(chip(&plan.target, plan.target_lane, 13.)),
                Strategy::Merge => h_flex().child("Merge changes"),
                Strategy::Replace => h_flex().gap(px(4.)).child("Replace with").child(chip(&plan.source, plan.source_lane, 13.)),
            };
            let count = match s {
                Strategy::Keep => tabs_have_changes(plan.changed.iter().filter(|r| r.ours != Change::None).count()),
                Strategy::Merge => format!("Combine all changes to {} {}", plan.changed.len(), if plan.changed.len() == 1 { "tab" } else { "tabs" }),
                Strategy::Replace => tabs_have_changes(plan.changed.iter().filter(|r| r.theirs != Change::None).count()),
            };
            // "Compare with base ↗": compare mode between the merge base and that side.
            let link = (s != Strategy::Merge).then(|| {
                let (doc, plan) = (doc.clone(), plan.clone());
                let side = if s == Strategy::Keep { (plan.target_side, plan.target.clone()) } else { (plan.source_side, plan.source.clone()) };
                div()
                    .id(SharedString::from(format!("merge-compare-{s:?}")))
                    .cursor_pointer()
                    .text_color(rgb(BLUE))
                    .hover(|d| d.underline())
                    .child("Compare with base ↗")
                    .on_click(move |_, window, cx| {
                        cx.stop_propagation();
                        window.close_dialog(cx);
                        // Live mg-909: that side against the merge base.
                        if let Err(e) = crate::compare_mode::compare(&doc, side.clone(), (Side::Entry(plan.base), plan.base_label.clone()), cx) {
                            doc.update(cx, |_, cx| cx.emit(DocEvent::Error(format!("{e:#}"))));
                        }
                    })
            });
            let state = state.clone();
            h_flex()
                .id(SharedString::from(format!("merge-card-{s:?}")))
                .w(px(card_w))
                .h_full()
                .items_start()
                .pt(px(10.))
                .pl(px(20.))
                .gap(px(16.))
                .cursor_pointer()
                .border_b(px(3.))
                .border_color(if on { rgb(BLUE).into() } else { gpui_kit::transparent_white() })
                .child(svg().path(icon).w(px(24.)).h(px(48.)).flex_none().text_color(color))
                .child(v_flex().text_size(px(13.)).line_height(px(20.)).text_color(color).child(first).child(div().italic().child(count)).children(link))
                .on_click(move |_, window, _| {
                    let mut st = state.borrow_mut();
                    st.overall = s;
                    st.per_tab.clear();
                    window.refresh();
                })
        };
        let overall = h_flex()
            .items_start()
            .pt(px(9.))
            .child(div().w(px(171.)).flex_none().pt(px(2.)).text_size(px(18.)).line_height(px(27.)).font_weight(FontWeight::MEDIUM).text_color(pal.text).child("Overall merge strategy"))
            .child(h_flex().w(px(656.)).h(px(80.)).border_1().border_color(rgb(0xcccccc)).children(cards.iter().map(|s| card(*s))));
        // The table.
        let change_cell = |c: Change, kept: Option<bool>| {
            let cell = h_flex().w(px(COL_CHANGE)).flex_none().gap(px(4.)).text_size(px(13.)).text_color(pal.text);
            match (c, kept) {
                (Change::None, _) => cell.pl(px(20.)).child(div().italic().child("No changes")),
                // The tab won't exist after the merge.
                (_, None) => cell.child(div().w(px(16.)).flex().justify_center().font_weight(FontWeight::BOLD).text_color(rgb(0xd88c00)).child("!")).child(c.label()),
                (_, Some(true)) => cell.child(os::glyph(os::GLYPH_CHECK_BOLD, 14., rgb(GREEN))).child(c.label()),
                (_, Some(false)) => cell.child(os::glyph(os::GLYPH_CROSS_BOLD, 12., rgb(RED))).child(c.label()),
            }
        };
        let rows = plan.changed.iter().enumerate().map(|(i, r)| {
            let eff = effective(r);
            // Whether each side's change survives, None: the tab is gone.
            let exists = match eff {
                Strategy::Keep => r.ours != Change::Deleted && !(r.ours == Change::None && r.theirs == Change::New),
                Strategy::Replace => r.theirs != Change::Deleted && !(r.theirs == Change::None && r.ours == Change::New),
                Strategy::Merge => true,
            };
            let kept = |mine: bool| exists.then_some(match eff {
                Strategy::Keep => mine,
                Strategy::Replace => !mine,
                Strategy::Merge => true,
            });
            let differs = eff != r.default_for(st.overall);
            let refused = plan.protected_with_changes;
            let (state, blob, options) = (state.clone(), r.blob.clone(), r.options());
            let icons: Vec<&'static str> = options.iter().map(|o| strategy_icon(*o, r)).collect();
            let plan2 = plan.clone();
            let probe: Rc<std::cell::Cell<Bounds<Pixels>>> = Default::default();
            let probe2 = probe.clone();
            let select = h_flex()
                .id(SharedString::from(format!("merge-strategy-{i}")))
                .relative()
                .flex_1()
                .h(px(ROW_H - 4.))
                .px(px(6.))
                .gap(px(6.))
                .cursor_pointer()
                // Live os-select: a bordered button; a tab whose strategy
                // differs from the overall one gets a yellow corner (mg-908).
                .rounded(px(3.))
                .border_1()
                .border_color(rgb(0xaaaaaa))
                .bg(crate::theme::p().panel)
                // Greyed while a protected target refuses the merge (live wp-937).
                .when(plan.protected_with_changes, |d| d.opacity(0.5))
                .when(differs, |d| {
                    d.child(
                        canvas(|_, _, _| {}, |b, _, window, _| {
                            let mut pb = PathBuilder::fill();
                            pb.move_to(b.origin);
                            pb.line_to(point(b.left() + px(6.), b.top()));
                            pb.line_to(point(b.left(), b.top() + px(6.)));
                            pb.close();
                            if let Ok(path) = pb.build() {
                                window.paint_path(path, rgb(DIFFERS));
                            }
                        })
                        .absolute()
                        .top_0()
                        .left_0()
                        .size(px(6.)),
                    )
                })
                .text_size(px(13.))
                .text_color(pal.text)
                .child(canvas(move |b, _, _| probe.set(b), |_, _, _, _| {}).absolute().top_0().left_0().size_full())
                .child(crate::assets::icon(strategy_icon(eff, r), 24., cx))
                .child(div().flex_1().truncate().child(strategy_label(eff, &plan)))
                .child(crate::ui::glyph("ui-caret-down", 10., pal.text))
                .on_click(move |_, window, cx| {
                    if refused {
                        return;
                    }
                    let b = probe2.get();
                    let (state, blob, plan, icons) = (state.clone(), blob.clone(), plan2.clone(), icons.clone());
                    let menu = crate::os_menu::OsMenu::build(window, cx, move |mut m, _, _| {
                        m = m.select_list(f32::from(b.size.width));
                        for s in options {
                            let (state, blob) = (state.clone(), blob.clone());
                            m = m.item(crate::os_menu::OsMenuItem::new(strategy_label(*s, &plan)).icon_name(icons[options.iter().position(|o| o == s).unwrap_or(0)]).checked(*s == eff).on_click(move |_, window, _| {
                                state.borrow_mut().per_tab.insert(blob.clone(), *s);
                                window.refresh();
                            }));
                        }
                        m
                    });
                    crate::os_menu::open_anchored(point(b.origin.x, b.origin.y + b.size.height), Anchor::TopLeft, menu, window, cx);
                });
            h_flex()
                .h(px(ROW_H))
                .pl(px(18.))
                .pr(px(4.))
                .when(i % 2 == 0, |d| d.bg(rgb(STRIPE)))
                .child(
                    h_flex()
                        .w(px(COL_NAME))
                        .flex_none()
                        .gap(px(4.))
                        .text_size(px(13.))
                        .text_color(pal.text)
                        .child(crate::assets::icon(r.icon, 18., cx))
                        .child(div().min_w_0().truncate().child(r.name.clone()))
                        // Live mg-903: ⓘ "For this tab type, edits from separate
                        // branches cannot be merged." and "Open in <source>".
                        .when(r.unmergeable_type, |d| {
                            let (doc, side) = (doc.clone(), plan.source_side);
                            d.child(div().id(SharedString::from(format!("merge-info-{i}"))).flex_none().tooltip(crate::ui::tooltip("For this tab type, edits from separate branches cannot be merged.")).child(crate::ui::glyph("info", 14., rgb(0x4d8ccf))))
                                .child(
                                    div()
                                        .id(SharedString::from(format!("merge-open-{i}")))
                                        .flex_none()
                                        .cursor_pointer()
                                        .tooltip(crate::ui::tooltip(format!("Open in {}", plan.source)))
                                        .child(crate::assets::icon("open-external", 14., cx))
                                        .on_click(move |_, window, cx| {
                                            window.close_dialog(cx);
                                            let r = doc.update(cx, |d, cx| -> anyhow::Result<()> {
                                                match side {
                                                    Side::Workspace(b) => d.session.switch_branch(b)?,
                                                    Side::Entry(mv) => d.session.view_microversion(Some(mv))?,
                                                }
                                                d.reloaded(cx);
                                                Ok(())
                                            });
                                            if let Err(e) = r {
                                                doc.update(cx, |_, cx| cx.emit(DocEvent::Error(format!("{e:#}"))));
                                            }
                                        }),
                                )
                        }),
                )
                .child(change_cell(r.ours, kept(true)))
                .child(change_cell(r.theirs, kept(false)))
                .child(select)
                .into_any_element()
        });
        // Live: 600 16px, the first cell 18 px in.
        let header = h_flex()
            .h(px(36.))
            .pl(px(18.))
            .border_t_1()
            .border_color(rgb(RULE))
            .text_size(px(16.))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(pal.text)
            .child(div().w(px(COL_NAME)).flex_none().child(tabs_have_changes(plan.changed.len())))
            .child(h_flex().w(px(COL_CHANGE)).flex_none().gap(px(4.)).child("Changes in").child(chip(&plan.target, plan.target_lane, 14.)))
            .child(h_flex().w(px(COL_CHANGE)).flex_none().gap(px(4.)).child("Changes in").child(chip(&plan.source, plan.source_lane, 14.)))
            .child(div().child("Tab merge strategy"));
        let n_same = plan.unchanged.len();
        let unchanged_open = st.unchanged_open;
        let toggle = state.clone();
        let same_header = h_flex()
            .id("merge-no-changes")
            .h(px(32.))
            .pl(px(if n_same > 0 { 4. } else { 18. }))
            .gap(px(4.))
            .border_t_1()
            .border_b_1()
            .border_color(rgb(RULE))
            .text_size(px(16.))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(pal.text)
            .when(n_same > 0, |d| {
                d.cursor_pointer()
                    .child(crate::ui::glyph(if unchanged_open { "ui-caret-down" } else { "ui-caret-right" }, 10., pal.text))
                    .on_click(move |_, window, _| {
                        let mut st = toggle.borrow_mut();
                        st.unchanged_open = !st.unchanged_open;
                        window.refresh();
                    })
            })
            .child(if n_same == 1 { "1 tab has no changes".to_string() } else { format!("{n_same} tabs have no changes") });
        let same_rows = plan.unchanged.iter().enumerate().filter(|_| unchanged_open).map(|(i, (name, icon))| {
            h_flex().h(px(ROW_H)).pl(px(18.)).gap(px(4.)).when(i % 2 == 0, |d| d.bg(rgb(STRIPE))).text_size(px(13.)).text_color(pal.text).child(crate::assets::icon(icon, 18., cx)).child(name.clone()).into_any_element()
        });
        // Notices (help merge-dialog-no-changes-02, protections-4.1-no-merge-01).
        let notice = if plan.protected_with_changes {
            // Live wp-937.
            Some(div().mx(px(10.)).mt(px(10.)).px(px(14.)).py(px(15.)).rounded(px(3.)).bg(rgb(0xfff8e6)).border_1().border_color(rgb(0xf5df9e)).text_size(px(14.)).text_color(pal.text).child("Workspace protection has been enabled. Merges are allowed only when the source workspace has all the changes from the protected workspace. Merge the protected workspace into the source workspace first and then retry."))
        } else if plan.up_to_date {
            Some(os::info_box(format!("{} is already up to date with {}.", plan.target, plan.source)).mx(px(10.)).mt(px(10.)))
        } else {
            None
        };
        let body = v_flex()
            .w_full()
            .min_h(px(340.))
            .child(div().px(px(10.)).child(overall))
            .child(div().px(px(10.)).pt(px(14.)).pb(px(6.)).text_size(px(18.)).line_height(px(27.)).font_weight(FontWeight::MEDIUM).text_color(pal.text).child("Tabs"))
            .child(header)
            .children(rows)
            .child(same_header)
            .children(same_rows)
            .children(notice);
        let merge_button = os::primary_button("merge-ok", "Merge").when(!can_merge, |b| b.disabled(true).opacity(0.4));
        let help = div().size(px(18.)).rounded_full().bg(rgb(0x999999)).flex().items_center().justify_center().text_size(px(13.)).font_weight(FontWeight::BOLD).text_color(gpui_kit::rgb_static(0xffffff)).child("?");
        let (doc, plan2, state2) = (doc.clone(), plan.clone(), state.clone());
        os::modal(dialog, "", W)
            .title(title)
            .child(body)
            .footer(os::modal_footer().child(help).child(div().flex_1()).child(os::modal_ok(merge_button)).child(os::modal_cancel(os::cancel_button("merge-cancel", "Cancel", cx))))
            .on_ok(move |_, _, cx| {
                if !can_merge {
                    return false;
                }
                let st = state2.borrow();
                let picks: HashMap<String, MergePick> = plan2
                    .changed
                    .iter()
                    .map(|r| {
                        let s = st.per_tab.get(&r.blob).copied().unwrap_or_else(|| r.default_for(st.overall));
                        (r.blob.clone(), match s {
                            Strategy::Keep => MergePick::Ours,
                            Strategy::Merge => MergePick::Auto,
                            Strategy::Replace => MergePick::Theirs,
                        })
                    })
                    .collect();
                let r = doc.update(cx, |d, cx| -> anyhow::Result<()> {
                    d.session.merge_picked(plan2.from, &plan2.source, plan2.from_branch, |n| picks.get(n).copied().unwrap_or(MergePick::Auto))?;
                    d.reloaded(cx);
                    Ok(())
                });
                if let Err(e) = r {
                    doc.update(cx, |_, cx| cx.emit(DocEvent::Error(format!("{e:#}"))));
                }
                true
            })
    });
}
