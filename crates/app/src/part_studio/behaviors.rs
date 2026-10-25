//! Onshape interaction behaviors of the Part Studio that are not tied to a
//! single panel: hide/show (Y, Shift+Y), isolate / make transparent
//! (Shift+I, Shift+T), Select other (`), the S shortcut toolbar, previous
//! view and zoom to selection/window, unified picking with filter feedback,
//! cross-highlighting between the feature list and the graphics area,
//! feature-list reordering rules and the automatic Add/Remove choices of a
//! new Extrude.

use super::{PartStudioView, feature_icon, kind_matches};
use crate::scene_builder::{PickKind, PickTarget};
use crate::theme::{p, with_alpha};
use glam::DVec2;
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::*;
use gpui_kit::prelude::FluentBuilder as _;
use noshape_model::display::{FeatureStatus, StudioDisplay};
use noshape_model::interaction;
use noshape_model::params::{ParamValue, QueryFilter};
use noshape_model::reference::{BodyId, FeatureId, Reference};
use noshape_render::Camera;
use std::collections::HashSet;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Isolate (Shift+I) keeps the chosen parts opaque; Make transparent
/// (Shift+T) makes them transparent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IsolateMode {
    Isolate,
    Transparent,
}

pub struct Isolation {
    pub mode: IsolateMode,
    pub bodies: Vec<BodyId>,
    /// The transparency slider, 0 to 1 (`iso_look`).
    pub opacity: f32,
    /// "Select transparent geometry": picks may land on transparent parts.
    pub select_transparent: bool,
    /// "Expand: distance" slider step, 0 to 10: parts within a growing
    /// distance join the chosen ones (`isolation_transparent`).
    pub expand: usize,
}

/// What the Dynamic suppression dialog sets: a feature's "Unsuppressed if"
/// expression, or that of every feature in a folder.
#[derive(Clone, Copy)]
pub enum DynTarget {
    Feature(FeatureId),
    Folder(u32),
}

/// Open "Select other" list: candidates nearest first, highlighted index.
pub struct SelectOther {
    pub hits: Vec<Reference>,
    /// The highlighted entry (bold): none when opened from a menu.
    pub index: Option<usize>,
    /// Viewport-local point it was opened for: the click zone's centre.
    pub at: DVec2,
    /// Where the ray met the nearest entity: a yellow point while the
    /// pointer is out of the click zone (SELECT_OTHER_POINT_*).
    pub point: Option<glam::DVec3>,
    pub in_zone: bool,
    pub moved_out: bool,
    pub scroll: ScrollHandle,
}

/// Onshape's Select other constants (woolsthorpe: SELECTION_DIALOG_OFFSET_X /
/// _Y, SELECT_OTHER_CLICK_ZONE_RADIUS_PX, _PICK_RADIUS_PX, _MAXIMUM_ELEMENTS)
/// and the dialog's size (dom/select-other-popup.txt: 220 × 149).
const SELECT_OTHER_OFFSET: (f32, f32) = (100., -25.);
const SELECT_OTHER_ZONE_PX: f64 = 8.;
const SELECT_OTHER_PICK_PX: f64 = 8.;
const SELECT_OTHER_MAX: usize = 20;
const SELECT_OTHER_SIZE: (f32, f32) = (220., 149.);

/// Consumed dialog picks: (picks, accent picks, before-model, lines).
pub type ConsumedLines = (Vec<Reference>, Vec<Reference>, Vec<Reference>, usize, Option<noshape_render::Instance>);

/// View state for the behaviors above.
pub struct UiState {
    /// Edit in context: the assembly shown around the part.
    pub(super) in_context: Option<super::in_context::InContextUi>,
    /// Assembly contexts › ⋯ › Rename…: the context and its name field.
    pub(super) context_rename: Option<(u32, Entity<gpui_kit::component::input::InputState>, std::rc::Rc<Subscription>)>,
    /// The feature whose context arrow is hovered: its context references
    /// are outlined in magenta (live assembly/506).
    pub(super) context_arrow_hover: Option<noshape_model::FeatureId>,
    pub isolate: Option<Isolation>,
    /// Derived rows whose derived sketches are listed under them (▸ open).
    pub derived_open: std::collections::HashSet<FeatureId>,
    /// The open dialog's feature info shown as the graphics bubble.
    pub dialog_info: Option<String>,
    /// Cameras before discrete view changes (Previous view).
    pub cam_history: Vec<Camera>,
    /// Last pointer position over the viewport (viewport-local).
    pub pointer: Option<DVec2>,
    /// The pick under the pointer before midpoint snapping
    /// ([`PartStudioView::hover_target`]).
    pub hover_pick: Option<PickTarget>,
    pub select_other: Option<SelectOther>,
    /// S shortcut toolbar position (viewport-local).
    pub mini_toolbar: Option<DVec2>,
    /// The open Camera and render options menu (its button shows pressed).
    pub camera_menu: Option<WeakEntity<crate::os_menu::OsMenu>>,
    /// Feature list search box.
    pub filter: Option<(Entity<InputState>, Subscription)>,
    pub filter_query: String,
    /// Feature row under the pointer (its geometry is highlighted).
    pub list_hover: Option<FeatureId>,
    /// Last clicked feature (anchor of Shift+click ranges).
    pub anchor: Option<FeatureId>,
    /// Last viewport click (double-click detection).
    pub last_click: Option<(Reference, Instant)>,
    /// Focus of the feature list (rollback bar arrow keys).
    pub list_focus: FocusHandle,
    /// The feature list's scroll position and size (qa4: rows out of view
    /// are drawn as empty rows, `render_feature_list`).
    pub list_scroll: ScrollHandle,
    /// Feature-list autoscroll while a row or the rollback bar is dragged:
    /// px/s (positive down) and whether its loop is running.
    pub list_autoscroll: f32,
    pub list_autoscrolling: bool,
    /// The rollback bar took the keyboard (↑/↓ or a click on it) while these
    /// features were selected; it reads light blue until the selection changes.
    pub rollback_keyboard: Option<Vec<noshape_model::reference::FeatureId>>,
    /// "Show dependencies" panel for a feature.
    pub dependencies: Option<FeatureId>,
    pub cube_hover: Option<super::view_cube::CubeCell>,
    pub cube_origin: Option<Point<Pixels>>,
    /// Measure details pinned open with `[`.
    pub measure_pinned: bool,
    /// Toolbar groups whose face is the last tool picked from the dropdown.
    pub toolbar_faces: std::collections::HashMap<&'static str, &'static str>,
    /// Search tools: name and description of the hovered result.
    /// Tool search: the highlighted result and "Highlight tool in toolbar".
    pub palette_sel: usize,
    /// Appearances panel: the "Sketches and curves" tab is open.
    pub appearance_sketches: bool,
    /// The open Graphics preferences dialog.
    pub graphics_dialog: Option<(Entity<crate::graphics_prefs_dialog::GraphicsPrefsDialog>, Subscription)>,
    pub palette_no_highlight: bool,
    /// Tool search: the viewport's top in window coordinates (the popover
    /// hangs from the window's right edge, over any right panel, as live).
    pub palette_top: std::rc::Rc<std::cell::Cell<Pixels>>,
    /// When the tool search last closed (a click on its toggle closes it
    /// through the outside click; the click itself must not reopen it).
    pub palette_closed: Option<std::time::Instant>,
    /// Variable hovered in the variable table.
    pub var_hover: Option<String>,
    /// The Variable table's own table is folded (header ⌄ / Collapse all tables).
    pub var_folded: bool,
    /// The Variable table's add row takes the focus on the next render
    /// (after a variable was added from it).
    pub var_focus_new: bool,
    /// Configured properties: the part shown ("Properties for ▾"), the
    /// selected cell (row, input id or property) and the table's focus.
    pub cp_part: Option<noshape_model::BodyId>,
    pub cp_selected: Option<(usize, String)>,
    pub cp_focus: Option<FocusHandle>,
    /// The table's input cells by row and column (`cell_input` keys; `None`
    /// for read-only cells): Enter moves down, Tab right (live 10-20 q).
    pub cp_grid: Vec<Vec<Option<String>>>,
    /// The "Variable input" / "Edit <name>" dialog of a configuration
    /// variable.
    pub var_input: Option<super::config_var_dialog::VarInputDialog>,
    /// Inspection › Datum: its dialog and the symbol being placed.
    pub datum_edit: Option<super::annotations_ui::DatumEdit>,
    /// An annotation picked with its menu's Select.
    pub annotation_selected: Option<u32>,
    pub annotation_drag: Option<super::annotations_ui::AnnotationDrag>,
    /// Variable table: where a dragged Variable Studio table would drop
    /// (`usize::MAX`: over the Part Studio's own table, refused).
    pub var_drop: Option<usize>,
    /// Inspection panel: the pressed tool (a pick mode until Esc) and the open ⋯.
    pub inspection_tool: Option<&'static str>,
    pub inspection_more: bool,
    /// Inspection panel: default tolerances picker and Characteristics filters.
    pub insp: super::default_tolerances::InspectionView,
    /// Custom tables panel (selected table, its result, the hovered row).
    pub ct: super::custom_tables::CtState,
    /// Variable Studio tables shown in the Variable table panel.
    pub var_tables: std::collections::HashMap<noshape_model::document::TabId, Entity<crate::variable_studio::VariableStudioView>>,
    /// Section view planes (Shift+X).
    pub section: Vec<super::section::SectionPlane>,
    /// Section view dialog, and the parts excluded (false) or the only ones
    /// included (true) in the cut.
    pub section_dialog: Option<super::section::SectionDialog>,
    /// The section offset box being typed into (section.rs).
    pub section_offset_edit: Option<(Entity<gpui_kit::component::input::InputState>, Subscription)>,
    /// The section manipulator: the last ring drag (its angle shows in the
    /// value box), the base moved over the plane, a base drag, the hovered handle.
    pub section_rot: Option<super::section_manip::RingDrag>,
    pub section_base: Option<glam::DVec3>,
    pub section_base_drag: bool,
    pub section_hover: Option<super::section_manip::SectionHandle>,
    pub section_parts: Option<(bool, Vec<BodyId>)>,
    /// Open Appearance editor, and faces of a hovered appearance row.
    pub appearance_edit: Option<super::appearance::AppearanceEdit>,
    pub appearance_hover: Vec<Reference>,
    /// Active analysis color map (Shift+C / Shift+D).
    pub analysis: Option<super::analysis::Analysis>,
    /// Per-vertex colors for the current display: (key, colors).
    pub vcolor_cache: Option<((usize, String), Option<super::appearance::VertexColors>)>,
    /// View in high quality (Shift+R).
    pub high_quality: bool,
    /// View in high quality was on in this session: Onshape keeps the Very
    /// fine meshes it refined until reload (live 10-23 a3), so the regen
    /// keeps the level after Shift+R goes off.
    pub high_quality_seen: bool,
    /// Parts list › Assign material…: the Material panel.
    pub material_panel: Option<super::material_panel::MaterialPanel>,
    /// Where panel dialogs were moved by their title.
    pub panels: crate::ui::PanelOffsets,
    /// The Configurations block's height after its splitter was dragged.
    pub cfg_box_h: Option<f32>,
    /// The Create selection dialog.
    pub create_selection: Option<super::create_selection::CreateSelection>,
    /// Where the last context menu opened (window coordinates).
    pub menu_pos: Point<Pixels>,
    /// Paused regeneration: the studio when paused (for Discard).
    pub regen_paused: Option<noshape_model::partstudio::PartStudio>,
    /// Run one regeneration although paused.
    pub force_regen: bool,
    /// Regeneration times panel.
    pub show_regen_times: bool,
    /// Its sort: column (0 #, 1 Feature name, 2 Time) and descending.
    pub regen_sort: (u8, bool),
    /// Parts list: collapsed sections, height, part properties shown.
    pub collapsed_sections: HashSet<&'static str>,
    /// The parts list's share of the list panel's height once the separator
    /// was dragged (None: 35%, live).
    pub parts_height: Option<f32>,
    pub show_part_props: bool,
    /// The whole Features and parts list collapsed.
    pub list_collapsed: bool,
    /// Mass properties panel on its Face tab.
    pub mass_faces: bool,
    /// Measure type filter (None = Show all).
    pub measure_type: Option<&'static str>,
    /// The Measure panel's Length / Angle unit selects (None = workspace units).
    pub measure_length_unit: Option<noshape_units::LengthUnit>,
    pub measure_angle_unit: Option<noshape_units::AngleUnit>,
    /// Feature dialog position after the user moved it.
    pub dialog_pos: Option<Point<Pixels>>,
    /// Feature dialog width after the user resized it.
    pub dialog_width: Option<Pixels>,
    /// Toolbar customization (removed tools, group order, S toolbar).
    pub toolbar: super::toolbar_prefs::ToolbarPrefs,
    /// Customize toolbar… while it is open.
    pub tb_custom: Option<crate::toolbar_customize::Customize>,
    /// The Isolate slider being dragged (pressed on it, button still down).
    pub iso_slider_drag: Option<&'static str>,
    /// The info message whose 5 s auto-dismiss is running (live: Onshape's
    /// message bubbles close after 5000 ms; errors and warnings stay).
    pub message_timed: Option<(noshape_model::display::FeatureStatus, String)>,
    /// Preview slider value and the model before the edited feature.
    pub preview_opacity: f32,
    pub preview_before: Option<Arc<StudioDisplay>>,
    /// Regeneration context before the edited feature.
    pub before_context: Option<Arc<noshape_model::regen::context::Context>>,
    /// The edited feature's parameters when std's editing logic last ran
    /// (its `oldDefinition`; empty for a new feature's first change).
    pub std_edit_old: Option<noshape_model::params::ParamValues>,
    /// std's editing logic running off the UI thread (std Extrude runs a
    /// whole trial extrude for its boolean heuristics).
    pub std_edit_task: Option<Task<()>>,
    pub preview_task: Option<Task<()>>,
    /// Remove / Intersect previews (live part-studio/234-237): the tool
    /// body (the feature regenerated as New) and the task computing it.
    pub preview_tool: Option<(Arc<StudioDisplay>, noshape_model::FeatureId)>,
    /// A feature drag's drop slot: the hovered row and whether the cursor is
    /// in its lower half (lands after it; live F33).
    pub feature_drop: std::rc::Rc<std::cell::Cell<Option<(usize, bool)>>>,
    /// The open Hole dialog's parameters last time (its linked depths).
    pub hole_prev: Option<(noshape_model::FeatureId, noshape_model::params::ParamValues)>,
    pub preview_tool_task: Option<Task<()>>,
    /// Measure graphics uploaded for a selection.
    pub measure_lines: Option<(Vec<Reference>, Option<String>, Option<noshape_render::Instance>)>,
    /// The Measure panel's distance row under the pointer (its label): live
    /// draws that one measurement's dashed line, nothing otherwise.
    pub measure_hover: Option<String>,
    /// Sheet metal table and flat view: the chosen context (else the first
    /// model by name) and the collapsed tables.
    pub sm_context: Option<noshape_model::reference::FeatureId>,
    pub sm_bends_closed: bool,
    pub sm_joints_closed: bool,
    /// The Bends table's horizontal scroll.
    pub sm_bends_scroll: ScrollHandle,
    /// The table cell under the pointer: (table: 0 Bends / 1 Other joints,
    /// row, column).
    pub sm_hover: Option<(u8, usize, usize)>,
    /// The Bends cell being edited (row, column 2 Radius / 5 K Factor), and
    /// whether its field still needs the focus.
    pub sm_edit: Option<(usize, usize)>,
    pub sm_edit_focus: bool,
    /// The selected cell (table, row, column; column 0 selects the row).
    pub sm_cell: Option<(u8, usize, usize)>,
    /// Dynamic suppression editor: feature, expression input.
    /// The Dynamic suppression dialog: what it sets and its input.
    pub dyn_suppression: Option<(DynTarget, Entity<InputState>, Subscription)>,
    /// Shortcut map search box.
    pub shortcut_filter: Option<(Entity<InputState>, Subscription)>,
    pub shortcut_query: String,
    /// Shortcut sheet tab (General, Part Studio, …).
    pub shortcut_tab: usize,
    /// The shortcut map's rows (Chrome's scrollbar, live menus/616).
    pub shortcut_scroll: gpui_kit::ScrollHandle,
    /// Recently started tools, newest first (Search tools with no query).
    pub recent_tools: Vec<&'static str>,
    /// Camera menu "Orient normal to sketch on edit" and its state: an
    /// orientation is pending until the sketch editor opens; `oriented`
    /// restores the previous view when the sketch closes.
    pub orient_on_edit: bool,
    pub orient_pending: bool,
    pub oriented: bool,
    /// Values of REMEMBER_PREVIOUS_VALUE parameters from the last accepted
    /// feature of each type (Onshape defaults new features to them).
    pub remembered: std::collections::HashMap<(String, &'static str), ParamValue>,
    /// Feature list: "Default geometry" and "Parts" folders collapsed.
    pub default_geometry_collapsed: bool,
    /// Folders the user opened (true) or closed (false) in this view, over
    /// the document's state (a new folder starts closed). View state, not
    /// an undo step (live qa4 r5 s8 (7)).
    pub folder_open: std::collections::HashMap<u32, bool>,
    /// The folder row selected in the feature list (a click on it).
    pub selected_folder: Option<u32>,
    /// A folder just made: its row related-highlighted until the next click.
    pub fresh_folder: Option<u32>,
    /// Entity whose graphics-area context menu is open (kept highlighted).
    pub menu_target: Option<Reference>,
    /// Feature whose list context menu is open: its row and geometry stay
    /// highlighted (live menus/rmb-ps-featurelist-fillet.png).
    pub list_menu: Option<FeatureId>,
    /// Dialog picks the preview consumed (a fillet's edges), drawn from the
    /// model before the feature: (picks, accent picks, before-model, lines).
    pub consumed_lines: Option<ConsumedLines>,
    /// "Show dimensions" of a sketch.
    pub shown_dims: Option<super::shown_dims::ShownDims>,
    /// The sketch draws something with GPUI that follows the camera (set
    /// by `render_sketch_labels`: the dimension box, glyphs, counts...).
    pub sketch_follows_camera: bool,
    /// Entity of the hovered dialog selection row.
    pub query_hover: Option<Reference>,
    /// The pointer is over the manipulator arrow (drawn outlined).
    pub manip_hover: bool,
    /// The view's world length per pixel when the screen-sized geometry the
    /// Part Studio builds itself (hole callouts, triad hit areas, datum
    /// boxes) was last sized (perf: rebuilt on zoom, not on render).
    pub sized_for_wpp: f64,
    /// The sketch dimension box moved by its drag strip (reset when a box opens).
    pub dim_box_offset: Point<Pixels>,
    /// The dimension box's text doesn't evaluate (live: the field's invalid
    /// state, ✓ disabled, the box stays open; cleared by typing).
    pub dim_invalid: bool,
    /// The translate triad's hovered handle and running drag (triad.rs).
    pub triad_hover: Option<usize>,
    pub triad_drag: Option<super::triad::TriadDrag>,
    /// Parts an open Transform moves (not copies): drawn moved and
    /// display-only, with the original as a faded, selectable ghost.
    pub transform_ghost: Vec<BodyId>,
    /// Default geometry row being renamed in place.
    pub dg_renaming: Option<(Reference, Entity<InputState>, Subscription)>,
    /// Parts list › Rename: the part's inline input (live part-studio/450).
    pub part_renaming: Option<(BodyId, Entity<InputState>, Subscription)>,
    /// "Show dependencies" of a default plane or the origin.
    pub plane_dependencies: Option<Reference>,
    /// "+ Configure features": picking a feature then a parameter for this input.
    pub configure_pick: Option<String>,
    /// Configurations panel: Configured properties tab shown, input kinds
    /// shown alone (List, Checkbox, Variable; none = all), folded inputs, the cell
    /// being edited (its key) and the filter text.
    pub cfg_properties_tab: bool,
    pub cfg_only_kinds: [bool; 3],
    pub cfg_folded: std::collections::HashSet<String>,
    /// While an input is dragged by its header: the section it would land
    /// next to, and whether after it.
    pub cfg_drop: Option<(String, bool)>,
    pub cfg_editing: Option<String>,
    /// The rename input still has to take the keyboard (once, when it opens).
    pub cfg_editing_focus: bool,
    pub cfg_filter: Option<Entity<InputState>>,
    /// Feature list "Configurations" box: one select per List input.
    pub cfg_selects: std::collections::HashMap<String, (Entity<crate::os_select::OsSelect>, Subscription)>,
    /// Open analysis tool dialog (Curve/surface, Dihedral…) and what each
    /// tool had selected last time.
    pub analysis_tool: Option<super::analysis_tools::AnalysisTool>,
    pub tool_memory: super::analysis_tools::ToolMemory,
    /// Zebra stripes left on after their dialog was accepted.
    pub zebra: Option<super::analysis_tools::ZebraSettings>,
    /// Reflection analysis left on after its dialog was accepted, its
    /// uploaded environment and the open "Select image" dialog.
    pub reflection: Option<super::reflection::ReflectionSettings>,
    pub env_gpu: Option<(String, noshape_render::ImageHandle)>,
    pub env_picker: Option<super::reflection::EnvPicker>,
    /// Thickness analysis (stays on after its dialog is accepted).
    pub thickness: Option<super::thickness::ThicknessState>,
    /// Pick kinds last given to the viewport (`Some(None)`: all).
    pub pick_filter: Option<Option<Vec<PickKind>>>,
    /// The open Flatten surfaces dialog.
    pub flatten: Option<super::flatten_tool::FlattenState>,
    /// Interference detection: the hovered row of its Interferences list.
    pub interference_hover: Option<usize>,
    /// The clicked Interferences row (bold; the view zoomed to it).
    pub interference_picked: Option<usize>,
    /// Curvature color map settings (while it is on).
    pub curvature_map: Option<super::thickness::CurvatureMap>,
    /// The open Draft analysis dialog.
    pub draft: Option<super::draft_tool::DraftState>,
}

impl UiState {
    pub fn new(cx: &mut App) -> Self {
        Self {
            in_context: None,
            context_rename: None,
            context_arrow_hover: None,
            isolate: None,
            derived_open: Default::default(),
            dialog_info: None,
            cam_history: Vec::new(),
            pointer: None,
            hover_pick: None,
            select_other: None,
            mini_toolbar: None,
            camera_menu: None,
            filter: None,
            filter_query: String::new(),
            list_hover: None,
            anchor: None,
            last_click: None,
            list_focus: cx.focus_handle(),
            list_scroll: ScrollHandle::new(),
            list_autoscroll: 0.0,
            list_autoscrolling: false,
            rollback_keyboard: None,
            dependencies: None,
            cube_hover: None,
            cube_origin: None,
            measure_pinned: false,
            toolbar_faces: Default::default(),
            palette_sel: 0,
            appearance_sketches: false,
            graphics_dialog: None,
            palette_no_highlight: false,
            palette_top: std::rc::Rc::new(std::cell::Cell::new(px(76.))),
            palette_closed: None,
            var_hover: None,
            var_folded: false,
            var_focus_new: false,
            cp_part: None,
            cp_selected: None,
            cp_focus: None,
            cp_grid: Vec::new(),
            var_input: None,
            datum_edit: None,
            annotation_selected: None,
            annotation_drag: None,
            var_drop: None,
            inspection_tool: None,
            inspection_more: false,
            insp: Default::default(),
            ct: Default::default(),
            var_tables: Default::default(),
            section: Vec::new(),
            section_dialog: None,
            section_offset_edit: None,
            section_rot: None,
            section_base: None,
            section_base_drag: false,
            section_hover: None,
            section_parts: None,
            appearance_edit: None,
            appearance_hover: Vec::new(),
            analysis: None,
            vcolor_cache: None,
            high_quality: false,
            high_quality_seen: false,
            create_selection: None,
            material_panel: None,
            panels: Default::default(),
            cfg_box_h: None,
            menu_pos: Point::default(),
            regen_paused: None,
            force_regen: false,
            show_regen_times: false,
            // Live (part-studio/1158): slowest first.
            regen_sort: (2, true),
            // Live part-studio/470: "Composite parts (N)" starts collapsed.
            collapsed_sections: HashSet::from(["composites"]),
            parts_height: None,
            // Preferences › Part Studio settings › Display parts list properties.
            show_part_props: crate::preferences_window::parts_list_properties(),
            list_collapsed: false,
            mass_faces: false,
            measure_type: None,
            measure_length_unit: None,
            measure_angle_unit: None,
            dialog_pos: None,
            dialog_width: None,
            toolbar: super::toolbar_prefs::ToolbarPrefs::load(),
            tb_custom: None,
            iso_slider_drag: None,
            message_timed: None,
            preview_opacity: 1.0,
            preview_before: None,
            before_context: None,
            std_edit_old: None,
            std_edit_task: None,
            preview_task: None,
            preview_tool: None,
            feature_drop: Default::default(),
            hole_prev: None,
            preview_tool_task: None,
            measure_lines: None,
            measure_hover: None,
            sm_context: None,
            sm_bends_closed: false,
            sm_joints_closed: false,
            sm_bends_scroll: ScrollHandle::new(),
            sm_hover: None,
            sm_edit: None,
            sm_edit_focus: false,
            sm_cell: None,
            dyn_suppression: None,
            shortcut_filter: None,
            shortcut_query: String::new(),
            shortcut_tab: 0,
            shortcut_scroll: gpui_kit::ScrollHandle::new(),
            recent_tools: Vec::new(),
            orient_on_edit: false,
            orient_pending: false,
            oriented: false,
            remembered: Default::default(),
            default_geometry_collapsed: false,
            selected_folder: None,
            fresh_folder: None,
            folder_open: Default::default(),
            menu_target: None,
            list_menu: None,
            consumed_lines: None,
            shown_dims: None,
            sketch_follows_camera: false,
            query_hover: None,
            manip_hover: false,
            sized_for_wpp: 0.0,
            dim_box_offset: Point::default(),
            dim_invalid: false,
            triad_hover: None,
            triad_drag: None,
            transform_ghost: Vec::new(),
            dg_renaming: None,
            part_renaming: None,
            plane_dependencies: None,
            configure_pick: None,
            cfg_properties_tab: false,
            cfg_only_kinds: [false; 3],
            cfg_folded: Default::default(),
            cfg_drop: None,
            cfg_editing: None,
            cfg_editing_focus: false,
            cfg_filter: None,
            cfg_selects: Default::default(),
            zebra: None,
            reflection: None,
            env_gpu: None,
            env_picker: None,
            thickness: None,
            pick_filter: None,
            flatten: None,
            interference_hover: None,
            interference_picked: None,
            curvature_map: None,
            draft: None,
            analysis_tool: None,
            tool_memory: Default::default(),
        }
    }
}

/// The info toast while a Remove / Intersect tool misses the parts (live 234).
pub(super) const NO_INTERSECTION: &str = "Selected tools and targets do not intersect.";

/// Marks a Transform ghost's body id (scene only).
const GHOST_INDEX_BIT: u32 = 1 << 31;

impl PartStudioView {
    // ------------------------------------------------------------ helpers

