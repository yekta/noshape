//! Versions and history panel: the document's version graph (every edit is
//! a microversion), named versions, branches, restore, merge and read-only
//! viewing of any point in history — Onshape's version graph, fully local.

use crate::doc_model::{DocEvent, DocModel};
use crate::os_ui as os;
use crate::theme::p;
use gpui_kit::component::WindowExt as _;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputEvent, InputState, Textarea, TextareaState};
use gpui_kit::component::{IconName, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use crate::os_menu::OsContextMenuExt as _;
use gpui_kit::*;
use noshape_store::{BranchId, MicroversionId, VersionGraph};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

/// Branch lane colors (Onshape `--os-version-graph-branch-0..5`).
const LANES: [u32; 6] = [0x1651b0, 0xfab601, 0x3ad4a7, 0xb689be, 0x3e94ff, 0xb88a00];

/// Panel width (Onshape: 380 px).
pub const PANEL_W: f32 = 380.;
/// Graph column width and row heights (live DOM).
const GRAPH_W: f32 = 140.;
const ROW_H: f32 = 50.;
const CHANGE_H: f32 = 26.;

#[derive(Clone, Copy, PartialEq, Eq)]
enum RowKind {
    Workspace(BranchId),
    Version,
    Start,
    Change,
}

/// One row of the table: a workspace (branch head), a version, the Start
/// of the document, or (under "Show changes…") a microversion.
#[derive(Clone)]
struct Row {
    kind: RowKind,
    mv: MicroversionId,
    lane: usize,
    time: i64,
    name: String,
    author: String,
    key: String,
    /// Microversions between this row and the next row down its branch.
    changes: Vec<MicroversionId>,
}

pub struct HistoryPanel {
    doc: Entity<DocModel>,
    graph: Option<VersionGraph>,
    search: Entity<InputState>,
    query: String,
    /// Rows whose "Show changes…" is open.
    expanded: HashSet<String>,
    show_graph: bool,
    /// Every row shows its changes (Onshape's automatic versions toggle).
    show_all_changes: bool,
    /// Result of "Compare": (description of the older point, diff).
    compare: Option<(String, Vec<noshape_model::compare::TabDiff>)>,
    _subs: Vec<Subscription>,
}

impl HistoryPanel {
    pub fn new(doc: Entity<DocModel>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search history"));
        let subs = vec![
            cx.subscribe_in(&doc, window, |this: &mut Self, _, _ev: &DocEvent, _, cx| {
                this.graph = None;
                cx.notify();
            }),
            cx.subscribe_in(&search, window, |this: &mut Self, st, ev: &InputEvent, _, cx| {
                if matches!(ev, InputEvent::Change) {
                    this.query = st.read(cx).value().to_string();
                    cx.notify();
                }
            }),
        ];
        Self { doc, graph: None, search, query: String::new(), expanded: HashSet::new(), show_graph: true, show_all_changes: false, compare: None, _subs: subs }
    }

    fn graph(&mut self, cx: &App) -> VersionGraph {
        if self.graph.is_none() {
            self.graph = self.doc.read(cx).session.store.graph().ok();
        }
        self.graph.clone().unwrap_or(VersionGraph { microversions: vec![], branches: vec![], versions: vec![] })
    }

    fn after_change(&mut self, cx: &mut Context<Self>) {
        self.graph = None;
        self.doc.update(cx, |d, cx| d.reloaded(cx));
        cx.notify();
    }

    fn error(&mut self, e: anyhow::Error, cx: &mut Context<Self>) {
        self.doc.update(cx, |_, cx| cx.emit(DocEvent::Error(format!("{e:#}"))));
    }

    fn view_mv(&mut self, mv: Option<MicroversionId>, cx: &mut Context<Self>) {
        let r = self.doc.update(cx, |d, _| d.session.view_microversion(mv));
        match r {
            Ok(()) => self.after_change(cx),
            Err(e) => self.error(e, cx),
        }
    }

    /// Compare a point in history with the current branch head.
    fn compare_with_current(&mut self, mv: MicroversionId, label: String, cx: &mut Context<Self>) {
        let r = (|| -> anyhow::Result<Vec<noshape_model::compare::TabDiff>> {
            let s = &self.doc.read(cx).session;
            let old = noshape_model::document::Document::from_blobs(&s.store.read_all(mv)?)?;
            let new = noshape_model::document::Document::from_blobs(&s.store.read_all(s.head()?)?)?;
            Ok(noshape_model::compare::compare_documents(&old, &new))
        })();
        match r {
            Ok(d) => self.compare = Some((label, d)),
            Err(e) => self.error(e, cx),
        }
        cx.notify();
    }

    fn render_compare(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        use noshape_model::compare::ChangeKind;
        let (label, diffs) = self.compare.as_ref()?;
        let pal = p();
        let color = |k: &ChangeKind| match k {
            ChangeKind::Added => pal.success,
            ChangeKind::Removed => pal.error,
            ChangeKind::Modified => pal.warning,
        };
        let sign = |k: &ChangeKind| match k {
            ChangeKind::Added => "+",
            ChangeKind::Removed => "−",
            ChangeKind::Modified => "~",
        };
        let mut body: Vec<AnyElement> = Vec::new();
        if diffs.is_empty() {
            body.push(div().text_xs().text_color(pal.text_faint).child("No differences.").into_any_element());
        }
        for d in diffs {
            body.push(
                h_flex()
                    .gap_1p5()
                    .pt_1()
                    .child(div().text_xs().font_weight(FontWeight::SEMIBOLD).text_color(color(&d.status)).child(sign(&d.status)))
                    .child(div().text_xs().font_weight(FontWeight::SEMIBOLD).text_color(pal.text).child(d.tab.clone()))
                    .child(div().text_xs().text_color(pal.text_faint).child(d.kind_name))
                    .into_any_element(),
            );
            for c in &d.changes {
                body.push(
                    h_flex()
                        .pl_3()
                        .gap_1p5()
                        .items_start()
                        .child(div().text_xs().text_color(color(&c.kind)).child(sign(&c.kind)))
                        .child(div().text_xs().text_color(pal.text_muted).child(c.text.clone()))
                        .into_any_element(),
                );
            }
        }
        Some(
            v_flex()
                .max_h(px(320.))
                .border_b_1()
                .border_color(pal.border)
                .child(
                    h_flex()
                        .px_2()
                        .pt_2()
                        .justify_between()
                        .child(div().text_xs().font_weight(FontWeight::SEMIBOLD).text_color(pal.text_muted).child("COMPARE WITH CURRENT"))
                        .child(Button::new("cmp-close").icon(IconName::Close).ghost().xsmall().on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.compare = None;
                            cx.notify();
                        }))),
                )
                .child(div().px_2().text_xs().text_color(pal.text_faint).truncate().child(format!("From: {label}")))
                .child(v_flex().id("cmp-body").px_2().pb_2().overflow_y_scroll().children(body))
                .into_any_element(),
        )
    }

    fn restore(&mut self, mv: MicroversionId, cx: &mut Context<Self>) {
        let r = self.doc.update(cx, |d, _| d.session.restore(mv));
        match r {
            Ok(()) => self.after_change(cx),
            Err(e) => self.error(e, cx),
        }
    }

    fn switch_branch(&mut self, b: BranchId, cx: &mut Context<Self>) {
        let r = self.doc.update(cx, |d, _| d.session.switch_branch(b));
        match r {
            Ok(()) => self.after_change(cx),
            Err(e) => self.error(e, cx),
        }
    }

    fn merge(&mut self, from: BranchId, cx: &mut Context<Self>) {
        let r = self.doc.update(cx, |d, _| d.session.merge_from(from));
        match r {
            Ok(outcome) => {
                let msg = if outcome.up_to_date {
                    "Already up to date".to_string()
                } else if outcome.conflicts.is_empty() {
                    format!("Merged {} tab(s)", outcome.taken_from_source.len())
                } else {
                    format!("Merged; {} tab(s) changed on both branches were merged feature by feature", outcome.conflicts.len())
                };
                self.after_change(cx);
                self.doc.update(cx, |_, cx| cx.emit(DocEvent::Info(msg)));
            }
            Err(e) => self.error(e, cx),
        }
    }

}

