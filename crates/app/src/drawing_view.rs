//! Drawing tab: a sheet canvas rendered on the GPU with pan/zoom, the
//! drawing toolbar (views, dimensions, annotations, sheet setup, export),
//! selection and dragging of views and annotations, context menus and
//! inline property panels.
//!
//! Data flow: the committed [`Drawing`] lives in the document; while a tool
//! or drag is active the editor shows a *preview* copy. View geometry (HLR)
//! is computed in the background — fast polygonal first, exact afterwards
//! (skipped while interacting) — and cached by content in a [`ViewCache`];
//! the sheet is then assembled on the UI thread (cheap) and converted to
//! render data (see `drawing_scene`). Referenced Part Studios are
//! regenerated in the background; edits to them mark the drawing out of
//! date until Update (Ctrl+Q), like Onshape.

use crate::actions::*;
use crate::doc_model::{DocEvent, DocModel};
use crate::drawing_scene::{self, Overlay};
use crate::gpu::{FrameResult, Gpu};
use crate::theme::p;
use gpui_kit::component::input::{InputState, TextareaState};
use gpui_kit::component::menu::{PopupMenu, PopupMenuItem};
use gpui_kit::component::v_flex;
use gpui_kit::*;
use glam::{DQuat, DVec3};
use noshape_model::document::{TabContent, TabId};
use noshape_model::assembly::eval::AssemblyEvaluator;
use noshape_model::drawing::build::{BuiltSheet, PickTarget, SheetInfo, assemble_sheet};
use noshape_model::drawing::views::{self, SourceKey, Sources, ViewCache, ViewGeometry};
use noshape_model::drawing::*;
use noshape_model::regen::Regenerator;
use noshape_render::worker::RenderJob;
use noshape_render::{Camera, Drawable, Instance, LinesHandle, MeshHandle, PointsHandle, Projection as CamProjection, Scene, Style, srgb, state};
use parking_lot::Mutex;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::Arc;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DimTool {
    Linear,
    Aligned,
    Horizontal,
    Vertical,
    Radius,
    Diameter,
    Angle,
    Chamfer,
    ArcLength,
}

impl DimTool {
    pub fn name(self) -> &'static str {
        match self {
            DimTool::Linear => "Linear dimension",
            DimTool::Aligned => "Aligned dimension",
            DimTool::Horizontal => "Horizontal dimension",
            DimTool::Vertical => "Vertical dimension",
            DimTool::Radius => "Radial dimension",
            DimTool::Diameter => "Diameter dimension",
            DimTool::Angle => "Angular dimension",
            DimTool::Chamfer => "Chamfer dimension",
            DimTool::ArcLength => "Arc length dimension",
        }
    }
}

/// A point picked in a view (view coordinates) with its model reference.
#[derive(Clone, Debug)]
pub struct PickedPoint {
    pub view: ViewId,
    pub p: [f64; 2],
    pub r: Option<PointRef>,
}

#[derive(Clone, Debug)]
pub enum Pick {
    Point(PickedPoint),
    Line { a: PickedPoint, b: PickedPoint },
    Circle { view: ViewId, center: [f64; 2], radius: f64, r: Option<PointRef> },
    /// An arc edge: center, radius, end points and a point on the arc.
    Arc { view: ViewId, center: [f64; 2], radius: f64, a: PickedPoint, b: PickedPoint, mid: [f64; 2], r: Option<PointRef> },
}

/// Kinds of drawing tables the table tools create.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TableTool {
    General,
    Bom,
    Hole,
}

#[derive(Clone, Debug)]
pub enum Tool {
    Select,
    InsertView { source: TabId, orientation: ViewOrientation, part: Option<noshape_model::BodyId> },
    Projected { parent: Option<ViewId> },
    Auxiliary { parent: Option<(ViewId, [f64; 2])> },
    Section { parent: Option<ViewId>, p0: Option<[f64; 2]>, line: Option<([f64; 2], [f64; 2])> },
    Detail { parent: Option<ViewId>, center: Option<[f64; 2]>, radius: Option<f64> },
    Dimension { kind: DimTool, picks: Vec<Pick> },
    Note,
    Centermark,
    Centerline { first: Option<PickedPoint> },
    Balloon { anchor: Option<PickedPoint> },
    /// Jogged section: orthogonal cutting line points (parent view coords);
    /// `done` once the line is finished (Enter / double-click).
    JoggedSection { parent: Option<ViewId>, points: Vec<[f64; 2]>, done: bool },
    AlignedSection { parent: Option<ViewId>, center: Option<[f64; 2]>, a: Option<[f64; 2]>, b: Option<[f64; 2]> },
    /// Crop a view with a rectangle (two corners) or a circle (center, radius).
    Crop { parent: Option<ViewId>, first: Option<[f64; 2]>, circle: bool },
    /// Break a view: two clicks give the removed band.
    Break { parent: Option<ViewId>, first: Option<[f64; 2]>, vertical: bool },
    /// Ordinate dimensions: datum, then the side/level of the values, then points.
    Ordinate { origin: Option<PickedPoint>, horizontal: Option<bool>, level: [f64; 2] },
    /// Place a table: BOM / hole tables need a view (and hole tables an origin).
    Table { kind: TableTool, view: Option<ViewId>, origin: Option<PickedPoint> },
    Gdt,
    Datum { anchor: Option<[f64; 2]> },
    SurfaceFinish,
}

