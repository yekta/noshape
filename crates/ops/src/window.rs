//! The document window as the agent API sees it: what a user can do with
//! the window that isn't an edit (the tab shown, panels, version viewing,
//! compare, repair…) lives here per open document, so an agent can do it
//! and read it back (`ui_state`). Each change is also a [`UiRequest`]: with
//! the app open on the document (live mode) the window does the same.

use glam::{DQuat, DVec2, DVec3};
use noshape_model::document::{Document, TabContent, TabId};
use noshape_render::Camera;
use noshape_store::{BranchId, MicroversionId};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

/// A window action an operation asks for. Without the app open there is no
/// window: the reply carries the target and [`WindowState`] the state.
#[derive(Clone, Debug, PartialEq)]
pub enum UiRequest {
    /// Show this tab.
    ActivateTab(TabId),
    /// Open a document in a window: its workspace, or `version`; with
    /// `external_key` (an `ext:` reference) the tab it names is shown.
    OpenDocument { path: PathBuf, version: Option<MicroversionId>, external_key: Option<String> },
    /// The Tab manager: open with this search, or closed.
    TabManager(Option<String>),
    /// A drawing's Sheets panel open / closed (Ctrl+S, "Display sheet menu").
    SheetsPanel { tab: TabId, open: bool },
    /// Enter compare mode, or leave it.
    Compare(Option<CompareRequest>),
    /// The Repair panel on a history entry (`label`), or closed.
    Repair(Option<(MicroversionId, String)>),
    /// A Part Studio's or Assembly's camera move (the window's graphics
    /// area does it, animated, at its own size).
    Camera { tab: TabId, mv: CameraMove },
    /// A drawing's zoom.
    SheetZoom { tab: TabId, mv: SheetMove },
    /// A Part Studio's or Assembly's display setting (view_display).
    Display { tab: TabId, set: DisplaySet },
}

/// A display setting as the view and context menus change it.
#[derive(Clone, Debug, PartialEq)]
pub enum DisplaySet {
    /// Isolate… / Make transparent… on these parts (Part Studio).
    IsolateParts { transparent: bool, parts: Vec<noshape_model::BodyId> },
    /// Isolate… / Make transparent… on these instances (Assembly).
    IsolateInstances { transparent: bool, instances: Vec<noshape_model::assembly::InstanceId> },
    /// Exit isolate.
    ExitIsolate,
    /// View › Highlight boundary edges.
    HighlightBoundary(bool),
    /// View › Orient normal to sketch on edit.
    OrientOnEdit(bool),
    /// Analysis › Turn thickness analysis off.
    ThicknessOff,
    /// Assembly: Hide/show mates (the mate indicators).
    ShowMates(bool),
    /// Assembly: Show mates mode.
    ShowMatesMode(bool),
    /// Assembly: Snap mode.
    SnapMode(bool),
}

/// Make isolate / Make transparent: which, and the parts' (`body:…`) or
/// instances' (`inst:…`) ids.
#[derive(Clone, Debug, PartialEq)]
pub struct Isolation {
    pub transparent: bool,
    pub ids: Vec<String>,
}

/// A 3D tab's display settings as the agent set them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TabDisplay {
    pub isolate: Option<Isolation>,
    pub highlight_boundary: bool,
    pub orient_on_edit: bool,
    pub show_mates: bool,
    pub show_mates_mode: bool,
    pub snap_mode: bool,
}

/// A camera move as the view menus do it.
#[derive(Clone, Debug, PartialEq)]
pub enum CameraMove {
    /// Zoom to fit, at this orientation (a standard view) if given.
    Fit(Option<DQuat>),
    /// Zoom to selection: frame this box (an assembly's instances: as a sphere).
    FitBox { lo: DVec3, hi: DVec3, sphere: bool },
    /// Z / Shift+Z.
    Step { zoom_in: bool },
    /// Zoom to window: the box between two points given as fractions of
    /// the view (0..1, from the top left).
    Window(DVec2, DVec2),
    /// Look along `forward`, `up` up (View normal to, Align view with
    /// vertical axis).
    Direction { forward: DVec3, up: DVec3 },
    /// Previous view.
    Previous,
    /// Perspective view on / off.
    Perspective(bool),
}