/// "1:08 PM Today", "3:40 PM Yesterday", "Oct 12" (local time).
fn history_time(ms: i64) -> String {
    crate::start_library::format_onshape_time(ms)
}

/// Workspace, version and Start rows, newest first (Onshape's order), each
/// with the microversions back to the next row of its branch.
fn build_rows(graph: &VersionGraph, lanes: &[BranchId]) -> Vec<Row> {
    let info: HashMap<MicroversionId, &noshape_store::MicroversionInfo> = graph.microversions.iter().map(|m| (m.id, m)).collect();
    let lane_of = |mv: MicroversionId| info.get(&mv).and_then(|m| lanes.iter().position(|b| *b == m.branch)).unwrap_or(0);
    let time_of = |mv: MicroversionId| info.get(&mv).map(|m| m.created).unwrap_or(0);
    let mut rows = Vec::new();
    for b in graph.branches.iter().filter(|b| !b.deleted) {
        let author = info.get(&b.head).map(|m| m.author.clone()).unwrap_or_default();
        // A workspace sits above the versions made from its head.
        let time = graph.versions.iter().filter(|v| v.microversion == b.head).map(|v| v.created).fold(time_of(b.head).max(b.created), i64::max);
        rows.push(Row { kind: RowKind::Workspace(b.id), mv: b.head, lane: lanes.iter().position(|x| *x == b.id).unwrap_or(0), time, name: b.name.clone(), author, key: format!("w{}", b.id.0), changes: vec![] });
    }
    for v in &graph.versions {
        rows.push(Row { kind: RowKind::Version, mv: v.microversion, lane: lane_of(v.microversion), time: v.created, name: v.name.clone(), author: v.author.clone(), key: format!("v{}", v.id.0), changes: vec![] });
    }
    if let Some(root) = graph.microversions.iter().find(|m| m.parents.is_empty()) {
        rows.push(Row { kind: RowKind::Start, mv: root.id, lane: 0, time: root.created, name: "Start".into(), author: root.author.clone(), key: "start".into(), changes: vec![] });
    }
    let rank = |k: RowKind| match k {
        RowKind::Workspace(_) => 0,
        RowKind::Version => 1,
        _ => 2,
    };
    rows.sort_by(|a, b| b.time.cmp(&a.time).then(rank(a.kind).cmp(&rank(b.kind))));
    let stops: HashSet<MicroversionId> = rows.iter().filter(|r| !matches!(r.kind, RowKind::Workspace(_))).map(|r| r.mv).collect();
    for r in rows.iter_mut().filter(|r| r.kind != RowKind::Start) {
        let mut cur = Some(r.mv);
        while let Some(mv) = cur {
            if (mv != r.mv && stops.contains(&mv)) || r.changes.len() >= 200 {
                break;
            }
            let Some(m) = info.get(&mv) else { break };
            if m.parents.is_empty() {
                break;
            }
            r.changes.push(mv);
            cur = m.parents.first().copied();
        }
    }
    rows
}