impl Tool {
    pub fn name(&self) -> &'static str {
        match self {
            Tool::Select => "Select",
            Tool::InsertView { .. } => "Insert view",
            Tool::Projected { .. } => "Projected view",
            Tool::Auxiliary { .. } => "Auxiliary view",
            Tool::Section { .. } => "Section view",
            Tool::Detail { .. } => "Detail view",
            Tool::Dimension { kind, .. } => kind.name(),
            Tool::Note => "Note",
            Tool::Centermark => "Centermark",
            Tool::Centerline { .. } => "Centerline",
            Tool::Balloon { .. } => "Callout",
            Tool::JoggedSection { .. } => "Jogged section view",
            Tool::AlignedSection { .. } => "Aligned section view",
            Tool::Crop { .. } => "Crop view",
            Tool::Break { .. } => "Break view",
            Tool::Ordinate { .. } => "Ordinate dimension",
            Tool::Table { kind: TableTool::General, .. } => "Table",
            Tool::Table { kind: TableTool::Bom, .. } => "BOM table",
            Tool::Table { kind: TableTool::Hole, .. } => "Hole table",
            Tool::Gdt => "Geometric tolerance",
            Tool::Datum { .. } => "Datum",
            Tool::SurfaceFinish => "Surface finish",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sel {
    View(ViewId),
    Annotation(u64),
    Table(u64),
    /// Break `1` of view `0`.
    Break(ViewId, usize),
}

#[derive(Clone, Debug)]
pub enum Drag {
    None,
    Pending { button: MouseButton, start: [f64; 2], start_px: [f64; 2], pick: Option<u32> },
    Pan { last_px: [f64; 2] },
    MoveViews { start: [f64; 2], orig: Vec<(ViewId, [f64; 2])>, constraint: Option<([f64; 2], [f64; 2])> },
    MoveAnnotation { start: [f64; 2], orig: Annotation },
    MoveTable { id: u64, start: [f64; 2], orig: [f64; 2] },
    /// Resize a table from its free corner (scales column widths and rows).
    ResizeTable { id: u64, start: [f64; 2], orig: Table, size: [f64; 2], widths: Vec<f64> },
    /// Move a section view's cutting line (parallel offset).
    MoveSectionLine { view: ViewId, start: [f64; 2], orig: ViewKind },
    /// Move a detail view's circle in its parent.
    MoveDetailCircle { view: ViewId, start: [f64; 2], orig: [f64; 2] },
}

/// Inline property panels shown over the canvas.
pub enum Panel {
    Note { input: Entity<TextareaState>, edit: Option<u64>, pos: [f64; 2] },
    /// Dimension properties (precision, units, tolerance, texts).
    Dimension { id: u64, angular: bool, units: Option<DimUnits>, show_units: bool, tol: usize, precision: Entity<InputState>, upper: Entity<InputState>, lower: Entity<InputState>, hole: Entity<InputState>, shaft: Entity<InputState>, prefix: Entity<InputState>, suffix: Entity<InputState>, text: Option<Entity<InputState>> },
    Gdt { edit: Option<u64>, pos: [f64; 2], symbol: usize, diameter: bool, modifier: usize, tolerance: Entity<InputState>, datums: [Entity<InputState>; 3] },
    Datum { edit: Option<u64>, anchor: [f64; 2], input: Entity<InputState> },
    SurfaceFinish { edit: Option<u64>, pos: [f64; 2], input: Entity<InputState> },
    Cell { table: u64, row: usize, col: usize, input: Entity<TextareaState> },
    SheetName { index: usize, input: Entity<InputState> },
    Rotation { view: ViewId, input: Entity<InputState> },
    NewTable { pos: [f64; 2], rows: Entity<InputState>, cols: Entity<InputState>, title: Entity<InputState>, header: bool },
    TitleBlock { inputs: Vec<(String, Entity<InputState>)> },
    Scale { view: Option<ViewId>, input: Entity<InputState> },
    Label { view: ViewId, input: Entity<InputState> },
}

/// A context/dropdown menu entry acting on the editor.
pub type EditorAction = Rc<dyn Fn(&mut DrawingEditor, &mut Window, &mut Context<DrawingEditor>)>;

#[derive(Clone)]
pub enum MenuEntry {
    Item { label: SharedString, checked: bool, act: EditorAction },
    Separator,
    Submenu(SharedString, Vec<MenuEntry>),
}

impl MenuEntry {
    pub fn item(label: impl Into<SharedString>, act: impl Fn(&mut DrawingEditor, &mut Window, &mut Context<DrawingEditor>) + 'static) -> Self {
        MenuEntry::Item { label: label.into(), checked: false, act: Rc::new(act) }
    }
    pub fn check(label: impl Into<SharedString>, checked: bool, act: impl Fn(&mut DrawingEditor, &mut Window, &mut Context<DrawingEditor>) + 'static) -> Self {
        MenuEntry::Item { label: label.into(), checked, act: Rc::new(act) }
    }
}

/// Fill a popup menu with entries bound to the editor.
pub fn fill_menu(mut menu: PopupMenu, entries: &[MenuEntry], ed: &Entity<DrawingEditor>, window: &mut Window, cx: &mut Context<PopupMenu>) -> PopupMenu {
    for e in entries {
        match e {
            MenuEntry::Item { label, checked, act } => {
                let ed = ed.clone();
                let act = act.clone();
                menu = menu.item(PopupMenuItem::new(label.clone()).checked(*checked).on_click(move |_, window, cx| {
                    let act = act.clone();
                    ed.update(cx, |t, cx| act(t, window, cx));
                }));
            }
            MenuEntry::Separator => menu = menu.separator(),
            MenuEntry::Submenu(label, children) => {
                let children = children.clone();
                let ed = ed.clone();
                menu = menu.submenu(label.clone(), window, cx, move |m, w, cx| fill_menu(m, &children, &ed, w, cx));
            }
        }
    }
    menu
}

pub struct DrawingEditor {
    pub doc: Entity<DocModel>,
    pub tab: TabId,
    gpu: Gpu,
    target_id: u64,
    pub focus: FocusHandle,

    pub drawing: Drawing,
    pub preview: Option<Drawing>,
    pub sheet: usize,

    pub sources: Sources,
    regens: HashMap<SourceKey, Arc<Mutex<Regenerator>>>,
    assemblies: Arc<Mutex<AssemblyEvaluator>>,
    regenerating: HashSet<SourceKey>,
    pub info: SheetInfo,
    pub out_of_date: bool,
    pub cache: Arc<ViewCache>,
    pub geoms: HashMap<ViewId, Arc<ViewGeometry>>,
    compute_task: Option<Task<()>>,
    compute_again: bool,
    pub computing: bool,
    pub built: Option<Arc<BuiltSheet>>,

    // Camera: sheet point at the canvas center and pixels per millimeter.
    pub center: [f64; 2],
    pub zoom: f64,
    pub bounds: Bounds<Pixels>,
    pub fitted: bool,

    // Rendering.
    lines: Option<LinesHandle>,
    lines_zoom: f64,
    fills: Option<MeshHandle>,
    paper: Option<(MeshHandle, MeshHandle, [f64; 2])>,
    overlay_handles: Option<(LinesHandle, PointsHandle)>,
    geometry_dirty: bool,
    frame_dirty: bool,
    seq: u64,
    last_presented: u64,
    image: Option<Arc<RenderImage>>,
    _frames: Task<()>,

    // Interaction.
    pub tool: Tool,
    pub hover: Option<u32>,
    pub hover_snap: Option<[f64; 2]>,
    pub selection: Vec<Sel>,
    pub drag: Drag,
    pub cursor: [f64; 2],
    pub overlay: Overlay,
    pub menu: Option<(Entity<PopupMenu>, Point<Pixels>, Subscription)>,
    pub panel: Option<Panel>,
    pub status: Option<SharedString>,
    pub _subs: Vec<Subscription>,
}

impl Focusable for DrawingEditor {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl DrawingEditor {
    pub fn new(doc: Entity<DocModel>, tab: TabId, gpu: Gpu, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let (target_id, rx) = gpu.register_target();
        let frames = cx.spawn_in(window, async move |this, cx| {
            while let Ok(res) = rx.recv().await {
                if this.update_in(cx, |this: &mut DrawingEditor, window, cx| this.present(res, window, cx)).is_err() {
                    break;
                }
            }
        });
        let subs = vec![cx.subscribe_in(&doc, window, |this: &mut Self, _, ev: &DocEvent, _window, cx| match ev {
            DocEvent::TabChanged(t) if *t == this.tab => this.reload(cx),
            DocEvent::TabChanged(t) => this.studio_changed(*t, cx),
            DocEvent::Reloaded | DocEvent::TabsChanged => {
                this.reload(cx);
                let mut tabs: Vec<TabId> = this.sources.keys().map(|k| k.tab).collect();
                tabs.dedup();
                for t in tabs {
                    this.studio_changed(t, cx);
                }
            }
            _ => {}
        })];
        let drawing = match doc.read(cx).session.doc.tab(tab).map(|t| &t.content) {
            Some(TabContent::Drawing(d)) => d.clone(),
            _ => Drawing::default(),
        };
        let mut this = Self {
            doc,
            tab,
            gpu,
            target_id,
            focus: cx.focus_handle(),
            drawing,
            preview: None,
            sheet: 0,
            sources: Sources::new(),
            regens: HashMap::new(),
            assemblies: Arc::new(Mutex::new(AssemblyEvaluator::new())),
            regenerating: HashSet::new(),
            info: SheetInfo::default(),
            out_of_date: false,
            cache: Arc::new(ViewCache::new()),
            geoms: HashMap::new(),
            compute_task: None,
            compute_again: false,
            computing: false,
            built: None,
            center: [210.0, 148.5],
            zoom: 2.0,
            bounds: Bounds::default(),
            fitted: false,
            lines: None,
            lines_zoom: 0.0,
            fills: None,
            paper: None,
            overlay_handles: None,
            geometry_dirty: true,
            frame_dirty: true,
            seq: 0,
            last_presented: 0,
            image: None,
            _frames: frames,
            tool: Tool::Select,
            hover: None,
            hover_snap: None,
            selection: Vec::new(),
            drag: Drag::None,
            cursor: [0.0, 0.0],
            overlay: Overlay::default(),
            menu: None,
            panel: None,
            status: None,
            _subs: subs,
        };
        this.ensure_sources(false, cx);
        this.refresh(cx);
        this
    }

    // ------------------------------------------------------------ model

    pub fn display(&self) -> &Drawing {
        self.preview.as_ref().unwrap_or(&self.drawing)
    }

    pub fn current_sheet(&self) -> &Sheet {
        let d = self.display();
        &d.sheets[self.sheet.min(d.sheets.len().saturating_sub(1))]
    }

    fn update_info(&mut self, cx: &App) {
        let d = self.doc.read(cx);
        self.info = SheetInfo { doc_name: d.name().to_string(), drawing_name: d.session.doc.tab(self.tab).map(|t| t.name.clone()).unwrap_or_default() };
    }

    fn reload(&mut self, cx: &mut Context<Self>) {
        self.update_info(cx);
        if let Some(TabContent::Drawing(d)) = self.doc.read(cx).session.doc.tab(self.tab).map(|t| &t.content) {
            self.drawing = d.clone();
        }
        if self.drawing.sheets.is_empty() {
            self.drawing.sheets.push(Sheet::default());
        }
        self.sheet = self.sheet.min(self.drawing.sheets.len() - 1);
        let sheet = &self.drawing.sheets[self.sheet];
        self.selection.retain(|s| match s {
            Sel::View(v) => sheet.view(*v).is_some(),
            Sel::Annotation(a) => sheet.annotations.iter().any(|x| x.id == *a),
            Sel::Table(t) => sheet.table(*t).is_some(),
            Sel::Break(v, i) => sheet.view(*v).is_some_and(|v| *i < v.breaks.len()),
        });
        self.ensure_sources(false, cx);
        self.refresh(cx);
    }

    /// Commit a new drawing state as one undoable change.
    pub fn commit(&mut self, description: &str, new: Drawing, cx: &mut Context<Self>) {
        self.drawing = new.clone();
        self.preview = None;
        let tab = self.tab;
        self.doc.update(cx, |d, cx| {
            d.edit_tab(tab, description, cx, |c| {
                if let TabContent::Drawing(dr) = c {
                    *dr = new;
                }
            });
        });
        self.refresh(cx);
    }

    /// Apply an edit to (a copy of) the committed drawing and commit it.
    pub fn edit(&mut self, description: &str, cx: &mut Context<Self>, f: impl FnOnce(&mut Drawing)) {
        let mut d = self.drawing.clone();
        f(&mut d);
        self.commit(description, d, cx);
    }

    pub fn edit_sheet(&mut self, description: &str, cx: &mut Context<Self>, f: impl FnOnce(&mut Sheet)) {
        let si = self.sheet;
        self.edit(description, cx, |d| {
            if let Some(s) = d.sheets.get_mut(si) {
                f(s);
            }
        });
    }

    pub fn set_preview(&mut self, d: Option<Drawing>, cx: &mut Context<Self>) {
        self.preview = d;
        self.refresh(cx);
    }

    // ------------------------------------------------------------ sources

    /// A tab of the document changed: sources built from it (or assemblies,
    /// which depend on every studio) are out of date.
    fn studio_changed(&mut self, tab: TabId, cx: &mut Context<Self>) {
        let doc = self.doc.read(cx);
        let stale = self.sources.iter().any(|(k, src)| {
            let depends = k.tab == tab || src.assembly;
            depends && views::source_hash(&doc.session.doc, k) != Some(src.hash)
        });
        if stale && !self.out_of_date {
            self.out_of_date = true;
            cx.notify();
        }
    }

    /// Regenerate referenced sources that are missing (or all, for Update).
    pub fn ensure_sources(&mut self, all: bool, cx: &mut Context<Self>) {
        let mut wanted = views::referenced_sources(self.display());
        if let Tool::InsertView { source, .. } = &self.tool {
            let k = SourceKey::studio(*source);
            if !wanted.contains(&k) {
                wanted.push(k);
            }
        }
        for key in wanted {
            if self.regenerating.contains(&key) || (!all && self.sources.contains_key(&key)) {
                continue;
            }
            let doc = self.doc.read(cx).session.doc.clone();
            if doc.tab(key.tab).is_none() {
                continue;
            }
            let regen = self.regens.entry(key.clone()).or_insert_with(|| Arc::new(Mutex::new(Regenerator::new()))).clone();
            let assemblies = self.assemblies.clone();
            self.regenerating.insert(key.clone());
            cx.spawn(async move |this, cx| {
                let k2 = key.clone();
                let src = cx.background_spawn(async move { views::regenerate_source(&doc, &k2, &mut regen.lock(), &mut assemblies.lock()) }).await;
                this.update(cx, |this, cx| {
                    this.regenerating.remove(&key);
                    if let Some(src) = src {
                        this.sources.insert(key, src);
                    }
                    if this.regenerating.is_empty() {
                        this.out_of_date = false;
                        if this.status.as_ref().is_some_and(|s| s.starts_with("Updating")) {
                            this.status = None;
                        }
                    }
                    this.refresh(cx);
                })
                .ok();
            })
            .detach();
        }
        cx.notify();
    }

    pub fn update_drawing(&mut self, cx: &mut Context<Self>) {
        self.ensure_sources(true, cx);
        self.status = Some("Updating drawing…".into());
    }

    // ------------------------------------------------------------ geometry

    /// Pick up cached view geometry, schedule missing computations and
    /// reassemble the sheet.
    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        let d = self.display().clone();
        let si = self.sheet.min(d.sheets.len().saturating_sub(1));
        let Some(sheet) = d.sheets.get(si) else { return };
        let interactive = self.preview.is_some();
        let mut geoms = HashMap::new();
        let mut need_fast = false;
        let mut need_exact = false;
        for v in &sheet.views {
            if let Some(k) = views::view_key(&d, sheet, v.id, &self.sources, true)
                && let Some(g) = self.cache.get(k)
            {
                geoms.insert(v.id, g);
                continue;
            }
            match views::view_key(&d, sheet, v.id, &self.sources, false) {
                Some(k) => match self.cache.get(k) {
                    Some(g) => {
                        geoms.insert(v.id, g);
                        need_exact = true;
                    }
                    None => {
                        need_fast = true;
                        // Keep showing the previous geometry of a moved view.
                        if let Some(old) = self.geoms.get(&v.id).filter(|g| g.frame == views::view_frame(&d, sheet, v.id)) {
                            geoms.insert(v.id, old.clone());
                        }
                    }
                },
                None => {}
            }
        }
        self.geoms = geoms;
        if need_fast {
            self.schedule_compute(false, cx);
        } else if need_exact && !interactive {
            self.schedule_compute(true, cx);
        }
        let built = assemble_sheet(&d, si, &self.geoms, &self.sources, &self.info);
        self.built = Some(Arc::new(built));
        self.geometry_dirty = true;
        self.update_hover(cx);
        cx.notify();
    }

    fn schedule_compute(&mut self, exact: bool, cx: &mut Context<Self>) {
        if self.compute_task.is_some() {
            self.compute_again = true;
            return;
        }
        let d = self.display().clone();
        let si = self.sheet;
        let sources = self.sources.clone();
        let cache = self.cache.clone();
        self.computing = true;
        self.compute_task = Some(cx.spawn(async move |this, cx| {
            cx.background_spawn(async move {
                if let Some(s) = d.sheets.get(si) {
                    views::compute_sheet_views(&d, s, &sources, &cache, exact);
                }
            })
            .await;
            this.update(cx, |this, cx| {
                this.compute_task = None;
                this.computing = false;
                this.compute_again = false;
                this.refresh(cx);
            })
            .ok();
        }));
    }

    // ------------------------------------------------------------ camera

    pub fn size_px(&self) -> [f64; 2] {
        [f64::from(f32::from(self.bounds.size.width)), f64::from(f32::from(self.bounds.size.height))]
    }

    pub fn to_sheet(&self, local: [f64; 2]) -> [f64; 2] {
        let s = self.size_px();
        [self.center[0] + (local[0] - s[0] / 2.0) / self.zoom, self.center[1] - (local[1] - s[1] / 2.0) / self.zoom]
    }

    #[allow(dead_code)]
    pub fn to_screen(&self, p: [f64; 2]) -> [f64; 2] {
        let s = self.size_px();
        [(p[0] - self.center[0]) * self.zoom + s[0] / 2.0, -(p[1] - self.center[1]) * self.zoom + s[1] / 2.0]
    }

    pub fn local(&self, p: Point<Pixels>) -> [f64; 2] {
        [f64::from(f32::from(p.x - self.bounds.origin.x)), f64::from(f32::from(p.y - self.bounds.origin.y))]
    }

    pub fn zoom_to_fit(&mut self, cx: &mut Context<Self>) {
        let s = self.size_px();
        if s[0] < 10.0 || s[1] < 10.0 {
            return;
        }
        let (w, h) = self.current_sheet().size.size_mm();
        self.zoom = ((s[0] - 40.0) / w).min((s[1] - 40.0) / h).max(0.05);
        self.center = [w / 2.0, h / 2.0];
        self.fitted = true;
        self.frame_dirty = true;
        cx.notify();
    }

    pub fn zoom_by(&mut self, factor: f64, at_local: [f64; 2], cx: &mut Context<Self>) {
        let before = self.to_sheet(at_local);
        self.zoom = (self.zoom * factor).clamp(0.05, 200.0);
        let after = self.to_sheet(at_local);
        self.center[0] += before[0] - after[0];
        self.center[1] += before[1] - after[1];
        self.frame_dirty = true;
        cx.notify();
    }

    fn camera(&self) -> Camera {
        let s = self.size_px();
        let mut c = Camera::default();
        c.projection = CamProjection::Orthographic;
        c.rotation = Camera::look_rotation(DVec3::NEG_Z, DVec3::Y);
        let _ = DQuat::IDENTITY;
        c.target = DVec3::new(self.center[0], self.center[1], 0.0);
        let half_h = (s[1] / 2.0 / self.zoom).max(1e-3);
        c.distance = half_h / (c.fov_y * 0.5).tan();
        c
    }

    // ------------------------------------------------------------ rendering

    fn rebuild_render_data(&mut self) {
        let Some(built) = self.built.clone() else { return };
        let up = self.gpu.uploader();
        let (w, h) = (built.geometry.width, built.geometry.height);
        if self.paper.as_ref().is_none_or(|(_, _, s)| s[0] != w || s[1] != h) {
            let shadow = up.upload_mesh(&drawing_scene::quad(2.0, -2.5, w + 2.0, h - 2.5, -2.0));
            let paper = up.upload_mesh(&drawing_scene::quad(0.0, 0.0, w, h, -1.0));
            self.paper = Some((shadow, paper, [w, h]));
        }
        let dangling: HashSet<u32> = built
            .picks
            .iter()
            .enumerate()
            .filter(|(_, t)| matches!(t, PickTarget::Annotation(a) if built.dangling.contains(a)))
            .map(|(i, _)| i as u32)
            .collect();
        self.lines = Some(up.upload_lines(&drawing_scene::sheet_lines(&built, self.zoom, &dangling)));
        self.lines_zoom = self.zoom;
        self.fills = Some(up.upload_mesh(&drawing_scene::sheet_fills(&built)));
        self.geometry_dirty = false;
    }

    fn rebuild_overlay(&mut self) {
        if self.overlay.is_empty() {
            self.overlay_handles = None;
            return;
        }
        let (l, pts) = self.overlay.to_render([139, 124, 255, 255]);
        let up = self.gpu.uploader();
        self.overlay_handles = Some((up.upload_lines(&l), up.upload_points(&pts)));
    }

    /// Pick ids belonging to a selection entry.
    fn picks_of(&self, sel: Sel) -> Vec<u32> {
        let Some(b) = &self.built else { return vec![] };
        b.picks
            .iter()
            .enumerate()
            .filter(|(_, t)| match (sel, t) {
                (Sel::View(v), t) => t.view() == Some(v),
                (Sel::Annotation(a), PickTarget::Annotation(b)) => a == *b,
                (Sel::Table(a), PickTarget::Table(b)) => a == *b,
                (Sel::Break(v, i), PickTarget::BreakLine { view, index }) => v == *view && i == *index,
                _ => false,
            })
            .map(|(i, _)| i as u32)
            .collect()
    }

    /// The selectable entity a pick id belongs to.
    pub fn sel_of_pick(&self, pick: u32) -> Option<Sel> {
        match self.built.as_ref()?.picks.get(pick as usize)? {
            PickTarget::View(v) | PickTarget::Edge { view: v, .. } | PickTarget::SectionLine(v) | PickTarget::DetailCircle(v) => Some(Sel::View(*v)),
            PickTarget::Annotation(a) => Some(Sel::Annotation(*a)),
            PickTarget::Table(t) => Some(Sel::Table(*t)),
            PickTarget::BreakLine { view, index } => Some(Sel::Break(*view, *index)),
            _ => None,
        }
    }

    fn build_scene(&self) -> Scene {
        let mut scene = Scene::default();
        if let Some((shadow, paper, _)) = &self.paper {
            scene.instances.push(Instance::new(Drawable::Mesh(shadow.clone())).with_color([0.0, 0.0, 0.0, 0.35]).unpickable().layer(-10));
            scene.instances.push(Instance::new(Drawable::Mesh(paper.clone())).with_color([1.0, 1.0, 1.0, 1.0]).unpickable().layer(-5));
        }
        if let Some(f) = &self.fills {
            scene.instances.push(Instance::new(Drawable::Mesh(f.clone())).with_color(drawing_scene::FILL_COLOR).unpickable());
        }
        if let Some(l) = &self.lines {
            scene.instances.push(Instance::new(Drawable::Lines(l.clone())).per_primitive_color());
        }
        if let Some((l, pts)) = &self.overlay_handles {
            scene.instances.push(Instance::new(Drawable::Lines(l.clone())).per_primitive_color().unpickable().layer(20));
            scene.instances.push(Instance::new(Drawable::Points(pts.clone())).per_primitive_color().unpickable().layer(21));
        }
        // Hover and selection highlight states.
        if let Some(b) = &self.built {
            scene.states = vec![0; b.picks.len()];
            for s in &self.selection {
                for p in self.picks_of(*s) {
                    scene.add_flag(p, state::SELECTED);
                }
            }
            if let Some(h) = self.hover {
                let single = matches!(self.tool, Tool::Dimension { .. } | Tool::Auxiliary { .. } | Tool::Centermark | Tool::Centerline { .. } | Tool::Balloon { .. } | Tool::Ordinate { .. });
                let group = if single { vec![h] } else { self.sel_of_pick(h).map(|s| self.picks_of(s)).unwrap_or_else(|| vec![h]) };
                for p in group {
                    scene.add_flag(p, state::HOVER);
                }
            }
        }
        scene
    }

    fn submit(&mut self, window: &mut Window) {
        let size = self.bounds.size;
        let scale = window.scale_factor();
        let w = (f32::from(size.width) * scale).round() as u32;
        let h = (f32::from(size.height) * scale).round() as u32;
        if w < 2 || h < 2 {
            return;
        }
        if self.geometry_dirty || (self.lines_zoom > 0.0 && (self.zoom / self.lines_zoom - 1.0).abs() > 0.08) {
            self.rebuild_render_data();
        }
        self.rebuild_overlay();
        let mut style = Style::default();
        let bg = srgb(0x3a, 0x3f, 0x4a);
        let bg2 = srgb(0x2a, 0x2e, 0x36);
        style.background_top = bg;
        style.background_bottom = bg2;
        style.hover_color = srgb(0x1f, 0x8f, 0xff);
        style.select_color = srgb(0xff, 0x8a, 0x00);
        style.show_edges = false;
        self.seq += 1;
        self.gpu.submit(RenderJob {
            target: self.target_id,
            scene: Arc::new(self.build_scene()),
            camera: self.camera(),
            width: w,
            height: h,
            device_pixel_ratio: scale,
            style,
            pick: false,
            seq: self.seq,
        });
        self.frame_dirty = false;
    }

    fn present(&mut self, res: FrameResult, window: &mut Window, cx: &mut Context<Self>) {
        let (seq, result, _ms) = res;
        if seq < self.last_presented {
            return;
        }
        self.last_presented = seq;
        match result {
            Ok((_, image)) => {
                if let Some(old) = self.image.replace(image) {
                    let _ = window.drop_image(old);
                }
            }
            Err(e) => log::error!("drawing render failed: {e}"),
        }
        cx.notify();
    }

    /// Request a new frame (hover/selection/camera/overlay changed).
    pub fn redraw(&mut self, cx: &mut Context<Self>) {
        self.frame_dirty = true;
        cx.notify();
    }

    // ------------------------------------------------------------ hover

    pub fn tol_mm(&self, px: f64) -> f64 {
        px / self.zoom
    }

    pub fn update_hover(&mut self, cx: &mut Context<Self>) {
        let tol = self.tol_mm(5.0);
        let hover = self.built.as_ref().and_then(|b| b.hit(self.cursor, tol));
        let hover = match (&self.tool, hover) {
            // Dimension-like tools only care about view edges.
            (Tool::Dimension { .. } | Tool::Auxiliary { .. } | Tool::Centermark | Tool::Centerline { .. } | Tool::Balloon { .. } | Tool::Ordinate { .. }, Some(h)) => {
                match self.built.as_ref().and_then(|b| b.picks.get(h as usize)) {
                    Some(PickTarget::Edge { .. }) => Some(h),
                    _ => None,
                }
            }
            (
                Tool::Select
                | Tool::Projected { parent: None }
                | Tool::Section { parent: None, .. }
                | Tool::Detail { parent: None, .. }
                | Tool::JoggedSection { parent: None, .. }
                | Tool::AlignedSection { parent: None, .. }
                | Tool::Crop { parent: None, .. }
                | Tool::Break { parent: None, .. }
                | Tool::Table { kind: TableTool::Bom | TableTool::Hole, view: None, .. },
                h,
            ) => h,
            _ => None,
        };
        // In select mode, hovering anywhere inside a view highlights it.
        let hover = match (hover, &self.tool, self.built.as_ref()) {
            (None, Tool::Select, Some(b)) => b
                .placements
                .iter()
                .filter(|p| p.contains(self.cursor, 1.0))
                .min_by(|a, c| ((a.max[0] - a.min[0]) * (a.max[1] - a.min[1])).total_cmp(&((c.max[0] - c.min[0]) * (c.max[1] - c.min[1]))))
                .and_then(|p| b.picks.iter().position(|t| *t == PickTarget::View(p.id)))
                .map(|i| i as u32),
            (h, _, _) => h,
        };
        if hover != self.hover {
            self.hover = hover;
            self.redraw(cx);
        }
    }

    pub fn placement(&self, view: ViewId) -> Option<noshape_model::drawing::build::ViewPlacement> {
        self.built.as_ref()?.placement(view).copied()
    }

    /// The view under a sheet point (smallest containing view rectangle).
    pub fn view_at(&self, s: [f64; 2]) -> Option<ViewId> {
        if let Some(h) = self.hover
            && let Some(Sel::View(v)) = self.sel_of_pick(h)
        {
            return Some(v);
        }
        let b = self.built.as_ref()?;
        b.placements
            .iter()
            .filter(|p| p.contains(s, 2.0))
            .min_by(|a, c| ((a.max[0] - a.min[0]) * (a.max[1] - a.min[1])).total_cmp(&((c.max[0] - c.min[0]) * (c.max[1] - c.min[1]))))
            .map(|p| p.id)
    }
}

impl Render for DrawingEditor {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.fitted && self.size_px()[0] > 10.0 {
            self.zoom_to_fit(cx);
        }
        if self.geometry_dirty || self.frame_dirty {
            self.submit(window);
        }
        let pal = p();
        let entity = cx.entity();
        let image = self.image.clone();
        let toolbar = self.render_toolbar(window, cx);
        let status = self.render_status(cx);
        let sheet_bar = self.render_sheet_bar(cx);
        let panel = self.render_panel(cx);
        let menu = self.menu.as_ref().map(|(m, pos, _)| (m.clone(), *pos));
        let canvas_el = div()
            .id("drawing-canvas")
            .flex_1()
            .min_h_0()
            .relative()
            .overflow_hidden()
            .bg(rgb(0x2f333b))
            .cursor(match self.tool {
                Tool::Select => CursorStyle::Arrow,
                _ => CursorStyle::Crosshair,
            })
            .child(
                canvas(
                    move |bounds, _window, cx| {
                        entity.update(cx, |this, cx| {
                            if this.bounds != bounds {
                                this.bounds = bounds;
                                this.frame_dirty = true;
                                cx.notify();
                            }
                        });
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .size_full(),
            )
            .children(image.map(|img| gpui_kit::img(img).absolute().top_0().left_0().size_full()))
            .child(status)
            .children(panel)
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_down(MouseButton::Right, cx.listener(Self::on_mouse_down))
            .on_mouse_down(MouseButton::Middle, cx.listener(Self::on_mouse_down))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_up(MouseButton::Right, cx.listener(Self::on_mouse_up))
            .on_mouse_up(MouseButton::Middle, cx.listener(Self::on_mouse_up))
            .on_scroll_wheel(cx.listener(Self::on_scroll));
        v_flex()
            .size_full()
            .bg(pal.app_bg)
            .key_context("Drawing")
            .track_focus(&self.focus)
            .on_action(cx.listener(|this, _: &Cancel, _, cx| this.cancel(cx)))
            .on_action(cx.listener(|this, _: &DeleteSelection, _, cx| this.delete_selection(cx)))
            .on_action(cx.listener(|this, _: &ZoomToFit, _, cx| this.zoom_to_fit(cx)))
            .on_action(cx.listener(|this, _: &ZoomIn, _, cx| {
                let s = this.size_px();
                this.zoom_by(1.25, [s[0] / 2.0, s[1] / 2.0], cx)
            }))
            .on_action(cx.listener(|this, _: &ZoomOut, _, cx| {
                let s = this.size_px();
                this.zoom_by(0.8, [s[0] / 2.0, s[1] / 2.0], cx)
            }))
            .on_action(cx.listener(|this, _: &DrawingUpdate, _, cx| this.update_drawing(cx)))
            // Onshape drawing letters that collide with Part Studio bindings.
            .on_action(cx.listener(|this, _: &ViewNormal, _, cx| this.set_tool(Tool::Note, cx)))
            .on_action(cx.listener(|this, _: &TogglePlanes, _, cx| this.set_tool(Tool::Projected { parent: None }, cx)))
            .on_key_down(cx.listener(Self::on_key))
            .child(toolbar)
            .child(canvas_el)
            .child(sheet_bar)
            .children(menu.map(|(m, pos)| deferred(anchored().position(pos).snap_to_window().child(m)).with_priority(2)))
    }
}
