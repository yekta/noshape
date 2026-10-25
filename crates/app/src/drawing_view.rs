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
use gpui_kit::component::v_flex;
use gpui_kit::*;
use glam::DVec3;
use noshape_model::document::{TabContent, TabId};
use noshape_model::assembly::eval::AssemblyEvaluator;
use noshape_model::drawing::build::{BuiltSheet, PickTarget, SheetInfo, assemble_sheet};
use noshape_model::drawing::views::{self, SourceKey, Sources, ViewCache, ViewGeometry};
use noshape_model::drawing::*;
use noshape_model::regen::Regenerator;
use noshape_render::worker::RenderJob;
use noshape_render::{Camera, Drawable, Instance, LinesHandle, MeshHandle, PointsHandle, Projection as CamProjection, Scene, Style, state};
use parking_lot::Mutex;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::Arc;

// The tool, pick, selection and drag types are GUI-free (shared with the
// agent API): noshape_scene::drawing_tools.
pub use noshape_scene::drawing_tools::{Button, DimTool, Drag, Pick, PickedPoint, Sel, TableTool, Tool};

/// The drag's mouse button.
pub fn drag_button(b: MouseButton) -> Button {
    match b {
        MouseButton::Left => Button::Left,
        MouseButton::Right => Button::Right,
        MouseButton::Middle => Button::Middle,
        _ => Button::Other,
    }
}

/// The gpui mouse button of a drag's button (`Other`: navigation buttons).
pub fn mouse_button(b: Button) -> MouseButton {
    match b {
        Button::Left => MouseButton::Left,
        Button::Right => MouseButton::Right,
        Button::Middle | Button::Other => MouseButton::Middle,
    }
}

/// Uploaded meshes with their linear colors.
pub type ColoredHandles = Vec<(MeshHandle, [f32; 4])>;

/// Top of the drawing tool dialogs: flush with the drawing area's top, over
/// the sheet chips (live drawing/122, 124, 127, 180, 200, 468, 494: y 76).
pub const DIALOG_TOP: f32 = 0.;

/// Inline property panels shown over the canvas.
pub enum Panel {
    /// `field`: a title block field's cell edited as a note (its key).
    Note { input: Entity<TextareaState>, edit: Option<u64>, pos: [f64; 2], height: Entity<InputState>, style: noshape_model::drawing::note::NoteStyle, field: Option<String> },
    /// Dimension properties (precision, units, tolerance, texts).
    Datum { edit: Option<u64>, anchor: [f64; 2], input: Entity<InputState> },
    SurfaceFinish { edit: Option<u64>, pos: [f64; 2], input: Entity<InputState> },
    /// A table cell's text; `note`: the Note panel's style of a general
    /// table cell (live 743: the cell is a note).
    Cell { table: u64, row: usize, col: usize, input: Entity<TextareaState>, note: Option<Box<CellNote>> },
    SheetName { index: usize, input: Entity<InputState> },
    Rotation { view: ViewId, input: Entity<InputState> },
    TitleBlock { inputs: Vec<(String, Entity<InputState>)> },
    /// Depth of a new (`edit` None) or existing broken-out section.
    BrokenOut { view: ViewId, edit: Option<usize>, profile: Vec<[f64; 2]>, spline: bool, depth: Entity<InputState>, depth_ref: Option<PointRef> },
    /// Angle of a straight cutting line.
    CutAngle { view: ViewId, input: Entity<InputState> },
    /// Table cell formatting; `scope`: 0 cell, 1 row, 2 column, 3 table.
    CellFormat { table: u64, row: usize, col: usize, scope: usize, align: CellAlign, bold: bool, height: Entity<InputState> },
    /// BOM cell: the value and "Override" (`enabled`: the row has a part number).
    BomCell { table: u64, item: String, col: usize, input: Entity<InputState>, over: bool, enabled: bool },
    /// Table cell › Resize: the column's width and the row height (sheet mm).
    CellResize { table: u64, col: usize, width: Entity<InputState>, height: Entity<InputState> },
    DatumTarget { id: u64, label: Entity<InputState>, area: usize, width: Entity<InputState>, height: Entity<InputState> },
}

/// The Note panel of a general table cell edited in place: its style (font,
/// bold, italic, underline, strikethrough, alignments) and text height.
pub struct CellNote {
    pub style: noshape_model::drawing::note::NoteStyle,
    pub height: Entity<InputState>,
}

/// A context/dropdown menu entry acting on the editor.
pub type EditorAction = Rc<dyn Fn(&mut DrawingEditor, &mut Window, &mut Context<DrawingEditor>)>;

#[derive(Clone)]
pub enum MenuEntry {
    Item { label: SharedString, checked: bool, act: EditorAction, icon: Option<&'static str>, disabled: bool },
    Separator,
    Submenu(SharedString, Vec<MenuEntry>),
}

impl MenuEntry {
    pub fn item(label: impl Into<SharedString>, act: impl Fn(&mut DrawingEditor, &mut Window, &mut Context<DrawingEditor>) + 'static) -> Self {
        MenuEntry::Item { label: label.into(), checked: false, act: Rc::new(act), icon: None, disabled: false }
    }
    pub fn check(label: impl Into<SharedString>, checked: bool, act: impl Fn(&mut DrawingEditor, &mut Window, &mut Context<DrawingEditor>) + 'static) -> Self {
        MenuEntry::Item { label: label.into(), checked, act: Rc::new(act), icon: None, disabled: false }
    }
    /// Greyed out (shown but not clickable).
    pub fn disabled(mut self, on: bool) -> Self {
        if let MenuEntry::Item { disabled, .. } = &mut self {
            *disabled = on;
        }
        self
    }
    /// The tool icon shown in Onshape's toolbar dropdowns.
    pub fn with_icon(mut self, name: &'static str) -> Self {
        if let MenuEntry::Item { icon, .. } = &mut self {
            *icon = Some(name);
        }
        self
    }
}

/// A drawing toolbar dropdown (Onshape's editor list: 26 px rows, 18 px
/// tool icons, 13.33 px text), opened under the toolbar from its caret.
pub fn open_tool_menu(entries: &[MenuEntry], ed: &Entity<DrawingEditor>, e: &ClickEvent, window: &mut Window, cx: &mut App) {
    let (entries, ed2) = (entries.to_vec(), ed.clone());
    let menu = crate::os_menu::OsMenu::build(window, cx, move |m, w, cx| fill_os_menu(m.toolbar_list(), &entries, &ed2, w, cx));
    let x = e.position().x - px(34.);
    crate::os_menu::open_anchored(point(x, px(crate::ui::NAVBAR_H + 36.)), Anchor::TopLeft, menu, window, cx);
}

/// [`fill_menu`] for Onshape's right-click menus ([`crate::os_menu`]).
pub fn fill_os_menu(mut menu: crate::os_menu::OsMenu, entries: &[MenuEntry], ed: &Entity<DrawingEditor>, window: &mut Window, cx: &mut Context<crate::os_menu::OsMenu>) -> crate::os_menu::OsMenu {
    for e in entries {
        match e {
            MenuEntry::Item { label, checked, act, icon, disabled } => {
                let ed = ed.clone();
                let act = act.clone();
                let handler: crate::os_ui::ClickHandler = Rc::new(move |_, window, cx| {
                    let act = act.clone();
                    ed.update(cx, |t, cx| act(t, window, cx));
                });
                let icon = icon.map(|n| crate::os_menu::MenuIcon::Named(SharedString::from(n)));
                menu = menu.item(crate::os_menu::OsMenuItem::Item { label: label.clone(), icon, checked: *checked, disabled: *disabled, handler: Some(handler), keys: Vec::new(), user: false });
            }
            MenuEntry::Separator => menu = menu.separator(),
            MenuEntry::Submenu(label, children) => {
                let children = children.clone();
                let ed = ed.clone();
                menu = menu.submenu(label.clone(), window, cx, move |m, w, cx| fill_os_menu(m, &children, &ed, w, cx));
            }
        }
    }
    menu
}

pub struct DrawingEditor {
    pub doc: Entity<DocModel>,
    pub tab: TabId,
    pub(crate) gpu: Gpu,
    target_id: u64,
    pub focus: FocusHandle,
    /// Cached parts of the editor (toolbar, panel layer): see `crate::cached_slice`.
    slices: Option<DrSlices>,

