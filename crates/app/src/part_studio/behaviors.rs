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
use gpui_kit::component::{Icon, IconName, Sizable as _, h_flex, v_flex};
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
    /// Opacity of the transparent parts (Onshape's transparency slider).
    pub opacity: f32,
    /// "Select transparent geometry": picks may land on transparent parts.
    pub select_transparent: bool,
    /// "Expand: distance": this many nearest parts join the chosen ones.
    pub expand: usize,
}

/// Open "Select other" list: candidates nearest first, highlighted index.
pub struct SelectOther {
    pub hits: Vec<Reference>,
    pub index: usize,
    /// Viewport-local position the list opens at.
    pub at: DVec2,
}

/// Consumed dialog picks: (picks, accent picks, before-model, lines).
pub type ConsumedLines = (Vec<Reference>, Vec<Reference>, usize, Option<noshape_render::Instance>);

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
    /// Cameras before discrete view changes (Previous view).
    pub cam_history: Vec<Camera>,
    /// Last pointer position over the viewport (viewport-local).
    pub pointer: Option<DVec2>,
    pub select_other: Option<SelectOther>,
    /// S shortcut toolbar position (viewport-local).
    pub mini_toolbar: Option<DVec2>,
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
    /// Inspection panel: the pressed tool (a pick mode until Esc) and the open ⋯.
    pub inspection_tool: Option<&'static str>,
    pub inspection_more: bool,
    /// Custom tables panel (selected table, its result, the hovered row).
    pub ct: super::custom_tables::CtState,
    /// Variable Studio tables shown in the Variable table panel.
    pub var_tables: std::collections::HashMap<noshape_model::document::TabId, Entity<crate::variable_studio::VariableStudioView>>,
    /// Section view planes (Shift+X).
    pub section: Vec<super::section::SectionPlane>,
    /// Section view dialog, and the parts excluded (false) or the only ones
    /// included (true) in the cut.
    pub section_dialog: Option<super::section::SectionDialog>,
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
    /// Whether the toolbar fits, from the last layout.
    pub tb_fit: super::ToolbarFit,
    /// Preview slider value and the model before the edited feature.
    pub preview_opacity: f32,
    pub preview_before: Option<Arc<StudioDisplay>>,
    /// Regeneration context before the edited feature.
    pub before_context: Option<Arc<noshape_model::regen::context::Context>>,
    pub preview_task: Option<Task<()>>,
    /// Remove / Intersect previews (live part-studio/234-237): the tool
    /// body (the feature regenerated as New) and the task computing it.
    pub preview_tool: Option<(Arc<StudioDisplay>, noshape_model::FeatureId)>,
    pub preview_tool_task: Option<Task<()>>,
    /// Measure graphics uploaded for a selection.
    pub measure_lines: Option<(Vec<Reference>, Option<noshape_render::Instance>)>,
    /// Dynamic suppression editor: feature, expression input.
    pub dyn_suppression: Option<(FeatureId, Entity<InputState>, Subscription)>,
    /// Shortcut map search box.
    pub shortcut_filter: Option<(Entity<InputState>, Subscription)>,
    pub shortcut_query: String,
    /// Shortcut sheet tab (General, Part Studio, …).
    pub shortcut_tab: usize,
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
    /// Entity of the hovered dialog selection row.
    pub query_hover: Option<Reference>,
    /// The pointer is over the manipulator arrow (drawn outlined).
    pub manip_hover: bool,
    /// The sketch dimension box moved by its drag strip (reset when a box opens).
    pub dim_box_offset: Point<Pixels>,
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
            cam_history: Vec::new(),
            pointer: None,
            select_other: None,
            mini_toolbar: None,
            filter: None,
            filter_query: String::new(),
            list_hover: None,
            anchor: None,
            last_click: None,
            list_focus: cx.focus_handle(),
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
            inspection_tool: None,
            inspection_more: false,
            ct: Default::default(),
            var_tables: Default::default(),
            section: Vec::new(),
            section_dialog: None,
            section_parts: None,
            appearance_edit: None,
            appearance_hover: Vec::new(),
            analysis: None,
            vcolor_cache: None,
            high_quality: false,
            create_selection: None,
            material_panel: None,
            panels: Default::default(),
            cfg_box_h: None,
            menu_pos: Point::default(),
            regen_paused: None,
            force_regen: false,
            show_regen_times: false,
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
            tb_fit: Default::default(),
            preview_opacity: 1.0,
            preview_before: None,
            before_context: None,
            preview_task: None,
            preview_tool: None,
            preview_tool_task: None,
            measure_lines: None,
            dyn_suppression: None,
            shortcut_filter: None,
            shortcut_query: String::new(),
            shortcut_tab: 0,
            recent_tools: Vec::new(),
            orient_on_edit: false,
            orient_pending: false,
            oriented: false,
            remembered: Default::default(),
            default_geometry_collapsed: false,
            menu_target: None,
            list_menu: None,
            consumed_lines: None,
            shown_dims: None,
            query_hover: None,
            manip_hover: false,
            dim_box_offset: Point::default(),
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