/// A dialog's create action (true when it ran and the dialog may close).
type CreateFn = Rc<dyn Fn(&mut Window, &mut App) -> bool>;

/// Onshape's "Create version from <workspace>" dialog.
pub fn open_create_version(doc: Entity<DocModel>, window: &mut Window, cx: &mut App) {
    let (branch, n) = {
        let s = &doc.read(cx).session;
        let g = s.store.graph().ok();
        let name = g.as_ref().and_then(|g| g.branches.iter().find(|b| b.id == s.branch).map(|b| b.name.clone())).unwrap_or_else(|| "Main".into());
        (name, g.map(|g| g.versions.len()).unwrap_or(0) + 1)
    };
    let name = cx.new(|cx| InputState::new(window, cx).placeholder("Name").default_value(format!("V{n}")));
    let desc = cx.new(|cx| TextareaState::new(window, cx).rows(4).placeholder("Description"));
    let create: CreateFn = {
        let (doc, name, desc) = (doc.clone(), name.clone(), desc.clone());
        let done = std::cell::Cell::new(false);
        Rc::new(move |_, cx| {
            let n = name.read(cx).value().trim().to_string();
            if n.is_empty() {
                return false;
            }
            // Enter reaches both the field and the dialog: create once.
            if done.replace(true) {
                return true;
            }
            let d = desc.read(cx).value().to_string();
            let r = doc.update(cx, |doc, _| doc.session.create_version(&n, &d));
            doc.update(cx, |doc, cx| match r {
                Ok(_) => doc.reloaded(cx),
                Err(e) => cx.emit(DocEvent::Error(format!("{e:#}"))),
            });
            true
        })
    };
    {
        let create = create.clone();
        window
            .subscribe(&name, cx, move |_, ev: &InputEvent, window, cx| {
                if let InputEvent::PressEnter { .. } = ev
                    && create(window, cx)
                {
                    window.close_dialog(cx);
                }
            })
            .detach();
    }
    let focus = name.clone();
    window.open_dialog(cx, move |dialog, _, cx| {
        let create = create.clone();
        os::modal(dialog, "", 950.)
            .title(version_dialog_title("Create version from", &branch))
            .child(version_dialog_body(&name, &desc, None))
            .footer(os::modal_footer().child(os::modal_ok(os::primary_button("ok", "Create"))).child(os::modal_cancel(os::cancel_button("cancel", "Cancel", cx))))
            .on_ok(move |_, window, cx| create(window, cx))
    });
    window.defer(cx, move |window, cx| {
        focus.update(cx, |s, cx| {
            s.focus(window, cx);
            s.select_all(window, cx);
        })
    });
}