    pub drawing: Drawing,
    pub preview: Option<Drawing>,
    pub sheet: usize,
    /// Sheets panel open (Onshape starts with it closed) and sorted by reference.
    pub sheets_open: bool,
    pub sheets_by_ref: bool,

    pub sources: Sources,
    regens: HashMap<SourceKey, Arc<Mutex<Regenerator>>>,
    assemblies: Arc<Mutex<AssemblyEvaluator>>,
    regenerating: HashSet<SourceKey>,
    /// Sources that couldn't be resolved (a deleted display state): retried
    /// only on Update.
    failed_sources: HashSet<SourceKey>,
    /// The current sheet is an imported DWG / DXF's model space: its
    /// background (Preferences › Drawings: Dark view black, Light view white).
    model_bg: Option<u32>,
    /// Dimension tool: the sketch entity picked (SK7), and the open value box.
    pub sketch_dim_pick: Option<u32>,
    pub sketch_dim_edit: Option<crate::drawing_sketch_dim::SketchDimEdit>,
    pub info: SheetInfo,
    pub out_of_date: bool,
    /// Opened after its sources changed: the document at the drawing's
    /// `sources_at` and the sources still made from it until Update (qa5 D17).
    old_state: Option<(noshape_model::Document, Vec<SourceKey>)>,
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
    /// Filled sheet text: (ink, dangling).
    texts: [Option<MeshHandle>; 2],
    /// Release management › Watermarks: the sheet's watermark text, and its
    /// mesh (rebuilt with the sheet).
    watermark: Option<String>,
    watermark_mesh: Option<MeshHandle>,
    /// Colored fills and texts (see `drawing_scene::sheet_colored`).
    colored: (ColoredHandles, ColoredHandles),
    /// Backdrops under the ink (see `drawing_scene::sheet_backdrops`).
    backdrops: ColoredHandles,
    paper: Option<(MeshHandle, MeshHandle, [f64; 2])>,
    overlay_handles: Option<(LinesHandle, PointsHandle)>,
    pub(crate) geometry_dirty: bool,
    frame_dirty: bool,
    seq: u64,
    last_presented: u64,
    image: Option<Arc<RenderImage>>,
    _frames: Task<()>,

    // Interaction.
    pub tool: Tool,
    /// The pointer is over the sheet, and the app cursor shown there.
    canvas_hovered: bool,
    cursor_sync: crate::cursor::CursorSync,
    pub hover: Option<u32>,
    pub hover_snap: Option<[f64; 2]>,
    pub selection: Vec<Sel>,
    pub drag: Drag,
    pub cursor: [f64; 2],
    pub overlay: Overlay,
    pub panel: Option<Panel>,
    /// View properties dialog (double-click a view; see `drawing_props`).
    pub view_props: Option<crate::drawing_props::ViewProps>,
    /// Insert view panel settings and its part chooser (see `drawing_insert`).
    pub insert: crate::drawing_insert::InsertViewState,
    /// Align views ▸ Vertically / Horizontally: (view to move, vertical)
    /// until the next click picks the view to align with.
    pub align_views: Option<(ViewId, bool)>,
    /// Rotate view vertically / horizontally: (view, vertical) until the
    /// next click picks the straight edge to line up.
    pub rotate_to_edge: Option<(ViewId, bool)>,
    /// Zoom window (`w`): armed, then the dragged box (canvas pixels).
    pub zoom_window: Option<Option<([f64; 2], [f64; 2])>>,
    /// Sheet properties panel (see `drawing_sheetprops`).
    pub sheet_props: Option<crate::drawing_sheetprops::SheetPropsState>,
    /// Drawing properties panel (see `drawing_drawprops`).
    pub drawing_props: crate::drawing_drawprops::DrawingPropsUi,
    /// Section view dialog (see `drawing_section`).
    pub section_dlg: Option<crate::drawing_section::SectionDlg>,
    /// Auxiliary view dialog (see `drawing_aux`) and its label text.
    pub aux_dlg: Option<crate::drawing_aux::AuxDlg>,
    pub aux_letter: String,
    /// Edit auxiliary view: the view and its dialog.
    pub aux_edit: Option<(noshape_model::drawing::ViewId, crate::drawing_aux::AuxDlg)>,
    /// Detail view dialog (see `drawing_detail`) and its label text.
    pub detail_dlg: Option<crate::drawing_detail::DetailDlg>,
    pub crop_dlg: Option<crate::drawing_crop::CropDlg>,
    pub chooser: crate::drawing_chooser::ChooserState,
    pub shaded: crate::drawing_shaded::ShadedViews,
    /// Revision table dialog (drawing_revision.rs).
    pub revision_dlg: Option<crate::drawing_revision::RevisionDlg>,
    /// Show/hide sketches dialog (drawing_view_sketches.rs).
    pub view_sketches: Option<crate::drawing_view_sketches::ViewSketchesDlg>,
    /// Documents at versions, for pinned view references.
    pub version_docs: HashMap<String, noshape_model::Document>,
    /// Color dialog (drawing_color.rs).
    pub color_dlg: Option<crate::drawing_color::ColorDlg>,
    /// Reference manager (drawing_refs.rs).
    pub ref_manager: Option<crate::drawing_refs::RefManager>,
    /// Selected table cells (drawing_cells.rs).
    pub cell_sel: Option<crate::drawing_cells::CellSel>,
    /// The floating Cell panel of the selected general table cells.
    pub cell_panel: Option<crate::drawing_cell_panel::CellPanel>,
    /// The Note panel's "Note color" panel.
    pub note_color: Option<crate::drawing_note_color::NoteColorDlg>,
    /// Paste as table: the cells waiting for the placing click.
    pub paste_table: Option<Vec<Vec<String>>>,
    /// Note tool: the leader anchored by the first click (on a snap point).
    pub note_leader: Option<Leader>,
    /// Add leader on a note: where the new leader starts (`drawing_leaders`).
    pub leader_from: Option<noshape_model::drawing::LeaderStart>,
    /// The selected note leader (note, leader): drawn amber with its nodes
    /// (live drw2 10-21 g (1)).
    pub leader_sel: Option<(u64, usize)>,
    /// The broken-out section a context menu was opened on (live: Edit
    /// section... / Remove section only there).
    pub menu_broken_out: Option<(ViewId, usize)>,
    /// Weld symbol dialog (drawing_weld.rs).
    pub weld_dlg: Option<crate::drawing_weld::WeldDlg>,
    /// The note editor's formatting ruler (drawing_ruler.rs).
    pub ruler: crate::drawing_ruler::RulerState,
    /// Callout dialog and Add stacked callouts panel (drawing_callout.rs).
    pub callout_dlg: Option<crate::drawing_callout::CalloutDlg>,
    pub stack_dlg: Option<crate::drawing_callout::StackDlg>,
    /// BOM / cut list table dialog (drawing_table_dlg.rs).
    pub table_dlg: Option<crate::drawing_table_dlg::TableDlg>,
    /// Geometric tolerance dialog (drawing_gdt.rs).
    pub gdt_dlg: Option<crate::drawing_gdt::GdtDlg>,
    /// Custom tables: the dialog and the evaluated tables.
    pub custom_tables: crate::drawing_custom_table::CustomState,
    /// Dimension tool: circle centers woken by hovering their edges (view,
    /// view point); shown until the tool changes (help drawing_dimensions).
    pub woken_centers: Vec<(ViewId, [f64; 2])>,
    /// Sketch tools: endpoints hovered so far (SK8: the cursor aligns with
    /// them along red dotted inference lines).
    pub woken_points: Vec<[f64; 2]>,
    /// Datum tool: the annotation line the datum's triangle was put on.
    pub datum_on: Option<noshape_model::drawing::DatumOn>,
    /// Datum target tool: the letter of the set being placed (A1, A2…; a
    /// new run of the tool starts the next letter's set).
    pub datum_target_letter: Option<char>,
    /// Where the last right-click menu opened (editor px), and the small
    /// dialogs opened from it.
    pub menu_at: [f64; 2],
    pub move_sheet: Option<crate::drawing_move_sheet::MoveSheetDlg>,
    pub table_props: Option<crate::drawing_move_sheet::TablePropsDlg>,
    pub linestyle: Option<crate::drawing_linestyle::LinestyleDlg>,
    pub hide_edges: Option<crate::drawing_hide_edges::HideEdgesDlg>,
    pub section_edit: Option<crate::drawing_section_edit::SectionEdit>,
    /// Select for replication / the Replicate region dialog.
    pub replication: Option<crate::drawing_replicate::Replication>,
    pub replicate_dlg: Option<crate::drawing_replicate::ReplicateDlg>,
    /// The dimension palette / hole callout panel (double-click).
    pub palette: Option<crate::drawing_palette::PaletteDlg>,
    /// The Datum dialog (Datum / Datum target tabs).
    pub datum_dlg: Option<crate::drawing_datum::DatumDlg>,
    /// Styles panel › Text gap spinbox: (annotation, field, its subscription).
    pub styles_gap: Option<(u64, Entity<InputState>, Subscription)>,
    pub table_tool: Option<crate::drawing_table_tool::TableToolDlg>,
    /// Show/hide parts dialog (drawing_hide_parts.rs).
    pub hide_parts: Option<crate::drawing_hide_parts::HidePartsDlg>,
    pub detail_label: String,
    /// Break view dialog (see `drawing_break`).
    pub break_dlg: Option<crate::drawing_break::BreakDlg>,
    /// Shift+Q: midpoints and quadrant points snap for the current dimension command.
    pub snap_mid_quad: bool,
    /// Placement inference lines (sheet mm) while placing or dragging annotations.
    pub infer_lines: Vec<([f64; 2], [f64; 2])>,
    /// `s` shortcut toolbar position (canvas px) and `[` Measure picks (see `drawing_keys`).
    pub shortcut_toolbar: Option<[f64; 2]>,
    /// Centermark dialog and Align centermark (see `drawing_centermark`).
    pub centermark_dlg: Option<crate::drawing_centermark::CentermarkDlg>,
    pub align_centermark: Option<u64>,
    /// Format painter (see `drawing_painter`).
    pub painter: Option<crate::drawing_painter::PainterState>,
    /// Right-side panels (Inspection table, Styles, MBD status; see
    /// `drawing_panels`) and the item a panel row cross-highlights.
    pub right_panel: Option<crate::drawing_panels::RightPanel>,
    /// MBD status panel › Model (0 Current sheet, 1 All models, then the parts).
    pub mbd_model: usize,
    /// Selected rows of the Sheets panel (sheet indices).
    pub sheet_rows: Vec<usize>,
    /// The note edited in place (its sheet text is hidden under the editor).
    pub inplace_hidden: Option<u64>,
    /// The general table cell edited in place (table, row, column).
    pub inplace_cell: Option<(u64, usize, usize)>,
    pub panel_highlight: Option<Sel>,
    pub insp_table: crate::drawing_panels::InspectionTableUi,
    /// Inserted image textures (see `drawing_images`).
    pub image_cache: crate::drawing_images::ImageCache,
    /// The hovered drawing-sketch constraint glyph (constraint id).
    pub glyph_hover: Option<u32>,
    /// The sketch entity whose glyphs show.
    pub glyph_entity: Option<u32>,
    /// Hatch region dialog (see `drawing_hatch`).
    pub hatch_dlg: Option<crate::drawing_hatch::HatchDlg>,
    /// Inspection item dialog (see `drawing_inspection`).
    pub inspection: Option<crate::drawing_inspection::InspectionDlg>,
    pub measure: Option<Vec<crate::drawing_keys::MeasurePick>>,
    pub status: Option<SharedString>,
    /// The green success bubble at the top centre of the sheet (`show_toast`)
    /// and its generation (a newer one replaces it).
    pub toast: Option<SharedString>,
    pub toast_gen: u64,
    pub _subs: Vec<Subscription>,
}

impl Focusable for DrawingEditor {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl DrawingEditor {
    /// A green success bubble at the top centre of the sheet (live: "✓
    /// Successfully replicated 7/7 annotations ×"), gone after 5 s or on ×.
    pub fn show_toast(&mut self, text: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.toast_gen += 1;
        let generation = self.toast_gen;
        self.toast = Some(text.into());
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(std::time::Duration::from_secs(5)).await;
            let _ = this.update(cx, |this, cx| {
                if this.toast_gen == generation {
                    this.toast = None;
                    cx.notify();
                }
            });
        })
        .detach();
        cx.notify();
    }