    /// Body owning a reference (faces, edges, vertices, the body itself).
    pub(super) fn body_of(&self, r: &Reference) -> Option<BodyId> {
        noshape_ops::studio::body_of(&self.display, r)
    }

    /// Entities that light up when a feature is hovered or selected in the
    /// list: its sketch, plane, mate connectors and the faces it created.
    pub(super) fn feature_geometry(&self, id: FeatureId) -> Vec<Reference> {
        let mut out = Vec::new();
        if let Some(sk) = self.display.sketches.iter().find(|s| s.feature == id) {
            out.extend(sk.curves.iter().map(|c| Reference::SketchEntity { sketch: id, entity: c.entity }));
            out.extend(sk.points.iter().map(|c| Reference::SketchEntity { sketch: id, entity: c.entity }));
        }
        out.extend(self.display.planes.iter().filter(|p| p.reference == Reference::FeaturePlane(id)).map(|p| p.reference.clone()));
        out.extend(self.display.connectors.iter().filter(|c| c.reference.depends_on() == Some(id)).map(|c| c.reference.clone()));
        for b in self.display.bodies.iter().filter(|b| !b.hidden) {
            out.extend(b.face_refs.iter().filter(|r| r.depends_on() == Some(id)).cloned());
        }
        out
    }

    /// Faces standing for a whole part (parts list hover/selection).
    pub(super) fn body_geometry(&self, id: &BodyId) -> Vec<Reference> {
        self.display.body(id).map(|b| b.face_refs.clone()).unwrap_or_default()
    }

    /// Selection plus the geometry of selected features and parts (for the
    /// graphics-area highlight).
    pub(super) fn highlighted_selection(&self) -> Vec<Reference> {
        let mut out = Vec::new();
        for r in &self.selection {
            match r {
                Reference::Body(b) => out.extend(self.body_geometry(b)),
                Reference::Sketch(f) | Reference::Feature(f) => out.extend(self.feature_geometry(*f)),
                _ => out.push(r.clone()),
            }
        }
        for f in &self.selected_features {
            out.extend(self.feature_geometry(*f));
        }
        // An analysis tool's active field highlights its entities (Onshape).
        if let Some(s) = &self.ui.flatten {
            match s.active {
                0 => out.extend(s.faces.iter().cloned()),
                1 => out.extend(s.rips.iter().cloned()),
                2 => out.extend(s.origin.iter().cloned()),
                _ => out.extend(s.position.iter().cloned()),
            }
        } else if let Some(t) = &self.ui.analysis_tool {
            out.extend(if t.second_active { &t.selection2 } else { &t.selection }.iter().cloned());
        }
        out
    }

    /// Whether the active dialog field takes `t`: its kind, the region a
    /// curve bounds or the part a face is on (None: no field is active).
    pub(super) fn field_takes(&self, t: &PickTarget, cx: &App) -> Option<bool> {
        let (_, filter, _) = self.edit.as_ref()?.dialog.read(cx).active_filter()?;
        Some(kind_matches(t.kind, &t.reference, filter) || self.curve_region(&t.reference, filter).is_some() || self.part_for_field(&t.reference, filter).is_some())
    }

    /// Hover target in the graphics area, expanded like the selection.
    pub(super) fn highlighted_hover(&self, cx: &App) -> Vec<Reference> {
        if let Some(f) = self.ui.list_hover.or(self.ui.list_menu).or(self.ui.insp.hover_feature) {
            return self.feature_geometry(f);
        }
        if self.ui.var_hover.is_some() {
            return self.variable_users(cx).into_iter().flat_map(|f| self.feature_geometry(f)).collect();
        }
        // A custom table row pre-highlights its entities; a part by its
        // edges only, faces keep their color (live 635).
        if let Some(refs) = &self.ui.ct.hover {
            return refs.iter().flat_map(|r| match r {
                Reference::Body(b) => self.display.body(b).map(|b| b.edge_refs.clone()).unwrap_or_default(),
                r => vec![r.clone()],
            }).collect();
        }
        if let Some(r) = &self.ui.menu_target {
            return vec![r.clone()];
        }
        // Select items to insert: a face pre-highlights its part, a curve its sketch.
        if let Some(item) = self.hover.as_ref().filter(|_| self.insert_go_open()).and_then(|r| self.insert_go_item(r)) {
            return match item {
                Reference::Body(b) => self.body_geometry(&b),
                Reference::Sketch(f) => self.feature_geometry(f),
                _ => Vec::new(),
            };
        }
        match &self.hover {
            Some(Reference::Body(b)) => self.body_geometry(b),
            // A sketch curve under a region field pre-highlights its region.
            Some(r @ Reference::SketchEntity { .. }) => {
                let filter = self.edit.as_ref().and_then(|e| e.dialog.read(cx).active_filter()).map(|(_, f, _)| f);
                vec![filter.and_then(|f| self.curve_region(r, f)).unwrap_or_else(|| r.clone())]
            }
            // A face under a parts field pre-highlights the whole part.
            Some(r @ (Reference::Face(_) | Reference::Edge(_) | Reference::Vertex(_))) => {
                let filter = self.edit.as_ref().and_then(|e| e.dialog.read(cx).active_filter()).map(|(_, f, _)| f);
                let kind = match r {
                    Reference::Face(_) => PickKind::Face,
                    Reference::Edge(_) => PickKind::Edge,
                    _ => PickKind::Vertex,
                };
                let wants_kind = filter.is_some_and(|f| kind_matches(kind, r, f));
                match filter.filter(|_| !wants_kind).and_then(|f| self.part_for_field(r, f)) {
                    Some(Reference::Body(b)) => self.body_geometry(&b),
                    _ => vec![r.clone()],
                }
            }
            Some(r) => vec![r.clone()],
            None => Vec::new(),
        }
    }

    /// Features related to the current selection (Onshape cross-highlight:
    /// selecting a face or part highlights the features that created it).
    /// Features using the variable hovered in the variable table.
    fn variable_users(&self, cx: &App) -> HashSet<FeatureId> {
        let (Some(name), Some(studio)) = (self.ui.var_hover.as_ref(), self.studio(cx)) else { return HashSet::new() };
        interaction::filter_features(studio, &self.display, &format!(":variable {name}")).unwrap_or_default()
    }

    pub(super) fn related_features(&self, cx: &App) -> HashSet<FeatureId> {
        let mut out = self.variable_users(cx);
        // A hovered Characteristics row lights up its feature (live tl-99a).
        out.extend(self.ui.insp.hover_feature);
        // In a feature dialog: what its fields picked (live part-studio/440:
        // Part 1 in Transform's field lights up Extrude 1).
        let picked = self.edit.as_ref().map(|e| e.dialog.read(cx).selected_refs());
        for r in picked.as_ref().unwrap_or(&self.selection) {
            match r {
                Reference::Body(b) => {
                    out.insert(b.feature);
                    if let Some(bd) = self.display.body(b) {
                        out.extend(bd.face_refs.iter().filter_map(|f| f.depends_on()));
                    }
                }
                Reference::Edge(n) | Reference::Vertex(n) => {
                    if let Some(f) = super::edge_creator(n) {
                        out.insert(f);
                    }
                }
                r => {
                    if let Some(f) = r.depends_on() {
                        out.insert(f);
                    }
                }
            }
        }
        out
    }

    // ------------------------------------------------------------ picking

    /// One pick from the graphics area or a list: goes to the active dialog
    /// field (with filter feedback) or toggles the selection.
    pub(super) fn pick(&mut self, target: PickTarget, cx: &mut Context<Self>) {
        if self.insert_go_pick(&target.reference, cx) {
            return;
        }
        if let Some(e) = &self.edit {
            let dialog = e.dialog.clone();
            let Some((_, filter, _)) = dialog.read(cx).active_filter() else {
                self.flash_info("Click a selection field in the dialog first.".into(), cx);
                return;
            };
            if !kind_matches(target.kind, &target.reference, filter) {
                // Clicking a sketch curve while a region field is active
                // takes the region it bounds; a face, edge or vertex in a
                // parts field takes its part (Onshape).
                if let Some(region) = self.curve_region(&target.reference, filter).or_else(|| self.part_for_field(&target.reference, filter)) {
                    dialog.update(cx, |d, cx| {
                        d.add_pick(region, cx);
                    });
                    self.refresh_scene(false, cx);
                    cx.notify();
                    return;
                }
                // Live ("qa4 round 5 s1 follow-up 2" (1)): an entity the
                // field doesn't take is neither highlighted nor picked, and
                // nothing tells you so.
                return;
            }
            dialog.update(cx, |d, cx| {
                d.add_pick(target.reference.clone(), cx);
            });
        } else if let Some(pos) = self.selection.iter().position(|r| *r == target.reference) {
            self.selection.remove(pos);
        } else {
            self.selection.push(target.reference);
        }
        self.refresh_scene(false, cx);
        cx.notify();
    }

    /// The sketch region a curve bounds, when `filter` takes regions but not
    /// curves (Onshape: hovering/clicking the curve picks the region).
    pub(super) fn curve_region(&self, r: &Reference, filter: QueryFilter) -> Option<Reference> {
        let Reference::SketchEntity { sketch, entity } = r else { return None };
        if !filter.intersects(QueryFilter::SKETCH_REGION) || filter.intersects(QueryFilter::ANY_EDGE) || filter.intersects(QueryFilter::AXIS) {
            return None;
        }
        let sk = self.display.sketches.iter().find(|s| s.feature == *sketch)?;
        interaction::region_for_curve(&sk.regions, *entity).map(|reg| Reference::SketchRegion { sketch: *sketch, key: reg.key.clone() })
    }

    /// The part of a face/edge/vertex when `filter` takes parts but not that kind.
    /// The active dialog field's selection filter.
    pub(super) fn active_field_filter(&self, cx: &App) -> Option<QueryFilter> {
        self.edit.as_ref().and_then(|e| e.dialog.read(cx).active_filter().map(|(_, f, _)| f))
    }

    /// Which pick kinds the graphics area offers: what the active dialog
    /// field or analysis tool can take (Onshape: only acceptable entities
    /// highlight and get picked), `None` for everything.
    pub(super) fn pick_kinds(&self, cx: &App) -> Option<Vec<PickKind>> {
        const ALL: [PickKind; 9] = [PickKind::Face, PickKind::Edge, PickKind::Vertex, PickKind::Plane, PickKind::Origin, PickKind::SketchCurve, PickKind::SketchPoint, PickKind::SketchRegion, PickKind::Connector];
        if self.sketch.is_some() {
            return None;
        }
        if let Some(f) = self.active_field_filter(cx) {
            let dummy = Reference::Origin;
            return Some(
                ALL.into_iter()
                    .filter(|k| {
                        kind_matches(*k, &dummy, f)
                            // A curve picks the region it bounds; part geometry picks its part.
                            || (*k == PickKind::SketchCurve && f.intersects(QueryFilter::SKETCH_REGION))
                            || (matches!(k, PickKind::Face | PickKind::Edge | PickKind::Vertex) && f.intersects(QueryFilter::BODY))
                    })
                    .collect(),
            );
        }
        let t = self.ui.analysis_tool.as_ref().filter(|_| self.edit.is_none())?;
        use super::analysis_tools::ToolKind as T;
        Some(match t.kind {
            T::CurveSurface | T::Connection => vec![PickKind::Edge, PickKind::Face, PickKind::SketchCurve],
            T::Deviation if t.options.surface => vec![PickKind::Face],
            T::Deviation => vec![PickKind::Edge, PickKind::SketchCurve],
            T::Dihedral => vec![PickKind::Edge],
            T::Thickness | T::Interference => vec![PickKind::Face, PickKind::Edge, PickKind::Vertex],
            T::Draft => match self.ui.draft.as_ref().map_or(0, |d| d.active) {
                0 => vec![PickKind::Face, PickKind::Plane, PickKind::Edge],
                _ => vec![PickKind::Face, PickKind::Edge, PickKind::Vertex],
            },
            T::Flatten => match self.ui.flatten.as_ref().map_or(0, |s| s.active) {
                0 => vec![PickKind::Face],
                1 => vec![PickKind::Edge],
                2 => vec![PickKind::Vertex],
                _ => vec![PickKind::Connector],
            },
            T::Zebra | T::Reflection => return None,
        })
    }

    /// Give the viewport the pick priorities for the current filter (only
    /// when it changes).
    pub(super) fn sync_pick_filter(&mut self, cx: &mut Context<Self>) {
        let kinds = self.pick_kinds(cx);
        if self.ui.pick_filter.as_ref() == Some(&kinds) {
            return;
        }
        let Some(built) = &self.built else { return };
        let prio = match &kinds {
            Some(k) => {
                let k = k.clone();
                built.picks.priority_fn_filtered(move |kind| k.contains(&kind))
            }
            None => built.picks.priority_fn(),
        };
        self.ui.pick_filter = Some(kinds);
        self.viewport.update(cx, |vp, _| vp.pick_priority = prio);
    }

    pub(super) fn part_for_field(&self, r: &Reference, filter: QueryFilter) -> Option<Reference> {
        if !filter.intersects(QueryFilter::BODY) || !matches!(r, Reference::Face(_) | Reference::Edge(_) | Reference::Vertex(_)) {
            return None;
        }
        self.body_of(r).map(Reference::Body)
    }

    /// Pick target for a reference shown in a list (default geometry, parts).
    pub(super) fn list_target(&self, r: &Reference) -> PickTarget {
        let kind = match r {
            Reference::Origin => PickKind::Origin,
            Reference::Plane(_) | Reference::FeaturePlane(_) => PickKind::Plane,
            _ => PickKind::Face,
        };
        PickTarget { reference: r.clone(), kind, anchor: None }
    }

    /// Parts list click: dialog field (parts only) or additive selection.
    pub(super) fn pick_body(&mut self, b: BodyId, cx: &mut Context<Self>) {
        let r = Reference::Body(b);
        if self.insert_go_pick(&r, cx) {
            return;
        }
        // An open analysis tool takes parts-list picks too (live analysis/81:
        // "Picking parts (from the Parts list or the viewport) adds chips").
        if self.edit.is_none() && self.ui.analysis_tool.is_some() && self.analysis_pick(&r, cx) {
            return;
        }
        if let Some(e) = &self.edit {
            let dialog = e.dialog.clone();
            let Some((_, filter, _)) = dialog.read(cx).active_filter() else { return };
            // Live: a pick the field doesn't take does nothing.
            if !filter.intersects(QueryFilter::BODY) {
                return;
            }
            dialog.update(cx, |d, cx| {
                d.add_pick(r, cx);
            });
        } else if let Some(pos) = self.selection.iter().position(|x| *x == r) {
            self.selection.remove(pos);
        } else {
            self.selection.push(r);
        }
        self.refresh_scene(false, cx);
        cx.notify();
    }

    /// Box selection result: into the active dialog field (filtered) or
    /// the selection; Ctrl+drag deselects (Onshape).
    pub(super) fn box_select(&mut self, ids: &[u32], deselect: bool, cx: &mut Context<Self>) {
        let Some(built) = &self.built else { return };
        let targets: Vec<PickTarget> = ids.iter().filter_map(|id| built.picks.get(*id).cloned()).filter(|t| !matches!(t.kind, PickKind::Plane | PickKind::Origin)).collect();
        if let Some(e) = &self.edit {
            let dialog = e.dialog.clone();
            if let Some((_, filter, _)) = dialog.read(cx).active_filter() {
                let current = dialog.read(cx).selected_refs();
                dialog.update(cx, |d, cx| {
                    for t in targets.iter().filter(|t| kind_matches(t.kind, &t.reference, filter)) {
                        if current.contains(&t.reference) == deselect {
                            d.add_pick(t.reference.clone(), cx);
                        }
                    }
                });
            }
        } else {
            for t in targets.iter().filter(|t| !matches!(t.kind, PickKind::Vertex | PickKind::SketchRegion)) {
                let pos = self.selection.iter().position(|r| *r == t.reference);
                match (pos, deselect) {
                    (Some(p), true) => {
                        self.selection.remove(p);
                    }
                    (None, false) => self.selection.push(t.reference.clone()),
                    _ => {}
                }
            }
        }
        self.refresh_scene(false, cx);
        cx.notify();
    }