/// Onshape's "Create workspace from <version>" dialog (Branch to create
/// workspace…); "Open new workspace" switches to it.
fn open_branch_dialog(doc: Entity<DocModel>, from: MicroversionId, from_name: String, window: &mut Window, cx: &mut App) {
    let n = doc.read(cx).session.store.graph().map(|g| g.branches.len()).unwrap_or(1);
    let name = cx.new(|cx| InputState::new(window, cx).placeholder("Name").default_value(format!("B{n}")));
    let desc = cx.new(|cx| TextareaState::new(window, cx).rows(4).placeholder("Description"));
    let open = Rc::new(std::cell::Cell::new(true));
    let create: CreateFn = {
        let (doc, name, open) = (doc.clone(), name.clone(), open.clone());
        let done = std::cell::Cell::new(false);
        Rc::new(move |_, cx| {
            let n = name.read(cx).value().trim().to_string();
            if n.is_empty() {
                return false;
            }
            // Enter reaches both the field and the dialog: create once.
            if done.replace(true) {
                return true;
            }
            let switch = open.get();
            let r = doc.update(cx, |doc, _| if switch { doc.session.branch_from(from, &n).map(|_| ()) } else { doc.session.store.create_branch(from, &n).map(|_| ()).map_err(Into::into) });
            doc.update(cx, |doc, cx| match r {
                Ok(()) => doc.reloaded(cx),
                Err(e) => cx.emit(DocEvent::Error(format!("{e:#}"))),
            });
            true
        })
    };
    {
        let create = create.clone();
        window
            .subscribe(&name, cx, move |_, ev: &InputEvent, window, cx| {
                if let InputEvent::PressEnter { .. } = ev
                    && create(window, cx)
                {
                    window.close_dialog(cx);
                }
            })
            .detach();
    }
    let focus = name.clone();
    window.open_dialog(cx, move |dialog, _, cx| {
        let create = create.clone();
        let open = open.clone();
        os::modal(dialog, "", 950.)
            .title(version_dialog_title("Create workspace from", &from_name))
            .child(version_dialog_body(&name, &desc, Some(open)))
            .footer(os::modal_footer().child(os::modal_ok(os::primary_button("ok", "Create"))).child(os::modal_cancel(os::cancel_button("cancel", "Cancel", cx))))
            .on_ok(move |_, window, cx| create(window, cx))
    });
    window.defer(cx, move |window, cx| {
        focus.update(cx, |s, cx| {
            s.focus(window, cx);
            s.select_all(window, cx);
        })
    });
}

/// "Create version from **Main**": a gray 18 px lead-in, the name 500.
fn version_dialog_title(lead: &str, name: &str) -> AnyElement {
    let pal = p();
    h_flex()
        .w(px(948.))
        .h(px(48.))
        .pl(px(10.))
        .pr(px(12.))
        .gap(px(6.))
        .border_b_1()
        .border_color(rgb(os::MODAL_DIVIDER))
        .child(div().text_size(px(18.)).text_color(pal.text_muted).child(lead.to_string()))
        .child(div().text_size(px(18.)).font_weight(FontWeight::MEDIUM).text_color(pal.text).child(name.to_string()))
        .child(div().flex_1())
        .child(div().flex_none().child(gpui_kit::component::dialog::DialogClose::new().child(div().id("modal-close").size(px(22.)).flex().items_center().justify_center().cursor_pointer().text_color(pal.text_faint).child(gpui_kit::component::Icon::empty().data(os::GLYPH_CLOSE).size(px(16.))))))
        .into_any_element()
}