    fn render_toast(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let text = self.toast.clone()?;
        let entity = cx.entity().downgrade();
        let close = move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
            let _ = entity.update(cx, |this, cx| {
                this.toast = None;
                cx.notify();
            });
        };
        Some(div().absolute().top(px(8.)).left_0().right_0().flex().justify_center().child(crate::os_ui::message_bubble("dr-toast", crate::os_ui::BubbleKind::Success, text, close, cx)).into_any_element())
    }

    pub fn new(doc: Entity<DocModel>, tab: TabId, gpu: Gpu, window: &mut Window, cx: &mut Context<Self>) -> Self {
        // Read before `doc` moves into the view (derived parts' sources, DER8).
        let studio_cache = doc.read(cx).studio_cache.clone();
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
            DocEvent::Reloaded | DocEvent::TabsChanged | DocEvent::UnitsChanged { .. } => {
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
        // The drawing opens on the sheet last shown.
        let first_sheet = drawing.active_sheet.min(drawing.sheets.len().saturating_sub(1));
        let mut this = Self {
            doc,
            tab,
            gpu,
            target_id,
            focus: cx.focus_handle(),
            slices: None,
            drawing,
            preview: None,
            sheet: first_sheet,
            sheets_open: false,
            sheets_by_ref: false,
            sources: Sources::new(),
            regens: HashMap::new(),
            assemblies: Arc::new(Mutex::new(crate::derived::assembly_evaluator(studio_cache))),
            regenerating: HashSet::new(),
            failed_sources: HashSet::new(),
            model_bg: None,
            sketch_dim_pick: None,
            sketch_dim_edit: None,
            info: SheetInfo::default(),
            out_of_date: false,
            old_state: None,
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
            texts: [None, None],
            watermark: None,
            watermark_mesh: None,
            colored: (Vec::new(), Vec::new()),
            backdrops: Vec::new(),
            paper: None,
            overlay_handles: None,
            geometry_dirty: true,
            frame_dirty: true,
            seq: 0,
            last_presented: 0,
            image: None,
            _frames: frames,
            tool: Tool::Select,
            canvas_hovered: false,
            cursor_sync: Default::default(),
            hover: None,
            hover_snap: None,
            selection: Vec::new(),
            drag: Drag::None,
            cursor: [0.0, 0.0],
            overlay: Overlay::default(),
            panel: None,
            view_props: None,
            insert: Default::default(),
            align_views: None,
            rotate_to_edge: None,
            zoom_window: None,
            sheet_props: None,
            drawing_props: Default::default(),
            section_dlg: None,
            break_dlg: None,
            detail_dlg: None,
            crop_dlg: None,
            chooser: Default::default(),
            shaded: Default::default(),
            revision_dlg: None,
            view_sketches: None,
            hide_parts: None,
            weld_dlg: None,
            ruler: Default::default(),
            callout_dlg: None,
            stack_dlg: None,
            table_dlg: None,
            gdt_dlg: None,
            custom_tables: Default::default(),
            woken_centers: Vec::new(),
            woken_points: Vec::new(),
            datum_on: None,
            datum_target_letter: None,
            menu_at: [0.0; 2],
            move_sheet: None,
            table_props: None,
            linestyle: None,
            hide_edges: None,
            section_edit: None,
            replication: None,
            replicate_dlg: None,
            palette: None,
            datum_dlg: None,
            styles_gap: None,
            table_tool: None,
            note_leader: None,
            leader_from: None,
            leader_sel: None,
            menu_broken_out: None,
            cell_sel: None,
            cell_panel: None,
            note_color: None,
            ref_manager: None,
            color_dlg: None,
            version_docs: HashMap::new(),
            paste_table: None,
            detail_label: String::new(),
            aux_dlg: None,
            aux_letter: String::new(),
            aux_edit: None,
            snap_mid_quad: false,
            infer_lines: Vec::new(),
            shortcut_toolbar: None,
            centermark_dlg: None,
            align_centermark: None,
            painter: None,
            inspection: None,
            hatch_dlg: None,
            glyph_hover: None,
            glyph_entity: None,
            image_cache: Default::default(),
            right_panel: None,
            mbd_model: 0,
            sheet_rows: Vec::new(),
            inplace_hidden: None,
            inplace_cell: None,
            panel_highlight: None,
            insp_table: Default::default(),
            measure: None,
            status: None,
            toast: None,
            toast_gen: 0,
            _subs: subs,
        };
        this.old_state = this.load_old_state(cx);
        this.ensure_sources(false, cx);
        this.refresh(cx);
        this
    }

    // ------------------------------------------------------------ model

    pub fn display(&self) -> &Drawing {
        self.preview.as_ref().unwrap_or(&self.drawing)
    }

    /// The editor state picking reads (see `noshape_scene::drawing_tools::SheetPick`).
    pub fn pk(&self) -> noshape_scene::drawing_tools::SheetPick<'_> {
        noshape_scene::drawing_tools::SheetPick { display: self.display(), sheet: self.sheet, built: self.built.as_deref(), geoms: &self.geoms, tool: &self.tool, hover: self.hover, cursor: self.cursor, zoom: self.zoom, snap_mid_quad: self.snap_mid_quad, drawing: &self.drawing, selection: &self.selection, sources: &self.sources }
    }

    pub fn current_sheet(&self) -> &Sheet {
        let d = self.display();
        &d.sheets[self.sheet.min(d.sheets.len().saturating_sub(1))]
    }

    fn update_info(&mut self, cx: &App) {
        let d = self.doc.read(cx);
        // Evaluated custom tables stay until they are re-evaluated.
        let custom_tables = std::mem::take(&mut self.info.custom_tables);
        self.info = SheetInfo { doc_name: d.name().to_string(), user_name: noshape_store::local_user_name(), drawing_name: d.session.doc.tab(self.tab).map(|t| t.name.clone()).unwrap_or_default(), revisions: crate::drawing_revision::revisions(d, self.tab), custom_tables };
    }

    fn reload(&mut self, cx: &mut Context<Self>) {
        self.update_info(cx);
        let sources_at = self.drawing.sources_at.clone();
        if let Some(TabContent::Drawing(d)) = self.doc.read(cx).session.doc.tab(self.tab).map(|t| &t.content) {
            // Undoing a sheet's views keeps its scale shown (live; qa5 D14).
            let shown: Vec<u32> = self.drawing.sheets.iter().filter(|s| s.scale_shown || !s.views.is_empty()).map(|s| s.uid).collect();
            self.drawing = d.clone();
            for s in &mut self.drawing.sheets {
                s.scale_shown |= shown.contains(&s.uid);
            }
        }
        if self.drawing.sheets.is_empty() {
            self.drawing.sheets.push(Sheet::default());
        }
        // The state the views are made from changed outside this editor (an
        // Update or its undo, or the agent API): remake the views from it.
        if self.drawing.sources_at != sources_at {
            match self.load_old_state(cx) {
                Some(old) => {
                    for k in &old.1 {
                        self.sources.remove(k);
                    }
                    self.old_state = Some(old);
                    self.ensure_sources(false, cx);
                }
                None if self.out_of_date || self.old_state.is_some() => self.ensure_sources(true, cx),
                None => {}
            }
        }
        // Undo / redo of a sheet switch: show the stored sheet.
        let active = self.drawing.active_sheet.min(self.drawing.sheets.len() - 1);
        if active != self.sheet {
            self.show_sheet(active, cx);
        }
        self.sheet = self.sheet.min(self.drawing.sheets.len() - 1);
        let n = self.drawing.sheets.len();
        self.sheet_rows.retain(|i| *i < n);
        let sheet = &self.drawing.sheets[self.sheet];
        self.selection.retain(|s| match s {
            Sel::View(v) => sheet.view(*v).is_some(),
            Sel::Annotation(a) => sheet.annotations.iter().any(|x| x.id == *a),
            Sel::Table(t) => sheet.table(*t).is_some(),
            Sel::Break(v, i) => sheet.view(*v).is_some_and(|v| *i < v.breaks.len()),
            Sel::Sketch(e) => sheet.sketch.sketch.entity(noshape_sketch::EntityId(*e)).is_some(),
            Sel::Hatch(h) => sheet.sketch.hatches.iter().any(|x| x.id == *h),
            Sel::Image(i) => sheet.images.iter().any(|x| x.id == *i),
            Sel::AutoCentermark(v, c) => sheet.view(*v).is_some_and(|v| !v.deleted_centermarks.contains(c)),
            Sel::ViewLabel(v) | Sel::AuxIndicator(v) => sheet.view(*v).is_some(),
        });
        self.ensure_sources(false, cx);
        self.refresh(cx);
    }

    /// Commit a new drawing state as one undoable change.
    pub fn commit(&mut self, description: &str, mut new: Drawing, cx: &mut Context<Self>) {
        // The sheet on screen goes with the change (undo returns to it).
        new.active_sheet = self.sheet;
        // A sheet that has had a view keeps showing its scale (qa5 D14).
        for s in &mut new.sheets {
            s.scale_shown |= !s.views.is_empty();
        }
        // Older drawings: DRAWN becomes the user who first edits them.
        if new.drawn_by.is_none() && !self.info.user_name.is_empty() {
            new.drawn_by = Some(self.info.user_name.clone());
        }
        // Sketch dimensions stay on their geometry (moved / deleted entities).
        if let Some(s) = new.sheets.get_mut(self.sheet) {
            s.sync_sketch_dimensions();
        }
        // Views linked across sheets: their mirrors follow the real views.
        new.sync_links();
        // Up-to-date views were made from the current document state.
        if !self.out_of_date && self.old_state.is_none() {
            new.sources_at = self.doc.read(cx).session.head().ok().map(|m| m.to_string());
        }
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

    /// A tab of the document changed: the drawing is out of date while any
    /// source differs from what its views were made of (an undo back to it
    /// clears it again).
    fn studio_changed(&mut self, tab: TabId, cx: &mut Context<Self>) {
        // Only sources built from this tab (or assemblies, which depend on
        // every studio) can have changed.
        if !self.out_of_date && !self.sources.iter().any(|(k, src)| k.tab == tab || src.assembly) {
            return;
        }
        let stale = self.sources_stale(cx);
        // Undoing the change makes the sources current again: no Update
        // needed (qa5 UNDO2; exact float round trips make the hashes match).
        if stale != self.out_of_date {
            self.out_of_date = stale;
            cx.notify();
        }
    }

    /// Whether a source the views use differs from the current document
    /// (pinned references never go out of date).
    fn sources_stale(&self, cx: &App) -> bool {
        let doc = &self.doc.read(cx).session.doc;
        views::referenced_sources(&self.drawing).iter().any(|k| k.version.is_none() && self.sources.get(k).is_some_and(|src| views::source_hash(doc, k) != Some(src.hash)))
    }

    /// The document state the views were last made from, when the sources
    /// changed since (the views keep their geometry until Update).
    fn load_old_state(&self, cx: &App) -> Option<(noshape_model::Document, Vec<SourceKey>)> {
        noshape_ops::drawing::old_state(&self.doc.read(cx).session, &self.drawing)
    }

    /// The document at a version (cached; versions don't change).
    pub fn version_doc(&mut self, id: &str, cx: &App) -> Option<noshape_model::Document> {
        if let Some(d) = self.version_docs.get(id) {
            return Some(d.clone());
        }
        // Another document's version (Insert view › Other documents).
        let d = match crate::derived::ExternalRef::parse(id) {
            Some(r) => (*crate::derived::external_snapshot(&r.path, r.microversion).ok()?).clone(),
            None => self.doc.read(cx).session.document_at_version(id).ok().flatten()?,
        };
        self.version_docs.insert(id.to_string(), d.clone());
        Some(d)
    }

    /// Regenerate referenced sources that are missing (or all, for Update).
    pub fn ensure_sources(&mut self, all: bool, cx: &mut Context<Self>) {
        let mut wanted = views::referenced_sources(self.display());
        if let Tool::InsertView { source, .. } = &self.tool {
            let k = SourceKey { version: self.insert.pinned.as_ref().map(|p| p.id.clone()), ..SourceKey::studio(*source) };
            // The preview of an assembly uses the panel's Explode/Position and Display state.
            let (explode, position) = match self.insert.asm_position {
                AsmPosition::Explode(e) => (Some(e), None),
                AsmPosition::Position(p) => (None, Some(p)),
                AsmPosition::Default => (None, None),
            };
            let k2 = SourceKey { explode, position, display: self.insert.display_state.clone(), ..k.clone() };
            for k in [k, k2] {
                if !wanted.contains(&k) {
                    wanted.push(k);
                }
            }
        }
        // The Insert view chooser lists the parts of every Part Studio.
        for k in self.chooser_sources(cx) {
            if !wanted.contains(&k) {
                wanted.push(k);
            }
        }
        // Update: every view from the current document.
        if all {
            self.old_state = None;
        }
        for key in wanted {
            if self.regenerating.contains(&key) || (!all && (self.sources.contains_key(&key) || self.failed_sources.contains(&key))) {
                continue;
            }
            // Pinned sources come from the document at their version; views
            // not updated since a change, from the state they were made of.
            let old = self.old_state.as_mut().and_then(|(d, keys)| {
                let i = keys.iter().position(|k| *k == key)?;
                keys.remove(i);
                Some(d.clone())
            });
            let doc = match (&key.version, old) {
                (Some(v), _) => match self.version_doc(v, cx) {
                    Some(d) => d,
                    None => continue,
                },
                (None, Some(d)) => d,
                (None, None) => self.doc.read(cx).session.doc.clone(),
            };
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
                    match src {
                        Some(src) => {
                            this.failed_sources.remove(&key);
                            this.sources.insert(key, src);
                        }
                        // A display state deleted from the assembly: its views
                        // go blank, with Onshape's info toast (live drawing/1112).
                        None => {
                            if key.display.is_some() {
                                this.status = Some("Failed to resolve display state.".into());
                            }
                            this.sources.remove(&key);
                            this.failed_sources.insert(key);
                        }
                    }
                    if this.regenerating.is_empty() {
                        // Out of date while a view's source differs from the
                        // document (other sources, e.g. the chooser's, don't
                        // update the views; qa5 D17).
                        this.out_of_date = this.sources_stale(cx);
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
        // The views are made from the current document from now on, also
        // when it is reopened.
        if self.out_of_date || self.old_state.is_some() {
            let head = self.doc.read(cx).session.head().ok().map(|m| m.to_string());
            self.edit("Update", cx, |d| d.sources_at = head);
        }
        self.ensure_sources(true, cx);
        self.status = Some("Updating drawing…".into());
    }

    // ------------------------------------------------------------ geometry

    /// Pick up cached view geometry, schedule missing computations and
    /// reassemble the sheet.
    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        self.ensure_custom_tables(cx);
        let mut d = self.display().clone();
        let si = self.sheet.min(d.sheets.len().saturating_sub(1));
        // A note edited in place: the editor shows its text.
        if let Some(id) = self.inplace_hidden
            && let Some(AnnotationKind::Note { text, .. }) = d.sheets.get_mut(si).and_then(|s| s.annotations.iter_mut().find(|a| a.id == id)).map(|a| &mut a.kind)
        {
            text.clear();
        }
        if let Some((id, r, c)) = self.inplace_cell
            && let Some(TableKind::General { cells, .. }) = d.sheets.get_mut(si).and_then(|s| s.tables.iter_mut().find(|t| t.id == id)).map(|t| &mut t.kind)
            && let Some(cell) = cells.get_mut(r).and_then(|row| row.get_mut(c))
        {
            cell.clear();
        }
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
            if let Some(k) = views::view_key(&d, sheet, v.id, &self.sources, false) {
                match self.cache.get(k) {
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
                }
            }
        }
        self.geoms = geoms;
        if need_fast {
            self.schedule_compute(false, cx);
        } else if need_exact && !interactive {
            self.schedule_compute(true, cx);
        }
        let mut built = assemble_sheet(&d, si, &self.geoms, &self.sources, &self.info);
        // Model space (live behaviours "Imported DWG/DXF drawings"): no paper,
        // the canvas in the model-space colour; on Dark view the foreground
        // ink (ACI 7) draws white, other colours stay.
        let model = d.sheets.get(si).and_then(|s| s.imported.as_ref()).is_some_and(|i| i.model);
        let dark = model && !crate::preferences_window::ListPrefs::load().drawing_light_model_space;
        self.model_bg = model.then_some(if dark { 0x000000 } else { 0xffffff });
        // A model space has no paper and no watermark.
        self.watermark = if model { None } else { crate::drawing_watermark::text(self.doc.read(cx), self.tab) };
        if dark {
            let g = &mut built.geometry;
            let all = noshape_model::drawing::sheet::ColorSpan { lines: 0..g.lines.len(), texts: 0..g.texts.len(), fills: 0..g.fills.len(), rgb: [255, 255, 255] };
            g.colors.insert(0, all);
        }
        self.built = Some(Arc::new(built));
        self.geometry_dirty = true;
        self.sync_shaded(cx);
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

    pub fn local(&self, p: Point<Pixels>) -> [f64; 2] {
        [f64::from(f32::from(p.x - self.bounds.origin.x)), f64::from(f32::from(p.y - self.bounds.origin.y))]
    }

    pub fn zoom_to_fit(&mut self, cx: &mut Context<Self>) {
        let s = self.size_px();
        if s[0] < 10.0 || s[1] < 10.0 {
            return;
        }
        // A model space has no paper: fit its content.
        let content = self.current_sheet().imported.as_ref().filter(|m| m.model).and_then(|m| m.extents());
        let (x0, y0, w, h) = match content {
            Some(b) => (b[0], b[1], (b[2] - b[0]).max(1.0), (b[3] - b[1]).max(1.0)),
            None => {
                let (w, h) = self.current_sheet().size.size_mm();
                (0.0, 0.0, w, h)
            }
        };
        // Live (drawing/156, ISO A3): 40 px clear on each side, the bottom
        // measured to the window's edge as if the canvas ran under the 30 px
        // tab bar (sheet 1304×922 centred at y 577 in a 1919×1079 window).
        const UNDER: f64 = 30.0;
        self.zoom = ((s[0] - 80.0) / w).min((s[1] + UNDER - 80.0) / h).clamp(0.05, 200.0);
        self.center = [x0 + w / 2.0, y0 + h / 2.0 + UNDER / 2.0 / self.zoom];
        self.fitted = true;
        self.frame_dirty = true;
        cx.notify();
    }

    /// Ctrl+Shift+arrow: move the sheet a tenth of the canvas width toward
    /// the arrow (`dx`, `dy` = ±1).
    pub fn pan_step(&mut self, dx: f64, dy: f64, cx: &mut Context<Self>) {
        let step = self.size_px()[0] * 0.1 / self.zoom;
        self.center[0] -= dx * step;
        self.center[1] += dy * step;
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
        let mut c = Camera { projection: CamProjection::Orthographic, rotation: Camera::look_rotation(DVec3::NEG_Z, DVec3::Y), target: DVec3::new(self.center[0], self.center[1], 0.0), ..Default::default() };
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
        // Shaded views with their picture: the edges only show hovered or
        // selected (live behaviours "Shaded drawing views").
        let shaded: Vec<(ViewId, std::ops::Range<usize>)> = built.geometry.view_edges.iter().filter(|(v, r)| !r.is_empty() && self.shaded.images.contains_key(v)).cloned().collect();
        self.shaded.edges = shaded.iter().map(|(v, r)| (*v, up.upload_lines(&drawing_scene::range_lines(&built, r.clone(), self.zoom, true)), up.upload_lines(&drawing_scene::range_lines(&built, r.clone(), self.zoom, false)))).collect();
        let skip: Vec<std::ops::Range<usize>> = shaded.into_iter().map(|(_, r)| r).collect();
        self.lines = Some(up.upload_lines(&drawing_scene::sheet_lines(&built, self.zoom, &dangling, &skip, self.model_bg.is_none())));
        self.lines_zoom = self.zoom;
        self.fills = Some(up.upload_mesh(&drawing_scene::sheet_fills(&built)));
        self.watermark_mesh = self.watermark.as_ref().and_then(|t| {
            let (pts, idx) = noshape_model::drawing::font::text_fill_font(&crate::drawing_watermark::geometry(t, w, h), None);
            let mut m = noshape_render::MeshData::default();
            for p in pts {
                m.positions.push([p[0] as f32, p[1] as f32, 0.5]);
                m.normals.push([0.0, 0.0, 1.0]);
                m.pick_ids.push(0);
            }
            m.indices = idx;
            (!m.indices.is_empty()).then(|| up.upload_mesh(&m))
        });
        let (ink, red) = drawing_scene::sheet_text(&built, &dangling);
        self.texts = [ink, red].map(|m| (!m.indices.is_empty()).then(|| up.upload_mesh(&m)));
        let lin = |c: [u8; 3]| crate::theme::lin(((c[0] as u32) << 16) | ((c[1] as u32) << 8) | c[2] as u32);
        let (cf, ct) = drawing_scene::sheet_colored(&built);
        self.colored = (cf.iter().map(|(m, c)| (up.upload_mesh(m), lin(*c))).collect(), ct.iter().map(|(m, c)| (up.upload_mesh(m), lin(*c))).collect());
        self.backdrops = drawing_scene::sheet_backdrops(&built).iter().map(|(m, c)| (up.upload_mesh(m), lin(*c))).collect();
        self.geometry_dirty = false;
        self.sync_images();
    }

    fn rebuild_overlay(&mut self) {
        let mut overlay = self.overlay.clone();
        overlay.points.extend(self.selection_handles());
        self.note_grip_overlay(&mut overlay);
        self.leader_overlay(&mut overlay);
        self.detail_circle_overlay(&mut overlay);
        if overlay.is_empty() {
            self.overlay_handles = None;
            return;
        }
        let (l, pts) = overlay.to_render([139, 124, 255, 255]);
        let up = self.gpu.uploader();
        self.overlay_handles = Some((up.upload_lines(&l), up.upload_points(&pts)));
    }

    /// Click on a selected dimension's flip node (the diamond near its
    /// arrows): flip the arrows. True when used.
    pub fn flip_node_click(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(b) = &self.built else { return false };
        let r = 6.0 / self.zoom.max(1e-6);
        let c = self.cursor;
        let hit = self.selection.iter().find_map(|s| {
            let Sel::Annotation(id) = *s else { return None };
            let picks = self.picks_of(*s);
            b.geometry.grips.iter().any(|g| g.kind == noshape_model::drawing::sheet::GripKind::Handle && picks.contains(&g.pick) && (g.pos[0] - c[0]).abs() <= r && (g.pos[1] - c[1]).abs() <= r).then_some(id)
        });
        let Some(id) = hit else { return false };
        self.edit_sheet("Flip arrows", cx, |s| {
            if let Some(a) = s.annotations.iter_mut().find(|a| a.id == id) {
                a.flip_arrows = !a.flip_arrows;
            }
        });
        true
    }

    /// Pick ids belonging to a selection entry.
    pub fn picks_of(&self, sel: Sel) -> Vec<u32> {
        self.pk().picks_of(sel)
    }

    /// The selectable entity a pick id belongs to.
    pub fn sel_of_pick(&self, pick: u32) -> Option<Sel> {
        self.pk().sel_of_pick(pick)
    }

    fn build_scene(&self) -> Scene {
        let mut scene = Scene::default();
        // Onshape: the white sheet sits flat on the gray surround (no shadow).
        if let Some((_shadow, paper, _)) = self.paper.as_ref().filter(|_| self.model_bg.is_none()) {
            scene.instances.push(Instance::new(Drawable::Mesh(paper.clone())).with_color([1.0, 1.0, 1.0, 1.0]).unlit().unpickable().layer(-5));
        }
        // Inserted images and backdrops, under the sheet geometry.
        scene.instances.extend(self.image_instances());
        scene.instances.extend(self.shaded_instances());
        for (m, color) in &self.backdrops {
            scene.instances.push(Instance::new(Drawable::Mesh(m.clone())).with_color(*color).unlit().unpickable().layer(-1));
        }
        if let Some(f) = &self.fills {
            scene.instances.push(Instance::new(Drawable::Mesh(f.clone())).with_color(drawing_scene::FILL_COLOR).unpickable());
        }
        if let Some(l) = &self.lines {
            scene.instances.push(Instance::new(Drawable::Lines(l.clone())).per_primitive_color());
        }
        scene.instances.extend(self.shaded_edge_instances());
        for (m, color) in self.texts.iter().zip([[0.0, 0.0, 0.0, 1.0], crate::theme::lin(drawing_scene::DANGLING_RGB)]) {
            if let Some(m) = m {
                scene.instances.push(Instance::new(Drawable::Mesh(m.clone())).with_color(color).unlit());
            }
        }
        // Colored fills (e.g. solid inspection items), then colored texts on top.
        for (m, color) in &self.colored.0 {
            scene.instances.push(Instance::new(Drawable::Mesh(m.clone())).with_color(*color).unlit().unpickable().layer(1));
        }
        for (m, color) in &self.colored.1 {
            scene.instances.push(Instance::new(Drawable::Mesh(m.clone())).with_color(*color).unlit().layer(2));
        }
        // The release watermark, translucent over the sheet's content (live).
        if let Some(m) = self.watermark_mesh.as_ref().filter(|_| self.paper.is_some() && self.model_bg.is_none()) {
            let [r, g, b, _] = crate::theme::lin(crate::drawing_watermark::EDITOR_COLOR);
            scene.instances.push(Instance::new(Drawable::Mesh(m.clone())).with_color([r, g, b, crate::drawing_watermark::EDITOR_ALPHA]).unlit().unpickable().over_lines());
        }
        if let Some((l, pts)) = &self.overlay_handles {
            scene.instances.push(Instance::new(Drawable::Lines(l.clone())).per_primitive_color().unpickable().layer(20));
            scene.instances.push(Instance::new(Drawable::Points(pts.clone())).per_primitive_color().unpickable().layer(21));
        }
        // Hover and selection highlight states.
        if let Some(b) = &self.built {
            scene.states = vec![0; b.picks.len()];
            // A selected view label has its own look (push_label_highlights),
            // and so does a selected datum (push_datum_selection).
            let datums = self.datum_selection_picks();
            for s in self.selection.iter().chain(self.panel_highlight.iter()).filter(|s| !matches!(s, Sel::ViewLabel(_))) {
                for p in self.picks_of(*s).into_iter().filter(|p| !datums.contains(p)) {
                    scene.add_flag(p, state::SELECTED);
                }
            }
            if let Some(h) = self.hover {
                let single = matches!(self.tool, Tool::Dimension { .. } | Tool::Auxiliary { .. } | Tool::Centermark | Tool::Centerline { .. } | Tool::Balloon { .. } | Tool::Ordinate { .. });
                match self.sel_of_pick(h) {
                    // Onshape (drawing/14): a hovered annotation turns tan with
                    // a tan label box and its grips, drawn on top below.
                    Some(s @ Sel::Annotation(_)) if !single => self.push_annotation_hover(&mut scene, b, &self.picks_of(s)),
                    // Hovered labels and indicators show their dots only.
                    Some(Sel::ViewLabel(_) | Sel::AuxIndicator(_)) if !single => {}
                    _ => {
                        let group = if single { vec![h] } else { self.sel_of_pick(h).map(|s| self.picks_of(s)).unwrap_or_else(|| vec![h]) };
                        for p in group {
                            scene.add_flag(p, state::HOVER);
                        }
                    }
                }
            }
            // The hovered / selected annotations' referenced edges (blue) and
            // group members (yellow).
            self.push_related_highlights(&mut scene);
            self.push_replicate_overlay(&mut scene);
            self.push_label_highlights(&mut scene);
            self.push_datum_selection(&mut scene);
            self.push_aux_states(&mut scene);
        }
        scene
    }

    /// Onshape's hovered-annotation look (live, drawing/14): its lines in
    /// `#ccb292`, a `#ccb292` box behind its text, attachment points as
    /// `#90cef1` dots, dimension-line ends as `#cc925c` dots and flip
    /// handles as `#cc925c` diamonds.
    fn push_annotation_hover(&self, scene: &mut Scene, built: &BuiltSheet, picks: &[u32]) {
        use noshape_model::drawing::font;
        use noshape_model::drawing::sheet::GripKind;
        use noshape_render::{LineData, LineSegment, MeshData, PointData, PointSprite};
        const TAN: [u8; 4] = [0xcc, 0xb2, 0x92, 255];
        const GRIP: [u8; 4] = [0xcc, 0x92, 0x5c, 255];
        const ORIGIN: [u8; 4] = [0x90, 0xce, 0xf1, 255];
        let g = &built.geometry;
        let mut lines = LineData::default();
        for l in g.lines.iter().filter(|l| picks.contains(&l.pick)) {
            let width = drawing_scene::px_width(l.style, self.zoom).max(1.5);
            for w in l.points.windows(2) {
                lines.segments.push(LineSegment { a: [w[0][0] as f32, w[0][1] as f32, 0.4], b: [w[1][0] as f32, w[1][1] as f32, 0.4], color: TAN, pick_id: 0, width, style: LineSegment::ON_TOP });
            }
        }
        // Label boxes (behind the text, above the paper) and diamond grips.
        let mut boxes = MeshData::default();
        let mut diamonds = MeshData::default();
        let quad = |m: &mut MeshData, c: [[f64; 2]; 4], z: f32| {
            let base = m.positions.len() as u32;
            for p in c {
                m.positions.push([p[0] as f32, p[1] as f32, z]);
                m.normals.push([0.0, 0.0, 1.0]);
                m.pick_ids.push(0);
            }
            m.indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        };
        for t in g.texts.iter().filter(|t| picks.contains(&t.pick)) {
            quad(&mut boxes, font::text_box(t), -0.5);
        }
        let mut points = PointData::default();
        let r = 6.0 / self.zoom.max(1e-6);
        // Onshape shows the grips instead of the arrowheads: cover the
        // arrows that end at this annotation's dimension-line ends.
        let ends: Vec<[f64; 2]> = g.grips.iter().filter(|gr| picks.contains(&gr.pick) && gr.kind == GripKind::End).map(|gr| gr.pos).collect();
        let mut covers = MeshData::default();
        for f in g.fills.iter().filter(|f| f.points.len() == 3 && ends.iter().any(|e| (e[0] - f.points[0][0]).abs() < 1e-6 && (e[1] - f.points[0][1]).abs() < 1e-6)) {
            let base = covers.positions.len() as u32;
            for p in &f.points {
                covers.positions.push([p[0] as f32, p[1] as f32, 0.2]);
                covers.normals.push([0.0, 0.0, 1.0]);
                covers.pick_ids.push(0);
            }
            covers.indices.extend_from_slice(&[base, base + 1, base + 2]);
        }
        for gr in g.grips.iter().filter(|gr| picks.contains(&gr.pick)) {
            let [x, y] = gr.pos;
            match gr.kind {
                GripKind::Handle => quad(&mut diamonds, [[x - r, y], [x, y - r], [x + r, y], [x, y + r]], 0.6),
                kind => points.points.push(PointSprite { position: [x as f32, y as f32, 0.7], color: if kind == GripKind::Origin { ORIGIN } else { GRIP }, pick_id: 0, size: 9.0, style: LineSegment::ON_TOP, _pad: 0 }),
            }
        }
        let up = self.gpu.uploader();
        if !boxes.indices.is_empty() {
            scene.instances.push(Instance::new(Drawable::Mesh(up.upload_mesh(&boxes))).with_color(crate::theme::lin(0xccb292)).unlit().unpickable().layer(-4));
        }
        if !covers.indices.is_empty() {
            scene.instances.push(Instance::new(Drawable::Mesh(up.upload_mesh(&covers))).with_color([1.0, 1.0, 1.0, 1.0]).unlit().unpickable().layer(19));
        }
        if !lines.segments.is_empty() {
            scene.instances.push(Instance::new(Drawable::Lines(up.upload_lines(&lines))).per_primitive_color().unpickable().layer(20));
        }
        if !diamonds.indices.is_empty() {
            scene.instances.push(Instance::new(Drawable::Mesh(up.upload_mesh(&diamonds))).with_color(crate::theme::lin(0xcc925c)).unlit().unpickable().layer(21));
        }
        if !points.points.is_empty() {
            scene.instances.push(Instance::new(Drawable::Points(up.upload_points(&points))).per_primitive_color().unpickable().layer(22));
        }
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
        // Gray drawing area around the white sheet; Onshape highlight colors.
        let bg = crate::theme::lin(self.model_bg.unwrap_or(crate::theme::viewport::DRAWING_BG));
        style.background_top = bg;
        style.background_bottom = bg;
        // Drawings use their own (behaviours 1480, asb/u-906/907): hover
        // #CCB292, selected #CC925C.
        style.hover_color = crate::theme::lin(0xccb292);
        style.select_color = crate::theme::lin(0xcc925c);
        // Other members of a hovered / selected annotation group (live
        // drawing/423: #EBEB92).
        style.accent_color = crate::theme::lin(0xebeb92);
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
        self.pk().tol_mm(px)
    }

    pub fn update_hover(&mut self, cx: &mut Context<Self>) {
        let hover = self.pk().hover_at();
        if hover != self.hover {
            self.hover = hover;
            self.redraw(cx);
        } else if self.replicate_dlg.is_some() {
            // The Replicate region box follows the pointer.
            self.redraw(cx);
        }
    }

    pub fn placement(&self, view: ViewId) -> Option<noshape_model::drawing::build::ViewPlacement> {
        self.pk().placement(view)
    }

    /// The view under a sheet point (smallest containing view rectangle).
    pub fn view_at(&self, s: [f64; 2]) -> Option<ViewId> {
        self.pk().view_at(s)
    }
}

/// The drawing editor's cached slices (see `crate::cached_slice`): a keystroke
/// or a caret blink in a note re-renders the editor but reuses these.
#[derive(Clone)]
struct DrSlices {
    toolbar: Entity<crate::cached_slice::Slice<DrawingEditor>>,
    panel: Entity<crate::cached_slice::Slice<DrawingEditor>>,
}

impl DrawingEditor {
    fn slices(&mut self, cx: &mut Context<Self>) -> DrSlices {
        if let Some(s) = &self.slices {
            return s.clone();
        }
        use crate::cached_slice::Slice;
        let me = cx.entity();
        let toolbar = Slice::new(&me, |this, window, cx| this.render_toolbar(window, cx), cx);
        let panel = Slice::new(&me, |this, _, cx| div().size_full().relative().children(this.render_panel(cx)).into_any_element(), cx);
        // Undo state comes from the document.
        for s in [&toolbar, &panel] {
            Slice::observing(s, &self.doc, cx);
        }
        let s = DrSlices { toolbar, panel };
        self.slices = Some(s.clone());
        s
    }
}

impl Render for DrawingEditor {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Live (dk-950): in dark mode only the shell is dark; the drawing's
        // toolbar, canvas, sheet and panels stay in the light theme.
        gpui_kit::with_light_colors(|| self.render_light(window, cx))
    }
}