    /// Double-click on a sketch in the graphics area opens it for editing.
    pub(super) fn is_double_click(&mut self, r: &Reference) -> bool {
        let now = Instant::now();
        let dbl = self.ui.last_click.as_ref().is_some_and(|(prev, t)| prev == r && now.duration_since(*t) < Duration::from_millis(450));
        self.ui.last_click = if dbl { None } else { Some((r.clone(), now)) };
        dbl
    }

    // ------------------------------------------------------------ preselection

    /// References standing for the selected features, for preselection:
    /// a selected sketch preselects its regions (or the sketch, or its
    /// curves) and other features themselves.
    pub(super) fn feature_preselection(&self, fields: &[QueryFilter]) -> Vec<Reference> {
        let wants = |q: QueryFilter| fields.iter().any(|f| f.intersects(q));
        let mut out = Vec::new();
        for &id in &self.selected_features {
            if let Some(sk) = self.display.sketches.iter().find(|s| s.feature == id) {
                if wants(QueryFilter::SKETCH_REGION) && !sk.regions.is_empty() {
                    out.extend(sk.regions.iter().map(|r| Reference::SketchRegion { sketch: id, key: r.key.clone() }));
                } else if wants(QueryFilter::SKETCH) {
                    out.push(Reference::Sketch(id));
                } else if wants(QueryFilter::SKETCH_EDGE) {
                    out.extend(sk.curves.iter().map(|c| Reference::SketchEntity { sketch: id, entity: c.entity }));
                } else if wants(QueryFilter::SKETCH_POINT) {
                    out.extend(super::sketch_points(sk));
                }
            } else if let Some(pl) = self.display.planes.iter().find(|p| p.reference == Reference::FeaturePlane(id)) {
                out.push(pl.reference.clone());
            } else if wants(QueryFilter::FEATURE) {
                out.push(Reference::Feature(id));
            }
        }
        out
    }

    // ------------------------------------------------------------ sketches

    /// Context menu "Copy sketch" (pasted onto a plane with "Paste sketch").
    pub(super) fn copy_sketch(&mut self, id: FeatureId, cx: &mut Context<Self>) {
        let prev = std::mem::replace(&mut self.selected_features, vec![id]);
        self.copy_selected_features(cx);
        self.selected_features = prev;
    }

    /// Sketch curve menu "Copy sketch entities": the selected curves of the
    /// sketch (with the clicked one) to the clipboard, the way sketch mode
    /// copies them, ready to paste into a sketch.
    pub(super) fn copy_sketch_entities(&mut self, sketch: FeatureId, clicked: u32, cx: &mut Context<Self>) {
        let mut ids: Vec<noshape_sketch::EntityId> = self
            .selection
            .iter()
            .filter_map(|r| match r {
                Reference::SketchEntity { sketch: s, entity } if *s == sketch => Some(noshape_sketch::EntityId(*entity)),
                _ => None,
            })
            .collect();
        if !ids.contains(&noshape_sketch::EntityId(clicked)) {
            ids.push(noshape_sketch::EntityId(clicked));
        }
        let Some(f) = self.studio(cx).and_then(|s| s.feature(sketch)) else { return };
        let Some(ParamValue::Sketch(data)) = f.params.get("sketch") else { return };
        let clip = data.sketch.extract(&ids);
        if let Ok(text) = serde_json::to_string(&serde_json::json!({ "noshape_sketch_entities": clip })) {
            cx.write_to_clipboard(ClipboardItem::new_string(text));
        }
    }

    /// Parts list "Copy here…": Transform preset to copy the parts, translated by XYZ.
    pub(super) fn copy_here(&mut self, bodies: Vec<BodyId>, window: &mut Window, cx: &mut Context<Self>) {
        self.selection = bodies.into_iter().map(Reference::Body).collect();
        self.start_feature("transform", window, cx);
        if let Some(e) = &self.edit {
            e.dialog.update(cx, |d, cx| {
                d.set_value("transformType", ParamValue::Enum("TRANSLATION_3D".into()), cx);
                d.set_value("makeCopy", ParamValue::Bool(true), cx);
            });
        }
    }

    /// Name of default geometry (renamed or Origin / Top / Front / Right).
    pub(super) fn default_name(&self, r: &Reference, cx: &App) -> String {
        if let Some(n) = self.studio(cx).and_then(|s| s.default_names.iter().find(|(x, _)| x == r).map(|(_, n)| n.clone())) {
            return n;
        }
        match r {
            Reference::Plane(noshape_model::reference::DatumPlane::Top) => "Top",
            Reference::Plane(noshape_model::reference::DatumPlane::Front) => "Front",
            Reference::Plane(noshape_model::reference::DatumPlane::Right) => "Right",
            _ => "Origin",
        }
        .to_string()
    }

    /// Rename on a default geometry row: an input in place (Enter or
    /// leaving it commits, Esc cancels).
    pub(super) fn start_default_rename(&mut self, r: Reference, window: &mut Window, cx: &mut Context<Self>) {
        let name = self.default_name(&r, cx);
        let st = cx.new(|cx| InputState::new(window, cx).default_value(name));
        let r2 = r.clone();
        let sub = cx.subscribe_in(&st, window, move |this: &mut Self, st, ev: &InputEvent, _, cx| match ev {
            InputEvent::PressEnter { .. } | InputEvent::Blur => {
                let text = st.read(cx).value().trim().to_string();
                this.ui.dg_renaming = None;
                if !text.is_empty() {
                    let r = r2.clone();
                    this.doc.update(cx, |doc, cx| {
                        doc.edit_studio(this.tab, &format!("Rename : {text}"), cx, |ps| {
                            ps.default_names.retain(|(x, _)| *x != r);
                            ps.default_names.push((r.clone(), text.clone()));
                        });
                    });
                }
                cx.notify();
            }
            _ => {}
        });
        st.update(cx, |s, cx| {
            s.focus(window, cx);
            s.select_all(window, cx);
        });
        self.ui.dg_renaming = Some((r, st, sub));
        cx.notify();
    }

    /// Parts list › Rename: an input in the row with the name selected
    /// (live dom/partslist-rename.txt); Enter or leaving it commits, Esc
    /// cancels.
    pub(super) fn start_part_rename(&mut self, body: BodyId, window: &mut Window, cx: &mut Context<Self>) {
        let name = self.display.body(&body).map(|b| b.name.clone()).unwrap_or_default();
        let st = cx.new(|cx| InputState::new(window, cx).default_value(name));
        let b2 = body.clone();
        let sub = cx.subscribe_in(&st, window, move |this: &mut Self, st, ev: &InputEvent, _, cx| match ev {
            InputEvent::PressEnter { .. } | InputEvent::Blur => {
                if this.ui.part_renaming.take().is_none() {
                    return;
                }
                let text = st.read(cx).value().trim().to_string();
                if !text.is_empty() {
                    let b = b2.clone();
                    this.doc.update(cx, |doc, cx| {
                        doc.edit_studio(this.tab, &noshape_ops::studio::rename_part_description(&text), cx, |ps| noshape_ops::studio::rename_part(ps, &b, &text));
                    });
                }
                cx.notify();
            }
            _ => {}
        });
        st.update(cx, |s, cx| {
            s.focus(window, cx);
            s.select_all(window, cx);
        });
        self.ui.part_renaming = Some((body, st, sub));
        cx.notify();
    }

    /// Plane menu "Size to Part Studio": the plane grows to frame the parts.
    pub(super) fn size_plane_to_studio(&mut self, r: Reference, cx: &mut Context<Self>) {
        let Some((hw, hh)) = noshape_ops::studio::plane_size_to_studio(&self.display, &r) else { return };
        self.doc.update(cx, |doc, cx| {
            doc.edit_studio(self.tab, noshape_ops::studio::SIZE_PLANE_DESCRIPTION, cx, |ps| noshape_ops::studio::set_plane_size(ps, &r, hw, hh));
        });
    }

    /// Whether the clipboard holds a sketch from "Copy sketch" (the menus
    /// then offer "Paste sketch").
    pub(super) fn clipboard_has_sketch(cx: &App) -> bool {
        cx.read_from_clipboard().and_then(|c| c.text()).is_some_and(|t| t.contains("\"noshape_features\"") && t.contains("\"type_id\":\"sketch\""))
    }

    /// Plane menus: "Hide other planes" (keep one) / "Hide all planes".
    pub(super) fn hide_planes(&mut self, keep: Option<&Reference>, cx: &mut Context<Self>) {
        let planes: Vec<Reference> = self.display.planes.iter().map(|p| p.reference.clone()).filter(|r| Some(r) != keep).collect();
        self.set_visibility(&planes, true, cx);
    }

    /// Sketch menus "Hide other sketches" (keep one) / "Hide all sketches".
    pub(super) fn hide_sketches(&mut self, keep: Option<FeatureId>, cx: &mut Context<Self>) {
        let sketches: Vec<Reference> = self.display.sketches.iter().filter(|s| !s.hidden && Some(s.feature) != keep).map(|s| Reference::Feature(s.feature)).collect();
        self.set_visibility(&sketches, true, cx);
    }

    /// "Paste sketch" on a plane or planar face: paste the copied sketch(es)
    /// and move them onto that plane.
    pub(super) fn paste_sketch_onto(&mut self, plane: Reference, cx: &mut Context<Self>) {
        let Some(text) = cx.read_from_clipboard().and_then(|c| c.text()) else { return };
        let Ok(mut v) = serde_json::from_str::<serde_json::Value>(&text) else { return };
        let Some(Ok(features)) = v.get_mut("noshape_features").map(|f| serde_json::from_value::<Vec<noshape_model::partstudio::Feature>>(f.take())) else { return };
        let sketches: Vec<_> = features.into_iter().filter(|f| f.type_id == "sketch").collect();
        if sketches.is_empty() {
            self.flash_info("Copy a sketch first (right-click a sketch > Copy sketch).".into(), cx);
            return;
        }
        self.doc.update(cx, |doc, cx| {
            doc.edit_studio(self.tab, "Paste sketch", cx, |ps| {
                noshape_ops::studio::paste_sketches_onto(ps, &sketches, plane);
            });
        });
    }

    /// "Export as DXF/DWG" for a sketch (DXF, millimeters, sketch plane
    /// coordinates; construction curves as hidden lines).
    pub(super) fn export_sketch(&mut self, id: FeatureId, ext: &'static str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(sk) = self.display.sketches.iter().find(|s| s.feature == id) else { return };
        let g = noshape_ops::sketch_export::sketch_sheet(sk);
        let name = sk.name.clone();
        let view = cx.entity();
        crate::files::request_save_path("Export sketch", format!("{name}.{ext}"), ext, window, cx, move |path, cx| {
            let data = if ext == "svg" { noshape_model::drawing::export::to_svg(&g) } else { noshape_model::drawing::export::to_dxf(&g) };
            let msg = match std::fs::write(&path, data) {
                Ok(()) => format!("Exported {}", path.display()),
                Err(e) => format!("Export failed: {e}"),
            };
            view.update(cx, |this, cx| this.flash_info(msg, cx));
        });
    }

    /// Face menu "Export as DXF/DWG…": the planar face's boundary in its plane.
    pub(super) fn export_face(&mut self, r: Reference, window: &mut Window, cx: &mut Context<Self>) {
        let Some(ctx) = self.context.as_ref() else { return };
        let Some(g) = noshape_ops::sketch_export::face_sheet(ctx, &self.display, &r) else { return };
        let Some(name) = self.display.bodies.iter().find(|b| b.face_refs.contains(&r)).map(|b| b.name.clone()) else { return };
        let view = cx.entity();
        crate::files::request_save_path("Export face", format!("{name} face.dxf"), "dxf", window, cx, move |path, cx| {
            let msg = match std::fs::write(&path, noshape_model::drawing::export::to_dxf(&g)) {
                Ok(()) => format!("Exported {}", path.display()),
                Err(e) => format!("Export failed: {e}"),
            };
            view.update(cx, |this, cx| this.flash_info(msg, cx));
        });
    }

    /// "Copy Part 1": the part goes to the clipboard; Ctrl+V in another
    /// Part Studio derives it (in this one, it copies it in place).
    pub(super) fn copy_part(&mut self, b: BodyId, cx: &mut Context<Self>) {
        let v = serde_json::json!({ "noshape_part": { "tab": self.tab.to_string(), "body": b } });
        cx.write_to_clipboard(ClipboardItem::new_string(v.to_string()));
        let name = self.display.body(&b).map(|d| d.name.clone()).unwrap_or_else(|| "part".into());
        self.flash_info(format!("Copied {name}"), cx);
    }

    /// Ctrl+V of a copied part: returns false when the clipboard holds none.
    pub(super) fn paste_part(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let Some(text) = cx.read_from_clipboard().and_then(|c| c.text()) else { return false };
        let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else { return false };
        let Some(p) = v.get("noshape_part") else { return false };
        let (Some(tab), Some(Ok(body))) = (p.get("tab").and_then(|t| t.as_str()).map(str::to_string), p.get("body").map(|b| serde_json::from_value::<BodyId>(b.clone()))) else { return false };
        if tab == self.tab.to_string() {
            self.copy_here(vec![body], window, cx);
            return true;
        }
        self.start_feature("importDerived", window, cx);
        if let Some(e) = &self.edit {
            e.dialog.update(cx, |d, cx| {
                d.set_value("partStudio", ParamValue::Text(tab.clone()), cx);
                d.set_value("parts", ParamValue::Query(vec![Reference::Body(body.clone())]), cx);
            });
        }
        true
    }

    // ------------------------------------------------------------ dynamic suppression

    /// "Unsuppress" on a feature with an expression: drop it and leave the
    /// feature active (one undo step).
    pub(super) fn clear_dynamic_suppression(&mut self, id: FeatureId, cx: &mut Context<Self>) {
        self.doc.update(cx, |doc, cx| {
            doc.edit_studio(self.tab, noshape_ops::studio::suppress_description(false), cx, |ps| noshape_ops::studio::set_suppressed(ps, &[id], false));
        });
    }

    /// Open Onshape's Dynamic suppression dialog: one "Unsuppressed if"
    /// expression (the feature is active while it is true; live
    /// dom/dialog-dynamic-suppression.txt).
    pub(super) fn edit_dynamic_suppression(&mut self, id: FeatureId, window: &mut Window, cx: &mut Context<Self>) {
        self.open_dynamic_suppression(DynTarget::Feature(id), window, cx);
    }

    /// The same on a folder: Onshape writes the expression onto every
    /// feature inside it (live ui 10-25 c); the field starts empty.
    pub(super) fn edit_folder_dynamic_suppression(&mut self, folder: u32, window: &mut Window, cx: &mut Context<Self>) {
        self.open_dynamic_suppression(DynTarget::Folder(folder), window, cx);
    }

    fn open_dynamic_suppression(&mut self, target: DynTarget, window: &mut Window, cx: &mut Context<Self>) {
        let current = self
            .studio(cx)
            .and_then(|s| match target {
                DynTarget::Feature(id) => s.feature(id).and_then(|f| f.suppression_expr.clone()),
                DynTarget::Folder(_) => None,
            })
            .unwrap_or_default();
        let st = cx.new(|cx| InputState::new(window, cx).default_value(current));
        let sub = cx.subscribe_in(&st, window, move |this: &mut Self, _, ev: &InputEvent, window, cx| {
            if let InputEvent::PressEnter { .. } = ev {
                this.accept_dynamic_suppression(window, cx);
            }
        });
        st.update(cx, |s, cx| s.focus(window, cx));
        self.ui.dyn_suppression = Some((target, st, sub));
        cx.notify();
    }

    fn accept_dynamic_suppression(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some((target, st, _)) = self.ui.dyn_suppression.take() else { return };
        let text = st.read(cx).value().to_string();
        // An empty field leaves the feature as it was (live: nothing
        // changes until an expression is committed).
        if !text.trim().is_empty() {
            self.doc.update(cx, |doc, cx| {
                doc.edit_studio(self.tab, noshape_ops::studio::suppression_expr_description(Some(&text)), cx, |ps| match target {
                    DynTarget::Feature(id) => noshape_ops::studio::set_suppression_expr(ps, id, Some(text.clone())),
                    DynTarget::Folder(fd) => ps.set_folder_suppression_expr(fd, Some(text.clone())),
                });
            });
        }
        window.focus(&self.focus, cx);
        cx.notify();
    }

    /// The 220 px feature-style dialog at the feature dialog's place:
    /// "Dynamic suppression" with ✓ ✕, the "Unsuppressed if" row, `?`.
    pub(super) fn render_dynamic_suppression(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let (_, st, _) = self.ui.dyn_suppression.as_ref()?;
        let pal = p();
        let field = gpui_kit::component::input::Input::new(st).appearance(false).text_size(px(13.33)).h(px(25.));
        let help = div().size(px(14.)).rounded_full().flex().items_center().justify_center().text_size(px(11.)).font_weight(FontWeight::BOLD).child(crate::ui::glyph("help-dot", 15.6, crate::theme::help_dot(0x888888)));
        Some(
            v_flex()
                .id("dyn-suppression")
                .occlude()
                .absolute()
                .top(px(3.))
                .left_0()
                .w(px(220.))
                .bg(pal.dialog_bg)
                .shadow(vec![BoxShadow { color: hsla(0., 0., 0., 0.2), offset: point(px(0.), px(5.)), blur_radius: px(10.), spread_radius: px(0.), inset: false }])
                .text_color(pal.text)
                .on_key_down(cx.listener(|this, ev: &KeyDownEvent, window, cx| {
                    if ev.keystroke.key == "escape" {
                        this.ui.dyn_suppression = None;
                        window.focus(&this.focus, cx);
                        cx.notify();
                    }
                }))
                .child(
                    h_flex()
                        .h(px(29.))
                        .bg(gpui_kit::rgb(0xffffff))
                        .border_b_1()
                        .border_color(gpui_kit::rgb(0xd7d7d7))
                        .child(div().flex_1().pl(px(6.)).text_size(px(13.)).font_weight(FontWeight::MEDIUM).truncate().child("Dynamic suppression"))
                        .child(
                            div()
                                .id("dyn-ok")
                                .size(px(28.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .bg(pal.accept)
                                .cursor_pointer()
                                .child(crate::assets::icon("dialog-ok", 24., cx))
                                .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.accept_dynamic_suppression(window, cx))),
                        )
                        .child(
                            div()
                                .id("dyn-x")
                                .size(px(28.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .cursor_pointer()
                                .hover(|d| d.bg(pal.panel_hover))
                                .child(crate::assets::icon("dialog-cancel", 24., cx))
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.ui.dyn_suppression = None;
                                    cx.notify();
                                })),
                        ),
                )
                .child(
                    h_flex()
                        .h(px(26.))
                        .child(div().w(px(96.)).pl(px(5.)).text_size(px(12.)).text_color(gpui_kit::rgb(0x666666)).child("Unsuppressed if"))
                        .child(div().w(px(98.)).border_b_1().border_color(gpui_kit::rgb(0x1651b0)).child(field)),
                )
                .child(h_flex().h(px(22.)).justify_end().pr(px(4.)).child(help))
                .into_any_element(),
        )
    }

    // ------------------------------------------------------------ regeneration

    /// Pause regeneration on / off (off regenerates).
    pub(super) fn toggle_pause_regen(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.ui.regen_paused.take().is_some() {
            self.schedule_regen(window, cx);
        } else {
            if self.sketch.is_some() {
                self.flash_info("Sketches cannot be edited while regeneration is paused.".into(), cx);
                return;
            }
            self.ui.regen_paused = self.studio(cx).cloned();
        }
        cx.notify();
    }

    /// Paused banner buttons: regenerate (stay paused), regenerate and exit,
    /// discard unregenerated changes and exit.
    fn paused_action(&mut self, action: u8, window: &mut Window, cx: &mut Context<Self>) {
        match action {
            0 => {
                self.ui.force_regen = true;
                self.ui.regen_paused = self.studio(cx).cloned();
                self.schedule_regen(window, cx);
            }
            1 => {
                self.ui.regen_paused = None;
                self.schedule_regen(window, cx);
            }
            _ => {
                if let Some(saved) = self.ui.regen_paused.take() {
                    self.doc.update(cx, |doc, cx| doc.edit_studio(self.tab, "Discard unregenerated changes", cx, |ps| *ps = saved));
                }
                self.schedule_regen(window, cx);
            }
        }
        cx.notify();
    }

    pub(super) fn render_regen_panels(&mut self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let pal = p();
        let mut out = Vec::new();
        if self.ui.regen_paused.is_some() {
            let btn = |id: &'static str, label: &'static str, action: u8, cx: &mut Context<Self>| {
                div()
                    .id(id)
                    .px_2()
                    .py_0p5()
                    .rounded_sm()
                    .text_xs()
                    .text_color(pal.accent)
                    .cursor_pointer()
                    .hover(|d| d.bg(pal.panel_hover))
                    .child(label)
                    .on_click(cx.listener(move |this, _: &ClickEvent, w, cx| this.paused_action(action, w, cx)))
            };
            out.push(
                h_flex()
                    .id("paused-banner")
                    .occlude()
                    .absolute()
                    .bottom(px(46.))
                    .left(px(260.))
                    .gap_2()
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .bg(pal.panel_raised)
                    .border_1()
                    .border_color(pal.warning)
                    .child(div().text_xs().text_color(pal.text).child("Paused regeneration"))
                    .child(btn("pr-regen", "Regenerate Part Studio", 0, cx))
                    .child(btn("pr-exit", "Regenerate features and exit", 1, cx))
                    .child(btn("pr-discard", "Discard unregenerated changes and exit", 2, cx))
                    .into_any_element(),
            );
        }
        if self.ui.show_regen_times {
            out.push(self.render_regen_times(cx));
        }
        out
    }

