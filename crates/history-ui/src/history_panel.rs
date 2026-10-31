//! Versions and history panel: the document's version graph (every edit is
//! a microversion), named versions, branches, restore, merge and read-only
//! viewing of any point in history — Onshape's version graph, fully local.

use crate::doc_model::{DocEvent, DocModel};
use crate::os_ui as os;
use crate::theme::p;
use gpui_kit::component::WindowExt as _;
use gpui_kit::component::input::{Input, InputEvent, InputState, Textarea, TextareaState};
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use crate::os_menu::OsContextMenuExt as _;
use gpui_kit::*;
use noshape_model::document::{TabContent, TabId};
use noshape_store::{BranchId, MicroversionId, VersionGraph};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

/// Branch lane colors (Onshape `--os-version-graph-branch-0..5`).
const LANES: [u32; 6] = [0x1651b0, 0xfab601, 0x3ad4a7, 0xb689be, 0x3e94ff, 0xb88a00];

/// A workspace lane's colour (graph lines and workspace chips).
pub(crate) fn lane_color(lane: usize) -> u32 {
    LANES[lane % LANES.len()]
}

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

/// A popup anchor: a field's bounds from the last paint.
type BoundsCell = Rc<std::cell::Cell<Bounds<Pixels>>>;

use noshape_ops::history_filter::{HistoryFilter, ModifiedBy};

/// Tab types of the filter panel, in Onshape's order (kind, icon, tooltip).
const FILTER_KINDS: [(&str, &str, &str); 5] = [("partstudio", "part-studio", "Part Studio"), ("assembly", "assembly", "Assembly"), ("drawing", "drawing", "Drawing"), ("featurestudio", "feature-studio", "Feature Studio"), ("file", "file", "File")];

/// The toggles under "Search history" (live documents/118–119): 30x30,
/// radius 3; on = `#eaeaea` with an inset 0 1 1 shadow, off = clear.
fn filter_toggle(id: &'static str, icon: &'static str, on: bool, cx: &App) -> Stateful<Div> {
    div()
        .id(id)
        .size(px(30.))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(3.))
        .cursor_pointer()
        .when(on, |d| d.bg(rgb(0xeaeaea)).shadow(vec![BoxShadow { color: hsla(0., 0., 0., 0.247), offset: point(px(0.), px(1.)), blur_radius: px(1.), spread_radius: px(0.), inset: true }]))
        .when(!on, |d| d.hover(|d| d.bg(rgb(0xeaeaea))))
        .child(crate::assets::icon(icon, 20., cx))
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
    /// Onshape's two toggles under the search box (live documents/118–119,
    /// both on by default): hide merges, hide automatic versions.
    hide_merges: bool,
    hide_auto_versions: bool,
    /// The search ▾ filter panel (live documents/117): open, the fields
    /// being edited, and the filter its Search button applied.
    filter_open: bool,
    filter_edit: HistoryFilter,
    filter: HistoryFilter,
    date_from: Entity<InputState>,
    date_to: Entity<InputState>,
    /// Tabs each microversion changed (store diffs, for the Tab filters).
    changed_tabs: HashMap<MicroversionId, Vec<TabId>>,
    /// Result of "Compare": (description of the older point, diff).
    _subs: Vec<Subscription>,
}

impl HistoryPanel {
    pub fn new(doc: Entity<DocModel>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search = cx.new(|cx| InputState::new(window, cx).select_on_focus(false).revert_on_escape(false).placeholder("Search history"));
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
        let date_from = cx.new(|cx| InputState::new(window, cx).placeholder("mm/dd/yyyy, --:--"));
        let date_to = cx.new(|cx| InputState::new(window, cx).placeholder("mm/dd/yyyy, --:--"));
        Self {
            doc,
            graph: None,
            search,
            query: String::new(),
            expanded: HashSet::new(),
            hide_merges: true,
            hide_auto_versions: true,
            filter_open: false,
            filter_edit: HistoryFilter::default(),
            filter: HistoryFilter::default(),
            date_from,
            date_to,
            changed_tabs: HashMap::new(),
            _subs: subs,
        }
    }

    /// Does a microversion pass the applied filter?
    fn change_matches(&mut self, m: &noshape_store::MicroversionInfo, cx: &App) -> bool {
        let f = self.filter.clone();
        let session = &self.doc.read(cx).session;
        let cache = &mut self.changed_tabs;
        f.matches(m, &session.doc, || cache.entry(m.id).or_insert_with(|| noshape_ops::history_filter::changed_tabs(&session.store, m)).clone())
    }

    /// Read the panel's date fields ("mm/dd/yyyy, hh:mm"; the time is optional).
    fn read_dates(&mut self, cx: &App) {
        let parse = |s: &str, end: bool| -> Option<i64> {
            use chrono::{Local, NaiveDate, NaiveTime, TimeZone as _};
            let s = s.trim();
            let (d, t) = s.split_once(',').map(|(d, t)| (d.trim(), Some(t.trim()))).unwrap_or((s, None));
            let date = NaiveDate::parse_from_str(d, "%m/%d/%Y").ok()?;
            let time = match t.filter(|t| !t.is_empty() && !t.starts_with('-')) {
                Some(t) => NaiveTime::parse_from_str(t, "%H:%M").or_else(|_| NaiveTime::parse_from_str(t, "%I:%M %p")).ok()?,
                None if end => NaiveTime::from_hms_opt(23, 59, 59)?,
                None => NaiveTime::MIN,
            };
            Local.from_local_datetime(&date.and_time(time)).single().map(|t| t.timestamp_millis())
        };
        self.filter_edit.from = parse(&self.date_from.read(cx).value(), false);
        self.filter_edit.to = parse(&self.date_to.read(cx).value(), true);
    }