impl DrawingEditor {
    fn render_light(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        // A closed view dialog took the focus with it (its fields): the sheet
        // takes it back, so shortcuts work at once (qa5 D5; live keeps them).
        if window.focused(cx).is_none() {
            window.focus(&self.focus, cx);
        }
        self.sync_section_dlg(window, cx);
        self.sync_break_dlg(window, cx);
        self.sync_aux_dlg(window, cx);
        self.sync_detail_dlg(window, cx);
        self.sync_crop_dlg();
        self.sync_chooser(window, cx);
        self.sync_revision_dlg(window, cx);
        self.sync_weld_dlg(window, cx);
        self.sync_callout_dlg(window, cx);
        self.sync_table_dlg();
        self.sync_custom_dlg(cx);
        self.sync_table_tool(window, cx);
        self.sync_cell_panel(window, cx);
        self.sync_datum_dlg(window, cx);
        self.sync_gdt_dlg(window, cx);
        // The label typed in the Auxiliary view dialog (pending_view has no cx).
        if let Some(d) = &self.aux_dlg {
            self.aux_letter = d.label.read(cx).value().trim().to_string();
        }
        self.sync_centermark_dlg();
        self.sync_insert_inputs(window, cx);
        self.sync_drawprops_inputs(window, cx);
        self.track_sheet_len_focus(window, cx);
        if !self.fitted && self.size_px()[0] > 10.0 {
            self.zoom_to_fit(cx);
        }
        if self.geometry_dirty || self.frame_dirty {
            self.submit(window);
        }
        let want = match self.tool {
            _ if !self.canvas_hovered => None,
            Tool::Select => None,
            Tool::Centermark => Some(crate::cursor::AppCursor::CrosshairBox),
            _ => Some(crate::cursor::AppCursor::Crosshair),
        };
        crate::cursor::CursorSync::sync(self, want, window, cx, |v| &mut v.cursor_sync);
        let pal = p();
        let entity = cx.entity();
        let image = self.image.clone();
        let slices = self.slices(cx);
        let status = self.render_status(cx);
        // The note edited in place hides its sheet text while its editor is open.
        let editing = match &self.panel {
            Some(Panel::Note { edit, .. }) => *edit,
            _ => None,
        };
        let cell = match &self.panel {
            Some(Panel::Cell { table, row, col, .. }) if self.current_sheet().table(*table).is_some_and(|t| matches!(t.kind, TableKind::General { .. })) => Some((*table, *row, *col)),
            _ => None,
        };
        if editing != self.inplace_hidden || cell != self.inplace_cell {
            self.inplace_hidden = editing;
            self.inplace_cell = cell;
            self.refresh(cx);
        }
        // The closed panel's sheet chips are drawn on the canvas under the
        // dialogs, which cover them (qa5 D16; live).
        let sheet_bar = self.sheets_open.then(|| self.render_sheet_bar(cx));
        let sheet_chips = (!self.sheets_open).then(|| self.render_sheet_tabs(cx));
        // The panel (the Note ✓ ✕ panel above a note typed in place, …) as a
        // cached layer: a keystroke or caret blink in the note re-renders
        // the editor, not the panel. It reads the note's text and caret, so
        // it follows the note's field.
        let editing_text = match &self.panel {
            Some(Panel::Note { input, .. } | Panel::Cell { input, .. }) => Some(input.clone()),
            _ => None,
        };
        crate::cached_slice::Slice::follow(&slices.panel, editing_text.as_ref(), cx);
        let panel = slices.panel.clone().cached(StyleRefinement::default().absolute().top_0().left_0().size_full());
        let canvas_el = div()
            .id("drawing-canvas")
            .flex_1()
            .min_h_0()
            .relative()
            .overflow_hidden()
            .bg(rgb(self.model_bg.unwrap_or(crate::theme::viewport::DRAWING_BG)))
            // Live (dom/cursors.txt): no tool `auto`; the tools Onshape's
            // DrawingCrosshair-01.cur, Centermark DrawingCrosshairBox3-01.cur.
            .on_hover(cx.listener(|this, h: &bool, _, cx| {
                this.canvas_hovered = *h;
                cx.notify();
            }))
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
            .child(self.sheets_toggle(0., cx))
            .children(sheet_chips)
            .child(status)
            .children(self.render_note_inplace(cx))
            .children(self.render_sketch_dim_box(cx))
            .children(self.render_cell_inplace(cx))
            .children(self.render_rotate_label())
            .children(self.render_cell_panel(cx))
            .child(panel)
            .children(self.render_view_props(cx))
            .children(self.render_zoom_window())
            .children(self.render_shortcut_toolbar(cx))
            .children(self.render_measure())
            .children(self.render_sheet_props(cx))
            .children(self.render_drawing_props(cx))
            .children(self.render_toast(cx))
            .children(self.render_section_dlg(cx))
            .children(self.render_break_dlg(cx))
            .children(self.render_aux_dlg(cx))
            .children(self.render_aux_edit(cx))
            .children(self.render_detail_dlg(cx))
            .children(self.render_crop_dlg(cx))
            .children(self.render_revision_dlg(cx))
            .children(self.render_weld_dlg(cx))
            .children(self.render_callout_dlg(cx))
            .children(self.render_stack_dlg(cx))
            .children(self.render_table_dlg(cx))
            .children(self.render_custom_dlg(cx))
            .children(self.render_move_sheet(cx))
            .children(self.render_table_props(cx))
            .children(self.render_linestyle(cx))
            .children(self.render_hide_edges(cx))
            .children(self.render_section_edit(cx))
            .children(self.render_replicate_dlg(cx))
            .children(self.render_palette(window, cx))
            .children(self.render_datum_dlg(cx))
            .children(self.render_table_tool(cx))
            .children(self.render_gdt_dlg(cx))
            .children(self.render_stack_banner(cx))
            .children(self.render_view_sketches(cx))
            .children(self.render_hide_parts(cx))
            .children(self.render_centermark_dlg(cx))
            .children(self.render_painter(cx))
            .children(self.render_inspection_dlg(cx))
            .children(self.render_hatch_dlg(cx))
            .children(self.render_sketch_glyphs(window, cx))
            .children(self.render_right_panel(window, cx))
            .child(self.render_panel_strip(cx))
            .children(self.render_insert_view_panel(cx))
            .children(self.render_insert_chooser(cx))
            .children(self.render_ref_manager(cx))
            .children(self.render_color_dialog(cx))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_down(MouseButton::Right, cx.listener(Self::on_mouse_down))
            .on_mouse_down(MouseButton::Middle, cx.listener(Self::on_mouse_down))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_up(MouseButton::Right, cx.listener(Self::on_mouse_up))
            .on_mouse_up(MouseButton::Middle, cx.listener(Self::on_mouse_up))
            .on_scroll_wheel(cx.listener(Self::on_scroll))
            .on_pinch(cx.listener(Self::on_pinch));
        v_flex()
            .size_full()
            .bg(pal.app_bg)
            .key_context("Drawing")
            .track_focus(&self.focus)
            // Esc closes the dialog whose field had the focus: the canvas
            // takes it back, so shortcuts (d, n, …) work at once (qa5 D5).
            .on_action(cx.listener(|this, _: &Cancel, window, cx| {
                this.cancel(cx);
                window.focus(&this.focus, cx);
            }))
            .on_action(cx.listener(|this, _: &DeleteSelection, _, cx| this.delete_selection(cx)))
            .on_action(cx.listener(|this, _: &Copy, _, cx| {
                if !this.copy_cells(cx) && !this.copy_labels(cx) {
                    this.copy_views(None, cx)
                }
            }))
            .on_action(cx.listener(|this, _: &ZoomWindow, _, cx| this.start_zoom_window(cx)))
            .on_action(cx.listener(|this, _: &ShortcutToolbar, _, cx| this.toggle_shortcut_toolbar(cx)))
            .on_action(cx.listener(|this, _: &MeasureTool, _, cx| this.toggle_measure(cx)))
            .on_action(cx.listener(|this, _: &RenameSelected, window, cx| this.rename_selected(window, cx)))
            .on_action(cx.listener(|this, _: &Paste, _, cx| {
                // Spreadsheet cells paste as a new table; copied views as views.
                if !this.start_paste_table(cx) {
                    this.paste_views(cx)
                }
            }))
            .on_action(cx.listener(|this, _: &ZoomToFit, _, cx| this.zoom_to_fit(cx)))
            // Live (navigation/50-55): Shift+Z / Z zoom ×2 / ×½ about the view
            // centre; Ctrl+Shift+arrows move the sheet 10 % of the canvas
            // width toward the arrow; plain arrows do nothing in drawings.
            .on_action(cx.listener(|this, _: &ZoomIn, _, cx| {
                let s = this.size_px();
                this.zoom_by(2.0, [s[0] / 2.0, s[1] / 2.0], cx)
            }))
            .on_action(cx.listener(|this, _: &ZoomOut, _, cx| {
                let s = this.size_px();
                this.zoom_by(0.5, [s[0] / 2.0, s[1] / 2.0], cx)
            }))
            .on_action(cx.listener(|this, _: &PanLeft, _, cx| this.pan_step(-1.0, 0.0, cx)))
            .on_action(cx.listener(|this, _: &PanRight, _, cx| this.pan_step(1.0, 0.0, cx)))
            .on_action(cx.listener(|this, _: &PanUp, _, cx| this.pan_step(0.0, -1.0, cx)))
            .on_action(cx.listener(|this, _: &PanDown, _, cx| this.pan_step(0.0, 1.0, cx)))
            .on_action(cx.listener(|this, _: &DrawingUpdate, _, cx| this.update_drawing(cx)))
            // Onshape drawing letters that collide with Part Studio bindings.
            .on_action(cx.listener(|this, _: &ViewNormal, _, cx| this.set_tool(Tool::Note, cx)))
            .on_action(cx.listener(|this, _: &TogglePlanes, _, cx| this.set_tool(Tool::Projected { parent: None }, cx)))
            .on_key_down(cx.listener(Self::on_key))
            .child(slices.toolbar.clone().cached(StyleRefinement::default().w_full().h(px(crate::ui::TOOLBAR_H)).flex_none()))
            .child({
                // The open Sheets panel pushes the canvas (live drawing/30);
                // the closed one's sheet chips are on the canvas.
                let canvas_el = canvas_el.h_full().min_w_0().border_t_1().border_color(pal.border);
                gpui_kit::component::h_flex().flex_1().min_h_0().relative().children(sheet_bar).child(canvas_el)
            })
            .into_any_element()
    }
}