    /// Onshape's "Regeneration times" dialog (`regen-info-dialog`, live
    /// part-studio/1158, behaviours "qa5 bracket"; qa5 REG1): 350 px on the
    /// feature list's edge, a table # | Feature name | Time sorted slowest
    /// first, a blue bar under each time (its share of the total), and the
    /// footer "Total regen time: 92ms".
    fn render_regen_times(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let studio = self.studio(cx).cloned().unwrap_or_default();
        let total: f32 = self.display.features.iter().map(|f| f.time_ms).sum();
        let (col, desc) = self.ui.regen_sort;
        let mut rows: Vec<(usize, &noshape_model::partstudio::Feature, f32, bool)> = studio
            .features
            .iter()
            .enumerate()
            .map(|(i, f)| {
                let st = self.display.feature_state(f.id);
                let failed = st.is_some_and(|s| s.status == FeatureStatus::Error);
                (i + 1, f, st.map_or(0.0, |s| s.time_ms), failed)
            })
            .collect();
        rows.sort_by(|a, b| {
            let o = match col {
                0 => a.0.cmp(&b.0),
                1 => a.1.name.cmp(&b.1.name),
                _ => a.2.total_cmp(&b.2).then_with(|| b.0.cmp(&a.0)),
            };
            if desc { o.reverse() } else { o }
        });
        let line = gpui_kit::rgb(0xaaaaaa);
        const COLS: [f32; 3] = [49., 199., 99.];
        let head = |i: u8, label: &'static str, cx: &mut Context<Self>| {
            let icon = (col == i).then_some(if desc { "sort-desc" } else { "sort-asc" });
            h_flex()
                .id(SharedString::from(format!("rt-th-{i}")))
                .w(px(COLS[i as usize] + 1.))
                .h_full()
                .border_r_1()
                .border_color(line)
                .cursor_pointer()
                .child(div().flex_1().px(px(6.)).text_size(px(12.)).font_weight(FontWeight::BOLD).child(label))
                .child(div().size(px(30.)).flex().items_center().justify_center().children(icon.map(|i| crate::ui::glyph(i, 20., pal.text))))
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    let (c, d) = this.ui.regen_sort;
                    // Time starts slowest first; # and the name ascending.
                    this.ui.regen_sort = if c == i { (i, !d) } else { (i, i == 2) };
                    cx.notify();
                }))
        };
        let body: Vec<AnyElement> = rows
            .into_iter()
            .enumerate()
            .map(|(k, (n, f, t, failed))| {
                let id = f.id;
                let cell = |w: f32| div().w(px(w + 1.)).h_full().flex().items_center().px(px(6.)).border_r_1().border_color(line);
                let bar = if total > 0.0 { t / total * COLS[2] } else { 0.0 };
                h_flex()
                    .id(SharedString::from(format!("rt-{id}")))
                    .h(px(25.6))
                    .text_size(px(12.))
                    .when(k % 2 == 0, |d| d.bg(gpui_kit::rgb(0xfafafa)))
                    .child(cell(COLS[0]).child(n.to_string()))
                    .child(
                        cell(COLS[1])
                            .pl(px(5.))
                            .gap(px(10.))
                            .when(failed, |d| d.text_color(pal.text_invalid))
                            .child(div().flex_none().opacity(0.8).child(crate::assets::icon(&feature_icon(&f.type_id), 16., cx)))
                            .child(div().flex_1().min_w_0().truncate().child(crate::ui::user_text(f.name.clone()))),
                    )
                    .child(cell(COLS[2]).relative().child(crate::ui::regen_time_text(t)).child(div().absolute().left_0().bottom_0().h(px(5.)).w(px(bar)).bg(pal.accent)))
                    .on_hover(cx.listener(move |this, h: &bool, _, cx| {
                        // Hovering a row highlights the feature's geometry.
                        this.ui.list_hover = h.then_some(id);
                        this.refresh_scene(false, cx);
                    }))
                    .into_any_element()
            })
            .collect();
        let shadow = vec![BoxShadow { color: hsla(0., 0., 0., 0.2), offset: point(px(0.), px(5.)), blur_radius: px(10.), spread_radius: px(0.), inset: false }];
        v_flex()
            .id("regen-times")
            .occlude()
            .absolute()
            .top(px(3.))
            .left(px(-1.))
            .w(px(351.))
            .bg(gpui_kit::rgb(0xffffff))
            .border_l_1()
            .border_color(gpui_kit::rgb(0xc3c3c3))
            .shadow(shadow)
            .text_color(pal.text)
            .child(
                h_flex()
                    .h(px(29.))
                    .border_b_1()
                    .border_color(gpui_kit::rgb(0xd7d7d7))
                    .child(div().flex_1().pl(px(7.)).text_size(px(14.)).child("Regeneration times"))
                    .child(
                        div()
                            .id("rt-close")
                            .size(px(28.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .opacity(0.5)
                            .child(crate::ui::glyph("close", 16., gpui_kit::rgb(0x000000)))
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.ui.show_regen_times = false;
                                cx.notify();
                            })),
                    ),
            )
            .child(h_flex().h(px(35.)).bg(gpui_kit::rgb(0xf5f5f5)).border_b_1().border_color(line).child(head(0, "#", cx)).child(head(1, "Feature name", cx)).child(head(2, "Time", cx)))
            .child(div().id("rt-rows").max_h(px(640.)).overflow_y_scroll().children(body))
            .child(
                h_flex()
                    .h(px(31.))
                    .px(px(6.))
                    .bg(gpui_kit::rgb(0xeeeeee))
                    .text_size(px(11.))
                    .child("Total regen time: ")
                    .child(div().font_weight(FontWeight::BOLD).child(crate::ui::regen_time_text(total))),
            )
            .into_any_element()
    }

    // ------------------------------------------------------------ select ▸

    /// Select ▸ / Create selection: add entities related to `seed` to the
    /// active dialog field (filtered) or the selection.
    /// Select ▸ on a sketch curve: the curves of its sketch joined to it end
    /// to end (loop/chain), or only through tangent joints.
    fn sketch_related(&self, seed: &Reference, c: noshape_model::select::Criterion) -> Vec<Reference> {
        use noshape_model::select::Criterion;
        // A region: the face-type items keep it; holes, fillets and loops find
        // nothing in a sketch (live).
        if let Reference::SketchRegion { .. } = seed {
            return match c {
                Criterion::Protrusion | Criterion::Pocket | Criterion::TangentFaces | Criterion::BoundedFaces => vec![seed.clone()],
                _ => Vec::new(),
            };
        }
        let Reference::SketchEntity { sketch, entity } = seed else { return Vec::new() };
        let Some(sd) = self.display.sketches.iter().find(|s| s.feature == *sketch) else { return Vec::new() };
        let p3 = |p: &[f32; 3]| glam::DVec3::new(p[0] as f64, p[1] as f64, p[2] as f64);
        let curves: Vec<(u32, Vec<glam::DVec3>)> = sd.curves.iter().map(|c| (c.entity, c.points.iter().map(p3).collect())).filter(|(_, pl): &(u32, Vec<glam::DVec3>)| pl.len() >= 2).collect();
        let Some(start) = curves.iter().position(|(e, _)| e == entity) else { return Vec::new() };
        // Straight: direction and length; curved: the radius through three points.
        let shape = |pl: &[glam::DVec3]| -> (Option<glam::DVec3>, f64) {
            let (a, b) = (pl[0], pl[pl.len() - 1]);
            let mid = pl[pl.len() / 2];
            let straight = pl.iter().all(|p| (*p - a).cross((b - a).normalize_or_zero()).length() < 1e-9 + 1e-6 * a.distance(b));
            if straight && a.distance(b) > 0.0 {
                return (Some((b - a).normalize()), a.distance(b));
            }
            // Circumradius of first, middle and a quarter point.
            let q = pl[pl.len() / 4];
            let (x, y) = (q - a, mid - a);
            let cr = x.cross(y);
            let r = if cr.length_squared() > 0.0 { (x.length() * y.length() * (x - y).length()) / (2.0 * cr.length()) } else { 0.0 };
            (None, r)
        };
        let (dir0, size0) = shape(&curves[start].1);
        let same = |s: f64| (s - size0).abs() <= 1e-7_f64.max(size0 * 1e-4);
        match c {
            Criterion::EqualRadius => {
                return curves.iter().filter(|(_, pl)| {
                    let (d, s) = shape(pl);
                    d.is_some() == dir0.is_some() && same(s)
                }).map(|(e, _)| Reference::SketchEntity { sketch: *sketch, entity: *e }).collect();
            }
            Criterion::ParallelEdges => {
                let Some(d0) = dir0 else { return Vec::new() };
                return curves.iter().filter(|(_, pl)| shape(pl).0.is_some_and(|d| d.dot(d0).abs() > 0.9998)).map(|(e, _)| Reference::SketchEntity { sketch: *sketch, entity: *e }).collect();
            }
            _ => {}
        }
        let tol = 1e-7;
        let tangent_only = c == Criterion::TangentEdges;
        let tol_g = noshape_model::analysis::ContinuityTolerances { g1_deg: 1.0, ..Default::default() };
        let mut seen = vec![false; curves.len()];
        seen[start] = true;
        let mut queue = vec![start];
        while let Some(i) = queue.pop() {
            let pl = &curves[i].1;
            for end in [pl[0], pl[pl.len() - 1]] {
                for (j, (_, other)) in curves.iter().enumerate() {
                    if seen[j] {
                        continue;
                    }
                    let joined = other[0].distance(end) < tol || other[other.len() - 1].distance(end) < tol;
                    if !joined {
                        continue;
                    }
                    if tangent_only {
                        let joint = noshape_model::analysis::curve_connections(&[pl.clone(), other.clone()], &tol_g);
                        if !joint.iter().any(|(p, k)| p.distance(end) < tol && *k >= noshape_model::analysis::Continuity::G1) {
                            continue;
                        }
                    }
                    seen[j] = true;
                    queue.push(j);
                }
            }
        }
        curves.iter().zip(seen).filter(|(_, s)| *s).map(|((e, _), _)| Reference::SketchEntity { sketch: *sketch, entity: *e }).collect()
    }

    /// Entities related to `seed` (a part face/edge or a sketch region/curve).
    pub(super) fn related(&self, seed: &Reference, c: noshape_model::select::Criterion) -> Vec<Reference> {
        let Some(ctx) = &self.context else { return Vec::new() };
        match seed {
            Reference::SketchEntity { .. } | Reference::SketchRegion { .. } => self.sketch_related(seed, c),
            _ => noshape_model::select::select_related(ctx, seed, c),
        }
    }

    pub(super) fn select_related(&mut self, seed: &Reference, c: noshape_model::select::Criterion, cx: &mut Context<Self>) {
        let Some(ctx) = self.context.clone() else { return };
        let found = match seed {
            // The other selected faces and edges bound the region; they stay
            // selected (live: face + edge stays 2 when nothing closes).
            Reference::Face(_) if c == noshape_model::select::Criterion::BoundedFaces => {
                let boundary: Vec<Reference> = self.selection.iter().filter(|r| *r != seed).cloned().collect();
                let mut found = noshape_model::select::select_bounded(&ctx, seed, &boundary);
                for r in boundary {
                    if !found.contains(&r) {
                        found.push(r);
                    }
                }
                found
            }
            _ => self.related(seed, c),
        };
        if let Some(e) = &self.edit {
            let dialog = e.dialog.clone();
            let Some((_, filter, _)) = dialog.read(cx).active_filter() else { return };
            let current = dialog.read(cx).selected_refs();
            dialog.update(cx, |d, cx| {
                for r in found {
                    let kind = match r {
                        Reference::Edge(_) => PickKind::Edge,
                        Reference::SketchEntity { .. } => PickKind::SketchCurve,
                        _ => PickKind::Face,
                    };
                    if kind_matches(kind, &r, filter) && !current.contains(&r) {
                        d.add_pick(r, cx);
                    }
                }
            });
        } else {
            // Each item replaces the selection with what it finds; nothing
            // found clears it (live).
            self.selection = found;
        }
        self.refresh_scene(false, cx);
        cx.notify();
    }

    // ------------------------------------------------------------ hide / show

    /// Y: hide what is under the cursor (or the selection if the cursor is
    /// over a selected entity or nothing).
    pub(super) fn hide_hovered_or_selected(&mut self, cx: &mut Context<Self>) {
        let targets: Vec<Reference> = match &self.hover {
            Some(h) if !self.selection.contains(h) => vec![h.clone()],
            _ => self.selection.clone(),
        };
        let mut targets = targets;
        targets.extend(self.selected_features.iter().map(|f| Reference::Feature(*f)));
        if targets.is_empty() {
            return;
        }
        self.set_visibility(&targets, true, cx);
        self.selection.clear();
        self.selected_features.clear();
        self.hover = None;
        cx.notify();
    }

    /// Hide or show parts, sketches, planes and default geometry.
    pub(super) fn set_visibility(&mut self, targets: &[Reference], hidden: bool, cx: &mut Context<Self>) {
        // A feature that made parts hides its parts; sketches, planes and
        // connectors hide themselves.
        let t = noshape_ops::studio::visibility_targets(targets, |f| self.display.bodies.iter().filter(|b| b.id.feature == f).map(|b| b.id.clone()).collect(), |r| self.body_of(r));
        if t.is_empty() {
            return;
        }
        self.doc.update(cx, |doc, cx| {
            doc.edit_studio(self.tab, noshape_ops::studio::visibility_description(hidden), cx, |ps| noshape_ops::studio::set_visibility(ps, &t, hidden));
        });
        let (feats, defaults) = (&t.features, &t.defaults);
        // Default geometry is shown from the regeneration's display, which a
        // visibility change doesn't regenerate: update it here (it stayed
        // drawn after Hide, user bug v0.4.0).
        if !defaults.is_empty() || !feats.is_empty() {
            let display = std::sync::Arc::make_mut(&mut self.display);
            for p in &mut display.planes {
                if defaults.contains(&p.reference) || matches!(&p.reference, Reference::FeaturePlane(f) if feats.contains(f)) {
                    p.hidden = hidden;
                }
            }
            for sk in &mut display.sketches {
                if feats.contains(&sk.feature) {
                    sk.hidden = hidden;
                }
            }
            if defaults.contains(&Reference::Origin) {
                display.show_origin = !hidden;
            }
            // Planes and the origin are baked into the built scene.
            self.rebuild_scene(cx);
        }
    }

    /// Shift+Y: show hidden selected items, otherwise every hidden part.
    pub(super) fn show_hidden(&mut self, cx: &mut Context<Self>) {
        let mut targets: Vec<Reference> = self.selection.clone();
        targets.extend(self.selected_features.iter().map(|f| Reference::Feature(*f)));
        if targets.is_empty() {
            self.show_all_parts(cx);
        } else {
            self.set_visibility(&targets, false, cx);
        }
    }

    /// Context menu "Show all": parts, sketches, planes and default geometry.
    pub(super) fn show_everything(&mut self, cx: &mut Context<Self>) {
        self.doc.update(cx, |doc, cx| {
            doc.edit_studio(self.tab, noshape_ops::studio::SHOW_ALL_DESCRIPTION, cx, noshape_ops::studio::show_all);
        });
        let display = std::sync::Arc::make_mut(&mut self.display);
        for p in &mut display.planes {
            if matches!(p.reference, Reference::Plane(_)) {
                p.hidden = false;
            }
        }
        display.show_origin = true;
        self.opts.show_planes = true;
        self.opts.show_sketches = true;
        self.rebuild_scene(cx);
        cx.notify();
    }

    /// Shift+P: hide all construction geometry (planes, sketches).
    pub(super) fn hide_construction(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let any_shown = self.opts.show_planes || self.opts.show_sketches;
        self.opts.show_planes = !any_shown;
        self.opts.show_sketches = !any_shown;
        let d = self.display.clone();
        self.apply_display(d, window, cx);
    }

    // ------------------------------------------------------------ isolate

    /// Parts targeted by Isolate / Make transparent: hovered or selected.
    fn isolate_targets(&self) -> Vec<BodyId> {
        let mut out: Vec<BodyId> = Vec::new();
        let hovered = self.hover.as_ref().and_then(|h| self.body_of(h));
        let from_sel = self.selection.iter().filter_map(|r| self.body_of(r));
        for b in hovered.into_iter().chain(from_sel) {
            if !out.contains(&b) {
                out.push(b);
            }
        }
        out
    }

    /// Shift+I / Shift+T (cumulative while active, like Onshape).
    pub(super) fn isolate(&mut self, mode: IsolateMode, window: &mut Window, cx: &mut Context<Self>) {
        let targets = self.isolate_targets();
        if targets.is_empty() {
            self.flash_info("Select or hover over a part first.".into(), cx);
            return;
        }
        match &mut self.ui.isolate {
            Some(iso) if iso.mode == mode => {
                for t in targets {
                    if !iso.bodies.contains(&t) {
                        iso.bodies.push(t);
                    }
                }
            }
            _ => self.ui.isolate = Some(Isolation { mode, bodies: targets, opacity: 0.0, select_transparent: false, expand: 0 }),
        }
        let d = self.display.clone();
        self.apply_display(d, window, cx);
    }

    pub(super) fn exit_isolate(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if self.ui.isolate.take().is_none() {
            return false;
        }
        let d = self.display.clone();
        self.apply_display(d, window, cx);
        true
    }

    /// Display to build the scene from: the regenerated display with
    /// isolate/transparency applied to part colors.
    /// Parts made transparent by Isolate / Make transparent, with the
    /// "Expand: distance" neighbours: live (behaviours.md "Part Studio
    /// Isolate…") joins parts whose gap to the chosen ones is within a radius
    /// growing with the square of the step (gaps 5 / 30 / 55 / 80 mm joined
    /// at 20 / 40 / 50 / 70 % in a 271 mm model: 0.815 × diagonal × step²).
    pub(super) fn isolation_transparent(&self) -> HashSet<BodyId> {
        let Some(iso) = &self.ui.isolate else { return HashSet::new() };
        let bounds = |b: &BodyId| self.display.body(b).and_then(|b| b.bounds);
        let gap = |a: (glam::DVec3, glam::DVec3), b: (glam::DVec3, glam::DVec3)| ((a.0 - b.1).max(b.0 - a.1)).max(glam::DVec3::ZERO).length();
        let all = self.display.bodies.iter().filter_map(|b| b.bounds).reduce(|a, b| (a.0.min(b.0), a.1.max(b.1)));
        let radius = all.map_or(0.0, |(lo, hi)| (hi - lo).length() * 0.815 * (iso.expand as f64 / 10.).powi(2));
        let mut chosen: Vec<BodyId> = iso.bodies.clone();
        if iso.expand > 0 {
            let seeds: Vec<_> = iso.bodies.iter().filter_map(bounds).collect();
            for b in &self.display.bodies {
                if !chosen.contains(&b.id)
                    && let Some(bb) = b.bounds
                    && seeds.iter().any(|s| gap(*s, bb) <= radius)
                {
                    chosen.push(b.id.clone());
                }
            }
        }
        self.display
            .bodies
            .iter()
            .filter(|b| match iso.mode {
                IsolateMode::Isolate => !chosen.contains(&b.id),
                IsolateMode::Transparent => chosen.contains(&b.id),
            })
            .map(|b| b.id.clone())
            .collect()
    }

    /// Regenerate the model before the edited feature (preview slider).
    pub(super) fn schedule_preview_before(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // Needed for the preview slider and for drag handles on geometry the
        // feature consumes (fillet edges, shell faces).
        // Also for every open feature: bodies the feature creates are drawn
        // as Onshape's see-through preview (display_for_scene).
        if self.edit.is_none() {
            self.ui.preview_before = None;
            return;
        }
        let Some((studio, _)) = self.effective_studio(cx) else { return };
        let Some(index) = self.edit.as_ref().map(|e| e.index) else { return };
        let units = self.doc.read(cx).session.doc.settings.units;
        let config = self.current_config(&studio);
        let doc_vars = noshape_model::variables::element_variables(&self.doc.read(cx).session.doc, self.tab);
        let derived: Arc<dyn noshape_model::regen::derived::DerivedProvider> = self.doc.read(cx).derived_provider();
        let custom: Arc<dyn noshape_model::custom::CustomFeatureProvider> = self.doc.read(cx).custom_features();
        let regen = self.regen.clone();
        self.ui.preview_task = Some(cx.spawn_in(window, async move |this, cx| {
            let display = cx
                .background_spawn(async move {
                    let opts = noshape_model::regen::RegenOptions { upto: Some(index), document_variables: doc_vars, preview_meshes: true, derived: Some(derived), custom_features: Some(custom), ..Default::default() };
                    let r = regen.lock().regenerate(&studio, &config, &units, &opts);
                    (r.display, r.context)
                })
                .await;
            let (display, context) = display;
            let _ = this.update_in(cx, |this: &mut Self, window, cx| {
                this.ui.before_context = Some(context);
                this.ui.preview_before = Some(Arc::new(display));
                // Changes made before the model was ready get std's editing logic now.
                if let Some(dialog) = this.edit.as_ref().map(|e| e.dialog.clone())
                    && noshape_model::custom::std_feature::has_editing_logic(&dialog.read(cx).feature.type_id)
                {
                    this.std_edit_logic(&dialog, cx);
                }
                // Chips of parts the feature consumes get their names now.
                if let Some(dialog) = this.edit.as_ref().map(|e| e.dialog.clone()) {
                    let labeler = this.labeler(cx);
                    dialog.update(cx, |d, cx| d.set_labeler(labeler, cx));
                }
                let d = this.display.clone();
                this.apply_display(d, window, cx);
            });
        }));
    }

    /// Remove / Intersect previews: regenerate the open feature as New to get
    /// its tool body (shown see-through by `display_for_scene`).
    pub(super) fn schedule_tool_preview(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // Also a failing Add: its tool shows red (live qa5 BOOL4: "No merge
        // scope selected." with the title and the preview red).
        let tool_op = self.edit.as_ref().is_some_and(|e| {
            let f = &e.dialog.read(cx).feature;
            let failing = self.display.feature_state(f.id).is_some_and(|s| s.status == FeatureStatus::Error);
            matches!(f.type_id.as_str(), "extrude" | "revolve" | "sweep" | "loft") && (matches!(f.params.enum_("operationType"), "REMOVE" | "INTERSECT") || (f.params.enum_("operationType") == "ADD" && failing))
        });
        if !tool_op {
            self.ui.preview_tool = None;
            self.ui.preview_tool_task = None;
            self.show_tool_miss(false, cx);
            return;
        }
        let Some((mut studio, upto)) = self.effective_studio(cx) else { return };
        let Some(index) = self.edit.as_ref().map(|e| e.index) else { return };
        let Some(fid) = self.edit.as_ref().map(|e| e.dialog.read(cx).feature.id) else { return };
        if let Some(f) = studio.features.get_mut(index) {
            f.params.set("operationType", noshape_model::params::ParamValue::Enum("NEW".into()));
        }
        let units = self.doc.read(cx).session.doc.settings.units;
        let config = self.current_config(&studio);
        let doc_vars = noshape_model::variables::element_variables(&self.doc.read(cx).session.doc, self.tab);
        let derived: Arc<dyn noshape_model::regen::derived::DerivedProvider> = self.doc.read(cx).derived_provider();
        let custom: Arc<dyn noshape_model::custom::CustomFeatureProvider> = self.doc.read(cx).custom_features();
        let regen = self.regen.clone();
        self.ui.preview_tool_task = Some(cx.spawn_in(window, async move |this, cx| {
            let display = cx
                .background_spawn(async move {
                    let opts = noshape_model::regen::RegenOptions { upto, document_variables: doc_vars, preview_meshes: true, derived: Some(derived), custom_features: Some(custom), ..Default::default() };
                    regen.lock().regenerate(&studio, &config, &units, &opts).display
                })
                .await;
            let _ = this.update_in(cx, |this: &mut Self, window, cx| {
                this.ui.preview_tool = Some((Arc::new(display), fid));
                // "Selected tools and targets do not intersect." is a Remove /
                // Intersect note; a failing Add only shows its tool red (BOOL4b).
                let removes = this.edit.as_ref().is_some_and(|e| matches!(e.dialog.read(cx).feature.params.enum_("operationType"), "REMOVE" | "INTERSECT"));
                let missed = removes && this.tool_missed();
                this.show_tool_miss(missed, cx);
                let d = this.display.clone();
                this.apply_display(d, window, cx);
            });
        }));
    }

    /// A Remove / Intersect whose tool reaches no part: every part comes out
    /// of the feature exactly as it went in.
    pub(super) fn tool_missed(&self) -> bool {
        let (Some(_), Some(before)) = (&self.ui.preview_tool, &self.ui.preview_before) else { return false };
        !before.bodies.is_empty()
            && before.bodies.len() == self.display.bodies.len()
            && before.bodies.iter().zip(&self.display.bodies).all(|(a, b)| a.id == b.id && a.bounds == b.bounds && a.face_refs.len() == b.face_refs.len() && a.mesh.positions.len() == b.mesh.positions.len())
    }

    /// Live (part-studio/234): the blue info toast while the tool misses.
    fn show_tool_miss(&mut self, missed: bool, cx: &mut Context<Self>) {
        let shown = self.message.as_ref().is_some_and(|(_, m)| m == NO_INTERSECTION);
        if missed && !shown {
            self.message = Some((noshape_model::display::FeatureStatus::Info, NO_INTERSECTION.into()));
            cx.notify();
        } else if !missed && shown {
            self.message = None;
            cx.notify();
        }
    }

    /// Bodies the open dialog's feature creates, shown in the preview lighting
    /// (not while a Remove / Intersect tool or the preview slider is shown).
    pub(super) fn preview_body_ids(&self) -> Vec<noshape_model::BodyId> {
        let (Some(_), Some(before)) = (&self.edit, &self.ui.preview_before) else { return Vec::new() };
        if self.ui.preview_tool.is_some() || self.ui.preview_opacity < 0.999 {
            return Vec::new();
        }
        let old: std::collections::HashSet<&noshape_model::BodyId> = before.bodies.iter().map(|b| &b.id).collect();
        self.display.bodies.iter().filter(|b| !old.contains(&b.id)).map(|b| b.id.clone()).collect()
    }

    /// The parts an open Transform moves away from where they were (no
    /// ghost while they still coincide, or when it copies them).
    pub(super) fn transform_ghost(&self, cx: &App) -> Vec<BodyId> {
        let (Some(e), Some(before)) = (&self.edit, &self.ui.preview_before) else { return Vec::new() };
        let d = e.dialog.read(cx);
        let p = &d.feature.params;
        if d.feature.type_id != "transform" || p.bool("makeCopy") || p.is("transformType", "COPY") {
            return Vec::new();
        }
        let moved = |id: &BodyId| match (before.bodies.iter().find(|b| &b.id == id), self.display.bodies.iter().find(|b| &b.id == id)) {
            (Some(a), Some(b)) => match (a.bounds, b.bounds) {
                (Some((a0, a1)), Some((b0, b1))) => (a0 - b0).length() > 1e-9 || (a1 - b1).length() > 1e-9,
                _ => false,
            },
            _ => false,
        };
        let mut out: Vec<BodyId> = Vec::new();
        for r in p.query("entities") {
            if let Some(b) = self.body_of(r).filter(|b| !out.contains(b) && moved(b)) {
                out.push(b);
            }
        }
        out
    }

    pub(super) fn display_for_scene_inner(&self) -> Arc<StudioDisplay> {
        /// Opacity of a feature's new bodies while its dialog is open (with
        /// the renderer's preview lighting, see the offscreen test
        /// `feature_preview_shading_matches_onshape`).
        const FEATURE_PREVIEW_ALPHA: f32 = 0.7;
        // Preview slider: blend the model before the feature (fading out)
        // with the result (fading in).
        if self.edit.is_some()
            && self.ui.preview_opacity < 0.999
            && let Some(before) = &self.ui.preview_before
        {
            let s = self.ui.preview_opacity;
            let mut d = (*self.display).clone();
            for b in &mut d.bodies {
                b.color[3] *= s.max(0.02);
            }
            for b in &before.bodies {
                let mut b = b.clone();
                b.color[3] *= (1.0 - s).max(0.02);
                d.bodies.push(b);
            }
            return Arc::new(d);
        }
        // Live (part-studio/234-237): a Remove / Intersect shows its tool body
        // see-through: dark red while it doesn't reach the parts (the
        // feature fails), dark gray once it cuts.
        if let (Some(_), Some((tool, fid)), Some(before)) = (&self.edit, &self.ui.preview_tool, &self.ui.preview_before) {
            let old: std::collections::HashSet<&noshape_model::BodyId> = before.bodies.iter().map(|b| &b.id).collect();
            let failed = self.tool_missed() || self.display.features.iter().any(|f| f.id == *fid && f.status == noshape_model::display::FeatureStatus::Error);
            let c = crate::theme::lin(if failed { crate::theme::viewport::TOOL_PREVIEW_FAIL } else { crate::theme::viewport::TOOL_PREVIEW });
            let mut d = (*self.display).clone();
            // No edges on the tool (live 236); red nearly opaque, gray see-through.
            for b in tool.bodies.iter().filter(|b| !old.contains(&b.id)) {
                let mut b = b.clone();
                b.color = [c[0], c[1], c[2], if failed { 0.9 } else { 0.6 }];
                b.mesh = Arc::new(noshape_model::display::MeshBuffers { edges: Vec::new(), vertices: Vec::new(), ..(*b.mesh).clone() });
                d.bodies.push(b);
            }
            return Arc::new(d);
        }
        // Live (part-studio/445-446): while a Transform moves parts, the
        // originals stay as faded ghosts carrying the selection outline;
        // the moved parts are display-only (scene_options).
        if let Some(before) = &self.ui.preview_before
            && !self.ui.transform_ghost.is_empty()
        {
            let mut d = (*self.display).clone();
            for b in before.bodies.iter().filter(|b| self.ui.transform_ghost.contains(&b.id)) {
                let mut b = b.clone();
                b.color[3] *= 0.3;
                // Its own id, so `display_only` (the moved copy) skips only the copy.
                b.id.index |= GHOST_INDEX_BIT;
                d.bodies.push(b);
            }
            return Arc::new(d);
        }
        // Live (part-studio/230): while a feature dialog is open, bodies the
        // feature creates show in the part shading but blue-gray and
        // see-through, so the picked regions show under them.
        if self.edit.is_some()
            && let Some(before) = &self.ui.preview_before
        {
            let old: std::collections::HashSet<&noshape_model::BodyId> = before.bodies.iter().map(|b| &b.id).collect();
            if self.display.bodies.iter().any(|b| !old.contains(&b.id)) {
                let mut d = (*self.display).clone();
                let c = crate::theme::lin(crate::theme::viewport::FEATURE_PREVIEW);
                for b in d.bodies.iter_mut().filter(|b| !old.contains(&b.id)) {
                    b.color = [c[0], c[1], c[2], FEATURE_PREVIEW_ALPHA];
                }
                return Arc::new(d);
            }
        }
        // Show dimensions: the parts turn see-through (Onshape).
        if self.ui.shown_dims.is_some() {
            let mut d = (*self.display).clone();
            for b in &mut d.bodies {
                b.color[3] = b.color[3].min(0.35);
            }
            return Arc::new(d);
        }
        let Some(iso) = &self.ui.isolate else { return self.display.clone() };
        let transparent = self.isolation_transparent();
        let look = iso_look(iso.opacity);
        let mut d = (*self.display).clone();
        // Isolate fades the planes to faint outlines and the ghosts are washed
        // out after shading (`scene_options`); the origin stays (live iso-901).
        for b in &mut d.bodies {
            if transparent.contains(&b.id) {
                match look {
                    IsoLook::Ghost => {}
                    IsoLook::SeeThrough(a) => b.color[3] = b.color[3].min(a),
                    IsoLook::Hidden => b.hidden = true,
                }
            }
        }
        Arc::new(d)
    }

    /// A click on a transparent part goes through it (Alt+click, or always
    /// while isolating without "Select transparent geometry"): the first
    /// entity of an opaque part along the ray, if any.
    pub(super) fn pick_through_transparent(&self, t: PickTarget, position: DVec2, alt: bool, cx: &App) -> Option<PickTarget> {
        let Some(body) = self.body_of(&t.reference) else { return Some(t) };
        let iso_t = self.isolation_transparent();
        let appearance_t = |b: &BodyId| self.display.body(b).is_some_and(|d| d.color[3] < 0.99);
        let select_t = self.ui.isolate.as_ref().is_some_and(|i| i.select_transparent);
        let through = |b: &BodyId| (iso_t.contains(b) && (alt || !select_t)) || (alt && appearance_t(b));
        if !through(&body) {
            return Some(t);
        }
        let (origin, dir, tol) = {
            let vp = self.viewport.read(cx);
            let (o, d) = vp.ray(position);
            (o, d, vp.camera.world_per_pixel(vp.size().y) * 5.0)
        };
        let hit = interaction::entities_along_ray(&self.display, origin, dir, tol, &|b| through(b), 1).into_iter().next()?;
        let kind = if matches!(hit.reference, Reference::Edge(_)) { PickKind::Edge } else { PickKind::Face };
        Some(PickTarget { reference: hit.reference, kind, anchor: None })
    }

    /// Onshape's Isolate / Make transparent dialog (`isolate_ui`, live
    /// cursors/641-_Isolate): here the Expand select is "Expand: distance"
    /// (the nearest parts join).
    pub(super) fn render_isolate_panel(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let iso = self.ui.isolate.as_ref()?;
        let title = match iso.mode {
            IsolateMode::Isolate => "Isolate",
            IsolateMode::Transparent => "Make transparent",
        };
        let dialog = crate::isolate_ui::dialog(
            crate::isolate_ui::IsoDialog {
                key: "ps-isolate",
                title,
                select_transparent: iso.select_transparent,
                transparency: iso.opacity,
                expand: iso.expand as f32 / 10.,
                // Live: connectivity is offered but disabled (no mates here).
                expand_options: vec!["Expand: connectivity".to_string(), "Expand: distance".to_string()],
                expand_disabled: vec![0],
                expand_ix: 1,
                expand_tip: "Expand isolate by physical proximity",
                legend: true,
                drag: |v: &mut Self| &mut v.ui.iso_slider_drag,
                close: |v: &mut Self, window, cx| {
                    v.exit_isolate(window, cx);
                },
                toggle_select: |v: &mut Self, _, cx| {
                    if let Some(iso) = &mut v.ui.isolate {
                        iso.select_transparent = !iso.select_transparent;
                    }
                    cx.notify();
                },
                set_transparency: std::rc::Rc::new(|v: &mut Self, f, window, cx| {
                    if let Some(iso) = &mut v.ui.isolate {
                        iso.opacity = f;
                    }
                    let d = v.display.clone();
                    v.apply_display(d, window, cx);
                }),
                // Live: snaps in 10 % steps.
                set_expand: std::rc::Rc::new(move |v: &mut Self, f, window, cx| {
                    let want = (f * 10.).round() as usize;
                    if let Some(iso) = &mut v.ui.isolate
                        && iso.expand != want
                    {
                        iso.expand = want;
                        let d = v.display.clone();
                        v.apply_display(d, window, cx);
                    }
                }),
                pick_expand: std::rc::Rc::new(|_, _, _| {}),
            },
            cx,
        );
        // Live: at the graphics area's top left, like the other panel dialogs.
        Some(crate::ui::moved_panel(div().absolute().top(px(3.)).left_0().child(dialog), self.ui.panels.get("ps-isolate")))
    }

    // ------------------------------------------------------------ camera

    pub(super) fn push_camera_history(&mut self, cx: &App) {
        let cam = self.viewport.read(cx).camera;
        if self.ui.cam_history.last() != Some(&cam) {
            self.ui.cam_history.push(cam);
            if self.ui.cam_history.len() > 50 {
                self.ui.cam_history.remove(0);
            }
        }
    }

    /// Camera menu "Previous view".
    pub(super) fn previous_view(&mut self, cx: &mut Context<Self>) {
        if let Some(cam) = self.ui.cam_history.pop() {
            self.viewport.update(cx, |vp, cx| vp.animate_to(cam, cx));
        }
    }

    pub(super) fn zoom_fit(&mut self, cx: &mut Context<Self>) {
        self.push_camera_history(cx);
        self.viewport.update(cx, |vp, cx| vp.zoom_to_fit(true, cx));
    }

    /// What zoom to fit frames (Onshape): visible parts and sketches; the
    /// default planes only when there is nothing else (`None`).
    pub(super) fn fit_bounds(&self) -> Option<(glam::DVec3, glam::DVec3)> {
        let mut acc = self.display.bounds();
        let mut add = |q: glam::DVec3| acc = Some(acc.map_or((q, q), |(lo, hi)| (lo.min(q), hi.max(q))));
        let p3 = |a: [f32; 3]| glam::DVec3::new(a[0] as f64, a[1] as f64, a[2] as f64);
        for sk in &self.display.sketches {
            let editing = self.opts.editing_sketch == Some(sk.feature);
            if !editing && (sk.hidden || !self.opts.show_sketches) {
                continue;
            }
            sk.curves.iter().flat_map(|c| c.points.iter()).for_each(|q| add(p3(*q)));
            sk.points.iter().for_each(|q| add(p3(q.position)));
        }
        acc
    }

    /// What Zoom to fit and the standard views frame (Onshape, live "Zoom to
    /// fit (perf 10-23 22:40)"): the visible parts and sketches, the origin
    /// point when it is shown, and the visible planes (the default ones
    /// span ±75 mm).
    pub(super) fn zoom_fit_bounds(&self) -> Option<(glam::DVec3, glam::DVec3)> {
        let origin = self.opts.show_origin && self.display.show_origin;
        self.fit_box(origin, |pl| self.opts.show_planes && !pl.hidden)
    }

    /// The camera a Part Studio opens with frames its parts and the default
    /// planes' box, also when those are hidden (live, the same capture).
    pub(super) fn load_fit_bounds(&self) -> Option<(glam::DVec3, glam::DVec3)> {
        self.fit_box(true, |pl| matches!(pl.reference, noshape_model::reference::Reference::Plane(_)))
    }

    /// The parts and sketches ([`Self::fit_bounds`]), the origin point if
    /// `origin`, and the planes `planes` takes.
    fn fit_box(&self, origin: bool, planes: impl Fn(&noshape_model::display::PlaneDisplay) -> bool) -> Option<(glam::DVec3, glam::DVec3)> {
        let mut acc = self.fit_bounds();
        let mut add = |q: glam::DVec3| acc = Some(acc.map_or((q, q), |(lo, hi)| (lo.min(q), hi.max(q))));
        if origin {
            add(glam::DVec3::ZERO);
        }
        for pl in self.display.planes.iter().filter(|p| planes(p)) {
            let f = &pl.frame;
            for (sx, sy) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
                add(f.origin + f.x * (sx * pl.half_width) + f.y * (sy * pl.half_height));
            }
        }
        acc
    }

    /// Logical pixels of the viewport covered by the feature dialog (top left).
    pub(super) fn fit_inset_left(&self) -> f64 {
        if self.edit.is_some() { 220.0 + 2.0 * 12.0 } else { 0.0 }
    }

    /// W: the next box drag zooms to the box (press W again to cancel).
    pub(super) fn toggle_zoom_window(&mut self, cx: &mut Context<Self>) {
        self.push_camera_history(cx);
        self.viewport.update(cx, |vp, cx| {
            vp.zoom_window = !vp.zoom_window;
            cx.notify();
        });
        cx.notify();
    }

    /// Ctrl+Shift+arrows (Onshape pans 150 px per step).
    pub(super) fn pan_by(&mut self, dx: f64, dy: f64, cx: &mut Context<Self>) {
        let step = crate::navigation::DEFAULT_PAN_AMOUNT_PX;
        self.viewport.update(cx, |vp, cx| vp.pan_step(dx / step, dy / step, cx));
    }

    /// Bounding box of the selection (parts, faces, edges, sketches, planes).
    fn selection_bounds(&self) -> Option<(glam::DVec3, glam::DVec3)> {
        use glam::DVec3;
        let mut lo = DVec3::splat(f64::INFINITY);
        let mut hi = DVec3::splat(f64::NEG_INFINITY);
        let mut add = |q: DVec3| {
            lo = lo.min(q);
            hi = hi.max(q);
        };
        let p3 = |a: [f32; 3]| DVec3::new(a[0] as f64, a[1] as f64, a[2] as f64);
        let mut refs = self.selection.clone();
        for f in &self.selected_features {
            refs.extend(self.feature_geometry(*f));
        }
        for r in &refs {
            match r {
                Reference::Body(b) => {
                    if let Some((a, c)) = self.display.body(b).and_then(|b| b.bounds) {
                        add(a);
                        add(c);
                    }
                }
                Reference::SketchEntity { sketch, .. } | Reference::SketchRegion { sketch, .. } | Reference::Sketch(sketch) => {
                    if let Some(sk) = self.display.sketches.iter().find(|s| s.feature == *sketch) {
                        sk.curves.iter().flat_map(|c| c.points.iter()).for_each(|q| add(p3(*q)));
                        sk.points.iter().for_each(|q| add(p3(q.position)));
                    }
                }
                Reference::Plane(_) | Reference::FeaturePlane(_) => {
                    if let Some(pl) = self.display.planes.iter().find(|p| &p.reference == r) {
                        for (u, v) in [(-1.0, -1.0), (1.0, 1.0), (-1.0, 1.0), (1.0, -1.0)] {
                            add(pl.frame.origin + pl.frame.x * (u * pl.half_width) + pl.frame.y * (v * pl.half_height));
                        }
                    }
                }
                Reference::Origin => add(DVec3::ZERO),
                r => {
                    for b in &self.display.bodies {
                        if let Some(fi) = b.face_refs.iter().position(|x| x == r)
                            && let Some(&(first, count)) = b.mesh.face_ranges.get(fi)
                        {
                            for &i in &b.mesh.indices[first as usize..(first + count) as usize] {
                                if let Some(q) = b.mesh.positions.get(i as usize) {
                                    add(p3(*q));
                                }
                            }
                        } else if let Some(ei) = b.edge_refs.iter().position(|x| x == r) {
                            b.mesh.edges.get(ei).into_iter().flatten().for_each(|q| add(p3(*q)));
                        } else if let Some(vi) = b.vertex_refs.iter().position(|x| x == r)
                            && let Some(q) = b.mesh.vertices.get(vi)
                        {
                            add(p3(*q));
                        }
                    }
                }
            }
        }
        (lo.x <= hi.x).then_some((lo, hi))
    }

    /// Context / camera menu "Zoom to selection".
    pub(super) fn zoom_to_selection(&mut self, cx: &mut Context<Self>) {
        let Some((lo, hi)) = self.selection_bounds() else { return };
        self.push_camera_history(cx);
        let inset = self.fit_inset_left();
        self.viewport.update(cx, |vp, cx| {
            let mut to = vp.camera;
            crate::viewport::fit_camera_to_box(&mut to, lo, hi, vp.size(), inset);
            vp.animate_to(to, cx);
        });
    }

    // ------------------------------------------------------------ select other

    /// ` (grave): list the faces and edges under the cursor, nearest first,
    /// the second one highlighted (Shift+`: the last); pressing it again in
    /// the click zone moves down the list (Onshape onSelectOtherCommand).
    pub(super) fn select_other(&mut self, backwards: bool, cx: &mut Context<Self>) {
        if self.ui.select_other.as_ref().is_some_and(|so| so.in_zone || !so.moved_out) {
            self.select_other_step(backwards, cx);
        } else {
            self.start_select_other(false, backwards, cx);
        }
    }

    /// The next (previous) entry: ` again, ↑ / ↓ (Onshape highlightNext /
    /// PreviousSelection → setCurrentSelection: bold, pre-highlighted,
    /// scrolled into view).
    pub(super) fn select_other_step(&mut self, backwards: bool, cx: &mut Context<Self>) {
        let Some(so) = &mut self.ui.select_other else { return };
        let n = so.hits.len();
        if n == 0 {
            return;
        }
        let i = if backwards { so.index.map_or(n - 1, |i| (i + n - 1) % n) } else { so.index.map_or(0, |i| (i + 1) % n) };
        so.index = Some(i);
        so.scroll.scroll_to_item(i);
        self.hover = so.hits.get(i).cloned();
        self.refresh_scene(false, cx);
        cx.notify();
    }

    /// Select other… from a context menu: nothing highlighted yet, the
    /// point it was opened for marked (Onshape startSelectOther with an
    /// event).
    pub(super) fn select_other_from_menu(&mut self, cx: &mut Context<Self>) {
        self.start_select_other(true, false, cx);
    }

    fn start_select_other(&mut self, from_menu: bool, backwards: bool, cx: &mut Context<Self>) {
        let Some(at) = self.ui.pointer else { return };
        let (origin, dir, tol) = {
            let vp = self.viewport.read(cx);
            let (o, d) = vp.ray(at);
            (o, d, vp.camera.world_per_pixel(vp.size().y) * SELECT_OTHER_PICK_PX)
        };
        let iso = self.ui.isolate.as_ref().map(|i| (i.mode, i.bodies.clone()));
        let skip = move |b: &BodyId| match &iso {
            Some((IsolateMode::Isolate, keep)) => !keep.contains(b),
            Some((IsolateMode::Transparent, gone)) => gone.contains(b),
            None => false,
        };
        let found = interaction::entities_along_ray(&self.display, origin, dir, tol, &skip, SELECT_OTHER_MAX);
        let point = found.first().map(|h| origin + dir.normalize_or_zero() * h.depth);
        let hits: Vec<Reference> = found.into_iter().map(|h| h.reference).collect();
        let n = hits.len();
        let index = (!from_menu && n > 0).then(|| if backwards { n - 1 } else { 1 % n });
        self.hover = if from_menu { None } else { index.and_then(|i| hits.get(i).cloned()) };
        let scroll = ScrollHandle::new();
        if let Some(i) = index {
            scroll.scroll_to_item(i);
        }
        self.ui.select_other = Some(SelectOther { hits, index, at, point, in_zone: !from_menu, moved_out: false, scroll });
        self.refresh_scene(false, cx);
        cx.notify();
    }

    /// The pointer over the graphics while Select other is open: in the
    /// click zone the highlighted entry is pre-highlighted, out of it the
    /// point shows instead (Onshape onMouseMove; picking under the pointer
    /// is off meanwhile).
    pub(super) fn select_other_pointer(&mut self, position: DVec2, cx: &mut Context<Self>) {
        let Some(so) = &mut self.ui.select_other else { return };
        let in_zone = (position - so.at).length() <= SELECT_OTHER_ZONE_PX;
        if in_zone == so.in_zone {
            return;
        }
        so.moved_out |= !in_zone;
        so.in_zone = in_zone;
        self.hover = if in_zone { so.index.and_then(|i| so.hits.get(i).cloned()) } else { None };
        self.refresh_scene(false, cx);
        cx.notify();
    }

    /// A click on the graphics while Select other is open: in the click zone
    /// it takes the highlighted entry, elsewhere it only closes the list
    /// (Onshape onCanvasClick; the click selects nothing else).
    pub(super) fn select_other_click(&mut self, position: DVec2, cx: &mut Context<Self>) {
        let Some(so) = &self.ui.select_other else { return };
        if (position - so.at).length() <= SELECT_OTHER_ZONE_PX && so.index.is_some() {
            self.choose_select_other(None, cx);
        } else {
            self.ui.select_other = None;
            self.hover = None;
            self.refresh_scene(false, cx);
        }
        cx.notify();
    }

    /// Pick the highlighted (or clicked) Select other entry; Enter with
    /// nothing highlighted does nothing.
    pub(super) fn choose_select_other(&mut self, index: Option<usize>, cx: &mut Context<Self>) {
        let Some(i) = index.or(self.ui.select_other.as_ref().and_then(|so| so.index)) else { return };
        let Some(so) = self.ui.select_other.take() else { return };
        let Some(r) = so.hits.get(i).cloned() else { return };
        let kind = if matches!(r, Reference::Edge(_)) { PickKind::Edge } else { PickKind::Face };
        self.hover = None;
        self.pick(PickTarget { reference: r, kind, anchor: None }, cx);
    }

    /// The yellow point the list was opened for, while the pointer is out of
    /// the click zone (SELECT_OTHER_POINT_COLOR rgb(255,194,14), size 9).
    pub(super) fn select_other_overlay(&self) -> Option<noshape_render::Instance> {
        let so = self.ui.select_other.as_ref().filter(|so| !so.in_zone)?;
        let p = so.point?.as_vec3();
        let pts = noshape_render::PointData { points: vec![noshape_render::PointSprite { position: p.into(), color: [255, 194, 14, 255], pick_id: 0, size: 9.0, style: noshape_render::LineSegment::ON_TOP, _pad: 0 }] };
        Some(noshape_render::Instance::new(noshape_render::Drawable::Points(self.gpu.uploader().upload_points(&pts))).per_primitive_color().unpickable().ignore_clip().layer(20))
    }

    /// Live (sketch/230–231, dom/select-other-popup.txt): a 220 px dialog
    /// 100 px right of and 25 px above the pointer, kept in the graphics
    /// area: "Select other" (13 px / 500) and the red ✕; the list in a 1 px
    /// #d7d7d7 box (blue under the pointer), 24 px rows of 13.33 px names,
    /// every second row 35 % white, the highlighted one bold, a slim
    /// scrollbar while the pointer is on it; the "?" row.
    pub(super) fn render_select_other(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let so = self.ui.select_other.as_ref()?;
        let pal = p();
        let labeler = self.labeler(cx);
        let size = self.viewport.read(cx).size();
        let left = (so.at.x as f32 + SELECT_OTHER_OFFSET.0).min(size.x as f32 - SELECT_OTHER_SIZE.0).max(0.);
        let top = (so.at.y as f32 + SELECT_OTHER_OFFSET.1).min(size.y as f32 - SELECT_OTHER_SIZE.1).max(0.);
        let mut rows: Vec<AnyElement> = so
            .hits
            .iter()
            .enumerate()
            .map(|(i, r)| {
                let r2 = r.clone();
                h_flex()
                    .id(SharedString::from(format!("so-{i}")))
                    .flex_none()
                    .h(px(24.))
                    .pl(px(6.))
                    .pr(px(20.))
                    .when(i % 2 == 1, |d| d.bg(gpui_kit::rgba(0xffffff59)))
                    .child(div().truncate().text_size(px(13.33)).text_color(pal.text).when(so.index == Some(i), |d| d.font_weight(FontWeight::BOLD)).child(labeler(r)))
                    // Hovering a row pre-highlights its entity (Onshape
                    // onMouseEnteredSelection); the bold row stays.
                    .on_hover(cx.listener(move |this, h: &bool, _, cx| {
                        this.hover = if *h { Some(r2.clone()) } else { None };
                        this.refresh_scene(false, cx);
                        cx.notify();
                    }))
                    .on_mouse_down(MouseButton::Left, cx.listener(move |this, _: &MouseDownEvent, _, cx| {
                        cx.stop_propagation();
                        this.choose_select_other(Some(i), cx);
                    }))
                    .into_any_element()
            })
            .collect();
        if rows.is_empty() {
            rows.push(
                h_flex()
                    .id("so-none")
                    .h(px(24.))
                    .pl(px(6.))
                    .text_size(px(13.33))
                    .text_color(pal.text)
                    .child("(No selections found)")
                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _: &MouseDownEvent, _, cx| {
                        cx.stop_propagation();
                        this.ui.select_other = None;
                        cx.notify();
                    }))
                    .into_any_element(),
            );
        }
        // The slim scrollbar: 7 px, #333 at 40 %, while the pointer is on the list.
        let (offset, max) = (f32::from(so.scroll.offset().y), f32::from(so.scroll.max_offset().y));
        let view_h = 90.;
        let thumb = (max > 0.5).then(|| {
            let h = (view_h * view_h / (view_h + max)).max(20.);
            let y = (-offset / max) * (view_h - h);
            div().absolute().right(px(1.)).top(px(y)).w(px(7.)).h(px(h)).rounded(px(7.)).bg(gpui_kit::rgb(0x333333)).opacity(0.).group_hover("so-list", |d| d.opacity(0.4))
        });
        let close = cx.listener(|this, _: &ClickEvent, _, cx| {
            this.ui.select_other = None;
            this.hover = None;
            this.refresh_scene(false, cx);
            cx.notify();
        });
        Some(
            v_flex()
                .id("select-other")
                .occlude()
                .absolute()
                .left(px(left))
                .top(px(top))
                .w(px(SELECT_OTHER_SIZE.0))
                .bg(pal.dialog_bg)
                .shadow(vec![BoxShadow { color: hsla(0., 0., 0., 0.2), offset: point(px(0.), px(5.)), blur_radius: px(10.), spread_radius: px(0.), inset: false }])
                .text_color(pal.text)
                .child(
                    h_flex()
                        .h(px(29.))
                        .flex_none()
                        .bg(gpui_kit::rgb(0xffffff))
                        .border_b_1()
                        .border_color(pal.border)
                        .child(div().flex_1().pl(px(9.)).text_size(px(13.)).font_weight(FontWeight::MEDIUM).child("Select other"))
                        .child(div().id("so-close").size(px(28.)).flex().items_center().justify_center().child(crate::assets::icon("dialog-cancel", 24., cx)).on_click(close)),
                )
                .child(
                    div()
                        .id("so-list")
                        .group("so-list")
                        .relative()
                        .h(px(92.))
                        .flex_none()
                        .border_1()
                        .border_color(pal.border)
                        .rounded(px(2.))
                        .hover(|d| d.border_color(gpui_kit::rgb(0x1651b0)))
                        .child(v_flex().id("so-rows").size_full().overflow_y_scroll().track_scroll(&so.scroll).children(rows))
                        .children(thumb),
                )
                // Live: 6 px under the list, the help row: its "?" 14 px, 4 px from the right.
                .child(h_flex().h(px(28.)).flex_none().items_start().justify_end().pt(px(10.)).pr(px(4.)).child(crate::ui::glyph("help-dot", 14., crate::theme::help_dot(0xa0a0a0))))
                .into_any_element(),
        )
    }

    /// Dialog picks that the feature's preview consumed (the edges of a
    /// fillet or chamfer, removed faces): Onshape keeps drawing them in the
    /// selection color where they were (live dialogs/fillet-02…04: the
    /// sharp edges orange over the rounded preview). Taken from the model
    /// before the feature; faces by their boundary edges.
    pub(super) fn consumed_picks_overlay(&mut self, selection: &[Reference], accent: &[Reference]) -> Vec<noshape_render::Instance> {
        use noshape_render::{Drawable, Instance, LineData};
        let (Some(_), Some(before), Some(built)) = (&self.edit, self.ui.preview_before.clone(), &self.built) else {
            self.ui.consumed_lines = None;
            return Vec::new();
        };
        let missing = |refs: &[Reference]| -> Vec<Reference> { refs.iter().filter(|r| matches!(r, Reference::Edge(_) | Reference::Face(_)) && built.picks.ids_for(r).is_empty()).cloned().collect() };
        let (mut sel, mut acc) = (missing(selection), missing(accent));
        // Hovering its chip lights the pick the query-hover yellow-green
        // #f2ff80, consumed or not (live: qa4 round 5 s7, CHIP-HOVER).
        let hov: Vec<Reference> = self.ui.query_hover.iter().filter(|h| sel.contains(h) || acc.contains(h)).cloned().collect();
        sel.retain(|r| !hov.contains(r));
        acc.retain(|r| !hov.contains(r));
        let key = Arc::as_ptr(&before) as usize;
        if let Some((s, a, h, k, inst)) = &self.ui.consumed_lines
            && *s == sel
            && *a == acc
            && *h == hov
            && *k == key
        {
            return inst.iter().cloned().collect();
        }
        let mut lines = LineData::default();
        for (refs, color) in [(&acc, crate::theme::viewport::PREHIGHLIGHT), (&sel, crate::theme::viewport::SELECTION), (&hov, 0xf2ff80)] {
            let rgba = [(color >> 16) as u8, (color >> 8) as u8, color as u8, 255];
            for r in refs.iter() {
                let edges = match r {
                    Reference::Face(_) => crate::scene_builder::face_edges(&before, r),
                    r => vec![r.clone()],
                };
                for e in &edges {
                    for b in before.bodies.iter().filter(|b| !b.hidden) {
                        if let Some(i) = b.edge_refs.iter().position(|x| x == e)
                            && let Some(pl) = b.mesh.edges.get(i)
                        {
                            lines.push_polyline(pl, rgba, 0, 3.0, 0);
                        }
                    }
                }
            }
        }
        let inst = (!lines.segments.is_empty()).then(|| Instance::new(Drawable::Lines(self.gpu.uploader().upload_lines(&lines))).per_primitive_color().unpickable().layer(12));
        self.ui.consumed_lines = Some((sel, acc, hov, key, inst.clone()));
        inst.into_iter().collect()
    }

    /// Measure graphics (Onshape): a dotted line between the closest points
    /// of the measured entities, with its X (red), Y (green) and Z (blue)
    /// components.
    pub(super) fn measure_overlay(&mut self, cx: &App) -> Vec<noshape_render::Instance> {
        use noshape_render::{Drawable, Instance, LineData, LineSegment};
        // Live (behaviours "ui (10-23 a)"): no measure lines from the
        // selection alone; hovering a distance row of the open Measure panel
        // draws that distance as one black dashed line.
        let hover = self.ui.measure_hover.clone().filter(|_| self.ui.measure_pinned);
        if self.edit.is_some() || self.show_mass || self.selection.len() < 2 || hover.is_none() {
            return Vec::new();
        }
        let sel = self.selection.clone();
        let Some(ctx) = self.measure_context(&sel, cx) else { return Vec::new() };
        // Uploaded once per selection and row (refresh_scene runs on every hover).
        if let Some((s, h, inst)) = &self.ui.measure_lines
            && *s == sel
            && *h == hover
        {
            return inst.iter().cloned().collect();
        }
        let results = match &self.measure_cache {
            Some((s, r)) if *s == sel => r.clone(),
            _ => {
                let r = super::menus::studio_measure(&ctx, &sel);
                self.measure_cache = Some((sel, r.clone()));
                r
            }
        };
        let mut l = LineData::default();
        for (label, m) in results {
            if Some(&label) == hover.as_ref()
                && let noshape_model::regen::measure::Measurement::Distance { a, b, .. } = m
            {
                let f = |v: glam::DVec3| -> [f32; 3] { v.as_vec3().into() };
                l.segments.push(LineSegment { a: f(a), b: f(b), color: [20, 20, 20, 255], pick_id: 0, width: 2.0, style: LineSegment::DASHED | LineSegment::ON_TOP });
            }
        }
        // Out of the scene bounds (qa4 R1): a distance to a default plane can
        // end far out on it.
        let inst = (!l.segments.is_empty()).then(|| Instance::new(Drawable::Lines(self.gpu.uploader().upload_lines(&l))).per_primitive_color().unpickable().ignore_clip().no_bounds().layer(21));
        self.ui.measure_lines = Some((self.selection.clone(), hover, inst.clone()));
        inst.into_iter().collect()
    }

    /// Arc-length midpoint of an edge polyline (display mesh).
    /// Polyline of a part edge (world space).
    pub(super) fn edge_points(&self, r: &Reference) -> Option<Vec<glam::DVec3>> {
        let p3 = |a: [f32; 3]| glam::DVec3::new(a[0] as f64, a[1] as f64, a[2] as f64);
        // Sketch curves: their displayed polyline.
        if let Reference::SketchEntity { sketch, entity } = r {
            let c = self.display.sketches.iter().find(|s| s.feature == *sketch)?.curves.iter().find(|c| c.entity == *entity)?;
            return Some(c.points.iter().map(|p| p3(*p)).collect());
        }
        self.display.bodies.iter().find_map(|b| {
            let ei = b.edge_refs.iter().position(|x| x == r)?;
            Some(b.mesh.edges.get(ei)?.iter().map(|p| p3(*p)).collect())
        })
    }

    /// Direction of a straight part edge.
    pub(super) fn linear_edge_direction(&self, r: &Reference) -> Option<glam::DVec3> {
        let pts = self.edge_points(r)?;
        let (a, b) = (*pts.first()?, *pts.last()?);
        let d = (b - a).try_normalize()?;
        let straight = pts.iter().all(|p| (*p - a).cross(d).length() <= 1e-6 + 1e-4 * a.distance(b));
        straight.then_some(d)
    }

    /// Edge menu "Align view with vertical axis": roll the view (keeping
    /// its direction) so the edge points up the screen.
    pub(super) fn align_view_vertical(&mut self, r: &Reference, cx: &mut Context<Self>) {
        let Some(d) = self.linear_edge_direction(r) else { return };
        let cam = self.viewport.read(cx).camera;
        let fwd = cam.forward();
        let Some(mut up) = (d - fwd * d.dot(fwd)).try_normalize() else { return };
        if up.dot(cam.up()) < 0.0 {
            up = -up;
        }
        self.push_camera_history(cx);
        self.viewport.update(cx, |vp, cx| vp.animate_to_direction(fwd, up, cx));
    }

    /// "Section view…" on a part: a plane through its center, facing the view.
    pub(super) fn section_part(&mut self, b: &BodyId, window: &mut Window, cx: &mut Context<Self>) {
        let center = self.display.body(b).and_then(|b| b.bounds).map(|(lo, hi)| (lo + hi) * 0.5).unwrap_or_default();
        let n = -self.viewport.read(cx).camera.forward();
        self.ui.section_dialog = None;
        self.restore_section(&[[n.x, n.y, n.z, n.dot(center)]], window, cx);
        self.open_section_dialog(window, cx);
    }

    fn edge_midpoint(&self, r: &Reference) -> Option<glam::DVec3> {
        let p3 = |a: [f32; 3]| glam::DVec3::new(a[0] as f64, a[1] as f64, a[2] as f64);
        // Open sketch curves have a midpoint too (circles have their center instead).
        let sketch_curve = match r {
            Reference::SketchEntity { sketch, entity } => self.display.sketches.iter().find(|s| s.feature == *sketch).and_then(|s| s.curves.iter().find(|c| c.entity == *entity)).map(|c| c.points.iter().map(|p| p3(*p)).collect::<Vec<_>>()),
            _ => None,
        };
        let sketch_pl = sketch_curve.filter(|pl| pl.len() >= 2 && pl[0].distance(pl[pl.len() - 1]) > 1e-9);
        sketch_pl.into_iter().chain(self.display.bodies.iter().filter_map(|b| {
            let ei = b.edge_refs.iter().position(|x| x == r)?;
            Some(b.mesh.edges.get(ei)?.iter().map(|p| p3(*p)).collect::<Vec<glam::DVec3>>())
        })).find_map(|pl| {
            let total: f64 = pl.windows(2).map(|w| w[0].distance(w[1])).sum();
            let mut acc = 0.0;
            for w in pl.windows(2) {
                let l = w[0].distance(w[1]);
                if acc + l >= total * 0.5 && l > 0.0 {
                    return Some(w[0].lerp(w[1], (total * 0.5 - acc) / l));
                }
                acc += l;
            }
            pl.first().copied()
        })
    }

    /// Midpoint marker of the hovered edge or sketch curve (no sketch open;
    /// with a dialog open only when its field takes points, like Onshape).
    pub(super) fn midpoint_overlay(&self, cx: &App) -> Vec<noshape_render::Instance> {
        use noshape_render::{Drawable, Instance, LineSegment, PointData, PointSprite};
        let mut points = Vec::new();
        let sprite = |m: glam::DVec3, color: u32, size: f32, style: u32| PointSprite { position: m.as_vec3().into(), color: [(color >> 16) as u8, (color >> 8) as u8, color as u8, 255], pick_id: 0, size, style: LineSegment::ON_TOP | style, _pad: 0 };
        // Live (part-studio/qa4-r5-s9-6d, 6e): the midpoint target is an
        // 11 px disc with a thin darker ring 3–4 px out and a light centre:
        // orange while hovered (#f8c488, #c49a6a, #ffd8ac), yellow-orange once
        // selected (#fac012, #c4960b, #ffd65b).
        let marker = |points: &mut Vec<PointSprite>, m: glam::DVec3, [disc, ring, centre]: [u32; 3]| {
            points.push(sprite(m, disc, 11.0, 0));
            points.push(sprite(m, ring, 8.0, PointSprite::RING));
            points.push(sprite(m, centre, 5.0, 0));
        };
        for r in &self.selection {
            if let Reference::Implicit { on, inference: noshape_model::reference::Inference::Midpoint } = r
                && let Some(m) = self.edge_midpoint(on)
            {
                marker(&mut points, m, [0xfac012, 0xc4960b, 0xffd65b]);
            }
        }
        match &self.hover {
            // The hovered midpoint target ([`Self::hover_target`]).
            // Not for Select other's highlight: it isn't a pick under the
            // pointer (Onshape suppresses mouse-move picking while it's open).
            Some(Reference::Implicit { on, inference: noshape_model::reference::Inference::Midpoint }) if self.sketch.is_none() && self.ui.select_other.is_none() => {
                if let Some(m) = self.edge_midpoint(on) {
                    marker(&mut points, m, [0xf8c488, 0xc49a6a, 0xffd8ac]);
                }
            }
            // Live (qa4-r5-s9-6a/6b): hovering anywhere on the edge shows a
            // 9 px dark ring at its midpoint, the edge seen through it.
            Some(r @ (Reference::Edge(_) | Reference::SketchEntity { .. }))
                if self.active_field_filter(cx).is_none_or(field_takes_points) && (self.edit.is_none() || self.active_field_filter(cx).is_some()) && self.sketch.is_none() && self.ui.select_other.is_none() =>
            {
                if let Some(m) = self.edge_midpoint(r) {
                    points.push(sprite(m, 0x000000, 9.0, PointSprite::RING));
                }
            }
            _ => {}
        }
        if points.is_empty() {
            return Vec::new();
        }
        let up = self.gpu.uploader();
        vec![Instance::new(Drawable::Points(up.upload_points(&PointData { points }))).per_primitive_color().unpickable().ignore_clip().layer(22)]
    }

    /// The hover target (Onshape, part-studio/qa4-r5-s9-6c/6d): near a
    /// hovered edge's midpoint the midpoint itself is hovered (orange marker,
    /// the edge loses its highlight), within the radius a click picks it
    /// ([`Self::midpoint_pick`]); fields that take no points keep the edge.
    pub(super) fn hover_target(&self, cx: &App) -> Option<Reference> {
        let t = self.ui.hover_pick.clone()?;
        match self.ui.pointer {
            Some(p) if matches!(t.kind, PickKind::Edge | PickKind::SketchCurve) && self.active_field_filter(cx).is_none_or(field_takes_points) => Some(self.midpoint_pick(t, p, cx).reference),
            _ => Some(t.reference),
        }
    }

    /// An edge click within ~8 px of its midpoint selects the midpoint.
    /// (Only for fields that take points, [`field_takes_points`].)
    pub(super) fn midpoint_pick(&self, t: PickTarget, position: DVec2, cx: &App) -> PickTarget {
        if self.sketch.is_some() {
            return t;
        }
        let Some(m) = self.edge_midpoint(&t.reference) else { return t };
        let Some(s) = self.viewport.read(cx).project(m) else { return t };
        if !targets_midpoint(s, position, true) {
            return t;
        }
        let r = Reference::Implicit { on: Box::new(t.reference.clone()), inference: noshape_model::reference::Inference::Midpoint };
        PickTarget { reference: r, kind: PickKind::Vertex, anchor: Some(m) }
    }

    // ------------------------------------------------------------ S toolbar

    /// S: the shortcut toolbar at the cursor (feature tools; sketch tools
    /// in a sketch). In a sketch S re-opens it at the cursor (live
    /// sketch/38–39); elsewhere S again closes it. Esc closes it too.
    pub(super) fn toggle_mini_toolbar(&mut self, cx: &mut Context<Self>) {
        // Its tools may have changed in Preferences › Shortcut toolbars.
        self.ui.toolbar = super::toolbar_prefs::ToolbarPrefs::load();
        let at = self.ui.pointer.unwrap_or_else(|| self.viewport.read(cx).size() * 0.5);
        if self.sketch.is_some() || self.ui.mini_toolbar.take().is_none() {
            self.ui.mini_toolbar = Some(at);
        }
        cx.notify();
    }

    /// The S shortcut toolbar (`.os-mini-toolbar-panel`, the same panel as
    /// the assembly's: live dom/assembly-shortcut-toolbar.json, behaviours
    /// "S key", qa5 STB1): left = cursor x + 10, top = cursor y − 30; 30 px
    /// tools (20 px icons, radius 3, hover `#eaeaea`) at a 34 px pitch, five
    /// per row, rows 1 px apart, 4.5 px side padding (175 px wide); on
    /// rgba(255,255,255,.95) with no border, square corners, a 0 5 10 shadow
    /// and no tooltips.
    pub(super) fn render_mini_toolbar(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let at = self.ui.mini_toolbar?;
        let btn = |id: SharedString, icon: SharedString, cx: &App| {
            div()
                .id(id)
                .size(px(30.))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(3.))
                .cursor_pointer()
                .hover(|d| d.bg(rgb(0xeaeaea)))
                .child(crate::assets::icon(&icon, 20., cx))
        };
        let mut buttons: Vec<AnyElement> = Vec::new();
        if self.sketch.is_some() {
            for tool in super::shortcut_cmds::sketch_shortcut_toolbar(&self.ui.toolbar) {
                buttons.push(
                    btn(SharedString::from(format!("mini-{}", tool.icon())), tool.icon().into(), cx)
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, _: &MouseDownEvent, window, cx| {
                                cx.stop_propagation();
                                this.ui.mini_toolbar = None;
                                window.focus(&this.focus, cx);
                                this.activate_tool(tool, window, cx);
                                cx.notify();
                            }),
                        )
                        .into_any_element(),
                );
            }
        } else {
            for id in self.ui.toolbar.shortcut_tools() {
                let Some(t) = noshape_model::features::get(&id) else { continue };
                let id: &'static str = t.id;
                buttons.push(
                    btn(SharedString::from(format!("mini-{id}")), feature_icon(id), cx)
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, _: &MouseDownEvent, window, cx| {
                                cx.stop_propagation();
                                this.ui.mini_toolbar = None;
                                this.start_feature(id, window, cx);
                            }),
                        )
                        .into_any_element(),
                );
            }
        }
        let mut rows = v_flex().gap(px(1.));
        let mut buttons = buttons.into_iter().peekable();
        while buttons.peek().is_some() {
            rows = rows.child(h_flex().gap(px(4.)).children(buttons.by_ref().take(5)));
        }
        Some(
            div()
                .id("mini-toolbar")
                .occlude()
                .absolute()
                .left(px(at.x as f32 + 10.))
                .top(px((at.y as f32 - 30.).max(0.)))
                .px(px(4.5))
                .bg(hsla(0., 0., 1., 0.95))
                .shadow(vec![BoxShadow { color: hsla(0., 0., 0., 0.2), offset: point(px(0.), px(5.)), blur_radius: px(10.), spread_radius: px(0.), inset: false }])
                .child(rows)
                .into_any_element(),
        )
    }

    // ------------------------------------------------------------ feature list

    /// Search box at the top of the feature list (created on first render).
    pub(super) fn ensure_filter_input(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.ui.filter.is_some() {
            return;
        }
        let st = cx.new(|cx| InputState::new(window, cx).select_on_focus(false).revert_on_escape(false).placeholder("Filter by name or type"));
        let sub = cx.subscribe_in(&st, window, |this: &mut Self, st, ev: &InputEvent, window, cx| match ev {
            InputEvent::Change => {
                // No `cx.notify()`: the field's own notification already
                // re-renders the Part Studio (its ancestor) with the new query,
                // and of its cached slices only the rows need it.
                this.ui.filter_query = st.read(cx).value().to_string();
                this.filter_rows_changed(cx);
            }
            InputEvent::PressEnter { .. } => window.focus(&this.focus, cx),
            _ => {}
        });
        self.ui.filter = Some((st, sub));
    }

    pub(super) fn set_filter(&mut self, text: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.ui.filter_query = text.to_string();
        if let Some((st, _)) = &self.ui.filter {
            st.update(cx, |s, cx| s.set_value(text.to_string(), window, cx));
        }
        cx.notify();
    }

    /// Feature list click with Onshape's additive / Shift-range rules.
    pub(super) fn click_feature(&mut self, id: FeatureId, shift: bool, cx: &mut Context<Self>) {
        let order: Vec<FeatureId> = self.studio(cx).map(|s| s.features.iter().map(|f| f.id).collect()).unwrap_or_default();
        self.selected_features = interaction::list_click(&order, &self.selected_features, self.ui.anchor.as_ref(), &id, shift);
        self.ui.selected_folder = None;
        self.ui.fresh_folder = None;
        self.ui.anchor = Some(id);
        self.refresh_scene(false, cx);
        cx.notify();
    }

    /// Drop of dragged features onto a folder row (live qa4 r5 s8 (6)): they
    /// become its first children, moved to the folder's place in the list
    /// (an empty folder's: where it is listed). One undo step; the same
    /// order check as a drop between rows.
    pub(super) fn drop_into_folder(&mut self, dragged: FeatureId, folder: u32, cx: &mut Context<Self>) {
        let Some(studio) = self.studio(cx) else { return };
        let moving: Vec<FeatureId> = if self.selected_features.contains(&dragged) { self.selected_features.clone() } else { vec![dragged] };
        let first = studio.folder_features(folder).into_iter().find(|f| !moving.contains(f));
        let empty_after = studio.folder(folder).and_then(|f| f.after);
        let (to, after) = match first {
            Some(f) => (studio.index_of(f), false),
            None => match empty_after {
                Some(a) => (studio.index_of(a), true),
                None => (Some(0), false),
            },
        };
        let current: Vec<FeatureId> = studio.features.iter().map(|f| f.id).collect();
        let order = to.and_then(|to| interaction::reordered_at(&studio.features, &moving, to, after)).unwrap_or(current);
        if let Err(msg) = interaction::check_order(&studio.features, &order) {
            self.message = Some((FeatureStatus::Error, msg));
            cx.notify();
            return;
        }
        let n = moving.len();
        self.doc.update(cx, |doc, cx| {
            doc.edit_studio(self.tab, if n == 1 { "Reorder feature" } else { "Reorder features" }, cx, |ps| {
                interaction::apply_order(ps, &order);
                for id in &moving {
                    if let Some(f) = ps.feature_mut(*id) {
                        f.folder = Some(folder);
                    }
                }
                // A folder with features is placed by them.
                if let Some(fd) = ps.folder_mut(folder) {
                    fd.after = None;
                }
            });
        });
    }

    /// Drop of dragged features onto the row at `to`: moves every selected
    /// feature when the dragged one is selected. Live (qa4 round 5 v3 s8):
    /// a move above a feature's dependencies is done all the same; the
    /// feature then fails on its missing selection.
    pub(super) fn drop_features(&mut self, dragged: FeatureId, to: usize, after: bool, cx: &mut Context<Self>) {
        let Some(studio) = self.studio(cx) else { return };
        let moving: Vec<FeatureId> = if self.selected_features.contains(&dragged) { self.selected_features.clone() } else { vec![dragged] };
        let Some(order) = interaction::reordered_at(&studio.features, &moving, to, after) else { return };
        let n = moving.len();
        self.doc.update(cx, |doc, cx| {
            doc.edit_studio(self.tab, if n == 1 { "Reorder feature" } else { "Reorder features" }, cx, |ps| interaction::apply_order(ps, &order));
        });
    }

    /// "Show dependencies": parents above the arrow, children below.
    pub(super) fn render_dependencies(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if let Some(r) = self.ui.plane_dependencies.clone() {
            return self.render_plane_dependencies(&r, cx);
        }
        let id = self.ui.dependencies?;
        let studio = self.studio(cx)?;
        let idx = studio.index_of(id)?;
        let name = studio.features[idx].name.clone();
        let names = |ids: Vec<FeatureId>| -> Vec<(FeatureId, String, String)> { ids.into_iter().filter_map(|i| studio.feature(i).map(|f| (i, f.name.clone(), f.type_id.clone()))).collect() };
        let parents = names(interaction::feature_dependencies(&studio.features, idx));
        let children = names(interaction::feature_dependents(&studio.features, idx));
        let this_row = (id, name.clone(), studio.features[idx].type_id.clone());
        // Under-defined sketches carry the feature list's blue badge here too (live part-studio/685).
        let underdefined: std::collections::HashSet<FeatureId> =
            parents.iter().chain(children.iter()).chain(std::iter::once(&this_row)).map(|r| r.0).filter(|f| self.display.feature_state(*f).is_some_and(|s| s.underdefined)).collect();
        let pal = p();
        // Onshape (live): 300 px panel at the dialog position; 26 px rows
        // (15 px feature icon, 12 px name); the parents, a ▲ separator
        // ("Features used by …"), the feature, a ▼ separator ("Features
        // using …"), the children. Hovering a row highlights its geometry.
        let row = |(fid, n, ty): (FeatureId, String, String), cx: &mut Context<Self>| {
            h_flex()
                .id(SharedString::from(format!("dep-{fid}")))
                .h(px(26.))
                .bg(with_alpha(gpui_kit::rgb(0x000000), 0.02))
                .cursor_pointer()
                .hover(|d| d.bg(pal.row_hover))
                // Live (part-studio/685): the feature list's full-colour icons (15 px ink, .8)
                // and its under-defined badge (12 px, 8 px in from the icon's top-left).
                .child(
                    div().w(px(25.)).flex().justify_center().child(
                        div()
                            .relative()
                            .child(div().opacity(0.8).child(crate::assets::icon(&feature_icon(&ty), crate::ui::LIST_ICON, cx)))
                            .when(underdefined.contains(&fid), |d| d.child(div().absolute().left(px(8.)).top(px(8.)).child(crate::assets::icon("list-underdefined-overlay", 12., cx)))),
                    ),
                )
                .child(div().px(px(6.)).text_size(px(12.)).text_color(pal.text).child(n))
                .on_hover(cx.listener(move |this, h: &bool, _, cx| {
                    this.ui.list_hover = h.then_some(fid);
                    this.refresh_scene(false, cx);
                }))
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.selected_features = vec![fid];
                    this.refresh_scene(false, cx);
                    cx.notify();
                }))
                .into_any_element()
        };
        let separator = |idn: &'static str, up: bool, tip: String| {
            div()
                .id(idn)
                .h(px(23.))
                .flex()
                .items_center()
                .justify_center()
                .bg(gpui_kit::rgb(0xeeeeee))
                .border_t_1()
                .border_b_1()
                .border_color(gpui_kit::rgb(0xd7d7d7))
                .text_size(px(10.))
                .text_color(with_alpha(pal.text, 0.5))
                .child(if up { "▲" } else { "▼" })
                .tooltip(crate::ui::tooltip(tip))
        };
        let parent_rows: Vec<AnyElement> = parents.into_iter().map(|x| row(x, cx)).collect();
        let child_rows: Vec<AnyElement> = children.into_iter().map(|x| row(x, cx)).collect();
        let me = row(this_row, cx);
        Some(
            v_flex()
                .id("dependencies")
                .occlude()
                .absolute()
                .top(px(0.))
                .left(px(0.))
                .w(px(300.))
                .bg(pal.dialog_bg)
                .shadow(vec![BoxShadow { color: hsla(0., 0., 0., 0.2), offset: point(px(0.), px(5.)), blur_radius: px(10.), spread_radius: px(0.), inset: false }])
                .child(
                    h_flex()
                        .h(px(29.))
                        .px(px(6.))
                        .justify_between()
                        .border_b_1()
                        .border_color(pal.border)
                        .child(div().text_size(px(13.)).font_weight(FontWeight::MEDIUM).text_color(pal.text).child(format!("Dependencies of {name}")))
                        .child(div().id("dep-close").px_1().cursor_pointer().child(crate::ui::glyph("close", 21., gpui_kit::rgb(0x333333))).opacity(0.86).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.ui.dependencies = None;
                            this.ui.list_hover = None;
                            this.refresh_scene(false, cx);
                            cx.notify();
                        }))),
                )
                .children(parent_rows)
                .child(separator("dep-up", true, format!("Features used by {name}")))
                .child(me)
                .child(separator("dep-down", false, format!("Features using {name}")))
                .children(child_rows)
                .into_any_element(),
        )
    }

    /// "Dependencies of Top": the feature rows that use a default plane or the origin.
    fn render_plane_dependencies(&mut self, r: &Reference, cx: &mut Context<Self>) -> Option<AnyElement> {
        let name = self.default_name(r, cx);
        let studio = self.studio(cx)?;
        let users: Vec<(FeatureId, String, String)> = interaction::features_using(&studio.features, r).into_iter().filter_map(|i| studio.feature(i).map(|f| (i, f.name.clone(), f.type_id.clone()))).collect();
        let pal = p();
        let rows: Vec<AnyElement> = users
            .into_iter()
            .map(|(fid, n, ty)| {
                h_flex()
                    .id(SharedString::from(format!("pdep-{fid}")))
                    .h(px(26.))
                    .bg(with_alpha(gpui_kit::rgb(0x000000), 0.02))
                    .hover(|d| d.bg(pal.row_hover))
                    // Live (part-studio/685): the feature list's full-colour icons (15 px ink, .8).
                .child(div().w(px(25.)).flex().justify_center().opacity(0.8).child(crate::assets::icon(&feature_icon(&ty), crate::ui::LIST_ICON, cx)))
                    .child(div().px(px(6.)).text_size(px(12.)).text_color(pal.text).child(n))
                    .on_hover(cx.listener(move |this, h: &bool, _, cx| {
                        this.ui.list_hover = h.then_some(fid);
                        this.refresh_scene(false, cx);
                    }))
                    .into_any_element()
            })
            .collect();
        let me = h_flex().h(px(26.)).child(div().w(px(25.)).flex().justify_center().child(crate::assets::icon(if matches!(r, Reference::Origin) { "origin" } else { "plane" }, 15., cx))).child(div().px(px(6.)).text_size(px(12.)).text_color(pal.text).child(crate::ui::user_text(name.clone())));
        Some(
            v_flex()
                .id("plane-dependencies")
                .occlude()
                .absolute()
                .top(px(0.))
                .left(px(0.))
                .w(px(300.))
                .bg(pal.dialog_bg)
                .shadow(vec![BoxShadow { color: hsla(0., 0., 0., 0.2), offset: point(px(0.), px(5.)), blur_radius: px(10.), spread_radius: px(0.), inset: false }])
                .child(
                    h_flex()
                        .h(px(29.))
                        .px(px(6.))
                        .justify_between()
                        .border_b_1()
                        .border_color(pal.border)
                        .child(div().text_size(px(13.)).font_weight(FontWeight::MEDIUM).text_color(pal.text).child(format!("Dependencies of {name}")))
                        .child(div().id("pdep-close").px_1().cursor_pointer().child(crate::ui::glyph("close", 21., gpui_kit::rgb(0x333333))).opacity(0.86).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.ui.plane_dependencies = None;
                            this.ui.list_hover = None;
                            this.refresh_scene(false, cx);
                            cx.notify();
                        }))),
                )
                .child(me)
                .child(div().h(px(23.)).flex().items_center().justify_center().bg(gpui_kit::rgb(0xeeeeee)).border_t_1().border_b_1().border_color(gpui_kit::rgb(0xd7d7d7)).text_size(px(10.)).text_color(with_alpha(pal.text, 0.5)).child("▼"))
                .children(rows)
                .into_any_element(),
        )
    }

    /// K: show or hide all mate connectors.
    pub(super) fn toggle_mate_connectors(&mut self, cx: &mut Context<Self>) {
        let ids: Vec<(FeatureId, bool)> = self.studio(cx).map(|s| s.features.iter().filter(|f| f.type_id == "mateConnector").map(|f| (f.id, f.hidden)).collect()).unwrap_or_default();
        if ids.is_empty() {
            return;
        }
        let hide = ids.iter().any(|(_, h)| !h);
        let targets: Vec<Reference> = ids.into_iter().map(|(id, _)| Reference::Feature(id)).collect();
        self.set_visibility(&targets, hide, cx);
    }

    /// After the sketch editor opened: orient normal to it if requested.
    pub(super) fn orient_for_sketch(&mut self, cx: &mut Context<Self>) {
        if self.ui.orient_pending && self.sketch.is_some() {
            self.ui.orient_pending = false;
            self.ui.oriented = true;
            self.view_normal(cx);
        }
    }

    /// When a sketch closes: return to the view before orienting.
    pub(super) fn restore_after_sketch(&mut self, cx: &mut Context<Self>) {
        self.ui.orient_pending = false;
        if std::mem::take(&mut self.ui.oriented) {
            self.previous_view(cx);
        }
    }

    // ------------------------------------------------------------ remembered values

    /// Store the REMEMBER_PREVIOUS_VALUE parameters of an accepted feature.
    pub(super) fn remember_values(&mut self, f: &noshape_model::partstudio::Feature) {
        let Some(ty) = f.feature_type() else { return };
        for spec in ty.params.iter().filter(|s| s.hints.has(noshape_model::params::Hints::REMEMBER_PREVIOUS_VALUE)) {
            if let Some(v) = f.params.get(spec.id) {
                self.ui.remembered.insert((f.type_id.clone(), spec.id), v.clone());
            }
        }
    }

    /// Defaults of a new feature from the last accepted one of its type.
    pub(super) fn apply_remembered(&self, f: &mut noshape_model::partstudio::Feature) {
        for ((ty, id), v) in &self.ui.remembered {
            if *ty == f.type_id {
                f.params.set(id, v.clone());
            }
        }
    }

    // ------------------------------------------------------------ dialogs

    /// Onshape's automatic choices in a new Extrude/Revolve: the operation
    /// becomes Add when the profile sits on a part (with that part as the
    /// merge scope), and choosing Remove/Intersect points the feature into
    /// the part. Fields the user set are left alone.
    pub(super) fn auto_operation(&mut self, cx: &mut Context<Self>) {
        let Some(e) = &self.edit else { return };
        // Live L49/L50: a tapped hole's Depth, Tapped depth and Tap
        // clearance stay linked, new or edited.
        if e.dialog.read(cx).feature.type_id == "hole" {
            let dialog = e.dialog.clone();
            let (id, now) = {
                let d = dialog.read(cx);
                (d.feature.id, d.feature.params.clone())
            };
            let units = self.doc.read(cx).session.doc.settings.units;
            if let Some((pid, prev)) = self.ui.hole_prev.take()
                && pid == id
            {
                let changes = noshape_model::features::hole_tables::link_tapped_depths(&prev, &now, &units);
                if !changes.is_empty() {
                    dialog.update(cx, |d, cx| {
                        for (k, v) in changes {
                            d.set_value_auto(k, v, cx);
                        }
                    });
                }
            }
            let now = dialog.read(cx).feature.params.clone();
            self.ui.hole_prev = Some((id, now));
        }
        let Some(e) = &self.edit else { return };
        // Std's editing logic runs on edits of existing features too.
        let ty = e.dialog.read(cx).feature.type_id.clone();
        if noshape_model::custom::std_feature::has_editing_logic(&ty) {
            let dialog = e.dialog.clone();
            self.std_edit_logic(&dialog, cx);
            return;
        }
        if !e.is_new {
            return;
        }
        let dialog = e.dialog.clone();
        if matches!(ty.as_str(), "deleteFace" | "replaceFace" | "rib" | "mateConnector" | "moveFace" | "draft") {
            self.direct_edit_logic(&dialog, cx);
            return;
        }
        let changes = {
            let Some(studio) = self.studio(cx) else { return };
            let d = dialog.read(cx);
            noshape_ops::studio::auto_operation(studio, &self.display, &d.feature, &|k| d.user_set.contains(k))
        };
        if changes.is_empty() {
            return;
        }
        dialog.update(cx, |d, cx| {
            for (k, v) in changes {
                d.set_value_auto(k, v, cx);
            }
        });
    }
}