    /// The search ▾ panel (live documents/117, dom/documents-history-search-filter):
    /// 260 px, white, 1 px `#d7d7d7`, radius 4, shadow 0 6 12 .2; 100 px label
    /// column (11 px `#666`), 148 px fields; Reset | Search.
    fn render_filter_panel(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let doc = self.doc.read(cx).session.doc.clone();
        let edit = self.filter_edit.clone();
        let label = |t: &'static str| div().w(px(100.)).flex_none().pt(px(12.)).pl(px(4.)).text_size(px(11.)).text_color(rgb(0x666666)).child(t);
        // Select lists hang under their field (bounds recorded on paint).
        let (tab_anchor, by_anchor): (BoundsCell, BoundsCell) = Default::default();
        let under = |a: &BoundsCell| {
            let b = a.get();
            point(b.left(), b.bottom())
        };
        let select = |id: &'static str, text: String, anchor: &BoundsCell| {
            let probe = anchor.clone();
            h_flex()
                .id(id)
                .relative()
                .child(canvas(move |b, _, _| probe.set(b), |_, _, _, _| {}).absolute().top_0().left_0().size_full())
                .w(px(148.))
                .h(px(34.))
                .pl(px(8.))
                .pr(px(6.))
                .border_b_1()
                .border_color(rgb(0xeaeaea))
                .cursor_pointer()
                .text_size(px(14.))
                .text_color(pal.text)
                .child(div().flex_1().min_w_0().truncate().child(text))
                .child(os::glyph(os::GLYPH_CARET, 10., rgb(0x666666)))
        };
        let tab_name = edit.tab.and_then(|t| doc.tab(t)).map(|t| t.name.clone()).unwrap_or_else(|| "Any".into());
        let view = cx.entity();
        let tabs: Vec<(TabId, String)> = doc.tabs.iter().map(|t| (t.id, t.name.clone())).collect();
        let ta = tab_anchor.clone();
        let tab_select = select("hist-f-tab", tab_name, &tab_anchor).on_click(move |_: &ClickEvent, window, cx| {
            let (view, tabs) = (view.clone(), tabs.clone());
            let menu = crate::os_menu::OsMenu::build(window, cx, move |mut m, _, _| {
                m = m.select_list(148.);
                let v = view.clone();
                m = m.item(crate::os_menu::OsMenuItem::new("Any").on_click(move |_, _, cx| v.update(cx, |t, cx| {
                    t.filter_edit.tab = None;
                    cx.notify();
                })));
                for (id, name) in &tabs {
                    let (v, id) = (view.clone(), *id);
                    m = m.item(crate::os_menu::OsMenuItem::new(name.clone()).on_click(move |_, _, cx| v.update(cx, |t, cx| {
                        t.filter_edit.tab = Some(id);
                        cx.notify();
                    })));
                }
                m
            });
            crate::os_menu::open(under(&ta), menu, window, cx);
        });
        let view = cx.entity();
        let ba = by_anchor.clone();
        let by_select = select("hist-f-by", edit.by.label().into(), &by_anchor).on_click(move |_: &ClickEvent, window, cx| {
            let view = view.clone();
            let menu = crate::os_menu::OsMenu::build(window, cx, move |mut m, _, _| {
                m = m.select_list(148.);
                for by in ModifiedBy::ALL {
                    let v = view.clone();
                    m = m.item(crate::os_menu::OsMenuItem::new(by.label()).on_click(move |_, _, cx| v.update(cx, |t, cx| {
                        t.filter_edit.by = by;
                        cx.notify();
                    })));
                }
                m
            });
            crate::os_menu::open(under(&ba), menu, window, cx);
        });
        let mut types = h_flex().w(px(148.)).flex_wrap().gap(px(6.)).pl(px(3.)).pt(px(4.));
        for (kind, icon, tip) in FILTER_KINDS {
            let on = edit.kinds.contains(&kind);
            types = types.child(crate::ui::icon_button(SharedString::from(format!("hist-f-kind-{kind}")), icon, 30., on, cx).tooltip(tip).on_click(cx.listener(move |t, _: &ClickEvent, _, cx| {
                if let Some(i) = t.filter_edit.kinds.iter().position(|k| *k == kind) {
                    t.filter_edit.kinds.remove(i);
                } else {
                    t.filter_edit.kinds.push(kind);
                }
                cx.notify();
            })));
        }
        let any_kind = !edit.kinds.is_empty();
        let date = |st: &Entity<InputState>, tip: &'static str, cx: &App| div().w(px(148.)).h(px(34.)).pl(px(4.)).border_b_1().border_color(rgb(0xeaeaea)).flex().items_center().id(tip).tooltip(crate::ui::tooltip(tip)).child(div().flex_1().child(Input::new(st).appearance(false).text_size(px(14.)))).child(div().pr(px(6.)).child(crate::assets::icon("calendar", 16., cx)));
        v_flex()
            .id("hist-filter-panel")
            .occlude()
            .w(px(260.))
            .bg(pal.panel)
            .border_1()
            .border_color(rgb(0xd7d7d7))
            .rounded(px(4.))
            .shadow(vec![BoxShadow { color: hsla(0., 0., 0., 0.2), offset: point(px(0.), px(6.)), blur_radius: px(12.), spread_radius: px(0.), inset: false }])
            .px(px(5.))
            .text_color(pal.text)
            .on_mouse_down_out(cx.listener(|t, _: &MouseDownEvent, _, cx| {
                if !crate::os_menu::is_open(cx) {
                    t.filter_open = false;
                    cx.notify();
                }
            }))
            .child(h_flex().h(px(36.)).items_start().child(label("Tab")).child(tab_select))
            .child(
                h_flex()
                    .mt(px(14.))
                    .items_start()
                    .child(label("Tab types"))
                    .child(
                        v_flex().w(px(148.)).child(types).child(
                            h_flex().justify_end().pt(px(3.)).child(
                                div()
                                    .id("hist-f-clear")
                                    .h(px(27.))
                                    .px(px(7.))
                                    .flex()
                                    .items_center()
                                    .rounded(px(3.5))
                                    .border_1()
                                    .border_color(rgb(0xaaaaaa))
                                    .text_size(px(12.25))
                                    .text_color(rgb(0x999999))
                                    .when(!any_kind, |d| d.opacity(0.4))
                                    .when(any_kind, |d| d.cursor_pointer().text_color(pal.text))
                                    .child("Clear")
                                    .on_click(cx.listener(|t, _: &ClickEvent, _, cx| {
                                        t.filter_edit.kinds.clear();
                                        cx.notify();
                                    })),
                            ),
                        ),
                    ),
            )
            .child(h_flex().mt(px(14.)).h(px(36.)).items_start().child(label("Modified by")).child(by_select))
            .child(h_flex().mt(px(14.)).items_start().child(label("Date modified")).child(v_flex().gap(px(14.)).child(date(&self.date_from, "Start date", cx)).child(date(&self.date_to, "End date", cx))))
            .child(
                h_flex()
                    .justify_end()
                    .gap(px(4.))
                    .pt(px(14.))
                    .pb(px(10.))
                    .pr(px(5.))
                    .child(os::secondary_button("hist-f-reset", "Reset", cx).on_click(cx.listener(|t, _: &ClickEvent, window, cx| {
                        t.filter_edit = HistoryFilter::default();
                        t.filter = HistoryFilter::default();
                        t.date_from.update(cx, |s, cx| s.set_value("", window, cx));
                        t.date_to.update(cx, |s, cx| s.set_value("", window, cx));
                        cx.notify();
                    })))
                    .child(os::primary_button("hist-f-search", "Search").on_click(cx.listener(|t, _: &ClickEvent, _, cx| {
                        t.read_dates(cx);
                        t.filter = t.filter_edit.clone();
                        t.filter_open = false;
                        cx.notify();
                    }))),
            )
            .into_any_element()
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