fn version_dialog_body(name: &Entity<InputState>, desc: &Entity<TextareaState>, open: Option<Rc<std::cell::Cell<bool>>>) -> AnyElement {
    let pal = p();
    let label = |t: &str| div().text_size(px(14.)).font_weight(FontWeight::SEMIBOLD).text_color(pal.text).child(t.to_string());
    os::modal_body()
        .gap(px(10.))
        .child(h_flex().gap(px(4.)).child(label("Name:")).child(div().w(px(300.)).child(Input::new(name).h(px(34.)).text_size(px(14.)))))
        .child(v_flex().gap(px(2.)).child(h_flex().gap(px(8.)).child(label("Description:")).child(div().text_size(px(14.)).child("(Maximum of 10000 characters)"))).child(Textarea::new(desc).h(px(96.)).text_size(px(14.))))
        .when_some(open, |d, open| {
            d.child(
                os::checkbox("open-ws", "Open new workspace", open.get()).on_click(move |_, window, _| {
                    open.set(!open.get());
                    window.refresh();
                }),
            )
        })
        .child(h_flex().gap(px(8.)).child(label("Note:")).child(div().text_size(px(14.)).child("Merging feature changes is only supported for Part Studio and Assembly tabs")))
        .into_any_element()
}

/// A row laid out at `y` (its top); `change` rows are 26 px high.
struct Placed {
    row: Row,
    y: f32,
    change: bool,
}