impl PartStudioView {
    /// The std "Editing Logic Function" of a feature std runs (Tab: picking
    /// the profile fills "Flange to merge" and the subtraction offset, live
    /// qa5 4.2), on the model before the feature; fields the user set are
    /// `specifiedParameters`.
    /// It runs in the background; a result for values the user has changed
    /// since is dropped (the run for the new values follows). Without the
    /// model before the feature yet, it runs once that arrives.
    pub(super) fn std_edit_logic(&mut self, dialog: &Entity<crate::feature_dialog::FeatureDialog>, cx: &mut Context<Self>) {
        let Some(ctx) = self.ui.before_context.clone() else { return };
        let (ty, id, name, params, user_set) = {
            let d = dialog.read(cx);
            (d.feature.type_id.clone(), d.feature.id, d.feature.name.clone(), d.feature.params.clone(), d.user_set.iter().cloned().collect::<Vec<String>>())
        };
        let old = self.ui.std_edit_old.replace(params.clone()).unwrap_or_default();
        if old == params {
            return;
        }
        let creating = self.edit.as_ref().is_some_and(|e| e.is_new);
        let units = self.doc.read(cx).session.doc.settings.units;
        let dialog = dialog.clone();
        let sent = params.clone();
        self.ui.std_edit_task = Some(cx.spawn(async move |this, cx| {
            let changed = cx.background_spawn(async move { noshape_model::custom::std_feature::editing_logic_in((*ctx).clone(), &ty, id, &name, &units, &old, &sent, &user_set, creating) }).await;
            let Some(changed) = changed else { return };
            let _ = this.update(cx, |this: &mut Self, cx| {
                if dialog.read(cx).feature.params != params {
                    return;
                }
                let changes: Vec<(String, ParamValue)> = changed.0.into_iter().filter(|(k, v)| params.get(k) != Some(v)).collect();
                if changes.is_empty() {
                    return;
                }
                let mut next = params;
                for (k, v) in &changes {
                    next.set(k, v.clone());
                }
                this.ui.std_edit_old = Some(next);
                dialog.update(cx, |d, cx| {
                    for (k, v) in changes {
                        d.set_value_auto(&k, v, cx);
                    }
                });
            });
        }));
    }