/// A drawing zoom.
#[derive(Clone, Debug, PartialEq)]
pub enum SheetMove {
    /// Zoom to fit / Zoom to sheet.
    Fit,
    /// Zoom in (factor > 1) or out about the canvas centre.
    Step(f64),
    /// Zoom to window (fractions of the canvas, as [`CameraMove::Window`]).
    Window([f64; 2], [f64; 2]),
}

/// The size of the agent's graphics area (logical pixels) until set.
pub const VIEWPORT: [f64; 2] = [1280.0, 800.0];

/// A 3D tab's view as the agent drives it.
#[derive(Clone, Debug, PartialEq)]
pub struct ViewCamera {
    pub camera: Camera,
    /// Previous view: the cameras before each change (latest last; 50 kept).
    pub history: Vec<Camera>,
    /// The graphics area's size (logical pixels).
    pub viewport: DVec2,
}

impl ViewCamera {
    /// Keep the camera for Previous view, then change it.
    pub fn set(&mut self, to: Camera) {
        if self.history.last() != Some(&self.camera) {
            self.history.push(self.camera);
            if self.history.len() > 50 {
                self.history.remove(0);
            }
        }
        self.camera = to;
    }
}

/// A drawing's zoom (pixels per mm) and the sheet point (mm) at the canvas
/// centre, for a canvas of `size` pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SheetView {
    pub zoom: f64,
    pub center: [f64; 2],
    pub size: [f64; 2],
}

/// One side of a compare: a workspace's head or a history entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompareSide {
    Workspace(BranchId),
    Entry(MicroversionId),
}

#[derive(Clone, Debug, PartialEq)]
pub struct CompareRequest {
    pub base: CompareSide,
    pub base_label: String,
    pub target: CompareSide,
    pub target_label: String,
}

/// The window's state for one document (session only, as the window's).
#[derive(Clone, Debug, Default)]
pub struct WindowState {
    /// The tab shown (None: the window's first tab).
    pub active_tab: Option<TabId>,
    /// The Tab manager is open, with this search text.
    pub tab_manager: Option<String>,
    /// Drawings whose Sheets panel is open.
    pub sheets_panel: BTreeSet<TabId>,
    /// Compare mode: (base label, target label).
    pub compare: Option<CompareRequest>,
    /// The Repair panel: the entry shown and its label.
    pub repair: Option<(MicroversionId, String)>,
    /// 3D tabs' cameras (set by view_camera).
    pub cameras: BTreeMap<TabId, ViewCamera>,
    /// Drawings' zoom (set by view_camera).
    pub sheets: BTreeMap<TabId, SheetView>,
    /// 3D tabs' display settings (set by view_display).
    pub display: BTreeMap<TabId, TabDisplay>,
}

impl WindowState {
    /// What `r` does to the window.
    pub fn apply(&mut self, r: &UiRequest) {
        match r {
            UiRequest::ActivateTab(t) => self.active_tab = Some(*t),
            UiRequest::OpenDocument { .. } => {}
            UiRequest::TabManager(s) => self.tab_manager = s.clone(),
            UiRequest::SheetsPanel { tab, open } => {
                if *open {
                    self.sheets_panel.insert(*tab);
                } else {
                    self.sheets_panel.remove(tab);
                }
            }
            UiRequest::Compare(c) => self.compare = c.clone(),
            UiRequest::Repair(r) => self.repair = r.clone(),
            // view_camera and view_display keep their state themselves.
            UiRequest::Camera { .. } | UiRequest::SheetZoom { .. } | UiRequest::Display { .. } => {}
        }
    }
}