    /// Compare: the document enters compare mode between what the window
    /// shows (base: the workspace, or the version viewed) and the row
    /// (target) (live ui2/950–954).
    fn compare_with_current(&mut self, target: crate::compare_mode::Side, label: String, cx: &mut Context<Self>) {
        let base = {
            let s = &self.doc.read(cx).session;
            match s.viewing {
                Some(mv) => (crate::compare_mode::Side::Entry(mv), s.viewing_version().unwrap_or_else(|| "History".into())),
                None => (crate::compare_mode::Side::Workspace(s.branch), s.store.branch(s.branch).map(|b| b.name).unwrap_or_else(|_| "Main".into())),
            }
        };
        if let Err(e) = crate::compare_mode::compare(&self.doc, base, (target, label), cx) {
            self.error(e, cx);
        }
        cx.notify();
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

    /// Workspace row › Delete, confirmed: the branch is marked deleted (☰ ›
    /// Restore deleted workspaces… brings it back). Deleting the open
    /// workspace first switches to the default one, as Onshape redirects.
    fn delete_workspace(&mut self, b: BranchId, name: String, cx: &mut Context<Self>) {
        let r = self.doc.update(cx, |d, _| -> anyhow::Result<bool> {
            let current = d.session.branch == b;
            if current {
                let main = d.session.store.default_branch()?;
                d.session.switch_branch(main)?;
            }
            d.session.store.delete_branch(b)?;
            Ok(current)
        });
        match r {
            Ok(current) => {
                self.after_change(cx);
                // Onshape's notices (i18n strings).
                let msg = if current { "Workspace has been deleted. Redirected to your default workspace.".to_string() } else { format!("Deleted workspace {name}.") };
                self.doc.update(cx, |_, cx| cx.emit(DocEvent::Info(msg)));
            }
            Err(e) => self.error(e, cx),
        }
    }

}

/// Workspace row › Delete: Onshape's confirmation (help image
/// delete-workspace-dialog-01): "Delete workspace?", "This will delete
/// workspace **B2**.", Delete | Cancel.
fn confirm_delete_workspace(view: Entity<HistoryPanel>, b: BranchId, name: String, window: &mut Window, cx: &mut App) {
    window.open_dialog(cx, move |dialog, _, cx| {
        let pal = p();
        let (view, n) = (view.clone(), name.clone());
        os::modal(dialog, "Delete workspace?", 600.)
            .child(os::modal_body().child(
                div()
                    .text_size(px(14.))
                    .text_color(pal.text)
                    .child(StyledText::new(format!("This will delete workspace {name}.")).with_highlights([(27..27 + name.len(), FontWeight::BOLD.into())])),
            ))
            .footer(os::modal_footer().child(os::modal_ok(os::primary_button("ok", "Delete"))).child(os::modal_cancel(os::cancel_button("cancel", "Cancel", cx))))
            .on_ok(move |_, _, cx| {
                view.update(cx, |t, cx| t.delete_workspace(b, n.clone(), cx));
                true
            })
    });
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

/// Store meta key marking a version as published FeatureScript ("Publish
/// custom features and tables"; advanced search › Published FeatureScript).
pub const PUBLISHED_FS_KEY: &str = noshape_ops::versions::PUBLISHED_FS_KEY;

/// Onshape's "Create version from **<workspace>**" dialog (Versions and
/// history, the rail button, Ctrl+Alt+V; live document/20).
pub fn open_create_version(doc: Entity<DocModel>, window: &mut Window, cx: &mut App) {
    create_version_dialog(doc, false, None, window, cx)
}

/// The same dialog opened from inside a tab (Feature Studio reference panel,
/// "Create a version in …" links): "Create version in **<document>** Main"
/// (live dom/fs-refpanel2-create-version-current.txt).
pub fn open_create_version_in(doc: Entity<DocModel>, window: &mut Window, cx: &mut App) {
    create_version_dialog(doc, true, None, window, cx)
}

/// Change row › Create version here… (live rp-904): the same dialog, the
/// version made at that history entry.
fn open_create_version_at(doc: Entity<DocModel>, at: MicroversionId, window: &mut Window, cx: &mut App) {
    create_version_dialog(doc, false, Some(at), window, cx)
}

fn create_version_dialog(doc: Entity<DocModel>, in_document: bool, at: Option<MicroversionId>, window: &mut Window, cx: &mut App) {
    let doc_name = doc.read(cx).name();
    // Live: the publish row shows when the document has a Feature Studio.
    let has_fs = doc.read(cx).session.doc.tabs.iter().any(|t| matches!(t.content, TabContent::FeatureStudio(_)));
    let publish = Rc::new(std::cell::Cell::new(false));
    // "Create version and edit properties": open the new version's Properties.
    let edit_after = Rc::new(std::cell::Cell::new(false));
    let (branch, n) = {
        let s = &doc.read(cx).session;
        let g = s.store.graph().ok();
        let name = g.as_ref().and_then(|g| g.branches.iter().find(|b| b.id == s.branch).map(|b| b.name.clone())).unwrap_or_else(|| "Main".into());
        (name, g.map(|g| g.versions.len()).unwrap_or(0) + 1)
    };
    let name = cx.new(|cx| InputState::new(window, cx).placeholder("Name").default_value(format!("V{n}")));
    let desc = cx.new(|cx| TextareaState::new(window, cx).rows(4).placeholder("Description"));
    let create: CreateFn = {
        let (doc, name, desc, publish, edit_after) = (doc.clone(), name.clone(), desc.clone(), publish.clone(), edit_after.clone());
        let done = std::cell::Cell::new(false);
        Rc::new(move |window, cx| {
            let n = name.read(cx).value().trim().to_string();
            if n.is_empty() {
                return false;
            }
            // Enter reaches both the field and the dialog: create once.
            if done.replace(true) {
                return true;
            }
            let d = desc.read(cx).value().to_string();
            let publish = publish.get();
            let r = doc.update(cx, |doc, _| {
                let v = match at {
                    Some(mv) => doc.session.store.create_version(mv, &n, &d)?,
                    None => doc.session.create_version(&n, &d)?,
                };
                if publish {
                    doc.session.store.set_meta(&format!("{PUBLISHED_FS_KEY}{v}"), Some(b"1"))?;
                }
                anyhow::Ok(v)
            });
            let edit = edit_after.get();
            doc.update(cx, |doc, cx| match &r {
                Ok(_) => doc.reloaded(cx),
                Err(e) => cx.emit(DocEvent::Error(format!("{e:#}"))),
            });
            if let (Ok(v), true) = (r, edit) {
                let doc = doc.clone();
                window.defer(cx, move |window, cx| crate::version_properties::open(doc, crate::version_properties::Item::Version(v), window, cx));
            }
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
        let title = if in_document { version_dialog_title_in(&doc_name, &branch) } else { version_dialog_title("Create version from", &branch) };
        let (create2, edit_after) = (create.clone(), edit_after.clone());
        os::modal(dialog, "", 950.)
            .title(title)
            .child(version_dialog_body(&name, &desc, None))
            .when(has_fs, |d| d.child(publish_row(publish.clone(), cx)))
            .footer(
                os::modal_footer()
                    .child(os::modal_ok(os::primary_button("ok", "Create")))
                    .child(os::cancel_button("create-edit", "Create version and edit properties", cx).on_click(move |_, window, cx| {
                        edit_after.set(true);
                        if create2(window, cx) {
                            window.close_dialog(cx);
                        }
                    }))
                    .child(os::modal_cancel(os::cancel_button("cancel", "Cancel", cx))),
            )
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
    // "Create workspace and edit properties": open the new workspace's Properties.
    let edit_after = Rc::new(std::cell::Cell::new(false));
    let create: CreateFn = {
        let (doc, name, desc, open, edit_after) = (doc.clone(), name.clone(), desc.clone(), open.clone(), edit_after.clone());
        let done = std::cell::Cell::new(false);
        Rc::new(move |window, cx| {
            let n = name.read(cx).value().trim().to_string();
            if n.is_empty() {
                return false;
            }
            // Enter reaches both the field and the dialog: create once.
            if done.replace(true) {
                return true;
            }
            let switch = open.get();
            let d = desc.read(cx).value().to_string();
            let r = doc.update(cx, |doc, _| {
                let b = if switch { doc.session.branch_from(from, &n)? } else { doc.session.store.create_branch(from, &n)? };
                if !d.is_empty() {
                    doc.session.store.set_meta(&format!("{}{b}", crate::version_properties::WORKSPACE_DESCRIPTION_KEY), Some(d.as_bytes()))?;
                }
                anyhow::Ok(b)
            });
            doc.update(cx, |doc, cx| match &r {
                Ok(_) => doc.reloaded(cx),
                Err(e) => cx.emit(DocEvent::Error(format!("{e:#}"))),
            });
            if let (Ok(b), true) = (r, edit_after.get()) {
                let doc = doc.clone();
                window.defer(cx, move |window, cx| crate::version_properties::open(doc, crate::version_properties::Item::Workspace(b), window, cx));
            }
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
        let (create2, edit_after) = (create.clone(), edit_after.clone());
        os::modal(dialog, "", 950.)
            .title(version_dialog_title("Create workspace from", &from_name))
            .child(version_dialog_body(&name, &desc, Some(open)))
            .footer(
                os::modal_footer()
                    .child(os::modal_ok(os::primary_button("ok", "Create")))
                    .child(os::cancel_button("create-edit", "Create workspace and edit properties", cx).on_click(move |_, window, cx| {
                        edit_after.set(true);
                        if create2(window, cx) {
                            window.close_dialog(cx);
                        }
                    }))
                    .child(os::modal_cancel(os::cancel_button("cancel", "Cancel", cx))),
            )
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
    version_dialog_title_w(lead, name, 950.)
}

fn version_dialog_title_w(lead: &str, name: &str, width: f32) -> AnyElement {
    let pal = p();
    h_flex()
        .w(px(width - 2.))
        .h(px(48.))
        .pl(px(10.))
        .pr(px(12.))
        .gap(px(6.))
        .border_b_1()
        .border_color(rgb(os::MODAL_DIVIDER))
        .child(div().text_size(px(18.)).font_weight(FontWeight::NORMAL).text_color(pal.text_muted).child(lead.to_string()))
        .child(div().text_size(px(18.)).font_weight(FontWeight::SEMIBOLD).text_color(pal.text).child(crate::ui::user_text(name.to_string())))
        .child(div().flex_1())
        .child(div().flex_none().child(gpui_kit::component::dialog::DialogClose::new().child(div().id("modal-close").size(px(22.)).flex().items_center().justify_center().cursor_pointer().text_color(pal.text_faint).child(gpui_kit::component::Icon::empty().data(os::GLYPH_CLOSE).size(px(16.))))))
        .into_any_element()
}

/// "Create version in **noshape mm test** Main": gray 18 px lead-in, the
/// document 500, the workspace 16 px gray.
fn version_dialog_title_in(doc_name: &str, workspace: &str) -> AnyElement {
    let pal = p();
    h_flex()
        .w(px(948.))
        .h(px(48.))
        .pl(px(10.))
        .pr(px(12.))
        .gap(px(6.))
        .border_b_1()
        .border_color(rgb(os::MODAL_DIVIDER))
        .child(div().text_size(px(18.)).font_weight(FontWeight::NORMAL).text_color(pal.text_muted).child("Create version in"))
        .child(div().text_size(px(18.)).font_weight(FontWeight::SEMIBOLD).text_color(pal.text).child(crate::ui::user_text(doc_name.to_string())))
        .child(div().text_size(px(16.)).font_weight(FontWeight::NORMAL).text_color(pal.text_muted).child(crate::ui::user_text(workspace.to_string())))
        .child(div().flex_1())
        .child(div().flex_none().child(gpui_kit::component::dialog::DialogClose::new().child(div().id("modal-close").size(px(22.)).flex().items_center().justify_center().cursor_pointer().text_color(pal.text_faint).child(gpui_kit::component::Icon::empty().data(os::GLYPH_CLOSE).size(px(16.))))))
        .into_any_element()
}

/// "☐ Publish custom features and tables  *Allow searches…*  ?" (live dom
/// fs-refpanel2-create-version-current: 10 px padding, `#eaeaea` rule
/// above, bold label, italic hint, help icon at the right).
fn publish_row(on: Rc<std::cell::Cell<bool>>, cx: &App) -> AnyElement {
    let pal = p();
    let checked = on.get();
    h_flex()
        .id("publish-fs")
        .w_full()
        .p(px(10.))
        .gap(px(8.))
        .border_t_1()
        .border_color(rgb(0xeaeaea))
        .cursor_pointer()
        .child(os::checkbox("publish-fs-box", "", checked))
        .child(div().text_size(px(14.)).font_weight(FontWeight::BOLD).text_color(pal.text).child("Publish custom features and tables"))
        .child(div().flex_1().text_size(px(14.)).italic().text_color(pal.text).child("Allow searches for published FeatureScript to find custom features and tables defined in this version"))
        .child(div().id("publish-fs-help").size(px(16.)).flex_none().tooltip(crate::ui::tooltip("Published custom features and tables can be found with the advanced search's Published FeatureScript criterion")).child(crate::assets::icon("help-faint", 16., cx)))
        .on_click(move |_, window, _| {
            on.set(!on.get());
            window.refresh();
        })
        .into_any_element()
}

/// Version row › Publish FeatureScript… (live document/74): "Publish
/// custom features and tables at **V3**", the requirements with a green ✓
/// or red ✗ each, and the Publish button enabled once all are met.
/// ("Document is public" is a cloud sharing state and is not listed.)
fn open_publish_dialog(doc: Entity<DocModel>, id: noshape_store::VersionId, window: &mut Window, cx: &mut App) {
    let key = format!("{PUBLISHED_FS_KEY}{id}");
    let Ok(info) = doc.read(cx).session.store.version(id) else { return };
    let checks = doc.read(cx).session.store.read_all(info.microversion).ok().and_then(|b| noshape_model::document::Document::from_blobs(&b).ok()).map(|d| noshape_ops::versions::publish_checklist(&d)).unwrap_or_default();
    // Compiling the version's studios replaced the current ones' cached results.
    let _ = doc.read(cx).custom_features();
    let ready = !checks.is_empty() && checks.iter().all(|(_, ok)| *ok);
    window.open_dialog(cx, move |dialog, _, cx| {
        let (doc, key) = (doc.clone(), key.clone());
        let pal = p();
        let rows = checks.iter().map(|(text, ok)| {
            let mark = if *ok {
                div().size(px(16.)).flex_none().text_color(rgb(0x009400)).child(gpui_kit::component::Icon::empty().data(os::GLYPH_CHECK_BOLD).size(px(16.)))
            } else {
                div().size(px(16.)).flex_none().flex().items_center().justify_center().text_color(rgb(0xbd3039)).child(gpui_kit::component::Icon::empty().data(os::GLYPH_CROSS_BOLD).size(px(10.)))
            };
            h_flex().h(px(22.)).gap(px(4.)).child(mark).child(div().child(*text))
        });
        os::modal(dialog, "", 598.)
            .title(version_dialog_title_w("Publish custom features and tables at", &info.name, 598.))
            .child(
                v_flex()
                    .px(px(15.))
                    // The kit leaves 8 px under the title.
                    .pt(px(4.))
                    .pb(px(9.))
                    .text_size(px(14.))
                    .line_height(px(21.))
                    .text_color(pal.text)
                    .child(div().child("Allow searches for published FeatureScript to find custom features and tables defined in this version"))
                    .child(v_flex().mt(px(9.)).pl(px(7.)).child(div().h(px(22.)).pl(px(3.)).font_weight(FontWeight::BOLD).child("Requirements to publish:")).children(rows)),
            )
            .footer(
                os::modal_footer()
                    .child(div().size(px(16.)).flex_none().child(crate::assets::icon("help-faint", 16., cx)))
                    .child(div().flex_1())
                    // Live's disabled primary is the blue at 40 % (162,185,223); `on_ok` ignores it.
                    .child(os::modal_ok(div().when(!ready, |d| d.opacity(0.4)).child(os::primary_button("ok", "Publish custom features and tables"))))
                    .child(os::modal_cancel(os::cancel_button("cancel", "Cancel", cx))),
            )
            .on_ok(move |_, _, cx| {
                if !ready {
                    return false;
                }
                let r = doc.update(cx, |doc, _| doc.session.store.set_meta(&key, Some(b"1")));
                if let Err(e) = r {
                    doc.update(cx, |_, cx| cx.emit(DocEvent::Error(format!("{e:#}"))));
                }
                true
            })
    });
}


fn version_dialog_body(name: &Entity<InputState>, desc: &Entity<TextareaState>, open: Option<Rc<std::cell::Cell<bool>>>) -> AnyElement {
    let pal = p();
    let label = |t: &str| div().text_size(px(14.)).font_weight(FontWeight::SEMIBOLD).text_color(pal.text).child(t.to_string());
    os::modal_body()
        .gap(px(10.))
        .child(h_flex().gap(px(4.)).child(label("Name:")).child(div().w(px(300.)).child(os::input_h(Input::new(name), 34.).text_size(px(14.)))))
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
        let filtering = self.filter.is_active();
        let hide_merges = self.hide_merges;
        for r in rows.iter().filter(|r| q.is_empty() || r.name.to_lowercase().contains(&q)) {
            placed.push(Placed { row: r.clone(), y, change: false });
            y += ROW_H;
            // Hide merges (on by default) drops merge microversions from the
            // change lists; an applied filter lists the matching changes.
            let changes: Vec<MicroversionId> = r
                .changes
                .iter()
                .filter(|mv| info.get(*mv).is_some_and(|m| !(hide_merges && m.parents.len() > 1)))
                .filter(|mv| !filtering || info.get(*mv).is_some_and(|m| self.change_matches(m, cx)))
                .copied()
                .collect();
            if (filtering || self.expanded.contains(&r.key)) && !changes.is_empty() {
                for mv in &changes {
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
        // Releases: versions with a release package get a triangle (solid;
        // with a dot when it contains an obsoletion).
        let releases = crate::release_dialog::release_log(self.doc.read(cx));
        let mut packages_at: HashMap<MicroversionId, Vec<noshape_model::release::Package>> = HashMap::new();
        for v in &graph.versions {
            for pkg in releases.at_version(&v.id.to_string()) {
                packages_at.entry(v.microversion).or_default().push(pkg.clone());
            }
        }
        let mark = |mv: &MicroversionId| -> u8 {
            match packages_at.get(mv) {
                Some(p) if p.iter().any(|p| p.kind == noshape_model::release::PackageKind::Obsoletion) => 2,
                Some(_) => 1,
                None => 0,
            }
        };
        let nodes: Vec<(usize, f32, RowKind, u8)> = placed.iter().map(|p| (p.row.lane, p.y + 13., p.row.kind, if p.row.kind == RowKind::Version { mark(&p.row.mv) } else { 0 })).collect();
        let graph_canvas = canvas(
            |_, _, _| {},
            move |bounds, _, window, _| {
                let o = bounds.origin;
                let pt = |x: f32, y: f32| point(o.x + px(x), o.y + px(y));
                let color = |l: usize| rgb(LANES[l % LANES.len()]);
                for &(l, a, b) in &segs {
                    let mut pb = PathBuilder::stroke(px(4.));
                    pb.move_to(pt(lane_x(l), a));
                    pb.line_to(pt(lane_x(l), b));
                    if let Ok(path) = pb.build() {
                        window.paint_path(path, color(l));
                    }
                }
                for &(l, parent, from_y, to_y) in &forks {
                    let (x0, x1) = (lane_x(l), lane_x(parent));
                    let mid = (to_y - 13.).max(from_y);
                    let mut pb = PathBuilder::stroke(px(4.));
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
                for &(l, y, kind, release) in &nodes {
                    let c = pt(lane_x(l), y);
                    if release > 0 {
                        // Release ▲ (with a white dot: contains obsoletion).
                        let mut f = PathBuilder::fill();
                        f.add_polygon(&[point(c.x, c.y - px(6.)), point(c.x + px(6.), c.y + px(4.5)), point(c.x - px(6.), c.y + px(4.5))], true);
                        if let Ok(path) = f.build() {
                            window.paint_path(path, color(l));
                        }
                        if release == 2 {
                            let mut f = PathBuilder::fill();
                            f.add_polygon(&circle(point(c.x, c.y + px(1.)), 1.6), true);
                            if let Ok(path) = f.build() {
                                window.paint_path(path, rgb(0xffffff));
                            }
                        }
                        continue;
                    }
                    match kind {
                        RowKind::Workspace(_) => {
                            let mut f = PathBuilder::fill();
                            // Live 22: a 7 px ring, 3 px thick.
                            f.add_polygon(&circle(c, 7.), true);
                            if let Ok(path) = f.build() {
                                window.paint_path(path, color(l));
                            }
                            let mut f = PathBuilder::fill();
                            f.add_polygon(&circle(c, 4.), true);
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
                        // Live 22: Start is a filled dot with a stub down to a
                        // 12 px bar 8 px below it.
                        RowKind::Start => {
                            let mut f = PathBuilder::fill();
                            f.add_polygon(&circle(c, 4.5), true);
                            if let Ok(path) = f.build() {
                                window.paint_path(path, color(l));
                            }
                            let mut pb = PathBuilder::stroke(px(4.));
                            pb.move_to(c);
                            pb.line_to(point(c.x, c.y + px(8.)));
                            if let Ok(path) = pb.build() {
                                window.paint_path(path, color(l));
                            }
                            let mut pb = PathBuilder::stroke(px(2.5));
                            pb.move_to(point(c.x - px(6.), c.y + px(8.)));
                            pb.line_to(point(c.x + px(6.), c.y + px(8.)));
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
        // Compare mode (live 952): the base row light blue with a blue bar,
        // the target row light pink with a red bar.
        let comparing = crate::compare_mode::rows(&self.doc, cx);
        // "Compare history entries": the first entry picked is blue.
        let first_pick = crate::compare_mode::first_pick(&self.doc, cx);
        let repair_picking = crate::repair_mode::picking(&self.doc, cx);
        // Repair is offered from a Part Studio (live rp-900).
        let repair_here = {
            let s = &self.doc.read(cx).session;
            s.active_tab.and_then(|t| s.doc.tab(t)).is_some_and(|t| matches!(t.content, TabContent::PartStudio(_)))
        };
        let mut list: Vec<AnyElement> = Vec::new();
        // A version made with no change since the row before it shares its
        // microversion (V1 right after Start): the version row is the one meant.
        let version_at = |mv: MicroversionId| placed.iter().any(|p| p.row.kind == RowKind::Version && p.row.mv == mv);
        for pl in &placed {
            let r = pl.row.clone();
            let is_row = |side: crate::compare_mode::Side| match r.kind {
                RowKind::Workspace(b) => side == crate::compare_mode::Side::Workspace(b),
                RowKind::Version => side == crate::compare_mode::Side::Entry(r.mv),
                _ => side == crate::compare_mode::Side::Entry(r.mv) && !version_at(r.mv),
            };
            let compare_side = comparing.map(|(base, target)| (is_row(base), is_row(target))).filter(|(b, t)| *b || *t);
            // Compare history entries: the first pick is selected (live cmp2-902).
            let picked = first_pick.is_some_and(is_row);
            let selected = comparing.is_none()
                && match r.kind {
                    RowKind::Workspace(b) => b == current_branch && viewing.is_none(),
                    _ => viewing == Some(r.mv),
                };
            let (mv, kind, name) = (r.mv, r.kind, r.name.clone());
            // Clicking a row leaves compare mode and opens it (live 954).
            let repair_label = if kind == RowKind::Change { format!("{current_name} :: {name}") } else { name.clone() };
            let repair_label2 = repair_label.clone();
            let pick_label = name.clone();
            let open = cx.listener(move |this, e: &ClickEvent, _, cx| match kind {
                // "Compare history entries" is on: the first pick is the base,
                // the second the target (help "Comparing").
                _ if crate::compare_mode::picking(&this.doc, cx) => {
                    let side = match kind {
                        RowKind::Workspace(b) => crate::compare_mode::Side::Workspace(b),
                        _ => crate::compare_mode::Side::Entry(mv),
                    };
                    if let Err(err) = crate::compare_mode::pick(&this.doc, side, pick_label.clone(), cx) {
                        this.error(err, cx);
                    }
                    cx.notify();
                }
                // The header wrench is on: this entry opens in Repair.
                RowKind::Version | RowKind::Change | RowKind::Start if crate::repair_mode::picking(&this.doc, cx) => {
                    if let Err(err) = crate::repair_mode::open(&this.doc, mv, repair_label.clone(), cx) {
                        this.error(err, cx);
                    }
                }
                RowKind::Workspace(b) if b != current_branch => {
                    if e.click_count() >= 2 {
                        this.switch_branch(b, cx)
                    }
                }
                RowKind::Workspace(_) => {
                    crate::compare_mode::stop(&this.doc, cx);
                    this.view_mv(None, cx)
                }
                _ => {
                    crate::compare_mode::stop(&this.doc, cx);
                    this.view_mv(Some(mv), cx)
                }
            });
            let (v, d, cur) = (view.clone(), doc.clone(), current_name.clone());
            let row_packages = if r.kind == RowKind::Version { packages_at.get(&mv).cloned().unwrap_or_default() } else { Vec::new() };
            let row = if pl.change {
                h_flex()
                    .id(SharedString::from(format!("hist-{}", r.key)))
                    .h(px(CHANGE_H))
                    .flex_none()
                    .cursor_pointer()
                    .text_size(px(13.))
                    .text_color(pal.text_muted)
                    // Live: #b2ddf6, dark #092046 (dk-9a0), not the list's #134e96.
                    .when(selected, |d| d.bg(crate::theme::themed(0xb2ddf6, 0x092046)))
                    .when_some(compare_side, |d, (base, _)| {
                        let (fill, bar) = if base { (crate::compare_mode::BASE_ROW, crate::compare_mode::BASE_BLUE) } else { (crate::compare_mode::TARGET_ROW, crate::compare_mode::TARGET_RED) };
                        d.bg(rgb(fill)).border_l(px(3.)).border_color(rgb(bar))
                    })
                    .when(picked, |d| d.bg(rgb(crate::compare_mode::BASE_ROW)))
                    .when(!selected && compare_side.is_none(), |d| d.hover(|d| d.bg(pal.panel_hover)))
                    .child(div().w(px(GRAPH_W + 5.)).flex_none())
                    .child(div().w(px(115.)).flex_none().truncate().child(crate::ui::user_text(r.name.clone())))
                    .child(div().pl(px(3.)).flex_1().min_w_0().truncate().child(history_time(r.time)))
            } else {
                let changes = r.changes.len();
                let key = r.key.clone();
                let open_changes = self.filter.is_active() || self.expanded.contains(&key);
                let name_el = match r.kind {
                    // A protected workspace carries a lock (help "Workspace Protections").
                    // A protected workspace's chip starts with a lock (live wp-929).
                    RowKind::Workspace(b) => h_flex().child(
                        h_flex()
                            .gap(px(2.))
                            .px(px(3.))
                            .rounded(px(3.))
                            .border_1()
                            .border_color(rgb(LANES[r.lane % LANES.len()]))
                            .bg(if r.lane == 0 { rgb(0xd0dbe5) } else { rgb(0xfff4d6) })
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(pal.text)
                            .when(self.doc.read(cx).session.branch_protected(b), |d| d.child(crate::assets::icon("lock", 12., cx)))
                            .child(crate::ui::user_text(r.name.clone())),
                    ),
                    _ => div().font_weight(FontWeight::MEDIUM).text_color(pal.text).child(crate::ui::user_text(r.name.clone())),
                };
                h_flex()
                    .id(SharedString::from(format!("hist-{}", r.key)))
                    .h(px(ROW_H))
                    .flex_none()
                    .items_start()
                    .pt(px(5.))
                    .cursor_pointer()
                    .text_size(px(14.))
                    .line_height(px(21.))
                    .text_color(pal.text_muted)
                    // Live: #b2ddf6, dark #092046 (dk-9a0), not the list's #134e96.
                    .when(selected, |d| d.bg(crate::theme::themed(0xb2ddf6, 0x092046)))
                    .when_some(compare_side, |d, (base, _)| {
                        let (fill, bar) = if base { (crate::compare_mode::BASE_ROW, crate::compare_mode::BASE_BLUE) } else { (crate::compare_mode::TARGET_ROW, crate::compare_mode::TARGET_RED) };
                        d.bg(rgb(fill)).border_l(px(3.)).border_color(rgb(bar))
                    })
                    .when(picked, |d| d.bg(rgb(crate::compare_mode::BASE_ROW)))
                    .when(!selected && compare_side.is_none(), |d| d.hover(|d| d.bg(pal.panel_hover)))
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
            let version_id = if r.kind == RowKind::Version { graph.versions.iter().find(|x| x.microversion == mv && x.name == r.name).map(|x| x.id) } else { None };
            // Main (the default workspace) can't be deleted (help: Versions and history).
            let deletable = matches!(r.kind, RowKind::Workspace(b) if graph.branches.iter().any(|x| x.id == b && !x.is_default));
            // A version of another workspace merges into this one too (help
            // "Merging": "merge a version from another branch").
            let other_version = r.kind == RowKind::Version && graph.microversions.iter().find(|m| m.id == mv).is_some_and(|m| m.branch != current_branch);
            let row = row.on_click(open).os_context_menu(move |m, window, cx| {
                // Live (menus/rmb-version-v1, dk-9a1): a Bootstrap dropdown's
                // 27 px rows and 14 px text, ~222 px wide.
                let m = m.row_height(27.).text_px(14.).text_inset(20.).min_width(222.);
                // Live menus/rmb-version-v1: Open, Branch to create workspace…,
                // Compare, Restore to <ws>…, Properties…, Releases ▸, Publish
                // FeatureScript… (browser tab / copy link are cloud-only).
                let (dr0, row_packages) = (d.clone(), row_packages.clone());
                let releases = move |m: crate::os_menu::OsMenu, window: &mut Window, cx: &mut Context<crate::os_menu::OsMenu>| {
                    if row_packages.is_empty() {
                        return m;
                    }
                    let (pk, dr) = (row_packages.clone(), dr0.clone());
                    m.submenu("Releases", window, cx, move |mut sm, _, _| {
                        for p in pk.clone() {
                            let dr = dr.clone();
                            let state = if p.kind == noshape_model::release::PackageKind::Obsoletion { "Obsolete" } else { "Released" };
                            sm = sm.item(crate::os_menu::OsMenuItem::new(format!("{} [{state}]", p.name)).on_click(move |_, window, cx| crate::release_dialog::open_review(dr.clone(), &p, window, cx)));
                        }
                        sm
                    })
                };
                let (v1, v3, v4) = (v.clone(), v.clone(), v.clone());
                let (d5, d6) = (d.clone(), d.clone());
                let (name1, name2, cur1) = (name.clone(), name.clone(), cur.clone());
                let (v5r, rl) = (v.clone(), repair_label2.clone());
                let d1 = d.clone();
                match kind {
                    RowKind::Workspace(b) => {
                        let m = m.item(crate::os_menu::OsMenuItem::new("Open").on_click(move |_, _, cx| v1.update(cx, |t, cx| if b == current_branch { t.view_mv(None, cx) } else { t.switch_branch(b, cx) })));
                        // Live wp-901: Open · Compare · Merge into current workspace… ·
                        // Workspace protection… · Properties… | Delete (browser tab /
                        // copy link are web-only).
                        let m = if b != current_branch {
                            m.item(crate::os_menu::OsMenuItem::new("Compare").on_click(move |_, _, cx| v3.update(cx, |t, cx| t.compare_with_current(crate::compare_mode::Side::Workspace(b), name2.clone(), cx))))
                                .item(crate::os_menu::OsMenuItem::new("Merge into current workspace…").on_click({
                                    let (dm, nm) = (d5.clone(), name.clone());
                                    move |_, window, cx| crate::merge_dialog::open(dm.clone(), crate::compare_mode::Side::Workspace(b), nm.to_string(), window, cx)
                                }))
                        } else {
                            m
                        };
                        let dp = d5.clone();
                        let m = m.item(crate::os_menu::OsMenuItem::new("Workspace protection…").on_click(move |_, window, cx| crate::history_hooks::workspace_protection(dp.clone(), b, window, cx)));
                        let m = m.item(crate::os_menu::OsMenuItem::new("Properties…").on_click(move |_, window, cx| crate::version_properties::open(d5.clone(), crate::version_properties::Item::Workspace(b), window, cx)));
                        // Live (qa5 DOC4): Delete on every workspace row; Main can't be
                        // deleted (help), so it is greyed there.
                        let (vd, nd) = (v4.clone(), name.clone());
                        m.separator().item(crate::os_menu::OsMenuItem::new("Delete").disabled(!deletable).on_click(move |_, window, cx| confirm_delete_workspace(vd.clone(), b, nd.to_string(), window, cx)))
                    }
                    _ => {
                        let m = m.item(crate::os_menu::OsMenuItem::new("Open").on_click(move |_, _, cx| v1.update(cx, |t, cx| t.view_mv(Some(mv), cx))));
                        // Live rp-904: change rows have no "Branch to create workspace…".
                        let m = if kind == RowKind::Change { m } else { m.item(crate::os_menu::OsMenuItem::new("Branch to create workspace…").on_click(move |_, window, cx| open_branch_dialog(d1.clone(), mv, name1.clone(), window, cx))) };
                        let m = m
                            .item(crate::os_menu::OsMenuItem::new("Compare").on_click(move |_, _, cx| v3.update(cx, |t, cx| t.compare_with_current(crate::compare_mode::Side::Entry(mv), name2.clone(), cx))));
                        let m = if other_version {
                            let (dm, nm) = (d.clone(), name.clone());
                            m.item(crate::os_menu::OsMenuItem::new("Merge into current workspace…").on_click(move |_, window, cx| crate::merge_dialog::open(dm.clone(), crate::compare_mode::Side::Entry(mv), nm.to_string(), window, cx)))
                        } else {
                            m
                        };
                        let m = m
                            // Live rp-904/905: change and version rows.
                            .item(crate::os_menu::OsMenuItem::new("View in repair").on_click(move |_, _, cx| {
                                let doc = v5r.read(cx).doc.clone();
                                if let Err(e) = crate::repair_mode::open(&doc, mv, rl.clone(), cx) {
                                    v5r.update(cx, |t, cx| t.error(e, cx));
                                }
                            }))
                            .item(crate::os_menu::OsMenuItem::new(format!("Restore to {cur1}…")).on_click(move |_, _, cx| v4.update(cx, |t, cx| t.restore(mv, cx))));
                        // Live rp-904: a change row ends with "Create version here…".
                        let dc = d.clone();
                        let m = if kind == RowKind::Change { m.item(crate::os_menu::OsMenuItem::new("Create version here…").on_click(move |_, window, cx| open_create_version_at(dc.clone(), mv, window, cx))) } else { m };
                        match version_id {
                            Some(id) => {
                                let m = m.item(crate::os_menu::OsMenuItem::new("Properties…").on_click(move |_, window, cx| crate::version_properties::open(d5.clone(), crate::version_properties::Item::Version(id), window, cx)));
                                releases(m, window, cx).item(crate::os_menu::OsMenuItem::new("Publish FeatureScript…").on_click(move |_, window, cx| open_publish_dialog(d6.clone(), id, window, cx)))
                            }
                            None => releases(m, window, cx),
                        }
                    }
                }
            });
            list.push(row.into_any_element());
        }

        let doc2 = self.doc.clone();
        let legend_item = |icon: AnyElement, label: &'static str, faint: bool| {
            let text = div().text_size(px(14.)).font_weight(if faint { FontWeight::NORMAL } else { FontWeight::SEMIBOLD }).text_color(if faint { rgb(0x888888) } else { pal.text }).child(label);
            // The workspace label sits in the workspace chip.
            let text = if label == "Workspace" { text.px(px(3.)).rounded(px(3.)).border_1().border_color(pal.accent).bg(rgb(0xd0dbe5)) } else { text };
            h_flex().gap(px(4.)).child(icon).child(text)
        };
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
                    .child(div().flex_1().text_size(px(14.)).font_weight(FontWeight::BOLD).text_color(pal.text).child("Versions and history"))
                    // Live order (document/22): release, create version, compare, ×.
                    // Release management: a candidate with the active tab's objects
                    // (Release__typical_release_workflow).
                    .child(crate::ui::icon_button("hist-release", "release-create", 30., false, cx).tooltip("Create release candidate").on_click({
                        let doc = self.doc.clone();
                        move |_, window, cx| {
                            let tab = doc.read(cx).session.active_tab;
                            let items = tab.map(|t| crate::release_dialog::tab_items(doc.read(cx), t)).unwrap_or_default();
                            crate::release_dialog::open_release_candidate(doc.clone(), items, window, cx)
                        }
                    }))
                    .child(crate::ui::icon_button("hist-create-version", "version", 30., false, cx).tooltip("Create version…").on_click(move |_, window, cx| open_create_version(doc2.clone(), window, cx)))
                    // Help "Comparing": pick two history entries (blue, then red).
                    .child(crate::ui::icon_button("hist-compare", "compare", 30., crate::compare_mode::picking(&self.doc, cx), cx).tooltip("Compare history entries").on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        if crate::repair_mode::picking(&this.doc, cx) {
                            crate::repair_mode::toggle_picking(&this.doc, cx);
                        }
                        crate::compare_mode::toggle_picking(&this.doc, cx);
                        cx.notify();
                    })))
                    // Live rp-900/901: "Repair" (Part Studios): pick a history entry to repair from.
                    .when(repair_here, |d| {
                        let doc = self.doc.clone();
                        d.child(crate::ui::icon_button("hist-repair", "repair", 30., repair_picking, cx).tooltip("Repair").on_click(move |_, _, cx| {
                            if crate::compare_mode::picking(&doc, cx) {
                                crate::compare_mode::toggle_picking(&doc, cx);
                            }
                            crate::repair_mode::toggle_picking(&doc, cx)
                        }))
                    })
                    .child(crate::ui::icon_button("hist-close", "close", 30., false, cx).tooltip("Close").on_click(|_, window, cx| window.dispatch_action(Box::new(crate::actions::ToggleVersionGraph), cx))),
            )
            // Live cmp2-901: "Select two history entries to compare."
            .when(crate::compare_mode::picking(&self.doc, cx), |d| {
                d.child(div().mx(px(10.)).mt(px(6.)).px(px(15.)).py(px(7.)).rounded(px(3.)).bg(pal.info_fill).border_1().border_color(pal.info_outline).text_size(px(14.)).text_color(pal.info_text).child("Select two history entries to compare."))
            })
            .when(repair_picking, |d| {
                d.child(div().mx(px(10.)).mt(px(6.)).px(px(15.)).py(px(7.)).rounded(px(3.)).bg(pal.info_fill).border_1().border_color(pal.info_outline).text_size(px(14.)).text_color(pal.info_text).child("Select a history entry to repair this Part Studio"))
            })
            .child(
                // Live document/22: the box with its ▾ (filter panel) and the
                // square search button, 6 px under the header.
                h_flex()
                    .px(px(10.))
                    .pt(px(6.))
                    .flex_none()
                    .child(
                        div()
                            .flex_1()
                            .relative()
                            .h(px(30.))
                            .flex()
                            .items_center()
                            .border_1()
                            .border_color(rgb(0xaaaaaa))
                            .rounded_l(px(4.))
                            .child(div().flex_1().child(Input::new(&self.search).appearance(false).text_size(px(14.)).pl(px(12.))))
                            .child(
                                div()
                                    .id("hist-filter-caret")
                                    .w(px(24.))
                                    .h_full()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .cursor_pointer()
                                    .child(os::glyph(os::GLYPH_CARET, 10., pal.text))
                                    .on_click(cx.listener(|t, _: &ClickEvent, _, cx| {
                                        t.filter_open = !t.filter_open;
                                        cx.notify();
                                    })),
                            )
                            .when(self.filter_open, |d| d.child(deferred(div().absolute().top(px(29.)).right(px(-1.)).child(self.render_filter_panel(cx))).with_priority(2))),
                    )
                    .child(div().size(px(30.)).flex().items_center().justify_center().border_1().border_l_0().border_color(rgb(0xaaaaaa)).rounded_r(px(4.)).child(crate::assets::icon("search", 20., cx))),
            )
            .child(
                h_flex()
                    .px(px(12.))
                    .pt(px(6.))
                    .gap(px(6.))
                    .flex_none()
                    .child(filter_toggle("hist-merges", "hide-merges", self.hide_merges, cx).tooltip(crate::ui::tooltip(if self.hide_merges { "Hide merges" } else { "Show merges" })).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.hide_merges = !this.hide_merges;
                        cx.notify();
                    })))
                    .child(filter_toggle("hist-auto", "hide-auto-versions", self.hide_auto_versions, cx).tooltip(crate::ui::tooltip(if self.hide_auto_versions { "Hide automatic versions" } else { "Show automatic versions" })).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.hide_auto_versions = !this.hide_auto_versions;
                        cx.notify();
                    }))),
            )
            .child(
                // Live DOM: the header row is 38 px, 1 px `#d0dbe5` under it, rows from y 187.
                h_flex()
                    .h(px(38.))
                    .mt(px(3.))
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
            .child(
                div()
                    .id("history-rows")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .child(div().relative().w_full().h(px(total_h)).child(v_flex().children(list)).child(graph_canvas)),
            )
            // Legend (live document/22): three centred rows, 14 px semibold; the
            // workspace label in its chip, "Change" faint. Then the 31 px `#eee`
            // hint bar.
            .child(
                v_flex()
                    .flex_none()
                    .items_center()
                    // Live: rows 33 px apart; the last one 82 px above the hint bar
                    // (Onshape's cloud "Upgrade" button sits in between).
                    .gap(px(10.))
                    .pb(px(71.))
                    .child(h_flex().gap(px(30.)).child(legend_item(crate::assets::icon("legend-workspace", 20., cx), "Workspace", false)).child(legend_item(crate::assets::icon("legend-version", 20., cx), "Version", false)).child(legend_item(crate::assets::icon("legend-change", 20., cx), "Change", true)))
                    .child(h_flex().gap(px(30.)).child(legend_item(crate::assets::icon("legend-release-candidate", 20., cx), "Release candidate", false)).child(legend_item(crate::assets::icon("legend-release", 20., cx), "Release", false)))
                    .child(h_flex().gap(px(30.)).child(legend_item(crate::assets::icon("legend-obsoletion", 20., cx), "Contains obsoletion", false)).child(legend_item(crate::assets::icon("hide-auto-versions", 20., cx), "Automatic version", false))),
            )
            .child(
                h_flex()
                    .h(px(31.))
                    .flex_none()
                    .px(px(5.))
                    .bg(rgb(0xeeeeee))
                    .border_t_1()
                    .border_color(rgb(0xcccccc))
                    .child(div().flex_1().text_size(px(12.)).text_color(pal.text_muted).child("Right-click on a row to perform actions"))
                    .child(crate::assets::icon("help-faint", 16., cx)),
            )
    }
}