    /// Onshape's deleteFaceEditLogic / replaceFaceEditLogic for a new
    /// feature, leaving fields the user set alone: Delete face of surface
    /// faces switches to Leave open (live L10); Replace face ticks Flip
    /// alignment when the replace-with face points against the replaced one
    /// (live L8).
    fn direct_edit_logic(&mut self, dialog: &Entity<crate::feature_dialog::FeatureDialog>, cx: &mut Context<Self>) {
        let (ty, params, user_set) = {
            let d = dialog.read(cx);
            (d.feature.type_id.clone(), d.feature.params.clone(), d.user_set.clone())
        };
        let ctx = self.ui.before_context.as_ref().or(self.context.as_ref());
        let mut changes: Vec<(&'static str, ParamValue)> = Vec::new();
        match ty.as_str() {
            "deleteFace" if !user_set.contains("healType") => {
                let faces = params.query("deleteFaces");
                let surface = |r: &Reference| self.body_of(r).and_then(|b| ctx?.body(&b).map(|b| b.kind == noshape_model::display::BodyKind::Surface)).unwrap_or(false);
                if !faces.is_empty() && faces.iter().all(surface) && !params.is("healType", "VOID") {
                    changes.push(("healType", ParamValue::Enum("VOID".into())));
                }
            }
            // moveFaceEditingLogic: Translate moves along the first planar
            // face moved.
            "moveFace" if params.is("moveFaceType", "TRANSLATE") && !user_set.contains("direction") => {
                let planar = params.query("moveFaces").iter().find(|r| ctx.is_some_and(|c| noshape_model::regen::measure::resolve_shapes(c, r).first().is_some_and(|f| matches!(f.face_surface(), Ok(noshape_kernel::Surface::Plane(_)))))).cloned();
                if let Some(f) = planar
                    && params.query("direction").first() != Some(&f)
                {
                    changes.push(("direction", ParamValue::Query(vec![f])));
                }
            }
            // draftEditLogic (flipAlongPull): the first parting edge picked
            // drafts the steeper of its faces.
            "draft" if params.is("draftFeatureType", "PARTING_LINE") && params.is("partingLineSides", "ONE_SIDED") && params.query("partingEdges").len() == 1 && !user_set.contains("alongPull") => {
                let units = self.doc.read(cx).session.doc.settings.units;
                if let Some(c) = ctx
                    && let Some(want) = noshape_model::interaction::draft_switch_face(c, &params, &units)
                    && params.bool("alongPull") != want
                {
                    changes.push(("alongPull", ParamValue::Bool(want)));
                }
            }
            // Live L22: a new Rib's Parts holds the studio's parts.
            "rib" if !user_set.contains("parts") && params.query("parts").is_empty() => {
                let solids: Vec<Reference> = ctx.map(|c| c.bodies.iter().filter(|b| b.kind == noshape_model::display::BodyKind::Solid).map(|b| Reference::Body(b.id.clone())).collect()).unwrap_or_default();
                if !solids.is_empty() {
                    changes.push(("parts", ParamValue::Query(solids)));
                }
            }
            // connectorEditLogic (live L37): the owner is the origin entity's
            // part, else the only part; attached to a picked face, else to
            // the owner.
            "mateConnector" if !user_set.contains("ownerPart") && params.query("ownerPart").is_empty() => {
                let origin = params.query("originQuery").first().cloned();
                let parts: Vec<BodyId> = ctx.map(|c| c.bodies.iter().map(|b| b.id.clone()).collect()).unwrap_or_default();
                let owner = origin.as_ref().and_then(|r| self.body_of(r)).or_else(|| (parts.len() == 1).then(|| parts[0].clone()));
                if let Some(o) = owner {
                    changes.push(("ownerPart", ParamValue::Query(vec![Reference::Body(o)])));
                    if !user_set.contains("attachmentOption") {
                        match &origin {
                            Some(r @ Reference::Face(_)) => {
                                changes.push(("attachmentOption", ParamValue::Enum("TO_SELECTION".into())));
                                changes.push(("attachTo", ParamValue::Query(vec![r.clone()])));
                            }
                            _ => changes.push(("attachmentOption", ParamValue::Enum("TO_OWNER".into()))),
                        }
                    }
                }
            }
            "replaceFace" if !user_set.contains("oppositeSense") => {
                let normal = |r: &Reference| ctx.and_then(|c| noshape_model::select::face_anchor(c, r)).map(|x| x.1).or_else(|| self.frame_of_ref(r).map(|f| f.normal));
                if let (Some(a), Some(b)) = (params.query("replaceFaces").first().and_then(normal), params.query("templateFace").first().and_then(normal)) {
                    let want = a.dot(b) < 0.0;
                    if params.bool("oppositeSense") != want {
                        changes.push(("oppositeSense", ParamValue::Bool(want)));
                    }
                }
            }
            _ => {}
        }
        if changes.is_empty() {
            return;
        }
        dialog.update(cx, |d, cx| {
            for (k, v) in changes {
                d.set_value_auto(k, v, cx);
            }
        });
    }

    /// A new Hole's Merge scope is the parts its holes reach
    /// (`FeatureState::touched`), and follows a deeper or shallower hole,
    /// until the user sets it (live L57: Blind 5 into the top of two stacked
    /// parts gives "Part 2", Depth 15 "Part 1, Part 2").
    pub(super) fn hole_scope_logic(&mut self, cx: &mut Context<Self>) {
        let Some(e) = &self.edit else { return };
        if !e.is_new {
            return;
        }
        let dialog = e.dialog.clone();
        let id = dialog.read(cx).feature.id;
        let Some((touched, behind)) = self.display.feature_state(id).filter(|s| s.status != FeatureStatus::RolledBack).map(|s| (s.touched.clone(), s.touched_behind.clone())) else { return };
        let changes = {
            let d = dialog.read(cx);
            noshape_ops::studio::hole_scope_logic(&d.feature, &|k| d.user_set.contains(k), &touched, &behind)
        };
        if changes.is_empty() {
            return;
        }
        dialog.update(cx, |d, cx| {
            for (k, v) in changes {
                d.set_value_auto(k, v, cx);
            }
        });
    }

    /// Onshape's booleanStepEditLogic (booleanHeuristics.fs) for a new
    /// Extrude / Revolve / Sweep / Loft, from the parts its tool touches in
    /// the preview (`FeatureState::touched`): one part → Add into it; none
    /// or several → New with an empty scope. Nothing once the user set the
    /// scope or "Merge with all"; a user-picked operation keeps it and only
    /// an empty scope is filled (live: an Add over three parts starts with
    /// an empty scope and fails "No merge scope selected.", qa5 BOOL4; a
    /// Remove over Part 1 gets "Part 1", part-studio/234).
    pub(super) fn boolean_edit_logic(&mut self, cx: &mut Context<Self>) {
        let Some(e) = &self.edit else { return };
        if !e.is_new {
            return;
        }
        let dialog = e.dialog.clone();
        let id = dialog.read(cx).feature.id;
        let Some(state) = self.display.feature_state(id).filter(|s| s.status != FeatureStatus::RolledBack) else { return };
        // Shared with the agent API: std booleanStepEditLogic, which leaves
        // the operation and scope alone when the tool can't be built.
        let changes = {
            let d = dialog.read(cx);
            noshape_ops::studio::boolean_choices(&d.feature, &|k| d.user_set.contains(k), state)
        };
        if changes.is_empty() {
            return;
        }
        dialog.update(cx, |d, cx| {
            for (k, v) in changes {
                d.set_value_auto(k, v, cx);
            }
        });
    }
}

/// The roll items of a feature row's menu, (Roll to here, Roll to end), for
/// row `i` with the rollback bar before row `rollback_at` (live "qa4 round 5
/// s2 rollback follow-up"): Roll to here unless the bar is right below the
/// feature, Roll to end while the bar isn't at the end.
pub(super) fn roll_items(i: usize, rollback_at: usize, n_features: usize) -> (bool, bool) {
    (rollback_at != i + 1, rollback_at < n_features)
}

/// The feature list's drag autoscroll (live "psb (10-24 k)"): px/s for a
/// pointer at `y` over a list spanning `top..bottom` (window px), positive
/// scrolling down. Only the vertical distance counts, also with the pointer
/// beside the panel. Live's measured speeds (± 15 %), by the distance inside
/// the edge (negative = past it), mean of the top and bottom edges.
pub(super) fn list_autoscroll_speed(y: f32, top: f32, bottom: f32) -> f32 {
    const TABLE: [(f32, f32); 12] = [
        (-192.0, 600.0),
        (-40.0, 370.0),
        (-10.0, 210.0),
        (0.0, 206.0),
        (2.0, 205.0),
        (10.0, 202.0),
        (20.0, 193.0),
        (30.0, 191.0),
        (50.0, 175.0),
        (58.0, 150.0),
        (70.0, 33.0),
        (75.0, 0.0),
    ];
    let speed = |d: f32| -> f32 {
        if d >= TABLE[TABLE.len() - 1].0 {
            return 0.0;
        }
        if d <= TABLE[0].0 {
            // On past the farthest sample at the last segment's slope.
            let ((d0, v0), (d1, v1)) = (TABLE[0], TABLE[1]);
            return v0 + (d - d0) * (v1 - v0) / (d1 - d0);
        }
        let k = TABLE.windows(2).position(|w| d >= w[0].0 && d <= w[1].0).unwrap_or(0);
        let ((d0, v0), (d1, v1)) = (TABLE[k], TABLE[k + 1]);
        v0 + (d - d0) * (v1 - v0) / (d1 - d0)
    };
    if bottom - top <= 0.0 {
        return 0.0;
    }
    // The nearer edge decides (a list shorter than two zones: no overlap
    // issue, the closer edge wins).
    let (to_top, to_bottom) = (y - top, bottom - y);
    if to_bottom <= to_top { speed(to_bottom) } else { -speed(to_top) }
}

/// How the parts left out look at transparency slider position `t` (live
/// pixel samples, behaviours.md "Part Studio Isolate…"): under 10 % light
/// opaque ghosts (back edges hidden); from 10 % see-through, most opaque
/// there and fading linearly (≈ 166 → 248 grey on white by 90 %); hidden
/// from ~95 %.
#[derive(Clone, Copy, PartialEq)]
pub(super) enum IsoLook {
    Ghost,
    SeeThrough(f32),
    Hidden,
}

pub(super) fn iso_look(t: f32) -> IsoLook {
    if t < 0.1 {
        IsoLook::Ghost
    } else if t >= 0.95 {
        IsoLook::Hidden
    } else {
        IsoLook::SeeThrough((0.66 - (t - 0.1) * 0.7625).max(0.03))
    }
}

/// The pick radius around an edge midpoint, in screen px (hover and click).
pub(super) const MIDPOINT_PICK_PX: f64 = 8.0;

/// Whether the pointer at `pointer` targets the edge midpoint drawn at `mid`
/// rather than the edge (Onshape): within [`MIDPOINT_PICK_PX`], when the
/// active field (if any) takes points.
pub(super) fn targets_midpoint(mid: DVec2, pointer: DVec2, takes_points: bool) -> bool {
    takes_points && mid.distance(pointer) <= MIDPOINT_PICK_PX
}

/// A field takes an edge's midpoint (Onshape's implicit point): it takes
/// points, or mate connectors without taking edges. A Direction field takes
/// mate connectors (their Z axis) and edges: a click at an edge's midpoint
/// picks the edge (qa4 F11, live).
pub(super) fn field_takes_points(f: QueryFilter) -> bool {
    f.intersects(QueryFilter(QueryFilter::VERTEX.0 | QueryFilter::SKETCH_POINT.0 | QueryFilter::POINT_BODY.0)) || (f.intersects(QueryFilter::MATE_CONNECTOR) && !f.intersects(QueryFilter(QueryFilter::ANY_EDGE.0 | QueryFilter::LINEAR_EDGE.0)))
}

#[cfg(test)]
mod midpoint_target_tests {
    use super::{MIDPOINT_PICK_PX, targets_midpoint};
    use glam::DVec2;