/// What a selection field accepts, for the rejection message.
pub fn filter_description(f: QueryFilter) -> String {
    let mut kinds: Vec<&str> = Vec::new();
    let mut add = |q: QueryFilter, s: &'static str| {
        if f.intersects(q) && !kinds.contains(&s) {
            kinds.push(s);
        }
    };
    add(QueryFilter::SKETCH_REGION, "sketch regions");
    if f.intersects(QueryFilter::FACE) {
        add(QueryFilter::FACE, "faces");
    } else {
        add(QueryFilter::PLANAR_FACE, "planar faces");
        add(QueryFilter::CYLINDRICAL_FACE, "cylindrical faces");
    }
    add(QueryFilter::PLANE, "planes");
    if f.intersects(QueryFilter::EDGE) {
        add(QueryFilter::EDGE, "edges");
    } else {
        add(QueryFilter(QueryFilter::LINEAR_EDGE.0 | QueryFilter::CIRCULAR_EDGE.0), "edges");
    }
    add(QueryFilter::SKETCH_EDGE, "sketch curves");
    add(QueryFilter(QueryFilter::VERTEX.0 | QueryFilter::SKETCH_POINT.0 | QueryFilter::POINT_BODY.0), "points");
    add(QueryFilter::BODY, "parts");
    add(QueryFilter::WIRE_BODY, "curves");
    add(QueryFilter::MATE_CONNECTOR, "mate connectors");
    add(QueryFilter::FEATURE, "features");
    add(QueryFilter::SKETCH, "sketches");
    match kinds.len() {
        0 => "That entity can't be used in the active field.".into(),
        1 => format!("Select {}.", kinds[0]),
        n => format!("Select {} or {}.", kinds[..n - 1].join(", "), kinds[n - 1]),
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
        match r {
            Reference::Body(b) => Some(b.clone()),
            _ => self.display.bodies.iter().find(|b| b.face_refs.contains(r) || b.edge_refs.contains(r) || b.vertex_refs.contains(r)).map(|b| b.id.clone()),
        }
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

    /// Hover target in the graphics area, expanded like the selection.
    pub(super) fn highlighted_hover(&self, cx: &App) -> Vec<Reference> {
        if let Some(f) = self.ui.list_hover.or(self.ui.list_menu) {
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
                dialog.update(cx, |d, cx| d.flash_active_field(cx));
                self.flash_info(filter_description(filter), cx);
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
    fn pick_kinds(&self, cx: &App) -> Option<Vec<PickKind>> {
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
            if !filter.intersects(QueryFilter::BODY) {
                dialog.update(cx, |d, cx| d.flash_active_field(cx));
                self.flash_info(filter_description(filter), cx);
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
                        doc.edit_studio(this.tab, &format!("Rename : {text}"), cx, |ps| {
                            ps.part_properties.entry(b.clone()).or_default().name = Some(text.clone());
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
        self.ui.part_renaming = Some((body, st, sub));
        cx.notify();
    }

    /// Plane menu "Size to Part Studio": the plane grows to frame the parts.
    pub(super) fn size_plane_to_studio(&mut self, r: Reference, cx: &mut Context<Self>) {
        let Some(pl) = self.display.planes.iter().find(|p| p.reference == r) else { return };
        let f = pl.frame;
        let (mut hw, mut hh) = (0.0f64, 0.0f64);
        for (lo, hi) in self.display.bodies.iter().filter(|b| !b.hidden).filter_map(|b| b.bounds) {
            for k in 0..8 {
                let c = glam::DVec3::new(if k & 1 == 0 { lo.x } else { hi.x }, if k & 2 == 0 { lo.y } else { hi.y }, if k & 4 == 0 { lo.z } else { hi.z }) - f.origin;
                hw = hw.max(c.dot(f.x).abs());
                hh = hh.max(c.dot(f.y).abs());
            }
        }
        if hw <= 0.0 || hh <= 0.0 {
            return;
        }
        let (hw, hh) = (hw * 1.1, hh * 1.1);
        self.doc.update(cx, |doc, cx| {
            doc.edit_studio(self.tab, "Size to Part Studio", cx, |ps| {
                ps.plane_sizes.retain(|(x, ..)| *x != r);
                ps.plane_sizes.push((r.clone(), hw, hh));
            });
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
                let at = ps.rollback_index();
                for id in ps.paste_features(&sketches, at) {
                    if let Some(f) = ps.feature_mut(id) {
                        f.params.set("sketchPlane", ParamValue::Query(vec![plane.clone()]));
                    }
                }
            });
        });
    }

    /// "Export as DXF/DWG" for a sketch (DXF, millimeters, sketch plane
    /// coordinates; construction curves as hidden lines).
    pub(super) fn export_sketch(&mut self, id: FeatureId, ext: &'static str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(sk) = self.display.sketches.iter().find(|s| s.feature == id) else { return };
        let g = sketch_sheet(sk);
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
        let Some(frame) = self.frame_of_ref(&r) else { return };
        let Some(b) = self.display.bodies.iter().find(|b| b.face_refs.contains(&r)) else { return };
        let fi = b.face_refs.iter().position(|x| *x == r).unwrap_or(0);
        let lines = b.mesh.face_edges(fi).into_iter().filter_map(|e| b.mesh.edges.get(e).map(|pl| (pl.clone(), false))).collect();
        let g = plane_sheet(&frame, lines);
        let name = b.name.clone();
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

    pub(super) fn set_suppression_expr(&mut self, id: FeatureId, expr: Option<String>, cx: &mut Context<Self>) {
        let expr = expr.filter(|e| !e.trim().is_empty());
        self.doc.update(cx, |doc, cx| {
            doc.edit_studio(self.tab, if expr.is_some() { "Dynamic suppression" } else { "Remove dynamic suppression" }, cx, |ps| {
                if let Some(f) = ps.feature_mut(id) {
                    f.suppression_expr = expr;
                }
            });
        });
    }

    /// "Unsuppress" on a feature with an expression: drop it and leave the
    /// feature active (one undo step).
    pub(super) fn clear_dynamic_suppression(&mut self, id: FeatureId, cx: &mut Context<Self>) {
        self.doc.update(cx, |doc, cx| {
            doc.edit_studio(self.tab, "Unsuppress", cx, |ps| {
                if let Some(f) = ps.feature_mut(id) {
                    f.suppression_expr = None;
                    f.suppressed = false;
                }
            });
        });
    }

    /// Open Onshape's Dynamic suppression dialog: one "Unsuppressed if"
    /// expression (the feature is active while it is true; live
    /// dom/dialog-dynamic-suppression.txt).
    pub(super) fn edit_dynamic_suppression(&mut self, id: FeatureId, window: &mut Window, cx: &mut Context<Self>) {
        let current = self.studio(cx).and_then(|s| s.feature(id)).and_then(|f| f.suppression_expr.clone()).unwrap_or_default();
        let st = cx.new(|cx| InputState::new(window, cx).default_value(current));
        let sub = cx.subscribe_in(&st, window, move |this: &mut Self, _, ev: &InputEvent, window, cx| {
            if let InputEvent::PressEnter { .. } = ev {
                this.accept_dynamic_suppression(window, cx);
            }
        });
        st.update(cx, |s, cx| s.focus(window, cx));
        self.ui.dyn_suppression = Some((id, st, sub));
        cx.notify();
    }

    fn accept_dynamic_suppression(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some((id, st, _)) = self.ui.dyn_suppression.take() else { return };
        let text = st.read(cx).value().to_string();
        // An empty field leaves the feature as it was (live: nothing
        // changes until an expression is committed).
        if !text.trim().is_empty() {
            self.set_suppression_expr(id, Some(text), cx);
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
        let help = div().size(px(14.)).rounded_full().bg(gpui_kit::rgb(0x888888)).flex().items_center().justify_center().text_size(px(11.)).font_weight(FontWeight::BOLD).text_color(gpui_kit::rgb(0xffffff)).child("?");
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
                                .tooltip(crate::ui::tooltip("Accept (Enter)"))
                                .child(Icon::new(IconName::Check).with_size(px(18.)).text_color(gpui_kit::rgb(0xffffff)))
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
                                .tooltip(crate::ui::tooltip("Cancel (Esc)"))
                                .child(Icon::new(IconName::Close).with_size(px(16.)).text_color(pal.cancel))
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
            let studio = self.studio(cx).cloned().unwrap_or_default();
            let total: f32 = self.display.features.iter().map(|f| f.time_ms).sum();
            let rows: Vec<AnyElement> = studio
                .features
                .iter()
                .map(|f| {
                    let t = self.display.feature_state(f.id).map(|s| s.time_ms).unwrap_or(0.0);
                    let id = f.id;
                    h_flex()
                        .id(SharedString::from(format!("rt-{id}")))
                        .gap_2()
                        .px_1()
                        .rounded_sm()
                        .hover(|d| d.bg(pal.panel_hover))
                        .child(div().flex_1().truncate().text_xs().text_color(pal.text).child(f.name.clone()))
                        .child(div().text_xs().text_color(if t > 500.0 { pal.warning } else { pal.text_muted }).child(format!("{t:.1} ms")))
                        .on_hover(cx.listener(move |this, h: &bool, _, cx| {
                            // Hovering a row highlights the feature's geometry.
                            this.ui.list_hover = h.then_some(id);
                            this.refresh_scene(false, cx);
                        }))
                        .into_any_element()
                })
                .collect();
            out.push(
                v_flex()
                    .id("regen-times")
                    .occlude()
                    .absolute()
                    .top_3()
                    .left(px(340.))
                    .w(px(260.))
                    .max_h(px(420.))
                    .overflow_y_scroll()
                    .p_2()
                    .gap_0p5()
                    .rounded_md()
                    .bg(pal.panel_raised)
                    .border_1()
                    .border_color(pal.border_strong)
                    .shadow_lg()
                    .child(
                        h_flex()
                            .justify_between()
                            .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).text_color(pal.text).child("Regeneration times"))
                            .child(div().id("rt-close").px_1().cursor_pointer().text_color(pal.text_faint).child("×").on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.ui.show_regen_times = false;
                                cx.notify();
                            }))),
                    )
                    .child(div().text_xs().text_color(pal.text_muted).child(format!("Total regeneration time: {total:.1} ms")))
                    .children(rows)
                    .into_any_element(),
            );
        }
        out
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
        let mut bodies: Vec<BodyId> = Vec::new();
        let mut feats: Vec<FeatureId> = Vec::new();
        let mut defaults: Vec<Reference> = Vec::new();
        for r in targets {
            match r {
                Reference::Origin | Reference::Plane(_) => defaults.push(r.clone()),
                Reference::SketchEntity { sketch, .. } | Reference::SketchRegion { sketch, .. } | Reference::Sketch(sketch) => feats.push(*sketch),
                Reference::FeaturePlane(f) | Reference::MateConnector { feature: f, .. } => feats.push(*f),
                Reference::Feature(f) => {
                    // A feature that made parts hides its parts; sketches,
                    // planes and connectors hide themselves.
                    let made: Vec<BodyId> = self.display.bodies.iter().filter(|b| b.id.feature == *f).map(|b| b.id.clone()).collect();
                    if made.is_empty() {
                        feats.push(*f);
                    } else {
                        bodies.extend(made);
                    }
                }
                r => {
                    if let Some(b) = self.body_of(r) {
                        bodies.push(b);
                    }
                }
            }
        }
        if bodies.is_empty() && feats.is_empty() && defaults.is_empty() {
            return;
        }
        let verb = if hidden { "Hide" } else { "Show" };
        self.doc.update(cx, |doc, cx| {
            doc.edit_studio(self.tab, verb, cx, |ps| {
                for b in &bodies {
                    ps.part_properties.entry(b.clone()).or_default().hidden = hidden;
                }
                for f in &feats {
                    if let Some(f) = ps.feature_mut(*f) {
                        f.hidden = hidden;
                    }
                }
                for d in &defaults {
                    ps.hidden_defaults.retain(|x| x != d);
                    if hidden {
                        ps.hidden_defaults.push(d.clone());
                    }
                }
            });
        });
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
            doc.edit_studio(self.tab, "Show all", cx, |ps| {
                for p in ps.part_properties.values_mut() {
                    p.hidden = false;
                }
                for f in &mut ps.features {
                    f.hidden = false;
                }
                ps.hidden_defaults.clear();
            });
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
            _ => self.ui.isolate = Some(Isolation { mode, bodies: targets, opacity: crate::isolate_ui::ALPHA_MIN, select_transparent: false, expand: 0 }),
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
    /// "Expand: distance" neighbors (nearest bounding-box centers first).
    pub(super) fn isolation_transparent(&self) -> HashSet<BodyId> {
        let Some(iso) = &self.ui.isolate else { return HashSet::new() };
        let center = |b: &BodyId| self.display.body(b).and_then(|b| b.bounds).map(|(lo, hi)| (lo + hi) * 0.5);
        let mut chosen: Vec<BodyId> = iso.bodies.clone();
        let mut others: Vec<(f64, BodyId)> = self
            .display
            .bodies
            .iter()
            .filter(|b| !chosen.contains(&b.id))
            .filter_map(|b| {
                let c = center(&b.id)?;
                let d = iso.bodies.iter().filter_map(center).map(|x| x.distance(c)).fold(f64::INFINITY, f64::min);
                Some((d, b.id.clone()))
            })
            .collect();
        others.sort_by(|a, b| a.0.total_cmp(&b.0));
        chosen.extend(others.into_iter().take(iso.expand).map(|(_, b)| b));
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
                let d = this.display.clone();
                this.apply_display(d, window, cx);
            });
        }));
    }

    /// Remove / Intersect previews: regenerate the open feature as New to get
    /// its tool body (shown see-through by `display_for_scene`).
    pub(super) fn schedule_tool_preview(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let tool_op = self.edit.as_ref().is_some_and(|e| {
            let f = &e.dialog.read(cx).feature;
            matches!(f.type_id.as_str(), "extrude" | "revolve" | "sweep" | "loft") && matches!(f.params.enum_("operationType"), "REMOVE" | "INTERSECT")
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
                let missed = this.tool_missed();
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
        let mut d = (*self.display).clone();
        for b in &mut d.bodies {
            if transparent.contains(&b.id) {
                b.color[3] = b.color[3].min(iso.opacity);
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
        let n = self.display.bodies.len();
        let dialog = crate::isolate_ui::dialog(
            crate::isolate_ui::IsoDialog {
                key: "ps-isolate",
                title,
                select_transparent: iso.select_transparent,
                transparency: crate::isolate_ui::alpha_t(iso.opacity),
                expand: if n == 0 { 0.0 } else { iso.expand as f32 / n as f32 },
                expand_options: vec!["Expand: distance".to_string()],
                expand_ix: 0,
                expand_tip: "Expand isolate by distance",
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
                        iso.opacity = crate::isolate_ui::t_alpha(f);
                    }
                    let d = v.display.clone();
                    v.apply_display(d, window, cx);
                }),
                set_expand: std::rc::Rc::new(move |v: &mut Self, f, window, cx| {
                    let want = (f * n as f32).round() as usize;
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

    /// What zoom to fit and the standard views frame: Onshape's `getFitBounds`
    /// = the model node's bounds, which hold the visible planes (default
    /// planes included) as well as parts and sketches.
    pub(super) fn zoom_fit_bounds(&self) -> Option<(glam::DVec3, glam::DVec3)> {
        let mut acc = self.fit_bounds();
        if self.opts.show_planes {
            for pl in self.display.planes.iter().filter(|p| !p.hidden) {
                let f = &pl.frame;
                for (sx, sy) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
                    let q = f.origin + f.x * (sx * pl.half_width) + f.y * (sy * pl.half_height);
                    acc = Some(acc.map_or((q, q), |(lo, hi)| (lo.min(q), hi.max(q))));
                }
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

    /// ` (grave): list the faces and edges under the cursor, nearest first;
    /// pressing it again moves down the list (Shift+` up).
    pub(super) fn select_other(&mut self, backwards: bool, cx: &mut Context<Self>) {
        if let Some(so) = &mut self.ui.select_other {
            let n = so.hits.len().max(1);
            so.index = if backwards { (so.index + n - 1) % n } else { (so.index + 1) % n };
            self.hover = so.hits.get(so.index).cloned();
            self.refresh_scene(false, cx);
            cx.notify();
            return;
        }
        let Some(at) = self.ui.pointer else { return };
        let (origin, dir, tol) = {
            let vp = self.viewport.read(cx);
            let (o, d) = vp.ray(at);
            (o, d, vp.camera.world_per_pixel(vp.size().y) * 8.0)
        };
        let iso = self.ui.isolate.as_ref().map(|i| (i.mode, i.bodies.clone()));
        let skip = move |b: &BodyId| match &iso {
            Some((IsolateMode::Isolate, keep)) => !keep.contains(b),
            Some((IsolateMode::Transparent, gone)) => gone.contains(b),
            None => false,
        };
        let hits: Vec<Reference> = interaction::entities_along_ray(&self.display, origin, dir, tol, &skip, 20).into_iter().map(|h| h.reference).collect();
        if hits.is_empty() {
            self.flash_info("Nothing under the cursor to select.".into(), cx);
            return;
        }
        // Start on the entry after the highlighted one (Onshape).
        let start = self.hover.as_ref().and_then(|h| hits.iter().position(|r| r == h)).map(|i| (i + 1) % hits.len()).unwrap_or(0);
        self.hover = hits.get(start).cloned();
        self.ui.select_other = Some(SelectOther { hits, index: start, at });
        self.refresh_scene(false, cx);
        cx.notify();
    }

    /// Pick the highlighted (or clicked) Select other entry.
    pub(super) fn choose_select_other(&mut self, index: Option<usize>, cx: &mut Context<Self>) {
        let Some(so) = self.ui.select_other.take() else { return };
        let Some(r) = so.hits.get(index.unwrap_or(so.index)).cloned() else { return };
        let kind = if matches!(r, Reference::Edge(_)) { PickKind::Edge } else { PickKind::Face };
        self.hover = None;
        self.pick(PickTarget { reference: r, kind, anchor: None }, cx);
    }

    pub(super) fn render_select_other(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let so = self.ui.select_other.as_ref()?;
        let pal = p();
        let labeler = self.labeler(cx);
        let rows: Vec<AnyElement> = so
            .hits
            .iter()
            .enumerate()
            .map(|(i, r)| {
                let active = i == so.index;
                let r2 = r.clone();
                let kind = if matches!(r, Reference::Edge(_)) { "Edge" } else { "Face" };
                h_flex()
                    .id(SharedString::from(format!("so-{i}")))
                    .gap_2()
                    .px_2()
                    .py_0p5()
                    .rounded_sm()
                    .text_xs()
                    .cursor_pointer()
                    .when(active, |d| d.bg(pal.accent_soft))
                    .hover(|d| d.bg(pal.panel_hover))
                    .child(div().w(px(32.)).text_color(pal.text_faint).child(kind))
                    .child(div().text_color(pal.text).child(labeler(r)))
                    .on_hover(cx.listener(move |this, h: &bool, _, cx| {
                        if *h {
                            if let Some(so) = &mut this.ui.select_other {
                                so.index = i;
                            }
                            this.hover = Some(r2.clone());
                            this.refresh_scene(false, cx);
                            cx.notify();
                        }
                    }))
                    .on_mouse_down(MouseButton::Left, cx.listener(move |this, _: &MouseDownEvent, _, cx| {
                        cx.stop_propagation();
                        this.choose_select_other(Some(i), cx);
                    }))
                    .into_any_element()
            })
            .collect();
        Some(
            v_flex()
                .id("select-other")
                .occlude()
                .absolute()
                .left(px(so.at.x as f32 + 14.0))
                .top(px(so.at.y as f32 + 10.0))
                .min_w(px(180.))
                .p_1()
                .rounded_md()
                .bg(pal.panel_raised)
                .border_1()
                .border_color(pal.border_strong)
                .shadow_lg()
                .child(div().px_2().pb_1().text_xs().text_color(pal.text_muted).child("Select other"))
                .children(rows)
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
        let (sel, acc) = (missing(selection), missing(accent));
        let key = Arc::as_ptr(&before) as usize;
        if let Some((s, a, k, inst)) = &self.ui.consumed_lines
            && *s == sel
            && *a == acc
            && *k == key
        {
            return inst.iter().cloned().collect();
        }
        let mut lines = LineData::default();
        for (refs, color) in [(&acc, crate::theme::viewport::PREHIGHLIGHT), (&sel, crate::theme::viewport::SELECTION)] {
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
        self.ui.consumed_lines = Some((sel, acc, key, inst.clone()));
        inst.into_iter().collect()
    }

    /// Measure graphics (Onshape): a dotted line between the closest points
    /// of the measured entities, with its X (red), Y (green) and Z (blue)
    /// components.
    pub(super) fn measure_overlay(&mut self, cx: &App) -> Vec<noshape_render::Instance> {
        use noshape_render::{Drawable, Instance, LineData, LineSegment};
        if self.edit.is_some() || self.show_mass || self.selection.len() < 2 {
            return Vec::new();
        }
        let sel = self.selection.clone();
        let Some(ctx) = self.measure_context(&sel, cx) else { return Vec::new() };
        // Uploaded once per selection (refresh_scene runs on every hover).
        if let Some((s, inst)) = &self.ui.measure_lines
            && *s == sel
        {
            return inst.iter().cloned().collect();
        }
        let results = match &self.measure_cache {
            Some((s, r)) if *s == sel => r.clone(),
            _ => {
                let r = noshape_model::regen::measure::measure(&ctx, &sel);
                self.measure_cache = Some((sel, r.clone()));
                r
            }
        };
        let mut l = LineData::default();
        for (_, m) in results {
            if let noshape_model::regen::measure::Measurement::Distance { a, b, .. } = m {
                let f = |v: glam::DVec3| -> [f32; 3] { v.as_vec3().into() };
                let style = LineSegment::DASHED | LineSegment::ON_TOP;
                l.segments.push(LineSegment { a: f(a), b: f(b), color: [20, 20, 20, 255], pick_id: 0, width: 2.0, style });
                let x = glam::DVec3::new(b.x, a.y, a.z);
                let y = glam::DVec3::new(b.x, b.y, a.z);
                for (p, q, c) in [(a, x, [220, 40, 40, 255]), (x, y, [40, 170, 60, 255]), (y, b, [40, 90, 220, 255])] {
                    if p.distance(q) > 1e-9 {
                        l.segments.push(LineSegment { a: f(p), b: f(q), color: c, pick_id: 0, width: 1.0, style });
                    }
                }
            }
        }
        let inst = (!l.segments.is_empty()).then(|| Instance::new(Drawable::Lines(self.gpu.uploader().upload_lines(&l))).per_primitive_color().unpickable().ignore_clip().layer(21));
        self.ui.measure_lines = Some((self.selection.clone(), inst.clone()));
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
        let sprite = |m: glam::DVec3, color: [u8; 4], size: f32| PointSprite { position: m.as_vec3().into(), color, pick_id: 0, size, style: LineSegment::ON_TOP, _pad: 0 };
        // Selected midpoints.
        for r in &self.selection {
            if let Reference::Implicit { on, inference: noshape_model::reference::Inference::Midpoint } = r
                && let Some(m) = self.edge_midpoint(on)
            {
                points.push(sprite(m, [255, 169, 10, 255], 11.0));
            }
        }
        if self.active_field_filter(cx).is_none_or(|f| f.intersects(QueryFilter::POINT_LIKE))
            && (self.edit.is_none() || self.active_field_filter(cx).is_some())
            && self.sketch.is_none()
            && let Some(r @ (Reference::Edge(_) | Reference::SketchEntity { .. })) = &self.hover
            && let Some(m) = self.edge_midpoint(r)
        {
            points.push(sprite(m, [255, 194, 14, 255], 9.0));
        }
        if points.is_empty() {
            return Vec::new();
        }
        let pts = PointData { points };
        vec![Instance::new(Drawable::Points(self.gpu.uploader().upload_points(&pts))).per_primitive_color().unpickable().ignore_clip().layer(22)]
    }

    /// An edge click within ~8 px of its midpoint selects the midpoint.
    pub(super) fn midpoint_pick(&self, t: PickTarget, position: DVec2, cx: &App) -> PickTarget {
        if self.sketch.is_some() {
            return t;
        }
        let Some(m) = self.edge_midpoint(&t.reference) else { return t };
        let Some(s) = self.viewport.read(cx).project(m) else { return t };
        if s.distance(position) > 8.0 {
            return t;
        }
        let r = Reference::Implicit { on: Box::new(t.reference.clone()), inference: noshape_model::reference::Inference::Midpoint };
        PickTarget { reference: r, kind: PickKind::Vertex, anchor: Some(m) }
    }

    // ------------------------------------------------------------ S toolbar

    /// S: the shortcut toolbar at the cursor (feature tools; sketch tools
    /// in a sketch). S again or Esc closes it.
    pub(super) fn toggle_mini_toolbar(&mut self, cx: &mut Context<Self>) {
        // Its tools may have changed in Preferences › Shortcut toolbars.
        self.ui.toolbar = super::toolbar_prefs::ToolbarPrefs::load();
        if self.sketch.is_some() {
            let at = self.ui.pointer.unwrap_or_else(|| self.viewport.read(cx).size() * 0.5);
            self.ui.mini_toolbar = Some(at + DVec2::new(10.0, -30.0));
        } else if self.ui.mini_toolbar.take().is_none() {
            let at = self.ui.pointer.unwrap_or_else(|| self.viewport.read(cx).size() * 0.5);
            self.ui.mini_toolbar = Some(at);
        }
        cx.notify();
    }

    pub(super) fn render_mini_toolbar(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let at = self.ui.mini_toolbar?;
        let pal = p();
        let in_sketch = self.sketch.is_some();
        let btn = |id: SharedString, icon: SharedString, tip: String| {
            div()
                .id(id)
                .size(px(30.))
                .flex()
                .items_center()
                .justify_center()
                .rounded_md()
                .cursor_pointer()
                .hover(|d| d.bg(pal.panel_hover))
                .child(svg().path(icon).size(px(18.)).text_color(pal.text))
                .tooltip(move |window, cx| gpui_kit::component::tooltip::Tooltip::new(tip.clone()).build(window, cx))
        };
        // Onshape: rows of five tools (sketch and Part Studio alike).
        let mut row = h_flex().gap_0p5().w(px(158.)).flex_wrap();
        if in_sketch {
            for tool in super::shortcut_cmds::sketch_shortcut_toolbar(&self.ui.toolbar) {
                row = row.child(btn(SharedString::from(format!("mini-{}", tool.icon())), crate::assets::icon_path(tool.icon()), tool.label().to_string()).on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _: &MouseDownEvent, window, cx| {
                        cx.stop_propagation();
                        this.ui.mini_toolbar = None;
                        window.focus(&this.focus, cx);
                        this.activate_tool(tool, window, cx);
                        cx.notify();
                    }),
                ));
            }
        } else {
            for id in self.ui.toolbar.shortcut_tools() {
                let Some(t) = noshape_model::features::get(&id) else { continue };
                let id: &'static str = t.id;
                let tip = match t.shortcut {
                    Some(s) => format!("{} ({})", t.name, s.replace("shift-", "Shift+").to_uppercase()),
                    None => t.name.to_string(),
                };
                row = row.child(btn(SharedString::from(format!("mini-{id}")), feature_icon(id), tip).on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _: &MouseDownEvent, window, cx| {
                        cx.stop_propagation();
                        this.ui.mini_toolbar = None;
                        this.start_feature(id, window, cx);
                    }),
                ));
            }
        }
        Some(
            div()
                .id("mini-toolbar")
                .occlude()
                .absolute()
                .left(px((at.x as f32 - 60.0).max(4.0)))
                .top(px((at.y as f32 - 44.0).max(4.0)))
                .p_1()
                .rounded_lg()
                .bg(pal.panel_raised)
                .border_1()
                .border_color(pal.border_strong)
                .shadow_lg()
                .child(row)
                .into_any_element(),
        )
    }

    // ------------------------------------------------------------ feature list

    /// Search box at the top of the feature list (created on first render).
    pub(super) fn ensure_filter_input(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.ui.filter.is_some() {
            return;
        }
        let st = cx.new(|cx| InputState::new(window, cx).placeholder("Filter by name or type"));
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
        self.ui.anchor = Some(id);
        self.refresh_scene(false, cx);
        cx.notify();
    }

    /// Drop of dragged features onto the row at `to`: moves every selected
    /// feature when the dragged one is selected; refuses moves above a
    /// feature's dependencies (Onshape) with an error message.
    pub(super) fn drop_features(&mut self, dragged: FeatureId, to: usize, cx: &mut Context<Self>) {
        let Some(studio) = self.studio(cx) else { return };
        let moving: Vec<FeatureId> = if self.selected_features.contains(&dragged) { self.selected_features.clone() } else { vec![dragged] };
        let Some(order) = interaction::reordered(&studio.features, &moving, to) else { return };
        if let Err(msg) = interaction::check_order(&studio.features, &order) {
            self.message = Some((FeatureStatus::Error, msg));
            cx.notify();
            return;
        }
        let n = moving.len();
        self.doc.update(cx, |doc, cx| {
            doc.edit_studio(self.tab, if n == 1 { "Reorder feature" } else { "Reorder features" }, cx, |ps| interaction::apply_order(ps, &order));
        });
    }

    /// Delete key with parts selected and no features: a Delete part
    /// feature (Onshape deletes parts parametrically).
    pub(super) fn delete_selected_parts(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let parts: Vec<Reference> = self.selection.iter().filter(|r| matches!(r, Reference::Body(_))).cloned().collect();
        if parts.is_empty() {
            return false;
        }
        let units = self.doc.read(cx).session.doc.settings.units;
        let Some(studio) = self.studio(cx) else { return false };
        let Some(mut f) = studio.new_feature("deleteBodies", &units) else { return false };
        f.params.set("entities", ParamValue::Query(parts));
        self.selection.clear();
        self.doc.update(cx, |doc, cx| {
            doc.edit_studio(self.tab, &format!("Insert feature : {}", f.name), cx, |ps| {
                ps.insert_at_rollback(f);
            });
        });
        let _ = window;
        true
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
                .child(div().w(px(25.)).flex().justify_center().child(svg().path(feature_icon(&ty)).size(px(15.)).text_color(pal.text).opacity(0.8)))
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
                        .child(div().id("dep-close").px_1().cursor_pointer().child(Icon::new(IconName::Close).small().text_color(pal.text_muted)).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
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
                    .child(div().w(px(25.)).flex().justify_center().child(svg().path(feature_icon(&ty)).size(px(15.)).text_color(pal.text).opacity(0.8)))
                    .child(div().px(px(6.)).text_size(px(12.)).text_color(pal.text).child(n))
                    .on_hover(cx.listener(move |this, h: &bool, _, cx| {
                        this.ui.list_hover = h.then_some(fid);
                        this.refresh_scene(false, cx);
                    }))
                    .into_any_element()
            })
            .collect();
        let me = h_flex().h(px(26.)).child(div().w(px(25.)).flex().justify_center().child(crate::assets::icon(if matches!(r, Reference::Origin) { "origin" } else { "plane" }, 15., cx))).child(div().px(px(6.)).text_size(px(12.)).text_color(pal.text).child(name.clone()));
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
                        .child(div().id("pdep-close").px_1().cursor_pointer().child(Icon::new(IconName::Close).small().text_color(pal.text_muted)).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
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
        if !e.is_new {
            return;
        }
        let dialog = e.dialog.clone();
        let ty = dialog.read(cx).feature.type_id.clone();
        if !matches!(ty.as_str(), "extrude" | "revolve" | "sweep" | "thicken") {
            return;
        }
        // Sweep's profiles are `profiles` (sweep.fs); the others' `entities`.
        let profiles_id = if ty == "sweep" { "profiles" } else { "entities" };
        let (op_user, flip_user, profiles, op, scope_user, flipped) = {
            let d = dialog.read(cx);
            (d.user_set.contains("operationType"), d.user_set.contains("oppositeDirection"), d.feature.params.query(profiles_id).to_vec(), d.feature.params.enum_("operationType").to_string(), d.user_set.contains("booleanScope"), d.feature.params.bool("oppositeDirection"))
        };
        // Parts the profiles lie on: a picked face, or the face a sketch was
        // drawn on. Thicken (thickenEditLogic) takes the owners of every
        // picked solid face.
        let host_of = |r: &Reference| match r {
            Reference::Face(_) => self.body_of(r),
            Reference::SketchRegion { sketch, .. } | Reference::SketchEntity { sketch, .. } | Reference::Sketch(sketch) if ty != "thicken" => {
                let plane = self.studio(cx)?.feature(*sketch)?.params.query("sketchPlane").first().cloned()?;
                self.body_of(&plane)
            }
            _ => None,
        };
        let hosts: Vec<BodyId> = if ty == "thicken" {
            let mut v: Vec<BodyId> = Vec::new();
            for b in profiles.iter().filter_map(host_of) {
                if !v.contains(&b) {
                    v.push(b);
                }
            }
            v
        } else {
            profiles.iter().find_map(host_of).into_iter().collect()
        };
        let mut changes: Vec<(&'static str, ParamValue)> = Vec::new();
        if !op_user {
            // thickenEditLogic: Remove when the wall points into the part.
            let want = match (hosts.is_empty(), ty.as_str()) {
                (true, _) => "NEW",
                (false, "thicken") if flipped => "REMOVE",
                _ => "ADD",
            };
            if op != want && (matches!(op.as_str(), "NEW" | "ADD") || ty == "thicken") {
                changes.push(("operationType", ParamValue::Enum(want.into())));
            }
            if !hosts.is_empty() && !scope_user && dialog.read(cx).feature.params.query("booleanScope").is_empty() {
                changes.push(("booleanScope", ParamValue::Query(hosts.iter().map(|b| Reference::Body(b.clone())).collect())));
            }
        }
        if ty == "extrude" && !flip_user && !hosts.is_empty() {
            let into = matches!(op.as_str(), "REMOVE" | "INTERSECT");
            if flipped != into {
                changes.push(("oppositeDirection", ParamValue::Bool(into)));
            }
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
}

/// A sketch as 2D sheet geometry in millimeters (plane coordinates).
/// Flat sheet (millimeters, plane coordinates) of world polylines.
fn plane_sheet(frame: &noshape_model::display::PlaneFrame, polylines: Vec<(Vec<[f32; 3]>, bool)>) -> noshape_model::drawing::sheet::SheetGeometry {
    use noshape_model::drawing::sheet::{LineStyle, Polyline2};
    let local = |p: [f32; 3]| {
        let d = glam::DVec3::new(p[0] as f64, p[1] as f64, p[2] as f64) - frame.origin;
        [d.dot(frame.x) * 1000.0, d.dot(frame.y) * 1000.0]
    };
    let lines: Vec<Polyline2> = polylines.into_iter().map(|(pts, hidden)| Polyline2 { points: pts.into_iter().map(local).collect(), style: if hidden { LineStyle::Hidden } else { LineStyle::Visible }, closed: false, pick: 0 }).collect();
    let (mut w, mut h) = (0.0f64, 0.0f64);
    for l in &lines {
        for p in &l.points {
            w = w.max(p[0].abs());
            h = h.max(p[1].abs());
        }
    }
    let mut g = noshape_model::drawing::sheet::SheetGeometry::new(w * 2.0, h * 2.0);
    g.lines = lines;
    g
}

pub fn sketch_sheet(sk: &noshape_model::display::SketchDisplay) -> noshape_model::drawing::sheet::SheetGeometry {
    plane_sheet(&sk.frame, sk.curves.iter().filter(|c| c.points.len() >= 2 && !c.external).map(|c| (c.points.clone(), c.construction)).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    // The glob import brings GPUI's `test` attribute; use the built-in one.
    use std::prelude::v1::test;

    #[test]
    fn rejection_messages_name_what_the_field_takes() {
        assert_eq!(filter_description(QueryFilter::EDGE), "Select edges.");
        assert_eq!(filter_description(QueryFilter::ANY_EDGE), "Select edges or sketch curves.");
        let m = filter_description(QueryFilter::SKETCH_REGION | QueryFilter::ANY_FACE);
        assert_eq!(m, "Select sketch regions or faces.");
        let m = filter_description(QueryFilter::PLANE_LIKE);
        assert_eq!(m, "Select planar faces, planes or mate connectors.");
    }
}