/// A tab's state in compare mode (the dot after its name, live 952).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TabCompare {
    /// Yellow: "Compare has found differences."
    Differs,
    /// No differences.
    Same,
    /// "Compare not supported for this tab." (drawings; can't be opened).
    NotSupported,
    /// "Tab only exists in '<base>'." (can't be opened).
    OnlyInBase,
}

impl TabCompare {
    pub fn name(self) -> &'static str {
        match self {
            TabCompare::Differs => "differs",
            TabCompare::Same => "same",
            TabCompare::NotSupported => "not_supported",
            TabCompare::OnlyInBase => "only_in_base",
        }
    }
}

/// Tab `tab`'s compare state between `base` and `target` (None: not in the
/// base). Shared by the app's compare mode and the API.
pub fn tab_compare(base: &Document, target: &Document, tab: TabId) -> Option<TabCompare> {
    let b = base.tab(tab)?;
    let Some(t) = target.tab(tab) else { return Some(TabCompare::OnlyInBase) };
    if matches!(b.content, TabContent::Drawing(_)) {
        return Some(TabCompare::NotSupported);
    }
    let same = b.name == t.name && serde_json::to_value(&b.content).ok() == serde_json::to_value(&t.content).ok();
    Some(if same { TabCompare::Same } else { TabCompare::Differs })
}

/// The Tab manager's sort orders.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TabSort {
    TabOrder,
    NameAsc,
    NameDesc,
    Type,
}

/// Tab kinds in Onshape's filter order (the Tab manager's type filters).
pub const TAB_KINDS: [&str; 7] = ["partstudio", "assembly", "drawing", "variables", "featurestudio", "materiallibrary", "file"];

/// The Tab manager's rows: (tab, name, kind) of the tabs matching `query`
/// (case-insensitive, in the name) and `kinds` (none: all), sorted. With
/// no search, no filter and tab order, folders show in folder order (their
/// contents included). `kind_of` names a tab's kind.
pub fn tab_manager_rows(d: &Document, query: &str, kinds: &[&str], sort: TabSort, kind_of: &dyn Fn(&TabContent) -> &'static str) -> Vec<(TabId, String, &'static str)> {
    fn walk(d: &Document, f: Option<TabId>, out: &mut Vec<TabId>) {
        for c in d.children(f) {
            out.push(c);
            if d.is_folder(c) {
                walk(d, Some(c), out);
            }
        }
    }
    let q = query.trim().to_lowercase();
    let tree = q.is_empty() && kinds.is_empty() && sort == TabSort::TabOrder;
    let order: Vec<TabId> = if tree {
        let mut v = Vec::new();
        walk(d, None, &mut v);
        v
    } else {
        d.ordered_tabs().iter().map(|t| t.id).collect()
    };
    let mut out: Vec<(TabId, String, &'static str)> = order
        .into_iter()
        .filter_map(|id| d.tab(id))
        .map(|t| (t.id, t.name.clone(), kind_of(&t.content)))
        .filter(|(_, n, k)| (q.is_empty() || n.to_lowercase().contains(&q)) && (kinds.is_empty() || kinds.contains(k)))
        .collect();
    match sort {
        TabSort::TabOrder => {}
        TabSort::NameAsc => out.sort_by_key(|t| t.1.to_lowercase()),
        TabSort::NameDesc => out.sort_by_key(|t| std::cmp::Reverse(t.1.to_lowercase())),
        TabSort::Type => out.sort_by_key(|t| TAB_KINDS.iter().position(|k| *k == t.2)),
    }
    out
}

/// A tab's kind as the Tab manager filters it (files: "file").
pub fn tab_kind(c: &TabContent) -> &'static str {
    match c {
        TabContent::PartStudio(_) => "partstudio",
        TabContent::Assembly(_) => "assembly",
        TabContent::Drawing(_) => "drawing",
        TabContent::VariableStudio(_) => "variables",
        TabContent::FeatureStudio(_) => "featurestudio",
        TabContent::MaterialLibrary(_) => "materiallibrary",
        TabContent::File(_) => "file",
    }
}