    #[test]
    fn hovering_near_an_edge_midpoint_targets_the_midpoint() {
        let m = DVec2::new(300.0, 200.0);
        assert!(targets_midpoint(m, m, true));
        assert!(targets_midpoint(m, m + DVec2::new(MIDPOINT_PICK_PX - 1.0, 0.0), true));
        assert!(targets_midpoint(m, m + DVec2::new(3.0, -4.0), true));
        // Farther along the edge: the edge stays the target.
        assert!(!targets_midpoint(m, m + DVec2::new(MIDPOINT_PICK_PX + 2.0, 0.0), true));
        // A field that takes no points (Fillet's edges) keeps the edge.
        assert!(!targets_midpoint(m, m, false));
    }
}

#[cfg(test)]
mod field_point_tests {
    use super::field_takes_points;
    use noshape_model::params::QueryFilter;

    #[test]
    fn face_fields_take_no_edges() {
        use crate::scene_builder::PickKind;
        use noshape_model::reference::Reference;
        let r = Reference::Origin;
        let km = super::super::kind_matches;
        assert!(!km(PickKind::Edge, &r, QueryFilter::ANY_FACE), "qa5 I2");
        assert!(!km(PickKind::SketchCurve, &r, QueryFilter::ANY_FACE));
        assert!(km(PickKind::Face, &r, QueryFilter::ANY_FACE));
        assert!(km(PickKind::Edge, &r, QueryFilter::AXIS) && km(PickKind::Edge, &r, QueryFilter::DIRECTION));
        assert!(km(PickKind::Face, &r, QueryFilter::AXIS), "a cylinder gives its axis");
        assert!(km(PickKind::SketchCurve, &r, QueryFilter::ANY_EDGE));
        // Fillet / Chamfer take part edges only (a sketch line lying on a
        // part's edge isn't picked); sketch-only fields take no part edges.
        let fillet = QueryFilter::SOLID_EDGE | QueryFilter::ANY_FACE;
        assert!(km(PickKind::Edge, &r, fillet) && !km(PickKind::SketchCurve, &r, fillet));
        assert!(km(PickKind::SketchCurve, &r, QueryFilter::SKETCH_EDGE) && !km(PickKind::Edge, &r, QueryFilter::SKETCH_EDGE));
        // HOLE-FACE (live): Hole locations take sketch points and explicit
        // mate connectors, not a face or a part vertex.
        let hole = QueryFilter::SKETCH_POINT | QueryFilter::MATE_CONNECTOR;
        assert!(!km(PickKind::Face, &r, hole) && !km(PickKind::Vertex, &r, hole));
        assert!(km(PickKind::SketchPoint, &r, hole) && km(PickKind::Connector, &r, hole));
        // A vertex field still takes part vertices.
        assert!(km(PickKind::Vertex, &r, QueryFilter::VERTEX | QueryFilter::MATE_CONNECTOR));
    }