impl Render for HistoryPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pal = p();
        let graph = self.graph(cx);
        let session = &self.doc.read(cx).session;
        let current_branch = session.branch;
        let viewing = session.viewing;
        let current_name = graph.branches.iter().find(|b| b.id == current_branch).map(|b| b.name.clone()).unwrap_or_else(|| "Main".into());
        let mut lanes: Vec<BranchId> = graph.branches.iter().filter(|b| !b.deleted).map(|b| b.id).collect();
        lanes.sort_by_key(|b| !graph.branches.iter().any(|x| x.id == *b && x.is_default));
        let rows = build_rows(&graph, &lanes);
        let info: HashMap<MicroversionId, &noshape_store::MicroversionInfo> = graph.microversions.iter().map(|m| (m.id, m)).collect();
        let q = self.query.trim().to_lowercase();

        // Lay out rows (with open "Show changes…" lists) to place the graph.
        let mut placed: Vec<Placed> = Vec::new();
        let mut y = 0.;
        for r in rows.iter().filter(|r| q.is_empty() || r.name.to_lowercase().contains(&q)) {
            placed.push(Placed { row: r.clone(), y, change: false });
            y += ROW_H;
            if (self.show_all_changes || self.expanded.contains(&r.key)) && !r.changes.is_empty() {
                for mv in &r.changes {
                    let Some(m) = info.get(mv) else { continue };
                    let c = Row { kind: RowKind::Change, mv: *mv, lane: r.lane, time: m.created, name: m.description.clone(), author: m.author.clone(), key: format!("c{}", mv.short()), changes: vec![] };
                    placed.push(Placed { row: c, y, change: true });
                    y += CHANGE_H;
                }
            }
        }
        let total_h = y;
        let nl = lanes.len().max(1) as f32;
        let lane_x = move |l: usize| GRAPH_W / 2. - 7. * (nl - 1.) + 14. * l as f32;
        // Graph geometry: per lane, the node span; branches curve into their
        // parent lane above the row they were created from.
        let mut segs: Vec<(usize, f32, f32)> = Vec::new();
        let mut forks: Vec<(usize, usize, f32, f32)> = Vec::new();
        for (l, b) in lanes.iter().enumerate() {
            let ys: Vec<f32> = placed.iter().filter(|p| p.row.lane == l).map(|p| p.y + 13.).collect();
            let (Some(top), Some(bottom)) = (ys.iter().copied().reduce(f32::min), ys.iter().copied().reduce(f32::max)) else { continue };
            segs.push((l, top, bottom));
            let from = graph.branches.iter().find(|x| x.id == *b).and_then(|x| x.created_from);
            if let Some(f) = from {
                let ft = info.get(&f).map(|m| m.created).unwrap_or(0);
                let parent = info.get(&f).and_then(|m| lanes.iter().position(|x| *x == m.branch)).unwrap_or(0);
                let mv_time = |p: &Placed| info.get(&p.row.mv).map(|m| m.created).unwrap_or(p.row.time);
                let below = |p: &&Placed| p.y + 13. > bottom && p.row.lane == parent;
                if let Some(t) = placed.iter().filter(below).find(|p| p.row.mv == f).or_else(|| placed.iter().filter(below).find(|p| mv_time(p) <= ft)) {
                    forks.push((l, parent, bottom, t.y + 13.));
                }
            }
        }
        let nodes: Vec<(usize, f32, RowKind)> = placed.iter().map(|p| (p.row.lane, p.y + 13., p.row.kind)).collect();
        let graph_canvas = canvas(
            |_, _, _| {},
            move |bounds, _, window, _| {
                let o = bounds.origin;
                let pt = |x: f32, y: f32| point(o.x + px(x), o.y + px(y));
                let color = |l: usize| rgb(LANES[l % LANES.len()]);
                for &(l, a, b) in &segs {
                    let mut pb = PathBuilder::stroke(px(3.));
                    pb.move_to(pt(lane_x(l), a));
                    pb.line_to(pt(lane_x(l), b));
                    if let Ok(path) = pb.build() {
                        window.paint_path(path, color(l));
                    }
                }
                for &(l, parent, from_y, to_y) in &forks {
                    let (x0, x1) = (lane_x(l), lane_x(parent));
                    let mid = (to_y - 13.).max(from_y);
                    let mut pb = PathBuilder::stroke(px(3.));
                    pb.move_to(pt(x0, from_y));
                    pb.line_to(pt(x0, (mid - 22.).max(from_y)));
                    pb.cubic_bezier_to(pt(x1, mid), pt(x0, mid - 8.), pt(x1, mid - 14.));
                    pb.line_to(pt(x1, to_y));
                    if let Ok(path) = pb.build() {
                        window.paint_path(path, color(l));
                    }
                }
                let circle = |c: Point<Pixels>, r: f32| (0..24).map(|i| {
                    let t = i as f32 / 24. * std::f32::consts::TAU;
                    point(c.x + px(r * t.cos()), c.y + px(r * t.sin()))
                }).collect::<Vec<_>>();
                for &(l, y, kind) in &nodes {
                    let c = pt(lane_x(l), y);
                    match kind {
                        RowKind::Workspace(_) => {
                            let mut f = PathBuilder::fill();
                            f.add_polygon(&circle(c, 6.5), true);
                            if let Ok(path) = f.build() {
                                window.paint_path(path, color(l));
                            }
                            let mut f = PathBuilder::fill();
                            f.add_polygon(&circle(c, 4.5), true);
                            if let Ok(path) = f.build() {
                                window.paint_path(path, rgb(0xffffff));
                            }
                        }
                        RowKind::Version | RowKind::Change => {
                            let mut f = PathBuilder::fill();
                            f.add_polygon(&circle(c, if kind == RowKind::Version { 4.5 } else { 3. }), true);
                            if let Ok(path) = f.build() {
                                window.paint_path(path, color(l));
                            }
                        }
                        RowKind::Start => {
                            let mut pb = PathBuilder::stroke(px(2.5));
                            pb.move_to(point(c.x - px(6.), c.y));
                            pb.line_to(point(c.x + px(6.), c.y));
                            if let Ok(path) = pb.build() {
                                window.paint_path(path, color(l));
                            }
                        }
                    }
                }
            },
        )
        .absolute()
        .top_0()
        .left_0()
        .w(px(GRAPH_W))
        .h(px(total_h));

        let view = cx.entity();
        let doc = self.doc.clone();
        let mut list: Vec<AnyElement> = Vec::new();
        for pl in &placed {
            let r = pl.row.clone();
            let selected = match r.kind {
                RowKind::Workspace(b) => b == current_branch && viewing.is_none(),
                _ => viewing == Some(r.mv),
            };
            let (mv, kind, name) = (r.mv, r.kind, r.name.clone());
            let open = cx.listener(move |this, e: &ClickEvent, _, cx| match kind {
                RowKind::Workspace(b) if b != current_branch => {
                    if e.click_count() >= 2 {
                        this.switch_branch(b, cx)
                    }
                }
                RowKind::Workspace(_) => this.view_mv(None, cx),
                _ => this.view_mv(Some(mv), cx),
            });
            let (v, d, cur) = (view.clone(), doc.clone(), current_name.clone());
            let row = if pl.change {
                h_flex()
                    .id(SharedString::from(format!("hist-{}", r.key)))
                    .h(px(CHANGE_H))
                    .flex_none()
                    .cursor_pointer()
                    .text_size(px(13.))
                    .text_color(pal.text_muted)
                    .when(selected, |d| d.bg(pal.accent_soft))
                    .when(!selected, |d| d.hover(|d| d.bg(pal.panel_hover)))
                    .child(div().w(px(GRAPH_W + 5.)).flex_none())
                    .child(div().w(px(115.)).flex_none().truncate().child(r.name.clone()))
                    .child(div().pl(px(3.)).flex_1().min_w_0().truncate().child(history_time(r.time)))
            } else {
                let changes = r.changes.len();
                let key = r.key.clone();
                let open_changes = self.show_all_changes || self.expanded.contains(&key);
                let name_el = match r.kind {
                    RowKind::Workspace(_) => div().px(px(3.)).rounded(px(3.)).border_1().border_color(rgb(LANES[r.lane % LANES.len()])).bg(if r.lane == 0 { rgb(0xd0dbe5) } else { rgb(0xfff4d6) }).font_weight(FontWeight::MEDIUM).text_color(pal.text).child(r.name.clone()),
                    _ => div().font_weight(FontWeight::MEDIUM).text_color(pal.text).child(r.name.clone()),
                };
                h_flex()
                    .id(SharedString::from(format!("hist-{}", r.key)))
                    .h(px(ROW_H))
                    .flex_none()
                    .items_start()
                    .pt(px(5.))
                    .cursor_pointer()
                    .text_size(px(14.))
                    .text_color(pal.text_muted)
                    .when(selected, |d| d.bg(pal.accent_soft))
                    .when(!selected, |d| d.hover(|d| d.bg(pal.panel_hover)))
                    .tooltip(crate::ui::tooltip(match r.kind {
                        RowKind::Workspace(_) => format!("Open workspace {}", r.name),
                        _ => format!("Open version {}", r.name),
                    }))
                    .child(div().w(px(GRAPH_W + 5.)).flex_none())
                    .child(
                        v_flex()
                            .w(px(115.))
                            .flex_none()
                            .child(h_flex().h(px(21.)).child(name_el))
                            .when(changes > 0, |d| {
                                d.child(
                                    h_flex()
                                        .id(SharedString::from(format!("chg-{key}")))
                                        .gap(px(3.))
                                        .child(crate::ui::glyph(if open_changes { "ui-chevron-down" } else { "ui-chevron-right" }, 10., pal.text_muted))
                                        .child(div().truncate().child(if open_changes { "Hide changes…" } else { "Show changes…" }))
                                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                            cx.stop_propagation();
                                            if !this.expanded.remove(&key) {
                                                this.expanded.insert(key.clone());
                                            }
                                            cx.notify();
                                        })),
                                )
                            }),
                    )
                    .child(v_flex().pl(px(3.)).flex_1().min_w_0().child(div().truncate().child(r.author.clone())).child(div().truncate().child(history_time(r.time))))
            };
            let row = row.on_click(open).os_context_menu(move |m, _, _| {
                let (v1, v2, v3, v4) = (v.clone(), v.clone(), v.clone(), v.clone());
                let (name1, name2, cur1) = (name.clone(), name.clone(), cur.clone());
                let d1 = d.clone();
                match kind {
                    RowKind::Workspace(b) => {
                        let m = m.item(crate::os_menu::OsMenuItem::new("Open").on_click(move |_, _, cx| v1.update(cx, |t, cx| if b == current_branch { t.view_mv(None, cx) } else { t.switch_branch(b, cx) })));
                        if b != current_branch {
                            m.item(crate::os_menu::OsMenuItem::new(format!("Merge into {cur1}…")).on_click(move |_, _, cx| v2.update(cx, |t, cx| t.merge(b, cx))))
                        } else {
                            m
                        }
                    }
                    _ => m
                        .item(crate::os_menu::OsMenuItem::new("Open").on_click(move |_, _, cx| v1.update(cx, |t, cx| t.view_mv(Some(mv), cx))))
                        .item(crate::os_menu::OsMenuItem::new("Branch to create workspace…").on_click(move |_, window, cx| open_branch_dialog(d1.clone(), mv, name1.clone(), window, cx)))
                        .item(crate::os_menu::OsMenuItem::new("Compare").on_click(move |_, _, cx| v3.update(cx, |t, cx| t.compare_with_current(mv, name2.clone(), cx))))
                        .item(crate::os_menu::OsMenuItem::new(format!("Restore to {cur1}…")).on_click(move |_, _, cx| v4.update(cx, |t, cx| t.restore(mv, cx)))),
                }
            });
            list.push(row.into_any_element());
        }

        let doc2 = self.doc.clone();
        let legend_item = |icon: AnyElement, label: &'static str, faint: bool| h_flex().gap(px(6.)).child(icon).child(div().text_size(px(13.)).font_weight(if faint { FontWeight::NORMAL } else { FontWeight::SEMIBOLD }).text_color(if faint { pal.text_muted } else { pal.text }).child(label));
        let dot = |r: f32| div().size(px(r * 2.)).rounded_full().bg(pal.accent).into_any_element();
        v_flex()
            .size_full()
            .bg(pal.panel)
            .child(
                h_flex()
                    .h(px(37.))
                    .flex_none()
                    .pl(px(5.))
                    .pr(px(6.))
                    .gap(px(4.))
                    .child(div().flex_1().text_size(px(14.)).font_weight(FontWeight::SEMIBOLD).text_color(pal.text).child("Versions and history"))
                    .child(crate::ui::icon_button("hist-create-version", "version", 30., false, cx).tooltip("Create version…").on_click(move |_, window, cx| open_create_version(doc2.clone(), window, cx)))
                    .child(crate::ui::icon_button("hist-compare", "compare", 30., false, cx).tooltip("Compare with the workspace").on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        if let Some(r) = this.graph(cx).versions.last().map(|v| (v.microversion, v.name.clone())) {
                            this.compare_with_current(r.0, r.1, cx);
                        }
                    })))
                    .child(crate::ui::icon_button("hist-close", "close", 30., false, cx).tooltip("Close").on_click(|_, window, cx| window.dispatch_action(Box::new(crate::actions::ToggleVersionGraph), cx))),
            )
            .child(
                h_flex()
                    .px(px(10.))
                    .pt(px(2.))
                    .flex_none()
                    .child(div().flex_1().h(px(30.)).border_1().border_color(rgb(0xaaaaaa)).rounded_l(px(4.)).child(Input::new(&self.search).appearance(false).text_size(px(14.)).italic()))
                    .child(div().size(px(30.)).flex().items_center().justify_center().border_1().border_l_0().border_color(rgb(0xaaaaaa)).rounded_r(px(4.)).child(crate::assets::icon("search", 20., cx))),
            )
            .child(
                h_flex()
                    .px(px(14.))
                    .pt(px(8.))
                    .gap(px(6.))
                    .flex_none()
                    .child(crate::ui::icon_button("hist-graph", "version-graph", 30., self.show_graph, cx).tooltip("Show the versions graph").on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.show_graph = !this.show_graph;
                        cx.notify();
                    })))
                    .child(crate::ui::icon_button("hist-auto", "auto-version", 30., self.show_all_changes, cx).tooltip("Show all changes").on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.show_all_changes = !this.show_all_changes;
                        cx.notify();
                    }))),
            )
            .child(
                h_flex()
                    .h(px(38.))
                    .mt(px(8.))
                    .flex_none()
                    .items_end()
                    .pb(px(5.))
                    .border_b_1()
                    .border_color(rgb(0xd0dbe5))
                    .text_size(px(14.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(pal.text)
                    .child(div().w(px(GRAPH_W + 5.)).flex_none())
                    .child(div().w(px(118.)).flex_none().child("Name"))
                    .child(div().child("Modified")),
            )
            .children(self.render_compare(cx))
            .child(
                div()
                    .id("history-rows")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .child(div().relative().w_full().h(px(total_h)).child(v_flex().children(list)).when(self.show_graph, |d| d.child(graph_canvas))),
            )
            .child(
                v_flex()
                    .flex_none()
                    .items_center()
                    .gap(px(10.))
                    .py(px(14.))
                    .child(
                        h_flex()
                            .gap(px(18.))
                            .child(legend_item(div().size(px(11.)).rounded_full().border_2().border_color(pal.accent).into_any_element(), "Workspace", false))
                            .child(legend_item(dot(4.5), "Version", false))
                            .child(legend_item(dot(2.5), "Change", true)),
                    )
                    .child(legend_item(crate::assets::icon("auto-version", 18., cx), "Automatic version", false)),
            )
    }
}