    #[test]
    fn direction_fields_take_the_edge_not_its_midpoint() {
        assert!(!field_takes_points(QueryFilter::DIRECTION));
        assert!(!field_takes_points(QueryFilter::AXIS));
        assert!(field_takes_points(QueryFilter::POINT_LIKE));
        assert!(field_takes_points(QueryFilter::MATE_CONNECTOR));
        assert!(field_takes_points(QueryFilter::SKETCH_POINT | QueryFilter::MATE_CONNECTOR));
    }
}

#[cfg(test)]
mod roll_item_tests {
    use super::roll_items;
    #[test]
    fn roll_items_as_live() {
        // 30 features, the bar just under Extrude 1 (row 1).
        assert_eq!(roll_items(1, 2, 30), (false, true), "Extrude 1: Roll to end only");
        assert_eq!(roll_items(29, 2, 30), (true, true), "Chamfer 5, rolled back");
        assert_eq!(roll_items(20, 2, 30), (true, true), "Fillet 5, rolled back");
        // The bar at the end.
        assert_eq!(roll_items(29, 30, 30), (false, false), "the last feature: neither");
        assert_eq!(roll_items(10, 30, 30), (true, false), "a middle feature: Roll to here");
    }
}

#[cfg(test)]
mod autoscroll_tests {
    use super::list_autoscroll_speed as v;
    // Live's list: y 144..708.
    const T: f32 = 144.0;
    const B: f32 = 708.0;
    fn within(got: f32, want: f32) -> bool {
        (got - want).abs() <= 0.15 * want.abs()
    }
    #[test]
    fn autoscroll_speeds_as_live() {
        // Bottom edge: live px/s by the distance above (+) / below (−) it.
        for (d, want) in [(70.0, 33.0), (50.0, 172.0), (30.0, 191.0), (20.0, 192.0), (10.0, 201.0), (2.0, 204.0), (-10.0, 203.0), (-40.0, 385.0), (-192.0, 600.0), (58.0, 155.0)] {
            let got = v(B - d, T, B);
            assert!(within(got, want), "{d} above the bottom: {got} vs live {want}");
        }
        // Top edge (scrolling up).
        for (d, want) in [(50.0, -177.0), (20.0, -194.0), (10.0, -203.0), (2.0, -207.0), (-10.0, -219.0), (-40.0, -347.0), (56.0, -142.0)] {
            let got = v(T + d, T, B);
            assert!(within(got, want), "{d} below the top: {got} vs live {want}");
        }
        // No autoscroll mid-list or 100 px inside an edge.
        for y in [450.0, B - 100.0, B - 150.0, T + 100.0] {
            assert_eq!(v(y, T, B), 0.0, "y {y}");
        }
    }
}
