//! Onshape assembly behaviours (child module of `assembly_view`): the
//! instance / mate / mate connector context menus in Onshape's order,
//! hide / show / isolate / make transparent on hovered or selected
//! instances, Unfix all, copy / paste of instances, inline rename (Shift+N),
//! the instance list filter with colon commands, mate Reset and Apply limit
//! position, and the S shortcut toolbar.

use super::*;
use gpui_kit::component::input::InputEvent;
use gpui_kit::component::Disableable as _;
use crate::os_menu::{OsMenu, OsMenuItem};
use noshape_model::assembly::ops::{FilterCommand, InstanceClipboard, ListFilter, RowInfo, RowKind};
use std::rc::Rc;

pub type MenuAct = Rc<dyn Fn(&mut AssemblyView, &mut Window, &mut Context<AssemblyView>)>;

/// One context menu entry (built from the view state when the menu opens).
#[derive(Clone)]
pub enum MenuEntry {
    Item { label: String, icon: Option<&'static str>, disabled: bool, act: MenuAct },
    Sub { label: String, children: Vec<MenuEntry> },
    Sep,
}

pub(super) fn item(label: impl Into<String>, act: impl Fn(&mut AssemblyView, &mut Window, &mut Context<AssemblyView>) + 'static) -> MenuEntry {
    MenuEntry::Item { label: label.into(), icon: None, disabled: false, act: Rc::new(act) }
}

fn icon_item(label: impl Into<String>, icon: &'static str, act: impl Fn(&mut AssemblyView, &mut Window, &mut Context<AssemblyView>) + 'static) -> MenuEntry {
    MenuEntry::Item { label: label.into(), icon: Some(icon), disabled: false, act: Rc::new(act) }
}

/// Fill a popup menu with entries; separators at the ends or doubled are dropped.
/// Onshape's right-click menu ([`OsMenu`]: 12 px, 22 px rows).
pub fn fill_menu(m: OsMenu, entries: Vec<MenuEntry>, view: Entity<AssemblyView>, window: &mut Window, cx: &mut Context<OsMenu>) -> OsMenu {
    // Live (behaviours "psui (2026-10-27, P3) MENU-TRUNCATE"; ASM-MENU-WRAP):
    // at most 250 px, a long item ends in "…" rather than wrapping.
    let mut m = m.truncate_labels();
    let mut cleaned: Vec<MenuEntry> = Vec::new();
    for e in entries {
        if matches!(e, MenuEntry::Sep) && cleaned.last().is_none_or(|l| matches!(l, MenuEntry::Sep)) {
            continue;
        }
        cleaned.push(e);
    }
    while cleaned.last().is_some_and(|l| matches!(l, MenuEntry::Sep)) {
        cleaned.pop();
    }
    for e in cleaned {
        match e {
            MenuEntry::Sep => m = m.separator(),
            MenuEntry::Item { label, icon, disabled, act } => {
                let v = view.clone();
                let handler: crate::os_ui::ClickHandler = Rc::new(move |_, window, cx| {
                    let act = act.clone();
                    v.update(cx, |this, cx| {
                        this.ui.ctx_menu = None;
                        act(this, window, cx);
                        cx.notify();
                    })
                });
                let mut it = OsMenuItem::Item { label: label.into(), icon: None, checked: false, disabled, handler: Some(handler), keys: Vec::new(), user: false };
                if let Some(i) = icon {
                    it = it.icon_name(i);
                }
                m = m.item(it);
            }
            MenuEntry::Sub { label, children } => {
                let v = view.clone();
                m = m.submenu(label, window, cx, move |sm, window, cx| fill_menu(sm, children.clone(), v.clone(), window, cx));
            }
        }
    }
    m
}

/// Instance row being dragged in the list.
#[derive(Clone)]
pub struct InstanceDrag {
    pub id: InstanceId,
    pub label: SharedString,
}

impl Render for InstanceDrag {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let pal = p();
        div().px_2().py_1().rounded_md().bg(pal.panel_raised).border_1().border_color(pal.accent).text_sm().text_color(pal.text).child(crate::ui::user_text(self.label.clone()))
    }
}

/// Mate feature row being dragged in the list (reorder).
#[derive(Clone)]
pub struct MateDrag {
    pub id: MateId,
    pub label: SharedString,
}

impl Render for MateDrag {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let pal = p();
        div().px_2().py_1().rounded_md().bg(pal.panel_raised).border_1().border_color(pal.accent).text_sm().text_color(pal.text).child(crate::ui::user_text(self.label.clone()))
    }
}

/// A section plane (origin, normal, offset along it, flipped) and its
/// in-plane X axis (the rotate handles turn it about X and Y).
#[derive(Clone, Debug)]
pub struct AsmSectionPlane {
    pub origin: DVec3,
    pub normal: DVec3,
    pub offset: f64,
    pub flipped: bool,
    pub x: DVec3,
}

impl AsmSectionPlane {
    pub fn new(origin: DVec3, normal: DVec3, x: DVec3) -> Self {
        let x = (x - normal * x.dot(normal)).try_normalize().unwrap_or_else(|| normal.any_orthonormal_vector());
        Self { origin, normal, offset: 0.0, flipped: false, x }
    }

    /// Clip plane (normal, d): geometry with dot(normal, p) > d is removed.
    pub fn clip(&self) -> (DVec3, f64) {
        let n = if self.flipped { -self.normal } else { self.normal };
        (n, n.dot(self.origin + self.normal * self.offset))
    }

    /// The manipulator's base (on the plane).
    pub fn base(&self) -> DVec3 {
        self.origin + self.normal * self.offset
    }

    /// The arrow's direction: the side cut away.
    pub fn dir(&self) -> DVec3 {
        if self.flipped { -self.normal } else { self.normal }
    }

    pub fn y(&self) -> DVec3 {
        self.normal.cross(self.x)
    }

    /// Live's chip suffix (behaviours "skb (10-24) assembly section"): the
    /// standard direction the arrow points along ("Top", "Back" when the
    /// Front plane is flipped, …), nothing once rotated off them.
    pub fn direction_name(&self) -> &'static str {
        let d = self.dir();
        let names = [(DVec3::Z, "Top"), (DVec3::NEG_Z, "Bottom"), (DVec3::NEG_Y, "Front"), (DVec3::Y, "Back"), (DVec3::X, "Right"), (DVec3::NEG_X, "Left")];
        names.iter().find(|(v, _)| d.dot(*v) > 1.0 - 1e-9).map(|(_, n)| *n).unwrap_or("")
    }
}

pub use crate::assembly_scene::SectionHandle;

/// Section view state (Onshape's Section view dialog, as in the Part Studio).
#[derive(Clone, Debug, Default)]
pub struct AsmSection {
    pub planes: Vec<AsmSectionPlane>,
    /// Include tab: only the listed instances are cut (Exclude: they aren't).
    pub include: bool,
    /// Instances in "Items to exclude/include".
    pub items: Vec<InstanceId>,
    /// The open dialog.
    pub dialog: Option<AsmSectionDialog>,
}

#[derive(Clone, Debug)]
pub struct AsmSectionDialog {
    /// The "Items" field takes picks (else "Section planes").
    pub items_active: bool,
    /// The plane the dialog edits (the manipulator's).
    pub active: usize,
    /// It was picked in this dialog: its chip is bold, its cap tan. Not in
    /// "Edit section view…" until a plane is picked (live dark-2437, 2403).
    pub chosen: bool,
    /// The section before the dialog opened (restored by Cancel).
    saved: Option<Box<AsmSection>>,
    /// The "Select default plane" popup and the plane it added.
    pub popup: Option<crate::section_dialog::PlanePopup>,
    pub popup_plane: Option<usize>,
}

/// Most section planes the renderer cuts with.
const MAX_SECTION_PLANES: usize = 3;

/// A drag of a section handle: the plane as it was at the press, the
/// press's grab (arrow: parameter along it; ring: the start vector around
/// its axis; base: the point on the plane) and the dragged value (length
/// from the start / angle).
#[derive(Clone, Debug)]
pub struct SectionDrag {
    pub handle: SectionHandle,
    pub plane0: AsmSectionPlane,
    pub s0: f64,
    pub v0: DVec3,
    pub grab0: DVec3,
    pub value: f64,
    pub moved: bool,
    /// Base drag: what the base snapped to under the pointer.
    pub snap: Option<SectionSnap>,
}

/// A section base snap (live behaviours "skb (10-24) section base snap + move
/// to origin" (1)): the world point, the entity highlighted orange (a face
/// outlined, an edge or vertex) and a face's corner dots.
#[derive(Clone, Debug)]
pub struct SectionSnap {
    pub point: DVec3,
    pub path: noshape_model::assembly::eval::InstancePath,
    pub entity: Reference,
    pub dots: Vec<DVec3>,
}

/// Snap distances in pixels (live: an edge of the face under the pointer
/// from ≈30–35 px, a vertex from ≈22 px).
const SECTION_SNAP_EDGE_PX: f64 = 35.0;
const SECTION_SNAP_VERTEX_PX: f64 = 22.0;

/// The value box after a section drag (live: Onshape's canvas number box,
/// focused and selected; Enter moves the plane from the drag's start).
pub struct SectionValue {
    pub handle: SectionHandle,
    pub plane0: AsmSectionPlane,
    pub input: Entity<crate::quantity_field::QuantityInput>,
    _sub: Subscription,
}

/// Row being renamed inline.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenameRow {
    Instance(InstanceId),
    Feature(MateId),
}

/// Add selection to folder…'s "Folder name" dialog: the members and the name.
pub struct FolderNameDialog {
    instances: Vec<InstanceId>,
    features: Vec<MateId>,
    input: Entity<InputState>,
    _sub: Subscription,
}

/// View state of the behaviours in this module.
#[derive(Default)]
pub struct AsmUi {
    /// A dialog or panel covers the viewport's top left (last render):
    /// zoom to fit centres the model in the rest (as the Part Studio).
    pub left_open: bool,
    /// Instance list filter box and its parsed value.
    pub filter_input: Option<Entity<InputState>>,
    pub filter: ListFilter,
    pub filter_sub: Option<Subscription>,
    /// The ≡ "Show search results as list" toggle (off: matches in their tree).
    pub filter_flat: bool,
    /// The filter box has focus (its help popover shows while it is empty).
    pub filter_focused: bool,
    /// Instance row under the cursor in the list (highlighted in the viewport).
    pub list_hover: Option<InstanceId>,
    /// Last clicked instance row: the anchor of Shift+click ranges.
    pub list_anchor: Option<InstanceId>,
    /// Pattern rows a Shift+click range ran over (selected with the
    /// instances; live: "Linear pattern 1" is in the range).
    pub range_patterns: Vec<MateId>,
    /// The face a graphics context menu was opened on (its Section view…
    /// cuts parallel to it).
    pub menu_face: Option<AsmPickTarget>,
    /// The graphics menu opened on the selected instance's triad or face:
    /// it starts with Move to origin (live ~14029; ASM-TRIAD-MENU).
    pub menu_move_origin: bool,
    /// Make transparent (Shift+T): instances drawn transparent.
    pub transparent: Option<HashSet<InstanceId>>,
    /// S shortcut toolbar position (viewport-local).
    pub mini_toolbar: Option<DVec2>,
    /// Graphics area context menu.
    pub ctx_menu: Option<(Point<Pixels>, Entity<OsMenu>, Subscription)>,
    /// Inline rename (Shift+N / Rename).
    pub rename: Option<(RenameRow, Entity<InputState>, Subscription)>,
    /// Instance list scrolling (selection in the graphics scrolls to the row).
    pub list_scroll: ScrollHandle,
    pub scroll_to_selection: bool,
    /// Mates expanded in the list (their implicit mate connectors shown).
    pub expanded_mates: HashSet<MateId>,
    /// Insert dialog: Sketches filter toggle (off by default, live).
    pub insert_sketches: bool,
    /// Insert dialog: Composite parts filter toggle (off by default, live 320).
    pub insert_composites: bool,
    /// The user's toolbar (Customize toolbar…) and the open customize mode.
    pub tb_prefs: super::toolbar::AsmToolbarPrefs,
    pub tb_custom: Option<crate::toolbar_customize::Customize>,
    /// The open Dynamic suppression dialog (instance, "Unsuppressed if").
    pub dynamic_suppression: Option<(super::props::DynTarget, Entity<InputState>, Subscription)>,
    /// Show instance list properties (state pills after the names).
    pub list_properties: bool,
    /// The instance list collapsed with the toggle at the viewport's left edge.
    pub list_collapsed: bool,
    /// Reference manager dialog (Change to version…, Update all references).
    pub ref_manager: Option<super::props::RefManager>,
    /// "Origin of new Part Studio" dialog: what its field holds.
    pub ps_in_context: Option<PsOrigin>,
    /// Tool search (Alt+C): input, query, selected row.
    pub tool_search: Option<(Entity<InputState>, Subscription)>,
    pub tool_query: String,
    pub tool_sel: usize,
    pub tool_no_highlight: bool,
    /// Named positions: the row being edited (yellow banner), the applied
    /// row (blue), the Driving mate selector, the typed cell, the toast.
    pub np_editing: Option<usize>,
    pub np_applied: Option<usize>,
    pub np_driver: Option<super::positions::NpDriver>,
    pub np_cell: Option<super::positions::NpCell>,
    pub np_toast: Option<String>,
    pub np_count: usize,
    /// The Shift+/ Keyboard shortcuts sheet (app-core's, shared by the studios).
    pub shortcut_sheet: Option<(Entity<crate::shortcut_sheet::ShortcutSheet>, Subscription)>,
    /// Insert dialog: search box, source (0 current document, 1 other
    /// documents) and element tab (0 Part Studios, 1 Assemblies).
    pub insert_search: Option<Entity<InputState>>,
    pub insert_search_sub: Option<Subscription>,
    /// The search box's text, kept here so the cached catalog tree doesn't
    /// read the box (which would re-render it on every caret blink).
    pub insert_query: String,
    /// Insert › Other documents (live 71): the "Search or paste URL" box, the
    /// open category (0 Owned by me … 4 Public) and the scanned documents.
    pub insert_url: Option<Entity<InputState>>,
    /// Its placeholder ('Search or paste URL' / 'Search in "<category>"').
    pub insert_url_ph: SharedString,
    pub insert_url_sub: Option<Subscription>,
    pub insert_other_cat: Option<usize>,
    /// Owned by me: the folder being browsed (None = the top level).
    pub insert_other_dir: Option<std::path::PathBuf>,
    pub insert_other_docs: Option<Vec<crate::library::DocEntry>>,
    pub insert_other_query: Option<String>,
    /// Latest version name of each listed document.
    pub insert_other_versions: HashMap<std::path::PathBuf, String>,
    /// The opened document (kept when the dialog closes: Onshape reopens
    /// the last document and version).
    pub insert_other_doc: Option<super::panels::OtherDoc>,
    /// Dragging a fixed instance (live 562): the cursor position for the fix
    /// glyph and the `no-drop` cursor; nothing moves, no message.
    pub fixed_drag: Option<DVec2>,
    /// The mate dialog's Animate preview ended (handled in render, which
    /// has the window to re-evaluate).
    pub preview_done: bool,
    pub insert_source: u8,
    /// Replace picker's Flat patterns filter (live 840).
    pub insert_flat: bool,
    /// Re-evaluate at the next render (a change with no Window at hand,
    /// e.g. the Replace instance preview).
    pub eval_requested: bool,
    pub insert_tab: u8,
    /// Part Studios tab type filters (Onshape's filter icons): show parts,
    /// show surfaces (both off: everything).
    pub insert_filter: (bool, bool),
    /// Instances inserted since the Insert dialog opened ("Inserted: N";
    /// undone ones drop out) and the Part Studios collapsed in its tree.
    pub insert_inserted: Vec<InstanceId>,
    pub insert_collapsed: HashSet<TabId>,
    /// Triad handle clicked (selected): right-clicking it opens its menu.
    pub manip_selected: Option<ManipHandle>,
    /// Grab offset while the floating mate toolbar is dragged.
    pub hud_grab: Arc<Mutex<Point<Pixels>>>,
    /// Insert dialog thumbnails.
    pub thumbs: super::thumbs::InsertThumbs,
    /// Show center of mass: (instances, empty = whole assembly; world point).
    pub com: Option<(Vec<InstanceId>, DVec3)>,
    /// Origin row › Hide: the assembly origin triad is not drawn (still pickable from the row).
    pub origin_hidden: bool,
    /// Snap-insert: (source connector on the inserted instance, target), and
    /// which of the instance's explicit mate connectors is the source.
    pub place_snap: Option<(MateConnectorRef, MateConnectorRef)>,
    /// The pointer moved over the graphics area since placing started.
    pub place_moved: bool,
    /// A just-inserted instance's selection: no transform manipulator on it
    /// until the selection changes (live qa5 pass 7, A15).
    pub inserted_selection: Vec<InstanceId>,
    /// The root row was clicked (it highlights only then; live A15).
    pub root_selected: bool,
    pub place_src_ix: usize,
    /// Faces, edges and vertices selected in the graphics area (live:
    /// additive clicks, each filled amber; their instances' rows selected),
    /// and their measurements for the bottom-right readout.
    pub entity_sel: Vec<(Vec<InstanceId>, Reference)>,
    pub entity_measure: Vec<(String, noshape_model::regen::measure::Measurement)>,
    /// The last graphics click (time, position): a second one there soon
    /// after is a double click, which clears the selection (live).
    pub last_click: Option<(std::time::Instant, DVec2)>,
    /// Measure tool (`[`): picked entities and their measurements.
    pub measure: Option<Vec<(Vec<InstanceId>, Reference)>>,
    pub measure_result: Vec<(String, noshape_model::regen::measure::Measurement)>,
    /// The Measure panel's type and unit choices.
    pub measure_settings: crate::measure_panel::MeasureSettings,
    /// Section view (Shift+X): plane origin, normal, offset, flipped, offset field.
    pub section: Option<AsmSection>,
    /// A drag of the section arrow in progress.
    pub section_drag: Option<SectionDrag>,
    /// The section manipulator's value box (after a drag).
    pub section_value: Option<SectionValue>,
    /// Isolate / Make transparent slider: opacity of the transparent instances.
    pub transparency: Option<f32>,
    /// Camera menu render choices (Onshape's display modes, hidden and
    /// tangent edges) and View in high quality.
    pub display_mode: crate::scene_builder::DisplayMode,
    pub hidden_edges_visible: bool,
    pub tangent_edges: crate::scene_builder::TangentEdges,
    pub high_quality: bool,
    /// Cameras before each view change, for "Previous view".
    pub cam_history: Vec<noshape_render::Camera>,
    /// The open Graphics preferences dialog.
    pub graphics_dialog: Option<(Entity<crate::graphics_prefs_dialog::GraphicsPrefsDialog>, Subscription)>,
    /// The open camera menu (its button shows the open state).
    pub camera_menu: Option<WeakEntity<crate::os_menu::OsMenu>>,
    /// The open Named views dialog's name field, and the view picked in its list.
    pub named_views: Option<(Entity<InputState>, Subscription)>,
    pub named_view_picked: Option<String>,
    /// The Graphics preferences the scene is built with (kept current).
    pub graphics: crate::graphics_prefs::GraphicsPrefs,
    /// Isolate dialog › Select transparent geometry; Alt held (pick through
    /// transparent instances).
    pub select_transparent: bool,
    pub alt_through: bool,
    /// BOM Table: the cell being typed in.
    pub bom_edit: Option<super::bom_panel::BomEdit>,
    /// BOM Table: the selected cell (row, column) and expanded Structured items.
    pub bom_selected: Option<(usize, noshape_model::assembly::bom::BomColumn)>,
    pub bom_expanded: HashSet<String>,
    /// Configurations: the current configuration (box above the instance
    /// list), the Configure assembly features mode, the "… added to
    /// Configuration" toast, the typed table cell, folded inputs, and the
    /// "Configured assembly properties" tab.
    pub config_values: ConfigValues,
    pub cfg_mode: Option<super::config_panel::CfgMode>,
    pub cfg_toast: Option<String>,
    pub cfg_edit: Option<super::config_panel::CfgEdit>,
    pub cfg_folded: HashSet<String>,
    /// Configurations panel: the "Filter inputs" field, the kind toggles
    /// (List, Checkbox, Configuration variable: show only those) and whether
    /// the toast is a green success one ("Inputs copied.").
    pub cfg_filter: Option<Entity<InputState>>,
    pub cfg_filter_sub: Option<Subscription>,
    pub cfg_only_kinds: [bool; 3],
    pub cfg_toast_ok: bool,
    pub cfg_props_tab: bool,
    /// Live mate value being typed (double-click a value label): mate, DOF.
    pub value_edit: Option<(MateId, usize, Entity<InputState>, Subscription)>,
    /// Mass and section properties dialog.
    pub mass: Option<super::mass::AsmMassUi>,
    /// The Isolate slider being dragged (pressed on it, button still down).
    pub iso_slider_drag: Option<&'static str>,
    /// Where panel dialogs were moved by their title.
    pub panels: crate::ui::PanelOffsets,
    /// Insert dialog: configuration list selects per (Part Studio, input).
    pub insert_cfg_selects: HashMap<(TabId, String), Entity<crate::os_select::OsSelect>>,
    pub insert_cfg_subs: Vec<Subscription>,
    /// The "Folder name" dialog of Add selection to folder….
    pub folder_name: Option<FolderNameDialog>,
    /// Zebra stripes: the open dialog, and the accepted settings shown after.
    pub zebra_dialog: Option<super::mass::ZebraDialog>,
    pub zebra: Option<super::mass::ZebraSettings>,
    /// Reflection analysis: the open dialog, the accepted settings, the last
    /// environment used, the uploaded environment and the picker preview.
    pub reflection_dialog: Option<super::reflection::ReflectionDialog>,
    /// Exploded views pane: the selected row (view, step) and the views
    /// expanded with their › expander.
    pub explode_sel: Option<(usize, Option<usize>)>,
    /// While the rollback bar is dragged: the view and the row boundary it
    /// would drop on (live 820: the nearest boundary, a blue line).
    pub explode_rb_drop: Option<(usize, usize)>,
    pub explode_open: HashSet<usize>,
    /// Width of the docked right pane (with its strip), set each render.
    pub dock_w: f32,
    /// The info notice whose fade-out is scheduled.
    pub msg_fading: Option<String>,
    /// Interference detection dialog.
    pub interference: Option<super::interference::InterferenceDialog>,
    pub reflection: Option<crate::reflection::ReflectionSettings>,
    pub reflection_last: Option<crate::reflection::ReflectionSettings>,
    pub env_gpu: std::cell::RefCell<Option<(String, noshape_render::ImageHandle)>>,
    pub env_preview: Option<(&'static str, Arc<RenderImage>)>,
    /// The angular value's radius, fixed when first shown for a mate.
    pub value_radius: std::cell::Cell<Option<(MateId, f64)>>,
    /// Insert dialog: configuration chosen per configured Part Studio.
    pub insert_config: HashMap<TabId, noshape_model::configuration::ConfigValues>,
    /// Display states dialog: open, name field, chosen state, renaming.
    pub display_states: bool,
    pub ds_name: Option<Entity<InputState>>,
    pub ds_selected: Option<usize>,
    pub ds_renaming: Option<Entity<InputState>>,
    /// Group dialog: (group being edited, instances).
    pub group: Option<(Option<MateId>, Vec<InstanceId>)>,
    /// Mate indicators in overlay pick order.
    pub overlay_mates: Vec<MateId>,
    /// Select other (`): its rows (the entity under the cursor, then the
    /// instances under it, nearest first), the bold one and where the list
    /// opens (viewport-local).
    pub select_other: Option<(Vec<SelectOtherRow>, usize, DVec2)>,
    /// Standard content tab of the Insert dialog and its edit dialog.
    pub std: StdUi,
    /// Isolate / Make transparent › Expand: (by connectivity, steps, the
    /// instances isolated before expanding).
    pub iso_expand: Option<(bool, usize, Vec<InstanceId>)>,
}

impl AssemblyView {
    // ------------------------------------------------------------ targets

    /// Top-level instance under the cursor (graphics or instance list).
    pub(super) fn hovered_top(&self) -> Option<InstanceId> {
        self.hover.as_ref().map(|t| t.path[0]).or(self.ui.list_hover)
    }

    /// Hovered instance plus the selection (Onshape's "selected or hovered").
    pub(super) fn hovered_or_selected(&self) -> Vec<InstanceId> {
        let mut out: Vec<InstanceId> = self.hovered_top().into_iter().collect();
        for id in &self.selected {
            if !out.contains(id) {
                out.push(*id);
            }
        }
        out
    }

    // ------------------------------------------------------------ visibility

    /// y: hide the hovered / selected instances (or the selected mate).
    pub(super) fn hide_hovered_or_selected(&mut self, cx: &mut Context<Self>) {
        if self.hover.is_none()
            && self.ui.list_hover.is_none()
            && self.selected.is_empty()
            && let Some(m) = self.selected_mate
        {
            self.edit("Hide", cx, |a| a.set_mates_hidden(&[m], true));
            return;
        }
        let ids = self.hovered_or_selected();
        if ids.is_empty() {
            self.message = Some((false, "Select or hover over an instance to hide it.".into()));
            cx.notify();
            return;
        }
        self.set_hidden(ids, true, cx);
    }

    /// Shift+Y: show hidden instances.
    pub(super) fn show_hidden(&mut self, cx: &mut Context<Self>) {
        let any = self.assembly(cx).is_some_and(|a| a.instances.iter().any(|i| i.hidden));
        if any {
            self.edit("Show all", cx, |a| a.set_all_hidden(false));
        }
    }

    pub(super) fn hide_other_instances(&mut self, keep: Vec<InstanceId>, cx: &mut Context<Self>) {
        self.edit("Hide other instances", cx, |a| a.hide_other_instances(&keep));
    }

    pub(super) fn hide_all_instances(&mut self, cx: &mut Context<Self>) {
        self.edit("Hide all instances", cx, |a| a.set_all_hidden(true));
    }

    pub(super) fn set_mates_hidden(&mut self, ids: Vec<MateId>, hidden: bool, cx: &mut Context<Self>) {
        self.edit(if hidden { "Hide" } else { "Show" }, cx, |a| a.set_mates_hidden(&ids, hidden));
        if !hidden && !self.show_mates {
            // Showing a mate indicator turns mate display on.
            self.show_mates = true;
        }
        self.refresh_scene(false, cx);
    }

    /// j: shows every mate when any is hidden (a new mate is), else hides
    /// them all, as Show all mates / Hide all mates (live; J-KEY-NO-EFFECT:
    /// it flipped a view flag that each mate's own Hidden overrode).
    pub(super) fn toggle_mates(&mut self, cx: &mut Context<Self>) {
        let Some(asm) = self.assembly(cx) else { return };
        if asm.mates.is_empty() {
            return;
        }
        let shown = self.show_mates && asm.mates.iter().all(|m| !m.hidden);
        let ids: Vec<MateId> = asm.mates.iter().map(|m| m.id).collect();
        self.set_mates_hidden(ids, shown, cx);
        cx.notify();
    }

    pub(super) fn show_all_mates(&mut self, cx: &mut Context<Self>) {
        let ids: Vec<MateId> = self.assembly(cx).map(|a| a.mates.iter().map(|m| m.id).collect()).unwrap_or_default();
        self.set_mates_hidden(ids, false, cx);
    }

    pub(super) fn show_all_connectors(&mut self, cx: &mut Context<Self>) {
        let ids: Vec<MateId> = self.assembly(cx).map(|a| a.connectors.iter().map(|c| c.id).collect()).unwrap_or_default();
        self.edit("Show all mate connectors", cx, |a| a.set_mates_hidden(&ids, false));
    }

    // ------------------------------------------------------------ isolate / transparent

    /// Shift+I / Shift+T (and the menus): cumulative while the mode is on,
    /// like Onshape (hover + key repeatedly adds instances).
    pub(super) fn isolate_cmd(&mut self, transparent: bool, ids: Vec<InstanceId>, cx: &mut Context<Self>) {
        if ids.is_empty() {
            self.message = Some((false, "Select or hover over an instance first.".into()));
            cx.notify();
            return;
        }
        self.ui.iso_expand = None;
        let set = if transparent { &mut self.ui.transparent } else { &mut self.isolate };
        match set {
            Some(s) => s.extend(ids),
            None => *set = Some(ids.into_iter().collect()),
        }
        if transparent {
            self.isolate = None;
        } else {
            self.ui.transparent = None;
        }
        self.rebuild_scene(true, cx);
        cx.notify();
    }

    pub(super) fn exit_isolate(&mut self, cx: &mut Context<Self>) -> bool {
        self.ui.iso_expand = None;
        if self.isolate.take().is_none() && self.ui.transparent.take().is_none() {
            return false;
        }
        self.rebuild_scene(true, cx);
        cx.notify();
        true
    }

    /// Onshape's Isolate / Make transparent dialog (`isolate_ui`); here the
    /// Expand select offers connectivity (by mates) and distance.
    pub(super) fn render_isolate_panel(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let title = if self.isolate.is_some() {
            "Isolate"
        } else if self.ui.transparent.is_some() {
            "Make transparent"
        } else {
            return None;
        };
        let (connectivity, steps) = self.ui.iso_expand.as_ref().map(|e| (e.0, e.1)).unwrap_or((true, 0));
        let max_steps = {
            let seeds: Vec<InstanceId> = match self.ui.iso_expand.as_ref() {
                Some(e) => e.2.clone(),
                None => self.isolate.clone().or_else(|| self.ui.transparent.clone()).map(|s| s.into_iter().collect()).unwrap_or_default(),
            };
            self.assembly(cx).map(|a| analysis::isolate_expansion(&self.result, a, &seeds, connectivity).len()).unwrap_or(0)
        };
        let alpha = self.ui.transparency.unwrap_or(if title == "Isolate" { crate::isolate_ui::ALPHA_MIN } else { 0.35 });
        let view = cx.entity();
        Some(crate::isolate_ui::dialog(
            crate::isolate_ui::IsoDialog {
                key: "asm-isolate",
                title,
                select_transparent: self.ui.select_transparent,
                transparency: crate::isolate_ui::alpha_t(alpha),
                expand: if max_steps == 0 { 0.0 } else { steps as f32 / max_steps as f32 },
                expand_options: vec!["Expand: connectivity".to_string(), "Expand: distance".to_string()],
                expand_disabled: Vec::new(),
                expand_ix: if connectivity { 0 } else { 1 },
                legend: true,
                expand_tip: if connectivity { "Expand isolate by mate connectivity" } else { "Expand isolate by distance" },
                drag: |v: &mut Self| &mut v.ui.iso_slider_drag,
                close: |v: &mut Self, _, cx| {
                    v.exit_isolate(cx);
                },
                toggle_select: |v: &mut Self, _, cx| {
                    v.ui.select_transparent = !v.ui.select_transparent;
                    // Pickability changes: the pick buffer must be redrawn.
                    v.rebuild_scene(true, cx);
                    cx.notify();
                },
                set_transparency: std::rc::Rc::new(|v: &mut Self, f, _, cx| {
                    v.ui.transparency = Some(crate::isolate_ui::t_alpha(f));
                    v.rebuild_scene(false, cx);
                    cx.notify();
                }),
                set_expand: std::rc::Rc::new(move |v: &mut Self, f, _, cx| {
                    let want = (f * max_steps as f32).round() as i32;
                    let have = v.ui.iso_expand.as_ref().map(|e| e.1).unwrap_or(0) as i32;
                    if want != have {
                        v.set_iso_expand(None, want - have, cx);
                    }
                }),
                pick_expand: std::rc::Rc::new(move |i, _, cx| view.update(cx, |this, cx| this.set_iso_expand(Some(i == 0), 0, cx))),
            },
            cx,
        ))
    }

    /// A click in the graphics area leaves text fields (like Onshape): the
    /// open mate dialog keeps the keyboard (Enter, a), else the view takes it.
    pub(super) fn focus_from_viewport(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(d) = self.mate.as_ref().map(|m| m.dialog.clone()) {
            let fh = d.read(cx).focus_handle(cx);
            if !fh.is_focused(window) {
                window.focus(&fh, cx);
            }
        } else if !self.focus.is_focused(window) {
            window.focus(&self.focus, cx);
        }
    }

    // ------------------------------------------------------------ a / q

    /// a (flip primary axis) / q (reorient secondary axis): the connector
    /// dragged in snap mode, else the open mate or mate connector dialog.
    pub(super) fn flip_or_reorient(&mut self, flip: bool, window: &mut Window, cx: &mut Context<Self>) {
        if flip && self.std_flip_placing(cx) {
            return;
        }
        if let Some(s) = &mut self.snap_drag
            && s.started
            && let Some(src) = s.sources.get_mut(s.source_ix)
        {
            if flip {
                src.flip = !src.flip;
            } else {
                src.rotate_quarters = (src.rotate_quarters + 1) % 4;
            }
            self.refresh_snap_preview(cx);
            return;
        }
        if let Some(d) = self.mate.as_ref().map(|m| m.dialog.clone()) {
            d.update(cx, |d, cx| if flip { d.flip(cx) } else { d.reorient(cx) });
            return;
        }
        if let Some(m) = &mut self.mc {
            if flip {
                m.flip = !m.flip;
            } else {
                m.reorient = (m.reorient + 1) % 4;
            }
            self.schedule_eval(window, cx);
            cx.notify();
        }
    }

    /// Onshape's instance warnings (Instances list): several fixed parts,
    /// several fixed parts inside a subassembly, fastened to the origin
    /// while the assembly has fixed parts.
    pub(super) fn instance_warning(&self, id: InstanceId, cx: &App) -> Option<&'static str> {
        let a = self.assembly(cx)?;
        let inst = a.instance(id)?;
        let fixed = a.fixed_instances();
        if inst.fixed && fixed.len() > 1 {
            return Some("More than one part is fixed");
        }
        let inside_fixed = match &inst.source {
            InstanceSource::Assembly { assembly, .. } => match self.doc.read(cx).session.doc.tab(*assembly).map(|t| &t.content) {
                Some(TabContent::Assembly(sub)) => sub.fixed_instances().len(),
                _ => 0,
            },
            _ => self.linked_assembly(id).map_or(0, |(_, sub)| sub.fixed_instances().len()),
        };
        if inside_fixed > 1 {
            return Some("More than one part inside is fixed");
        }
        let to_origin = a.mates.iter().any(|m| {
            !m.suppressed && m.mate_type == MateType::Fastened && ((m.a.instance == id && m.b.is_origin()) || (m.b.instance == id && m.a.is_origin()))
        });
        (to_origin && !fixed.is_empty()).then_some("Fastened to origin even though Assembly has fixed part(s)")
    }

    /// An instance of another document's Assembly: (its name there, content).
    pub(super) fn linked_assembly(&self, id: InstanceId) -> Option<noshape_model::assembly::eval::LinkedAssembly> {
        self.result.node(&[id]).and_then(|n| n.linked.clone())
    }

    // ------------------------------------------------------------ fix

    pub(super) fn unfix_all(&mut self, cx: &mut Context<Self>) {
        self.edit("Unfix all", cx, |a| {
            a.unfix_all();
        });
    }

    // ------------------------------------------------------------ camera

    pub(super) fn zoom_to_instances(&mut self, ids: Vec<InstanceId>, cx: &mut Context<Self>) {
        let mut bb: Option<(DVec3, DVec3)> = None;
        for id in &ids {
            if let Some((a, b)) = self.result.instance_bounds(*id) {
                bb = Some(bb.map(|(x, y)| (x.min(a), y.max(b))).unwrap_or((a, b)));
            }
        }
        let Some((lo, hi)) = bb else { return };
        self.viewport.update(cx, |vp, cx| {
            let to = noshape_scene::camera_ops::fit_instances(&vp.camera, lo, hi, vp.size());
            vp.animate_to(to, cx);
        });
    }

    // ------------------------------------------------------------ copy / paste

    /// Ctrl+C: copy the selected instances (with the mates and mate
    /// connectors among them) to the clipboard.
    pub(super) fn copy_selection(&mut self, cx: &mut Context<Self>) {
        let Some(asm) = self.assembly(cx) else { return };
        let ids: Vec<InstanceId> = self.selected.iter().copied().filter(|id| asm.generated_by(*id).is_none()).collect();
        if ids.is_empty() {
            self.message = Some((false, "Select instances to copy.".into()));
            cx.notify();
            return;
        }
        let clip = InstanceClipboard::copy(asm, &ids);
        let n = clip.instances.len();
        if let Ok(json) = serde_json::to_string(&serde_json::json!({ "noshape_instances": clip })) {
            cx.write_to_clipboard(ClipboardItem::new_string(json));
            self.message = Some((false, if n == 1 { format!("Copied {}", clip.instances[0].name) } else { format!("Copied {n} items") }));
            cx.notify();
        }
    }

    /// Ctrl+V: paste copied instances; they follow the cursor until a
    /// click places them (Onshape: pasted where the mouse is clicked).
    pub(super) fn paste_instances(&mut self, cx: &mut Context<Self>) {
        let Some(text) = cx.read_from_clipboard().and_then(|c| c.text()) else { return };
        let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else { return };
        let Some(clip) = v.get("noshape_instances").and_then(|c| serde_json::from_value::<InstanceClipboard>(c.clone()).ok()) else {
            self.message = Some((false, "Nothing to paste: copy instances first.".into()));
            cx.notify();
            return;
        };
        // Sources must exist in this document (Part Studios / assemblies).
        let doc = self.doc.read(cx).session.doc.clone();
        let valid = clip.instances.iter().all(|i| match &i.source {
            InstanceSource::Part { studio, .. } | InstanceSource::PartStudio { studio, .. } | InstanceSource::MirroredPart { studio, .. } | InstanceSource::Sketch { studio, .. } => doc.tab(*studio).is_some(),
            InstanceSource::Assembly { assembly, .. } => doc.tab(*assembly).is_some() && !contains_assembly(&doc, *assembly, self.tab, 0),
            InstanceSource::External { .. } | InstanceSource::Standard { .. } => true,
        });
        if !valid {
            self.message = Some((true, "The copied instances come from another document.".into()));
            cx.notify();
            return;
        }
        let ids = self.edit_insert(&noshape_ops::assembly::paste_description(&clip), cx, |a| clip.paste(a, [0.0; 3])).unwrap_or_default();
        self.start_placing(ids, cx);
    }

    // ------------------------------------------------------------ rename

    pub(super) fn start_row_rename(&mut self, row: RenameRow, window: &mut Window, cx: &mut Context<Self>) {
        let Some(asm) = self.assembly(cx) else { return };
        let current = match row {
            RenameRow::Instance(id) => asm.instance(id).map(|i| i.name.clone()),
            RenameRow::Feature(id) => asm
                .mate(id)
                .map(|m| m.name.clone())
                .or_else(|| asm.relations.iter().find(|r| r.id == id).map(|r| r.name.clone()))
                .or_else(|| asm.groups.iter().find(|g| g.id == id).map(|g| g.name.clone()))
                .or_else(|| asm.connector(id).map(|c| c.name.clone()))
                .or_else(|| asm.pattern(id).map(|p| p.name.clone())),
        };
        let current = current.or_else(|| match row {
            RenameRow::Feature(id) => asm.folders.iter().find(|f| f.id == id).map(|f| f.name.clone()),
            RenameRow::Instance(_) => None,
        });
        let Some(current) = current else { return };
        let st = cx.new(|cx| InputState::new(window, cx).default_value(current));
        // Focus after the context menu that started the rename has closed
        // (it gives focus back to where it was opened).
        let st2 = st.clone();
        cx.spawn_in(window, async move |_, cx| {
            cx.background_executor().timer(Duration::from_millis(40)).await;
            let _ = st2.update_in(cx, |s, window, cx| {
                s.focus(window, cx);
                s.select_all(window, cx);
            });
        })
        .detach();
        let sub = cx.subscribe_in(&st, window, |this: &mut Self, _, ev: &InputEvent, window, cx| match ev {
            InputEvent::PressEnter { .. } => {
                this.commit_row_rename(cx);
                window.focus(&this.focus, cx);
            }
            InputEvent::Blur => this.commit_row_rename(cx),
            _ => {}
        });
        self.ui.rename = Some((row, st, sub));
        cx.notify();
    }

    pub(super) fn commit_row_rename(&mut self, cx: &mut Context<Self>) {
        let Some((row, st, _)) = self.ui.rename.take() else { return };
        let name = st.read(cx).value().trim().to_string();
        let unchanged = self.assembly(cx).is_some_and(|a| match row {
            RenameRow::Instance(id) => a.instance(id).is_some_and(|i| i.name == name),
            RenameRow::Feature(_) => false,
        });
        if !name.is_empty() && !unchanged {
            let (inst, feat) = match row {
                RenameRow::Instance(id) => (Some(id), None),
                RenameRow::Feature(id) => (None, Some(id)),
            };
            self.edit(&format!("Rename : {name}"), cx, |a| a.rename(inst, feat, &name));
        }
        cx.notify();
    }

    /// The inline rename input when `row` is being renamed. Esc cancels it:
    /// the old name stays (live; ASM-RENAME-ESC).
    pub(super) fn rename_field(&self, row: RenameRow, cx: &mut Context<Self>) -> Option<AnyElement> {
        let (r, st, _) = self.ui.rename.as_ref()?;
        (*r == row).then(|| {
            div()
                .flex_1()
                .on_key_down(cx.listener(|this, ev: &KeyDownEvent, window, cx| {
                    if ev.keystroke.key == "escape" {
                        // Dropping the subscription first: the blur commits nothing.
                        this.ui.rename = None;
                        window.focus(&this.focus, cx);
                        cx.stop_propagation();
                        cx.notify();
                    }
                }))
                .child(Input::new(st).xsmall())
                .into_any_element()
        })
    }

    /// Shift+N: rename the selected instance or mate feature.
    pub(super) fn rename_selected(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(m) = self.selected_mate {
            self.start_row_rename(RenameRow::Feature(m), window, cx);
        } else if let Some(&id) = self.selected.first() {
            self.start_row_rename(RenameRow::Instance(id), window, cx);
        }
    }

    // ------------------------------------------------------------ mate positions

    /// Mate context menu > Reset: back to the mate's home position (all of
    /// its free values zero), other mates enforced.
    pub(super) fn reset_mate(&mut self, id: MateId, cx: &mut Context<Self>) {
        let Some(m) = self.assembly(cx).and_then(|a| a.mate(id).cloned()) else { return };
        if matches!(m.mate_type, MateType::Fastened | MateType::Tangent | MateType::Width) {
            return;
        }
        let solved = self.result.prepared.solve(&SolveOptions { locks: vec![(id, [0.0; 4])], ..Default::default() });
        self.apply_solved(solved, cx);
        self.edit(&format!("Reset : {}", m.name), cx, |_| {});
    }

    /// Mate context menu > Apply limit position > Min/Max: move the mate to
    /// the limit of one degree of freedom (the others stay where they are).
    pub(super) fn apply_limit_position(&mut self, id: MateId, key: &'static str, max: bool, cx: &mut Context<Self>) {
        let Some(m) = self.assembly(cx).and_then(|a| a.mate(id).cloned()) else { return };
        if self.result.mates.iter().any(|s| s.id == id && !s.ok) {
            self.message = Some((true, "Mates with errors cannot be applied.".into()));
            cx.notify();
            return;
        }
        let units = self.units(cx);
        let Some(ix) = noshape_model::assembly::eval::limit_index(key) else { return };
        let Some(l) = m.limits.iter().find(|(k, _)| k == key).map(|(_, l)| l.clone()) else { return };
        let dim = if ix == 0 { noshape_units::Dim::ANGLE } else { noshape_units::Dim::LENGTH };
        let e = if max { l.max } else { l.min };
        let Some(v) = e.and_then(|e| noshape_units::evaluate_as(&e.text, dim, &units, &noshape_units::EmptyScope).ok()).map(|q| q.value) else {
            self.message = Some((true, "Unable to apply limit".into()));
            cx.notify();
            return;
        };
        let solved = self.result.prepared.solve(&SolveOptions { hold_values: vec![(id, ix, v)], ..Default::default() });
        self.apply_solved(solved, cx);
        self.edit(&format!("Apply limit position : {}", m.name), cx, |_| {});
        let axis = match key {
            "rz" => "Z",
            "x" => "X",
            "y" => "Y",
            _ => "Z",
        };
        self.message = Some((false, format!("Moved mate to {} {axis} limit", if max { "max" } else { "min" })));
        cx.notify();
    }

    // ------------------------------------------------------------ context menus

    /// Instance context menu, in Onshape's order: the Instances list row
    /// menu, or (`graphics`) the menu of the selected instances in the
    /// graphics area (live Onshape, 2026-10-18).
    pub(super) fn instance_menu(&self, top: InstanceId, graphics: bool, cx: &App) -> Vec<MenuEntry> {
        let Some(asm) = self.assembly(cx) else { return vec![] };
        let Some(inst) = asm.instance(top).cloned() else { return vec![] };
        let targets = self.targets(Some(top));
        let multi = targets.len() > 1;
        let (fixed, hidden, suppressed) = (inst.fixed, inst.hidden, inst.suppressed);
        let is_asm = matches!(inst.source, InstanceSource::Assembly { .. }) || self.linked_assembly(top).is_some();
        let isolating = self.isolate.is_some() || self.ui.transparent.is_some();
        let name = inst.name.clone();
        let mut m: Vec<MenuEntry> = Vec::new();
        if isolating {
            m.push(item("Exit isolate", |t, _, cx| {
                t.exit_isolate(cx);
            }));
            m.push(MenuEntry::Sep);
        }
        // Properties (list only): the part's metadata and the instance's own.
        if !graphics && !multi {
            if matches!(inst.source, InstanceSource::Part { .. } | InstanceSource::Assembly { .. } | InstanceSource::PartStudio { .. }) {
                m.push(item("Properties…", move |t, w, cx| t.open_instance_metadata(top, w, cx)));
            }
            m.push(item("Instance properties…", move |t, w, cx| t.open_instance_properties(top, w, cx)));
            m.push(MenuEntry::Sep);
        }
        // Visibility group.
        let tg = targets.clone();
        m.push(icon_item(if hidden { "Show" } else { "Hide" }, "eye-off", move |t, _, cx| t.set_hidden(tg.clone(), !hidden, cx)));
        let tg = targets.clone();
        m.push(item(crate::i18n::template("Hide other __label__", &[("label", "instances")]), move |t, _, cx| t.hide_other_instances(tg.clone(), cx)));
        m.push(item(crate::i18n::template("Hide all __label__", &[("label", "instances")]), |t, _, cx| t.hide_all_instances(cx)));
        let tg = targets.clone();
        let com_on = self.ui.com.as_ref().is_some_and(|(c, _)| *c == targets);
        m.push(icon_item(if com_on { "Hide center of mass" } else { "Show center of mass" }, "com-indicator", move |t, _, cx| t.toggle_center_of_mass(tg.clone(), cx)));
        let tg = targets.clone();
        m.push(item("Isolate…", move |t, _, cx| t.isolate_cmd(false, tg.clone(), cx)));
        let tg = targets.clone();
        m.push(item("Make transparent…", move |t, _, cx| t.isolate_cmd(true, tg.clone(), cx)));
        if graphics {
            m.push(icon_item("Section view…", "section-view-3d", |t, w, cx| t.toggle_section(w, cx)));
        }
        let tg = targets.clone();
        if inst.unsuppressed_if.is_some() && !multi {
            // Suppressed with an expression (live assembly/142): Unsuppress
            // (drops the expression) and Edit dynamic suppression.
            m.extend(AssemblyView::dynamic_suppression_entries(super::props::DynTarget::Instance(top), true));
        } else {
            let sup = if suppressed { "Unsuppress" } else { "Suppress" };
            m.push(item(if graphics { format!("{sup} {name}") } else { sup.to_string() }, move |t, _, cx| t.set_suppressed(tg.clone(), !suppressed, cx)));
            // Dynamic suppression ▸ Add expression…
            if !graphics && !multi {
                m.extend(AssemblyView::dynamic_suppression_entries(super::props::DynTarget::Instance(top), false));
            }
        }
        let tg = targets.clone();
        m.push(icon_item(if fixed { "Unfix" } else { "Fix" }, "fix-instance", move |t, _, cx| t.set_fixed(tg.clone(), !fixed, cx)));
        // Use best available tessellation (finest meshes for these instances).
        let best = inst.best_tessellation;
        let tg = targets.clone();
        m.push(item(if best { "Use automatic tessellation setting" } else { "Use best available tessellation" }, move |t, _, cx| t.set_best_tessellation(tg.clone(), !best, cx)));
        // Live (behaviours "qa4 S2" instance menu, "skb sc stack" (7);
        // locales/de-menu-long-asm-instance-*): with mates, "Show mates" and
        // "Hide mates" follow Use best available tessellation in both menus
        // (they show / hide the selected instances' mates).
        let mates: Vec<MateId> = asm.mates.iter().filter(|mt| mt.instances().iter().any(|i| targets.contains(i))).map(|mt| mt.id).collect();
        if !mates.is_empty() {
            let ids = mates.clone();
            m.push(item("Show mates", move |t, _, cx| t.set_mates_hidden(ids.clone(), false, cx)));
            m.push(item("Hide mates", move |t, _, cx| t.set_mates_hidden(mates.clone(), true, cx)));
        }
        // (With mates too: their order against Check interference… is
        // UNCONFIRMED.)
        if is_asm || multi {
            let tg = targets.clone();
            m.push(item("Check interference…", move |t, _, cx| t.open_interference_with(tg.clone(), cx)));
        }
        m.push(MenuEntry::Sep);
        let tg = targets.clone();
        let copy_label = if multi { format!("Copy {} items", targets.len()) } else { format!("Copy {name}") };
        let copy = icon_item(copy_label, "copy", move |t, _, cx| {
            t.selected = tg.clone();
            t.copy_selection(cx);
        });
        let drawing = self.instance_drawing_entry(&inst, multi);
        let tg = targets.clone();
        // Live (menus/rmb-asm-two-instances-selected, rmb-asm-list-instance): no icon.
        let export = item("Export…", move |t, w, cx| t.open_asm_export(Some(tg.clone()), w, cx));
        // Edit group: mate connector, replace, switch to the source.
        let mut edit: Vec<MenuEntry> = Vec::new();
        // Graphics (live rmb-asm-two-instances-selected): Edit in context,
        // Find in instance list, Add mate connector…, Replace…, Switch to….
        if !multi && matches!(inst.source, InstanceSource::Part { .. } | InstanceSource::PartStudio { .. }) {
            // With contexts already in the Part Studio: pick one or Create
            // new context; Update context ▸ re-snapshots one (help:
            // Modeling in context).
            let studio = inst.source.studio();
            let contexts = studio.map(|s| self.contexts_in(s, cx)).unwrap_or_default();
            let mine = self.instance_contexts(top, cx);
            if contexts.is_empty() {
                edit.push(item("Edit in context", move |t, _, cx| t.edit_in_context(top, cx)));
            } else if let Some(studio) = studio {
                // Live 513–518: Edit in context ▸ [Edit Context 1 | Create new
                // context]; Update context ▸ [• Context 1] (dot = update
                // available); on other instances of the part, Set as primary
                // instance ▸ [Context 1].
                let mut sub: Vec<MenuEntry> = contexts.iter().map(|(id, name)| {
                    let id = *id;
                    item(format!("Edit {name}"), move |t, _, cx| t.edit_in_context_with(top, Some(id), cx))
                }).collect();
                sub.push(item("Create new context", move |t, _, cx| t.edit_in_context_with(top, None, cx)));
                edit.push(MenuEntry::Sub { label: "Edit in context".into(), children: sub });
                let upd: Vec<MenuEntry> = contexts.iter().map(|(id, name)| {
                    let id = *id;
                    let stale = mine.iter().any(|c| c.1 == id && c.4);
                    item(if stale { format!("• {name}") } else { name.clone() }, move |t, _, cx| t.update_context(top, studio, id, cx))
                }).collect();
                edit.push(MenuEntry::Sub { label: "Update context".into(), children: upd });
                let not_primary: Vec<MenuEntry> = mine.iter().filter(|c| !c.3).map(|c| {
                    let id = c.1;
                    item(c.2.clone(), move |t, _, cx| t.set_primary_instance(top, studio, id, cx))
                }).collect();
                if !not_primary.is_empty() {
                    edit.push(MenuEntry::Sub { label: "Set as primary instance".into(), children: not_primary });
                }
            }
        }
        if graphics {
            edit.push(item("Find in instance list", move |t, _, cx| {
                t.selected = vec![top];
                t.ui.scroll_to_selection = true;
                t.refresh_scene(false, cx);
            }));
        }
        if !multi {
            edit.push(icon_item("Add mate connector to instance origin…", "add-mate-connector", move |t, w, cx| t.mc_at_instance_origin(top, w, cx)));
        }
        edit.push(item(
            match (graphics, multi) {
                (_, true) => format!("Replace {} instances…", targets.len()),
                (true, false) => format!("Replace {name}…"),
                (false, false) => "Replace instances…".to_string(),
            },
            move |t, _, cx| t.open_replace(top, cx),
        ));
        let switch_tab = match &inst.source {
            InstanceSource::Assembly { assembly, .. } => Some(*assembly),
            other => other.studio(),
        };
        if let Some(tab) = switch_tab.filter(|_| !multi)
            && let Some(tname) = self.doc.read(cx).session.doc.tab(tab).map(|t| t.name.clone())
        {
            edit.push(icon_item(format!("Switch to {tname}"), if is_asm { "assembly" } else { "part-studio" }, move |_, _, cx| cx.emit(AssemblyViewEvent::ActivateTab(tab))));
        }
        // From another document: "Open linked document" in that slot (bundle
        // `switchToImageOwner`), opening the referenced version.
        if !multi
            && let Some(key) = inst.source.external_key().map(|k| k.to_string())
            && let Some(r) = crate::derived::ExternalRef::parse(&key)
        {
            let gpu = self.gpu.clone();
            edit.push(item("Open linked document", move |_, _, cx| {
                use noshape_model::assembly::eval::ExternalStudios as _;
                // The referenced version, on the referenced tab.
                let Ok(handle) = crate::assembly_hooks::open_version_window(r.path.clone(), r.microversion, gpu.clone(), cx) else { return };
                if let Some((_, tab)) = AppExternal.studio(&key) {
                    let _ = handle.update(cx, |_, window, cx| window.defer(cx, move |window, cx| crate::actions::request_activate_tab(tab, window, cx)));
                }
            }));
        }
        // A configured assembly: "Configure suppression" (live 199).
        if !multi && self.can_configure_suppression(top, cx) {
            edit.push(item("Configure suppression", move |t, _, cx| t.configure_instance_suppression(top, cx)));
        }
        edit.extend(self.instance_source_entries(&inst, multi, cx));
        if graphics {
            m.push(copy);
            m.push(MenuEntry::Sep);
            m.extend(drawing);
            let tab = self.tab;
            let aname = self.doc.read(cx).session.doc.tab(tab).map(|t| t.name.clone()).unwrap_or_default();
            m.push(icon_item(format!("Create Drawing of {aname}…"), "menu-create-drawing", move |_, w, cx| {
                let source = Some(noshape_model::drawing::create::DrawingSource { tab, part: None, config: ConfigValues::default(), sketch: None });
                crate::drawing_create::request_create_drawing(crate::drawing_create::CreateRequest { source, name: Some(aname.clone()) }, w, cx);
            }));
            m.push(export);
            m.push(MenuEntry::Sep);
            m.extend(edit);
            m.push(MenuEntry::Sep);
            // Live sel-17: Clear selection | Select other… in one group (no
            // Clear selection on an unselected instance with nothing selected).
            if !self.selected.is_empty() || !self.ui.entity_sel.is_empty() {
                m.push(item("Clear selection", |t, _, cx| {
                    t.clear_selection();
                    t.refresh_scene(false, cx);
                }));
            }
            m.push(item("Select other…", |t, _, cx| t.select_other(false, cx)));
            m.push(MenuEntry::Sep);
            // Live (menus/rmb-asm-two-instances-selected): the view items have no icons.
            m.push(item("Zoom to fit", |t, _, cx| t.viewport.update(cx, |vp, cx| vp.zoom_to_fit(true, cx))));
            let tg = targets.clone();
            m.push(item("Zoom to selection", move |t, _, cx| t.zoom_to_instances(tg.clone(), cx)));
            if let Some((forward, up)) = self.hovered_face_normal(cx) {
                m.push(item("View normal to", move |t, _, cx| t.viewport.update(cx, |vp, cx| vp.animate_to_direction(forward, up, cx))));
            }
            m.push(MenuEntry::Sep);
            let tg = targets;
            m.push(icon_item(if multi { "Delete".to_string() } else { format!("Delete {name}") }, "delete", move |t, _, cx| t.delete_instances(tg.clone(), cx)));
            return m;
        }
        m.push(MenuEntry::Sep);
        m.extend(edit);
        m.push(MenuEntry::Sep);
        m.push(copy);
        m.push(MenuEntry::Sep);
        let tg = targets.clone();
        m.push(item("Move to new subassembly", move |t, w, cx| t.move_to_new_subassembly(tg.clone(), w, cx)));
        if is_asm {
            let tg: Vec<InstanceId> = targets.iter().copied().filter(|id| asm.instance(*id).is_some_and(|i| matches!(i.source, InstanceSource::Assembly { .. }))).collect();
            m.push(item("Dissolve subassembly", move |t, _, cx| t.dissolve_subassemblies(tg.clone(), cx)));
        }
        if matches!(inst.source, InstanceSource::Part { .. } | InstanceSource::PartStudio { .. } | InstanceSource::External { .. }) {
            let tg = targets.clone();
            m.push(icon_item("Change to version…", "change-version", move |t, _, cx| t.open_reference_manager(Some(tg.clone()), cx)));
        }
        m.push(export);
        // Live qa5 CA2: Release… after Export… (the instances' parts).
        if matches!(inst.source, InstanceSource::Part { .. }) {
            let tg = targets.clone();
            m.push(item("Release…", move |t, w, cx| t.release_instances(tg.clone(), w, cx)));
        }
        m.push(MenuEntry::Sep);
        let tg = targets.clone();
        m.push(item("Zoom to selection", move |t, _, cx| t.zoom_to_instances(tg.clone(), cx)));
        m.push(MenuEntry::Sep);
        m.extend(drawing);
        m.push(MenuEntry::Sep);
        let tg = targets.clone();
        m.push(item("Add selection to folder…", move |t, w, cx| t.folder_from_instances(tg.clone(), w, cx)));
        let has_children = self.result.node(&[top]).is_some_and(|n| !n.children.is_empty());
        if has_children {
            m.push(MenuEntry::Sub {
                label: "Expand".into(),
                children: vec![
                    item("Expand selected", move |t, _, cx| {
                        t.expanded.insert(vec![top]);
                        cx.notify();
                    }),
                    item("Expand all", |t, _, cx| t.expand_all(true, cx)),
                ],
            });
            m.push(MenuEntry::Sub {
                label: "Collapse".into(),
                children: vec![
                    item("Collapse selected", move |t, _, cx| {
                        t.expanded.retain(|p| p[0] != top);
                        cx.notify();
                    }),
                    item("Collapse all", |t, _, cx| t.expand_all(false, cx)),
                ],
            });
        }
        m.push(item("Create empty subassembly", |t, _, cx| t.create_empty_subassembly(cx)));
        m.push(MenuEntry::Sep);
        let tg = targets;
        m.push(icon_item("Delete", "delete", move |t, _, cx| t.delete_instances(tg.clone(), cx)));
        m
    }

    /// "Create Drawing of <part>…" for the part (or subassembly) an instance shows.
    fn instance_drawing_entry(&self, inst: &Instance, multi: bool) -> Option<MenuEntry> {
        let (tab, part, config) = match &inst.source {
            InstanceSource::Part { studio, body, config } => (*studio, Some(body.clone()), config.clone()),
            InstanceSource::PartStudio { studio, config } => (*studio, None, config.clone()),
            InstanceSource::Assembly { assembly, .. } => (*assembly, None, ConfigValues::default()),
            _ => return None,
        };
        if multi {
            return None;
        }
        // The part's own name (the instance name without " <n>").
        let base = inst.name.rsplit_once(" <").map(|(b, _)| b.to_string()).unwrap_or_else(|| inst.name.clone());
        Some(icon_item(format!("Create Drawing of {base}…"), "menu-create-drawing", move |_, w, cx| {
            let source = Some(noshape_model::drawing::create::DrawingSource { tab, part: part.clone(), config: config.clone(), sketch: None });
            crate::drawing_create::request_create_drawing(crate::drawing_create::CreateRequest { source, name: Some(base.clone()) }, w, cx);
        }))
    }

    /// Source-specific instance entries: Lock/follow position (subassemblies),
    /// Edit standard content, Change configuration.
    fn instance_source_entries(&self, inst: &Instance, multi: bool, cx: &App) -> Vec<MenuEntry> {
        let mut m = Vec::new();
        let top = inst.id;
        let sub_positions: Option<Vec<String>> = match &inst.source {
            InstanceSource::Assembly { assembly, .. } => Some(match self.doc.read(cx).session.doc.tab(*assembly).map(|t| &t.content) {
                Some(TabContent::Assembly(a)) => a.named_positions.iter().map(|n| n.name.clone()).collect(),
                _ => vec![],
            }),
            // Another document's Assembly: its named positions there.
            _ => self.linked_assembly(top).map(|(_, a)| a.named_positions.iter().map(|n| n.name.clone()).collect()),
        };
        if let (false, Some(names)) = (multi, sub_positions) {
            // Lock/follow position to ▸ Lock current position | <named positions> (Onshape).
            let cur = inst.lock.clone();
            let mut children = vec![MenuEntry::Item {
                label: if cur == SubassemblyLock::Current { "✓ Lock current position".into() } else { "Lock current position".into() },
                icon: None,
                disabled: false,
                act: Rc::new(move |t: &mut AssemblyView, _: &mut Window, cx: &mut Context<AssemblyView>| t.set_sub_lock(top, SubassemblyLock::Current, cx)),
            }];
            for n in names {
                let on = cur == SubassemblyLock::Named(n.clone());
                let n2 = n.clone();
                children.push(item(if on { format!("✓ {n}") } else { n }, move |t, _, cx| t.set_sub_lock(top, SubassemblyLock::Named(n2.clone()), cx)));
            }
            if !cur.is_flexible() {
                children.push(MenuEntry::Sep);
                children.push(item("Unlock", move |t, _, cx| t.set_sub_lock(top, SubassemblyLock::Flexible, cx)));
            }
            m.push(MenuEntry::Sub { label: "Lock/follow position to".into(), children });
        }
        if matches!(inst.source, InstanceSource::Standard { .. }) {
            m.push(icon_item("Edit standard content instance…", "standard-content", move |t, _, cx| t.edit_standard_instance(top, cx)));
        }
        if !multi && matches!(inst.source, InstanceSource::Part { .. } | InstanceSource::PartStudio { .. } | InstanceSource::Standard { .. }) {
            // Change configuration: only for configured Part Studios.
            let configured = inst.source.studio().and_then(|s| self.doc.read(cx).session.doc.tab(s)).is_some_and(|t| matches!(&t.content, TabContent::PartStudio(ps) if !ps.configuration.inputs.is_empty()));
            if configured && !matches!(inst.source, InstanceSource::Standard { .. }) {
                // Live (assembly/1381): the blue configuration-table icon.
                m.push(icon_item("Change configuration…", "configuration-table", move |t, w, cx| t.open_config(top, w, cx)));
                // Live qa5 CA2: both selections follow Change configuration….
                m.push(item("Select instances with same configuration", move |t, _, cx| t.select_same_configuration(top, false, cx)));
                m.push(item("Select instances with same part and same configuration", move |t, _, cx| t.select_same_configuration(top, true, cx)));
            }
        }
        m
    }

    /// Instance menu "Select instances with same (part and same) configuration".
    fn select_same_configuration(&mut self, top: InstanceId, same_part: bool, cx: &mut Context<Self>) {
        let Some(ids) = self.assembly(cx).map(|a| noshape_ops::assembly::same_configuration(a, top, same_part)) else { return };
        self.selected = ids;
        self.selected_mate = None;
        self.refresh_scene(false, cx);
        cx.notify();
    }

    /// Instance menu Release…: the Release candidate dialog with the
    /// instances' parts (in their configurations), as the Part Studio's.
    fn release_instances(&mut self, ids: Vec<InstanceId>, window: &mut Window, cx: &mut Context<Self>) {
        let Some(asm) = self.assembly(cx) else { return };
        let mut items = Vec::new();
        for id in &ids {
            let Some(inst) = asm.instance(*id) else { continue };
            let InstanceSource::Part { studio, body, config } = &inst.source else { continue };
            // The part's name: the instance name without its " <n>".
            let name = inst.name.rsplit_once(" <").map_or(inst.name.as_str(), |(n, _)| n).to_string();
            for it in crate::assembly_hooks::release_part_items(self.doc.read(cx), *studio, std::slice::from_ref(body), &[name], config) {
                if !items.iter().any(|x: &noshape_model::release::ReleaseItem| x.same_object(&it)) {
                    items.push(it);
                }
            }
        }
        if !items.is_empty() {
            crate::assembly_hooks::open_release_candidate(self.doc.clone(), items, window, cx);
        }
    }

    /// View normal to: camera direction into the hovered planar face (and an up vector).
    fn hovered_face_normal(&self, cx: &App) -> Option<(DVec3, DVec3)> {
        let t = self.hover.as_ref().filter(|t| t.kind == PickKind::Face)?;
        let o = self.result.occurrences.get(t.occ)?;
        let r = Reference::Implicit { on: Box::new(t.reference.clone()), inference: Inference::Centroid };
        let f = noshape_model::assembly::eval::resolve_connector_in(&o.context, Some(&o.body), &r, &self.units(cx))?;
        let w = noshape_model::assembly::eval::matrix_frame(&o.transform).mul(&f);
        let n = w.z();
        Some((-n, noshape_scene::camera_ops::face_normal_up(n)))
    }

    // ------------------------------------------------------------ folders

    /// Add selection to folder…: the "Folder name" dialog (live assembly/536:
    /// 220 px, ✓ ✕, the name field "New folder" selected); ✓ creates the
    /// folder around the instances.
    pub(super) fn folder_from_instances(&mut self, ids: Vec<InstanceId>, window: &mut Window, cx: &mut Context<Self>) {
        self.open_folder_name(ids, Vec::new(), window, cx);
    }

    pub(super) fn folder_from_features(&mut self, ids: Vec<MateId>, window: &mut Window, cx: &mut Context<Self>) {
        self.open_folder_name(Vec::new(), ids, window, cx);
    }

    fn open_folder_name(&mut self, instances: Vec<InstanceId>, features: Vec<MateId>, window: &mut Window, cx: &mut Context<Self>) {
        let Some(name) = self.assembly(cx).map(|a| a.next_folder_name()) else { return };
        let input = cx.new(|cx| InputState::new(window, cx).default_value(name));
        let st = input.clone();
        cx.spawn_in(window, async move |_, cx| {
            cx.background_executor().timer(Duration::from_millis(40)).await;
            let _ = st.update_in(cx, |s, window, cx| {
                s.focus(window, cx);
                s.select_all(window, cx);
            });
        })
        .detach();
        let sub = cx.subscribe_in(&input, window, |this: &mut Self, _, ev: &InputEvent, window, cx| {
            if let InputEvent::PressEnter { .. } = ev {
                this.close_folder_name(true, window, cx);
            }
        });
        self.ui.folder_name = Some(FolderNameDialog { instances, features, input, _sub: sub });
        cx.notify();
    }

    pub(super) fn close_folder_name(&mut self, accept: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(d) = self.ui.folder_name.take() else { return };
        window.focus(&self.focus, cx);
        let name = d.input.read(cx).value().trim().to_string();
        if !accept || name.is_empty() {
            cx.notify();
            return;
        }
        let (instances, features) = (d.instances, d.features);
        let Some(fid) = self.edit(noshape_ops::assembly::ADD_FOLDER_DESCRIPTION, cx, |a| noshape_ops::assembly::add_folder(a, &instances, &features, &name)) else {
            return;
        };
        if features.is_empty() {
            self.expanded.insert(vec![InstanceId(fid.0)]);
        } else {
            self.ui.expanded_mates.insert(fid);
        }
        cx.notify();
    }

    pub(super) fn render_folder_name(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let d = self.ui.folder_name.as_ref()?;
        let pal = p();
        Some(
            v_flex()
                .id("folder-name-dialog")
                .occlude()
                .w(px(220.))
                .bg(pal.dialog_bg)
                .shadow(vec![BoxShadow { color: hsla(0., 0., 0., 0.2), offset: point(px(0.), px(5.)), blur_radius: px(10.), spread_radius: px(0.), inset: false }])
                .text_color(pal.text)
                .child(
                    h_flex()
                        .h(px(29.))
                        .pl(px(6.))
                        .bg(rgb(0xffffff))
                        .border_b_1()
                        .border_color(rgb(0xd7d7d7))
                        .child(div().flex_1().text_size(px(13.)).font_weight(FontWeight::MEDIUM).child("Folder name"))
                        .child(
                            div()
                                .id("folder-name-ok")
                                .size(px(28.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .bg(pal.accept)
                                .cursor_pointer()
                                .child(crate::assets::icon("dialog-ok", 24., cx))
                                .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.close_folder_name(true, window, cx))),
                        )
                        .child(
                            div()
                                .id("folder-name-cancel")
                                .size(px(28.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .cursor_pointer()
                                .hover(|d| d.bg(pal.panel_hover))
                                .cursor(gpui_kit::CursorStyle::Arrow).child(crate::assets::icon("dialog-cancel", 24., cx))
                                .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.close_folder_name(false, window, cx))),
                        ),
                )
                // Live: a 214 px white field with a 1 px `#1651b0` border, 3 px in.
                .child(gpui_kit::base::input::input_frame(div().m(px(3.)).h(px(30.)).px(px(4.)).flex().items_center().border_1().border_color(rgb(0x1651b0)).bg(rgb(0xffffff)).child(Input::new(&d.input).appearance(false).text_size(px(13.33)).text_color(rgb(0x000000)).pl(px(2.)))))
                .into_any_element(),
        )
    }

    /// Folder row (instance list or Mate features list).
    pub(super) fn folder_row(&self, folder: &noshape_model::assembly::ListFolder, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let id = folder.id;
        let mate_folder = folder.is_mate_folder();
        let expanded = if mate_folder { self.ui.expanded_mates.contains(&id) } else { self.expanded.contains(&vec![InstanceId(id.0)]) };
        let asm = self.assembly(cx);
        let members: Vec<InstanceId> = folder.instances.clone();
        let hidden = if mate_folder {
            asm.is_some_and(|a| folder.features.iter().all(|f| a.mate(*f).is_some_and(|m| m.hidden) || a.connector(*f).is_some_and(|c| c.hidden)))
        } else {
            !members.is_empty() && asm.is_some_and(|a| members.iter().all(|m| a.instance(*m).is_some_and(|i| i.hidden)))
        };
        let count = if mate_folder { folder.features.len() } else { folder.instances.len() };
        let view = cx.entity();
        let features = folder.features.clone();
        h_flex()
            .id(SharedString::from(format!("folder-{}", id.0)))
            .group(SharedString::from(format!("folder-row-{}", id.0)))
            .h(px(24.))
            .pl(px(6.))
            .pr_1()
            .gap_1p5()
            .rounded_sm()
            .text_sm()
            .text_color(if hidden { pal.text_faint } else { pal.text })
            .hover(|d| d.bg(pal.panel_hover))
            .child(div().w(px(12.)).child(svg().path(icon_path(if expanded { "chevron-down" } else { "chevron-right" })).size_3().text_color(pal.text_faint)))
            .child(svg().path(icon_path("folder")).size_3p5().text_color(pal.text_muted))
            .child(match self.rename_field(RenameRow::Feature(id), cx) {
                Some(field) => field,
                None => div().flex_1().truncate().child(format!("{} ({count})", folder.name)).into_any_element(),
            })
            .child(
                div().when(!hidden, |d| crate::ui::hover_reveal(d, SharedString::from(format!("folder-row-{}", id.0)))).child(
                    Button::new(SharedString::from(format!("feye-{}", id.0))).icon(Icon::empty().path(icon_path(if hidden { "eye-off" } else { "eye" }))).ghost().xsmall().on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        cx.stop_propagation();
                        if mate_folder {
                            this.set_mates_hidden(features.clone(), !hidden, cx);
                        } else {
                            this.set_hidden(members.clone(), !hidden, cx);
                        }
                    })),
                ),
            )
            .drag_over::<InstanceDrag>(move |st, _, _, _| st.bg(with_alpha(pal.accent, 0.15)))
            .on_drop(cx.listener(move |this, d: &InstanceDrag, _, cx| {
                if !mate_folder {
                    let iid = d.id;
                    this.edit("Add to folder", cx, |a| a.move_into_folder(iid, id));
                }
            }))
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                if mate_folder {
                    if !this.ui.expanded_mates.remove(&id) {
                        this.ui.expanded_mates.insert(id);
                    }
                } else {
                    let key = vec![InstanceId(id.0)];
                    if !this.expanded.remove(&key) {
                        this.expanded.insert(key);
                    }
                }
                cx.notify();
            }))
            .os_context_menu(move |menu, window, cx| {
                let entries = view.read(cx).folder_menu(id, cx);
                fill_menu(menu, entries, view.clone(), window, cx)
            })
            .into_any_element()
    }

    /// Mate features rows with mate folders folded in (a folder sits at the
    /// place of its first member).
    pub(super) fn fold_mate_rows(&self, asm: &Assembly, ids: Vec<MateId>, rows: Vec<AnyElement>, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let folders: Vec<&noshape_model::assembly::ListFolder> = asm.folders.iter().filter(|f| f.is_mate_folder()).collect();
        if folders.is_empty() {
            return rows;
        }
        let mut members: HashMap<MateId, Vec<AnyElement>> = HashMap::new();
        let mut order: Vec<(Option<MateId>, Option<AnyElement>)> = Vec::new();
        for (id, row) in ids.into_iter().zip(rows) {
            match folders.iter().find(|f| f.features.contains(&id)) {
                Some(f) => {
                    let e = members.entry(f.id).or_default();
                    if e.is_empty() {
                        order.push((Some(f.id), None));
                    }
                    e.push(row);
                }
                None => order.push((None, Some(row))),
            }
        }
        let mut out = Vec::new();
        for (fid, row) in order {
            match (fid, row) {
                (Some(fid), _) => {
                    let Some(f) = folders.iter().find(|f| f.id == fid) else { continue };
                    out.push(self.folder_row(f, cx));
                    let rows = members.remove(&fid).unwrap_or_default();
                    if self.ui.expanded_mates.contains(&fid) {
                        out.extend(rows.into_iter().map(|r| div().pl(px(14.)).child(r).into_any_element()));
                    }
                }
                (None, Some(r)) => out.push(r),
                _ => {}
            }
        }
        out
    }

    /// Folder context menu (Onshape order).
    pub(super) fn folder_menu(&self, id: MateId, cx: &App) -> Vec<MenuEntry> {
        let Some(f) = self.assembly(cx).and_then(|a| a.folders.iter().find(|f| f.id == id).cloned()) else { return vec![] };
        let mut m = vec![item("Rename", move |t, w, cx| t.start_row_rename(RenameRow::Feature(id), w, cx)), MenuEntry::Sep];
        if f.is_mate_folder() {
            let feats = f.features.clone();
            let (f1, f2, f3) = (feats.clone(), feats.clone(), feats.clone());
            m.push(icon_item("Hide", "eye-off", move |t, _, cx| t.set_mates_hidden(f1.clone(), true, cx)));
            m.push(item("Show", move |t, _, cx| t.set_mates_hidden(f2.clone(), false, cx)));
            m.push(item("Hide other mate connectors and mates", move |t, _, cx| {
                t.edit("Hide other mate connectors and mates", cx, |a| a.hide_other_mates(&f3));
            }));
            m.push(item("Hide all mate connectors and mates", |t, _, cx| {
                t.edit("Hide all mate connectors and mates", cx, |a| a.hide_other_mates(&[]));
            }));
            let insts: Vec<InstanceId> = {
                let mut v = Vec::new();
                for x in &feats {
                    for i in self.mate_instances(*x, cx) {
                        if !v.contains(&i) {
                            v.push(i);
                        }
                    }
                }
                v
            };
            let i2 = insts.clone();
            m.push(item("Isolate…", move |t, _, cx| t.isolate_cmd(false, insts.clone(), cx)));
            m.push(item("Make transparent…", move |t, _, cx| t.isolate_cmd(true, i2.clone(), cx)));
            let f4 = feats.clone();
            m.push(item("Suppress", move |t, _, cx| {
                t.edit("Suppress", cx, |a| {
                    for x in &f4 {
                        if let Some(mm) = a.mates.iter_mut().find(|mm| mm.id == *x) {
                            mm.suppressed = true;
                        }
                    }
                });
            }));
            m.push(MenuEntry::Sep);
            if let Some(sel) = self.selected_mate.filter(|s| !feats.contains(s)) {
                m.push(item("Add selection to folder…", move |t, _, cx| {
                    t.edit("Add selection to folder", cx, |a| {
                        for x in &mut a.folders {
                            x.features.retain(|y| *y != sel);
                        }
                        if let Some(x) = a.folders.iter_mut().find(|x| x.id == id) {
                            x.features.push(sel);
                        }
                    });
                }));
            }
            m.push(item("Expand", move |t, _, cx| {
                t.ui.expanded_mates.insert(id);
                cx.notify();
            }));
            m.push(item("Collapse", move |t, _, cx| {
                t.ui.expanded_mates.remove(&id);
                cx.notify();
            }));
        } else {
            let insts = f.instances.clone();
            let (a1, a2, a3, a4, a5) = (insts.clone(), insts.clone(), insts.clone(), insts.clone(), insts.clone());
            let hidden = self.assembly(cx).is_some_and(|a| !insts.is_empty() && insts.iter().all(|i| a.instance(*i).is_some_and(|x| x.hidden)));
            m.push(icon_item(if hidden { "Show" } else { "Hide" }, "eye-off", move |t, _, cx| t.set_hidden(a1.clone(), !hidden, cx)));
            m.push(item(crate::i18n::template("Hide other __label__", &[("label", "instances")]), move |t, _, cx| t.hide_other_instances(a2.clone(), cx)));
            m.push(item(crate::i18n::template("Hide all __label__", &[("label", "instances")]), |t, _, cx| t.hide_all_instances(cx)));
            m.push(item("Isolate…", move |t, _, cx| t.isolate_cmd(false, a3.clone(), cx)));
            m.push(item("Make transparent…", move |t, _, cx| t.isolate_cmd(true, a4.clone(), cx)));
            let suppressed = self.assembly(cx).is_some_and(|a| !insts.is_empty() && insts.iter().all(|i| a.instance(*i).is_some_and(|x| x.suppressed)));
            if f.unsuppressed_if.is_some() {
                m.extend(AssemblyView::dynamic_suppression_entries(super::props::DynTarget::Folder(id), true));
            } else {
                m.push(item(if suppressed { "Unsuppress" } else { "Suppress" }, move |t, _, cx| t.set_suppressed(a5.clone(), !suppressed, cx)));
                m.extend(AssemblyView::dynamic_suppression_entries(super::props::DynTarget::Folder(id), false));
            }
            // Live assembly/537: Unfix all on a folder holding fixed instances.
            let fixed: Vec<InstanceId> = self.assembly(cx).map(|a| insts.iter().copied().filter(|i| a.instance(*i).is_some_and(|x| x.fixed)).collect()).unwrap_or_default();
            if !fixed.is_empty() {
                m.push(icon_item("Unfix all", "fix", move |t, _, cx| t.set_fixed(fixed.clone(), false, cx)));
            }
            m.push(MenuEntry::Sep);
            // Live 537: always offered; the Folder name dialog makes a new
            // folder from the selection (the folder's own instances when the
            // folder row itself is the selection).
            let sel: Vec<InstanceId> = if self.selected.is_empty() { insts.clone() } else { self.selected.clone() };
            m.push(item("Add selection to folder…", move |t, w, cx| t.folder_from_instances(sel.clone(), w, cx)));
            // Expand ▸ / Collapse ▸ (live assembly/537).
            let key = vec![InstanceId(id.0)];
            let k2 = key.clone();
            m.push(MenuEntry::Sub {
                label: "Expand".into(),
                children: vec![
                    item("Expand selected", move |t, _, cx| {
                        t.expanded.insert(key.clone());
                        cx.notify();
                    }),
                    item("Expand all", |t, _, cx| t.expand_all(true, cx)),
                ],
            });
            m.push(MenuEntry::Sub {
                label: "Collapse".into(),
                children: vec![
                    item("Collapse selected", move |t, _, cx| {
                        t.expanded.remove(&k2);
                        cx.notify();
                    }),
                    item("Collapse all", |t, _, cx| t.expand_all(false, cx)),
                ],
            });
        }
        m.push(MenuEntry::Sep);
        m.push(icon_item("Delete", "remove-circle", move |t, _, cx| {
            t.edit("Delete folder", cx, |a| a.delete_folder(id));
        }));
        m.push(item("Unpack folder", move |t, _, cx| {
            t.edit(noshape_ops::assembly::UNPACK_FOLDER_DESCRIPTION, cx, |a| a.unpack_folder(id));
        }));
        m
    }

    // ------------------------------------------------------------ subassemblies

    /// Lock/follow position to: lock a subassembly at its current position or
    /// a named position (rigid), or unlock it (flexible).
    pub(super) fn set_sub_lock(&mut self, id: InstanceId, lock: SubassemblyLock, cx: &mut Context<Self>) {
        let name = self.assembly(cx).and_then(|a| a.instance(id).map(|i| i.name.clone())).unwrap_or_default();
        let (desc, message) = noshape_ops::assembly::lock_description(&name, &lock);
        self.message = Some((false, message));
        self.edit(&desc, cx, |a| noshape_ops::assembly::set_lock(a, id, lock));
    }

    /// Structural edit touching several tabs, with the solved placements folded in.
    fn edit_doc(&mut self, desc: &str, cx: &mut Context<Self>, f: impl FnOnce(&mut noshape_model::Document, TabId)) {
        let solved = self.result.solved.placements.clone();
        let tab = self.tab;
        self.doc.update(cx, |d, cx| {
            d.edit_document(desc, cx, |doc| {
                if let Some(TabContent::Assembly(a)) = doc.tab_mut(tab).map(|t| &mut t.content) {
                    for (id, p) in &solved {
                        if let Some(i) = a.instance_mut(*id) {
                            i.placement = *p;
                        }
                    }
                }
                f(doc, tab)
            })
        });
    }

    /// Move to new subassembly: a new Assembly tab holding the instances
    /// (and their mates and dependent patterns), inserted here in their
    /// place. Live (request 1564): when patterns depend on the selection, a
    /// confirm modal lists the selected and the dependent references first.
    pub(super) fn move_to_new_subassembly(&mut self, ids: Vec<InstanceId>, window: &mut Window, cx: &mut Context<Self>) {
        use gpui_kit::component::WindowExt as _;
        let Some(a) = self.assembly(cx) else { return };
        let ids = noshape_ops::assembly::movable_to_subassembly(a, &ids);
        if ids.is_empty() {
            return;
        }
        let dependent: Vec<(String, &'static str)> = a
            .dependent_patterns(&ids)
            .iter()
            .filter_map(|id| a.pattern(*id))
            .map(|p| {
                let icon = match p.kind {
                    PatternKind::Linear { .. } | PatternKind::PartPattern { .. } => "linear-pattern",
                    PatternKind::Circular { .. } => "circular-pattern",
                    PatternKind::Mirror { .. } => "mirror",
                    PatternKind::Replicate { .. } => "replicate",
                };
                (p.name.clone(), icon)
            })
            .collect();
        if dependent.is_empty() {
            self.move_ids_to_new_subassembly(ids, cx);
            return;
        }
        let selected: Vec<(String, &'static str)> =
            ids.iter().filter_map(|id| a.instance(*id)).map(|i| (i.name.clone(), if matches!(i.source, InstanceSource::Assembly { .. }) { "assembly" } else { "part" })).collect();
        let this = cx.entity();
        window.open_dialog(cx, move |dialog, _, cx| {
            use crate::os_ui as os;
            let pal = p();
            let section = |title: String, rows: &[(String, &'static str)], cx: &App| {
                let mut list = v_flex().py(px(4.));
                for (name, icon) in rows {
                    list = list.child(h_flex().h(px(31.)).pl(px(16.)).gap(px(4.)).child(crate::assets::icon(icon, 14., cx)).child(div().truncate().child(crate::ui::user_text(name.clone()))));
                }
                v_flex()
                    .border_1()
                    .border_color(pal.border)
                    .child(h_flex().h(px(31.)).px(px(6.)).gap(px(2.)).bg(pal.panel_hover).font_weight(FontWeight::SEMIBOLD).child(Icon::new(IconName::ChevronDown).size(px(12.))).child(title))
                    .child(list)
            };
            let this = this.clone();
            let ids = ids.clone();
            os::modal(dialog, "Move to new subassembly", 598.)
                .child(
                    os::modal_body()
                        .gap(px(10.))
                        .child(section(format!("{} Selected references to be moved", selected.len()), &selected, cx))
                        .child(section(format!("{} Dependent references to be moved", dependent.len()), &dependent, cx)),
                )
                .footer(os::modal_footer().child(os::modal_ok(os::primary_button("move-sub-ok", "Move"))).child(os::modal_cancel(os::cancel_button("move-sub-cancel", "Cancel", cx))))
                .on_ok(move |_, _, cx| {
                    let ids = ids.clone();
                    this.update(cx, |t, cx| t.move_ids_to_new_subassembly(ids, cx));
                    true
                })
        });
    }

    fn move_ids_to_new_subassembly(&mut self, ids: Vec<InstanceId>, cx: &mut Context<Self>) {
        let mut new_id = None;
        self.edit_doc(noshape_ops::assembly::MOVE_TO_NEW_SUBASSEMBLY, cx, |doc, tab| new_id = noshape_ops::assembly::move_to_new_subassembly(doc, tab, &ids));
        if let Some(id) = new_id {
            self.selected = vec![id];
            self.selected_mate = None;
        }
    }

    /// Drag and drop onto a subassembly row: the instances move into it.
    pub(super) fn move_into_subassembly(&mut self, ids: Vec<InstanceId>, sub_inst: InstanceId, cx: &mut Context<Self>) {
        let Some(InstanceSource::Assembly { assembly: sub_tab, .. }) = self.assembly(cx).and_then(|a| a.instance(sub_inst)).map(|i| i.source.clone()) else { return };
        let doc = self.doc.read(cx).session.doc.clone();
        // Never move a subassembly into itself or one that contains it.
        if ids.iter().any(|id| self.assembly(cx).and_then(|a| a.instance(*id)).is_some_and(|i| matches!(i.source, InstanceSource::Assembly { assembly, .. } if contains_assembly(&doc, assembly, sub_tab, 0)))) {
            self.message = Some((true, "Cannot move the instance into the subassembly because it would create cyclical references".into()));
            cx.notify();
            return;
        }
        let mut moved = Vec::new();
        self.edit_doc("Move to subassembly", cx, |doc, tab| {
            let Some(TabContent::Assembly(mut sub)) = doc.tab(sub_tab).map(|t| t.content.clone()) else { return };
            if let Some(TabContent::Assembly(parent)) = doc.tab_mut(tab).map(|t| &mut t.content) {
                moved = parent.move_into_subassembly(&ids, sub_inst, &mut sub);
            }
            if let Some(t) = doc.tab_mut(sub_tab) {
                t.content = TabContent::Assembly(sub);
            }
        });
        if !moved.is_empty() {
            self.selected = vec![sub_inst];
            self.expanded.insert(vec![sub_inst]);
        }
    }

    /// Create empty subassembly: a new, empty Assembly tab inserted here.
    pub(super) fn create_empty_subassembly(&mut self, cx: &mut Context<Self>) {
        let mut new_id = None;
        self.edit_doc(noshape_ops::assembly::CREATE_EMPTY_SUBASSEMBLY, cx, |doc, tab| new_id = noshape_ops::assembly::create_empty_subassembly(doc, tab));
        if let Some(id) = new_id {
            self.selected = vec![id];
        }
    }

    /// Dissolve subassembly: its instances and mates move up into this
    /// assembly; the subassembly tab is left empty when nothing else uses it.
    pub(super) fn dissolve_subassemblies(&mut self, ids: Vec<InstanceId>, cx: &mut Context<Self>) {
        if ids.is_empty() {
            return;
        }
        let mut created = Vec::new();
        self.edit_doc(noshape_ops::assembly::DISSOLVE_SUBASSEMBLY, cx, |doc, tab| created = noshape_ops::assembly::dissolve_subassemblies(doc, tab, &ids));
        self.selected = created;
    }

    /// Top-level assembly row menu (Onshape: Rename, Show center of mass,
    /// Check interference… | Create Drawing…, Add exploded view… | Export… |
    /// Expand all, Collapse all).
    pub(super) fn assembly_menu(&self, cx: &App) -> Vec<MenuEntry> {
        let any_fixed = self.assembly(cx).is_some_and(|a| a.instances.iter().any(|i| i.fixed));
        let tab = self.tab;
        let list_props = self.ui.list_properties;
        let mut m = vec![
            item("Properties…", move |t, w, cx| t.open_assembly_properties(tab, w, cx)),
            item(if list_props { "Hide instance list properties" } else { "Show instance list properties" }, move |t, _, cx| {
                t.ui.list_properties = !list_props;
                // Kept as Preferences › Assembly settings › Display instance list properties.
                crate::assembly_hooks::set_instance_list_properties(!list_props);
                cx.notify();
            }),
            MenuEntry::Sep,
            item("Rename", |t, w, cx| t.rename_tab(w, cx)),
            icon_item(if self.ui.com.as_ref().is_some_and(|(c, _)| c.is_empty()) { "Hide center of mass" } else { "Show center of mass" }, "com-indicator", |t, _, cx| t.toggle_center_of_mass(Vec::new(), cx)),
        ];
        // Live assembly/530–532: right after Show center of mass, once an
        // instance at this level is fixed.
        if any_fixed {
            m.push(icon_item("Unfix all", "fix", |t, _, cx| t.unfix_all(cx)));
        }
        m.push(item("Check interference…", |t, _, cx| {
            let all: Vec<InstanceId> = t.assembly(cx).map(|a| a.instances.iter().filter(|i| !i.suppressed).map(|i| i.id).collect()).unwrap_or_default();
            t.open_interference_with(all, cx);
        }));
        m.push(MenuEntry::Sep);
        let name = self.doc.read(cx).session.doc.tab(tab).map(|t| t.name.clone()).unwrap_or_default();
        m.push(icon_item(format!("Create Drawing of {name}…"), "menu-create-drawing", move |_, w, cx| {
            let source = Some(noshape_model::drawing::create::DrawingSource { tab, part: None, config: ConfigValues::default(), sketch: None });
            crate::drawing_create::request_create_drawing(crate::drawing_create::CreateRequest { source, name: Some(name.clone()) }, w, cx);
        }));
        m.push(item("Add exploded view…", |t, _, cx| {
            t.panel = Some(SidePanel::Explode);
            t.add_exploded_view(cx);
        }));
        m.push(MenuEntry::Sep);
        m.push(item("Export…", |t, w, cx| t.open_asm_export(None, w, cx)));
        // Live (qa5 CA2): Release… after Export… (the assembly's release items).
        m.push(item("Release…", |t, w, cx| {
            let items = crate::assembly_hooks::release_tab_items(t.doc.read(cx), t.tab);
            crate::assembly_hooks::open_release_candidate(t.doc.clone(), items, w, cx);
        }));
        m.push(MenuEntry::Sep);
        m.push(item("Expand all", |t, _, cx| t.expand_all(true, cx)));
        m.push(item("Collapse all", |t, _, cx| t.expand_all(false, cx)));
        m
    }

    /// Mate (and relation / group) context menu.
    pub(super) fn mate_menu(&self, id: MateId, cx: &App) -> Vec<MenuEntry> {
        let Some(asm) = self.assembly(cx) else { return vec![] };
        let mate = asm.mate(id).cloned();
        let suppressed = mate.as_ref().map(|m| m.suppressed).or_else(|| asm.relations.iter().find(|r| r.id == id).map(|r| r.suppressed)).or_else(|| asm.groups.iter().find(|g| g.id == id).map(|g| g.suppressed)).unwrap_or(false);
        let isolating = self.isolate.is_some() || self.ui.transparent.is_some();
        let mut m: Vec<MenuEntry> = Vec::new();
        if isolating {
            m.push(item("Exit isolate", |t, _, cx| {
                t.exit_isolate(cx);
            }));
            m.push(MenuEntry::Sep);
        }
        // Onshape: Rename, Edit… | Reset | Hide, Hide other mates, Hide all
        // mates (hidden: Show, Show all mates), Isolate…, Make transparent…,
        // Suppress | Animate… | Add selection to folder… | Delete.
        let np_editing = self.ui.np_editing.is_some();
        if !np_editing {
            m.push(item("Rename", move |t, w, cx| t.start_row_rename(RenameRow::Feature(id), w, cx)));
            m.push(item("Edit…", move |t, w, cx| t.edit_mate(id, w, cx)));
        }
        let movable = mate.as_ref().is_some_and(|mt| mt.mate_type.dof() > 0 && !matches!(mt.mate_type, MateType::Tangent | MateType::Width));
        if let Some(mt) = &mate {
            m.push(MenuEntry::Sep);
            if movable && !np_editing {
                m.push(item("Reset", move |t, _, cx| t.reset_mate(id, cx)));
            }
            let keys: Vec<&'static str> = noshape_model::assembly::eval::limit_keys(mt.mate_type).iter().copied().filter(|k| *k != "cone" && mt.limits.iter().any(|(x, _)| x == k)).collect();
            if !keys.is_empty() {
                let mut children = Vec::new();
                for k in keys {
                    let axis = match k {
                        "rz" => "Z angle",
                        "x" => "X",
                        "y" => "Y",
                        _ => "Z",
                    };
                    children.push(item(format!("Min {axis} limit"), move |t, _, cx| t.apply_limit_position(id, k, false, cx)));
                    children.push(item(format!("Max {axis} limit"), move |t, _, cx| t.apply_limit_position(id, k, true, cx)));
                }
                m.push(MenuEntry::Sub { label: "Apply limit position".into(), children });
            }
            m.push(MenuEntry::Sep);
            let hidden = mt.hidden || !self.show_mates;
            if hidden {
                m.push(icon_item("Show", "eye", move |t, _, cx| t.set_mates_hidden(vec![id], false, cx)));
                m.push(item("Show all mates", |t, _, cx| t.show_all_mates(cx)));
            } else {
                m.push(icon_item("Hide", "eye-off", move |t, _, cx| t.set_mates_hidden(vec![id], true, cx)));
                m.push(item("Hide other mates", move |t, _, cx| {
                    t.edit("Hide other mates", cx, |a| a.hide_other_mates(&[id]));
                    t.refresh_scene(false, cx);
                }));
                m.push(item("Hide all mates", |t, _, cx| {
                    let ids: Vec<MateId> = t.assembly(cx).map(|a| a.mates.iter().map(|m| m.id).collect()).unwrap_or_default();
                    t.set_mates_hidden(ids, true, cx);
                }));
            }
        }
        let ids = self.mate_instances(id, cx);
        let ids2 = ids.clone();
        m.push(item("Isolate…", move |t, _, cx| t.isolate_cmd(false, ids.clone(), cx)));
        m.push(item("Make transparent…", move |t, _, cx| t.isolate_cmd(true, ids2.clone(), cx)));
        // Named-position editing mode: no driver, suppression or Animate
        // items (live 152).
        // Live (dom/context-menus-assembly-revolute.txt; ASM-MATE-MENU-SEPARATORS):
        // in the Hide … Dynamic suppression group, no separators around it.
        if movable && !np_editing {
            m.push(item("Edit named position driver mate…", move |t, _, cx| t.open_np_driver(id, cx)));
        }
        match &mate {
            _ if np_editing => {}
            Some(mt) if mt.unsuppressed_if.is_some() => m.extend(AssemblyView::dynamic_suppression_entries(super::props::DynTarget::Mate(id), true)),
            _ => {
                m.push(item(if suppressed { "Unsuppress" } else { "Suppress" }, move |t, _, cx| t.toggle_mate_suppressed(id, cx)));
                if mate.is_some() {
                    m.extend(AssemblyView::dynamic_suppression_entries(super::props::DynTarget::Mate(id), false));
                }
            }
        }
        // Live: no Animate… for Ball (or Fastened).
        if movable && !np_editing && mate.as_ref().is_some_and(|mt| mt.mate_type != MateType::Ball) {
            m.push(MenuEntry::Sep);
            m.push(item("Animate…", move |t, w, cx| {
                t.selected_mate = Some(id);
                t.start_animation(w, cx);
            }));
        }
        m.push(MenuEntry::Sep);
        m.push(item("Add selection to folder…", move |t, w, cx| t.folder_from_features(vec![id], w, cx)));
        // Expand ▸ / Collapse ▸: the mate rows' connector children (live mate menu).
        m.push(MenuEntry::Sub {
            label: "Expand".into(),
            children: vec![
                item("Expand selected", move |t, _, cx| {
                    t.ui.expanded_mates.insert(id);
                    cx.notify();
                }),
                item("Expand all", |t, _, cx| {
                    let ids: Vec<MateId> = t.assembly(cx).map(|a| a.mates.iter().map(|m| m.id).collect()).unwrap_or_default();
                    t.ui.expanded_mates.extend(ids);
                    cx.notify();
                }),
            ],
        });
        m.push(MenuEntry::Sub {
            label: "Collapse".into(),
            children: vec![
                item("Collapse selected", move |t, _, cx| {
                    t.ui.expanded_mates.remove(&id);
                    cx.notify();
                }),
                item("Collapse all", |t, _, cx| {
                    t.ui.expanded_mates.clear();
                    cx.notify();
                }),
            ],
        });
        // Named-position editing mode: no Delete either (qa5 NP1, live 152).
        if !np_editing {
            m.push(MenuEntry::Sep);
            m.push(icon_item("Delete", "delete", move |t, _, cx| t.delete_mate(id, cx)));
        }
        m
    }

    /// Assembly mate connector context menu.
    pub(super) fn connector_menu(&self, id: MateId, cx: &App) -> Vec<MenuEntry> {
        let Some(c) = self.assembly(cx).and_then(|a| a.connector(id).cloned()) else { return vec![] };
        let hidden = c.hidden;
        vec![
            item("Rename", move |t, w, cx| t.start_row_rename(RenameRow::Feature(id), w, cx)),
            icon_item("Edit…", "edit", move |t, w, cx| t.edit_mate(id, w, cx)),
            MenuEntry::Sep,
            icon_item(if hidden { "Show" } else { "Hide" }, "eye-off", move |t, _, cx| {
                t.edit(if hidden { "Show mate connector" } else { "Hide mate connector" }, cx, |a| a.set_mates_hidden(&[id], !hidden));
            }),
            item("Hide other mate connectors", move |t, _, cx| {
                t.edit("Hide other mate connectors", cx, |a| {
                    for x in &mut a.connectors {
                        x.hidden = x.id != id;
                    }
                });
            }),
            item("Show all mate connectors", |t, _, cx| t.show_all_connectors(cx)),
            item(if c.suppressed { "Unsuppress" } else { "Suppress" }, move |t, _, cx| {
                t.edit("Suppress mate connector", cx, |a| {
                    if let Some(x) = a.connectors.iter_mut().find(|x| x.id == id) {
                        x.suppressed = !x.suppressed;
                    }
                });
            }),
            MenuEntry::Sep,
            icon_item("Delete", "delete", move |t, _, cx| t.delete_mate(id, cx)),
        ]
    }

    /// Graphics area context menu (instance under the cursor, or the
    /// general menu on empty space).
    pub(super) fn open_graphics_menu(&mut self, top: Option<InstanceId>, position: Point<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        let entries = match top {
            Some(t) if std::mem::take(&mut self.ui.menu_move_origin) => {
                let mut m = vec![item("Move to origin", |t, _, cx| t.triad_action(TriadAction::MoveToOrigin, cx))];
                m.extend(self.instance_menu(t, true, cx));
                m
            }
            Some(t) => self.instance_menu(t, true, cx),
            None => {
                // Onshape: Show all | Create Drawing of <assembly>… | Zoom to fit, Isometric.
                let mut m = Vec::new();
                if self.isolate.is_some() || self.ui.transparent.is_some() {
                    m.push(item("Exit isolate", |t, _, cx| {
                        t.exit_isolate(cx);
                    }));
                    m.push(MenuEntry::Sep);
                }
                m.push(item("Show all", |t, _, cx| t.show_hidden(cx)));
                // Live (behaviours "skb (10-24) assembly section" (4)): Show all |
                // Edit section view… (section icon) | Turn off section view | —.
                if self.ui.section.as_ref().is_some_and(|s| s.dialog.is_none() && !s.planes.is_empty()) {
                    m.push(icon_item("Edit section view…", "menu-section-view", |t, _, cx| t.edit_section(cx)));
                    m.push(item("Turn off section view", |t, w, cx| t.toggle_section(w, cx)));
                }
                m.push(MenuEntry::Sep);
                let tab = self.tab;
                let aname = self.doc.read(cx).session.doc.tab(tab).map(|t| t.name.clone()).unwrap_or_default();
                m.push(icon_item(format!("Create Drawing of {aname}…"), "menu-create-drawing", move |_, w, cx| {
                    let source = Some(noshape_model::drawing::create::DrawingSource { tab, part: None, config: ConfigValues::default(), sketch: None });
                    crate::drawing_create::request_create_drawing(crate::drawing_create::CreateRequest { source, name: Some(aname.clone()) }, w, cx);
                }));
                m.push(MenuEntry::Sep);
                // Live (menus/rmb-asm-empty): no icons, as in the Part Studio.
                m.push(item("Zoom to fit", |t, _, cx| t.viewport.update(cx, |vp, cx| vp.zoom_to_fit(true, cx))));
                m.push(item("Isometric", |t, _, cx| t.view(StandardView::Isometric, cx)));
                if !self.selected.is_empty() || self.selected_mate.is_some() {
                    m.push(item("Clear selection", |t, _, cx| {
                        t.clear_selection();
                        t.refresh_scene(false, cx);
                    }));
                }
                m
            }
        };
        if entries.is_empty() {
            return;
        }
        let view = cx.entity();
        let menu = OsMenu::build(window, cx, move |m, window, cx| fill_menu(m, entries.clone(), view.clone(), window, cx));
        let sub = cx.subscribe(&menu, |this: &mut Self, _, _: &DismissEvent, cx| {
            this.ui.ctx_menu = None;
            cx.notify();
        });
        menu.update(cx, |m, cx| window.focus(&m.focus_handle(cx), cx));
        self.ui.ctx_menu = Some((position, menu, sub));
        cx.notify();
    }

    /// Mate indicator context menu in the graphics area.
    pub(super) fn open_mate_menu(&mut self, id: MateId, position: Point<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        let entries = self.mate_menu(id, cx);
        let view = cx.entity();
        let menu = OsMenu::build(window, cx, move |m, window, cx| fill_menu(m, entries.clone(), view.clone(), window, cx));
        let sub = cx.subscribe(&menu, |this: &mut Self, _, _: &DismissEvent, cx| {
            this.ui.ctx_menu = None;
            cx.notify();
        });
        menu.update(cx, |m, cx| window.focus(&m.focus_handle(cx), cx));
        self.ui.ctx_menu = Some((position, menu, sub));
        self.refresh_scene(false, cx);
        cx.notify();
    }

    pub(super) fn render_graphics_menu(&self) -> Option<AnyElement> {
        let (pos, menu, _) = self.ui.ctx_menu.as_ref()?;
        Some(deferred(anchored().position(*pos).snap_to_window_with_margin(px(8.)).child(menu.clone())).with_priority(3).into_any_element())
    }

    pub(super) fn expand_all(&mut self, expand: bool, cx: &mut Context<Self>) {
        if expand {
            fn walk(nodes: &[InstanceNode], out: &mut HashSet<Vec<InstanceId>>) {
                for n in nodes {
                    if !n.children.is_empty() {
                        out.insert(n.path.clone());
                        walk(&n.children, out);
                    }
                }
            }
            let mut set = HashSet::new();
            walk(&self.result.nodes, &mut set);
            if let Some(a) = self.assembly(cx) {
                for p in &a.patterns {
                    set.insert(vec![InstanceId(p.id.0)]);
                }
                self.ui.expanded_mates.extend(a.mates.iter().map(|m| m.id));
            }
            self.expanded = set;
        } else {
            self.expanded.clear();
            self.ui.expanded_mates.clear();
        }
        cx.notify();
    }

    /// Rename the assembly tab (top-level assembly row > Rename).
    pub(super) fn rename_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let name = self.doc.read(cx).session.doc.tab(self.tab).map(|t| t.name.clone()).unwrap_or_default();
        let st = cx.new(|cx| InputState::new(window, cx).default_value(name));
        let st2 = st.clone();
        cx.spawn_in(window, async move |_, cx| {
            cx.background_executor().timer(Duration::from_millis(40)).await;
            let _ = st2.update_in(cx, |s, window, cx| {
                s.focus(window, cx);
                s.select_all(window, cx);
            });
        })
        .detach();
        let sub = cx.subscribe_in(&st, window, |this: &mut Self, st, ev: &InputEvent, window, cx| {
            if matches!(ev, InputEvent::PressEnter { .. } | InputEvent::Blur) {
                let name = st.read(cx).value().trim().to_string();
                let tab = this.tab;
                if !name.is_empty() {
                    this.doc.update(cx, |d, cx| {
                        d.edit_document(&format!("Rename tab : {name}"), cx, |doc| {
                            if let Some(t) = doc.tab_mut(tab) {
                                t.name = name.clone();
                            }
                        })
                    });
                }
                this.ui.rename = None;
                window.focus(&this.focus, cx);
                cx.notify();
            }
        });
        self.ui.rename = Some((RenameRow::Instance(InstanceId::ORIGIN), st, sub));
        cx.notify();
    }

    // ------------------------------------------------------------ list filter

    pub(super) fn ensure_filter_input(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.ui.filter_input.is_some() {
            return;
        }
        let st = cx.new(|cx| InputState::new(window, cx));
        let sub = cx.subscribe(&st, |this: &mut Self, st, ev: &InputEvent, cx| match ev {
            InputEvent::Change => {
                // No `cx.notify()`: the field's own notification re-renders
                // the view (its ancestor) and the list head; of the cached
                // slices only the rows show the filter.
                this.ui.filter = ListFilter::parse(&st.read(cx).value());
                this.filter_rows_changed(cx);
            }
            InputEvent::Focus | InputEvent::Blur => {
                this.ui.filter_focused = matches!(ev, InputEvent::Focus);
                cx.notify();
            }
            _ => {}
        });
        self.ui.filter_input = Some(st);
        self.ui.filter_sub = Some(sub);
        // Its placeholder is drawn italic over the empty box (`insert_search_box`).
        let st = cx.new(|cx| InputState::new(window, cx));
        self.ui.insert_search_sub = Some(cx.subscribe(&st, |this: &mut Self, st, ev: &InputEvent, cx| {
            // No `cx.notify()`: the field's own notification re-renders the
            // panel around it; of the cached slices only the catalog tree
            // shows the query.
            if matches!(ev, InputEvent::Change) {
                this.ui.insert_query = st.read(cx).value().to_string();
                this.insert_search_changed(cx);
            }
        }));
        self.ui.insert_search = Some(st);
        // The placeholder is drawn italic over the empty box (live 610).
        let st = cx.new(|cx| InputState::new(window, cx));
        self.ui.insert_url_ph = "Search or paste URL".into();
        self.ui.insert_url_sub = Some(cx.subscribe(&st, |this: &mut Self, st, ev: &InputEvent, cx| {
            if let InputEvent::PressEnter { .. } = ev {
                let text = st.read(cx).value().trim().to_string();
                this.search_other_documents(text, cx);
            }
        }));
        self.ui.insert_url = Some(st);
    }

    /// i / toolbar Insert: open the Insert dialog, or close it when open.
    pub(super) fn toggle_insert(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.insert_open {
            self.close_insert(window, cx);
        } else {
            self.close_mate(window, cx);
            self.insert_open = true;
            self.ui.insert_inserted.clear();
            self.std_take_selection(cx);
            cx.notify();
        }
    }

    /// "Inserted: N" of the Insert dialog: placed instances inserted since
    /// it opened that still exist (Undo removes them).
    /// The Insert dialog's "Inserted: N": instances inserted since it
    /// opened, including the one still following the cursor (live 04).
    pub(super) fn inserted_count(&self, cx: &App) -> usize {
        let Some(a) = self.assembly(cx) else { return 0 };
        self.ui.insert_inserted.iter().filter(|id| a.instance(**id).is_some()).count()
    }

    /// The Insert dialog's undo icon: undo the last insert (an instance
    /// still following the cursor is dropped first).
    pub(super) fn undo_last_insert(&mut self, cx: &mut Context<Self>) {
        if self.inserted_count(cx) == 0 && self.placing.is_none() {
            return;
        }
        // The Insert session is one pending step: take the last instance
        // out of it; once committed, undo until that instance is gone.
        let targets: Vec<InstanceId> = match self.placing.take() {
            Some(ids) => {
                self.placing_point = None;
                self.message = None;
                ids
            }
            None => self.assembly(cx).and_then(|a| self.ui.insert_inserted.iter().rev().find(|id| a.instance(**id).is_some()).copied()).into_iter().collect(),
        };
        if self.doc.read(cx).has_pending() {
            self.edit_insert("Insert", cx, |a| noshape_ops::assembly::delete_instances(a, &targets));
            cx.notify();
            return;
        }
        for _ in 0..targets.len() + 2 {
            self.doc.update(cx, |d, cx| d.undo(cx));
            if self.assembly(cx).is_none_or(|a| targets.iter().all(|id| a.instance(*id).is_none())) {
                break;
            }
        }
        cx.notify();
    }

    /// Close the Insert dialog (checkmark / Esc): an instance still following
    /// the cursor stays where it is.
    pub(super) fn close_insert(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.placing.is_some() {
            self.finish_placing(true, window, cx);
        }
        self.insert_open = false;
        self.end_insert_step(cx);
        // Rescanned next time; the opened document and category stay.
        self.ui.insert_other_docs = None;
        self.ui.std.auto_size = false;
        self.ui.std.holes.clear();
        self.ui.std.flip = false;
        self.update_capture(cx);
        cx.notify();
    }

    pub(super) fn set_list_filter(&mut self, text: &str, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(st) = &self.ui.filter_input {
            st.update(cx, |s, cx| s.set_value(text.to_string(), window, cx));
        }
        self.ui.filter = ListFilter::parse(text);
        cx.notify();
    }

    /// Filter box with its colon-command dropdown (top of the instance list);
    /// focused and empty, live's help popover under it.
    pub(super) fn render_list_filter(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let Some(st) = self.ui.filter_input.clone() else { return div().into_any_element() };
        let view = cx.entity();
        let current = self.ui.filter.clone();
        // Onshape filter row (live assembly/10): funnel, a 30 px "Filter by name"
        // box (1 px #aaa, radius 3, italic gray placeholder), then the ≡ list button.
        h_flex()
            .h(px(30.))
            .mt(px(7.))
            .flex_none()
            .pl(px(5.))
            .pr(px(8.))
            .gap(px(12.))
            .child(
                // Live assembly/752: the funnel turns blue while the filter has text.
                Button::new("asm-filter-cmds").ghost().child(crate::assets::icon(if current.is_empty() { "filter" } else { "filter-active" }, 20., cx)).w(px(22.)).h(px(26.)).p_0().tooltip("Filter").os_dropdown_menu("asm-filter-cmds-dd", false, move |mut menu, _, _| {
                    for c in FilterCommand::ALL {
                        let v = view.clone();
                        let current = current.clone();
                        menu = menu.item(crate::os_menu::OsMenuItem::new(format!("{}  {}", c.label(), c.token())).checked(current.commands.contains(&c)).on_click(move |_, window, cx| {
                            v.update(cx, |this, cx| {
                                let text = this.ui.filter_input.as_ref().map(|s| s.read(cx).value().to_string()).unwrap_or_default();
                                let mut words: Vec<String> = text.split_whitespace().map(|w| w.to_string()).collect();
                                if let Some(i) = words.iter().position(|w| w.eq_ignore_ascii_case(c.token())) {
                                    words.remove(i);
                                } else {
                                    words.insert(0, c.token().to_string());
                                }
                                this.set_list_filter(&words.join(" "), window, cx);
                            })
                        }));
                    }
                    menu
                }),
            )
            .child(
                div()
                    .w(px(122.))
                    .flex_none()
                    .relative()
                    .h(px(30.))
                    .px(px(1.))
                    .flex()
                    .items_center()
                    .border_1()
                    .border_color(gpui_kit::rgb(0xaaaaaa))
                    .rounded(px(3.))
                    .child(crate::ui::input_with_placeholder(&st, "Filter by name", cx).appearance(false).text_size(px(13.33)).pl(px(5.)))
                    .when(self.ui.filter_focused && self.ui.filter.is_empty(), |d| d.child(div().absolute().top(px(31.)).left_0().child(deferred(anchored().child(filter_help())).with_priority(2)))),
            )
            .child(div().flex_1())
            .child({
                // Live assembly/750–753: a 30 px toggle (no menu); hover #eaeaea,
                // on `os-active`: #eaeaea pressed in under a #b6b6b6 top edge.
                let flat = self.ui.filter_flat;
                div()
                    .id("asm-filter-flat")
                    .size(px(30.))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(2.))
                    .cursor_pointer()
                    .hover(|d| d.bg(gpui_kit::rgb(0xeaeaea)))
                    .relative()
                    .when(flat, |d| d.bg(gpui_kit::rgb(0xeaeaea)).border_l_1().border_r_1().border_color(gpui_kit::rgb(0xe4e4e4)).child(div().absolute().top_0().left_0().right_0().h(px(1.)).bg(gpui_kit::rgb(0xb6b6b6))))
                    .child(crate::assets::icon("instance-list-view", 20., cx))
                    .tooltip(|w, cx| gpui_kit::component::tooltip::Tooltip::new("Show search results as list").build(w, cx))
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.ui.filter_flat = !this.ui.filter_flat;
                        cx.notify();
                    }))
            })
            .into_any_element()
    }

    /// Does an instance row pass the list filter?
    pub(super) fn instance_passes(&self, inst: &Instance, node: Option<&InstanceNode>, cx: &App) -> bool {
        let f = &self.ui.filter;
        if f.is_empty() {
            return true;
        }
        if !f.wants_instances() {
            return false;
        }
        let kind = match inst.source {
            InstanceSource::Assembly { .. } => RowKind::Assembly,
            _ if self.linked_assembly(inst.id).is_some() => RowKind::Assembly,
            _ => RowKind::Part,
        };
        let warning = self.instance_warning(inst.id, cx).is_some();
        f.matches(&RowInfo { kind, name: &inst.name, error: node.is_some_and(|n| n.error.is_some()), warning, suppressed: inst.suppressed, hidden: inst.hidden, has_limits: false })
    }

    /// Does a folder row pass the list filter (`:folder`, or its name)?
    pub(super) fn folder_passes(&self, folder: &noshape_model::assembly::ListFolder) -> bool {
        let f = &self.ui.filter;
        !f.is_empty() && f.matches(&RowInfo { kind: RowKind::Folder, name: &folder.name, error: false, warning: false, suppressed: false, hidden: false, has_limits: false })
    }

    /// Does a mate feature row pass the list filter?
    pub(super) fn feature_passes(&self, kind: RowKind, name: &str, error: bool, suppressed: bool, hidden: bool, has_limits: bool) -> bool {
        let f = &self.ui.filter;
        if f.is_empty() {
            return true;
        }
        f.wants_mate_features() && f.matches(&RowInfo { kind, name, error, warning: false, suppressed, hidden, has_limits })
    }

    // ------------------------------------------------------------ S shortcut toolbar

    pub(super) fn toggle_mini_toolbar(&mut self, cx: &mut Context<Self>) {
        if self.ui.mini_toolbar.take().is_none() {
            self.ui.mini_toolbar = Some(self.pointer);
        }
        cx.notify();
    }

    /// The S shortcut toolbar (live behaviours "Assembly" S toolbar): at the
    /// cursor (left = x + 10, top = y − 30), 30 px tools at a 34 px pitch,
    /// five per row, on rgba(255,255,255,.95), no border, square corners, a
    /// 0 5 10 shadow, no tooltips; the ticked tools of Preferences › Shortcut
    /// toolbars › Assembly in list order (`AsmToolbarPrefs`).
    pub(super) fn render_mini_toolbar(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let at = self.ui.mini_toolbar?;
        let ticked = AsmToolbarPrefs::load().shortcut_tools();
        let shown: Vec<(&'static str, &'static str)> = shortcut_catalog().into_iter().filter(|(id, _, _)| ticked.iter().any(|t| t == id)).map(|(id, _, icon)| (id, icon)).collect();
        let mut rows = v_flex().gap(px(1.));
        for chunk in shown.chunks(5) {
            let mut row = h_flex().gap(px(4.));
            for &(id, icon) in chunk {
                row = row.child(
                    div()
                        .id(SharedString::from(format!("asm-mini-{id}")))
                        .size(px(30.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(3.))
                        .cursor_pointer()
                        .hover(|d| d.bg(rgb(0xeaeaea)))
                        .child(crate::assets::icon(icon, 20., cx))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, _: &MouseDownEvent, window, cx| {
                                cx.stop_propagation();
                                this.ui.mini_toolbar = None;
                                this.run_toolbar_tool(id, window, cx);
                                cx.notify();
                            }),
                        ),
                );
            }
            rows = rows.child(row);
        }
        Some(
            div()
                .id("asm-mini-toolbar")
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

    // ------------------------------------------------------------ shortcut sheet

    /// Shift+/ toggles the Keyboard shortcuts sheet (app-core's, shared by
    /// the studios; it replaced the assembly's own short list), on its
    /// Assembly tab.
    pub(super) fn toggle_shortcut_sheet(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.ui.shortcut_sheet.take().is_some() {
            window.focus(&self.focus, cx);
        } else {
            let sheet = crate::shortcut_sheet::ShortcutSheet::open(crate::shortcuts::Tab::Assembly, window, cx);
            let sub = cx.subscribe_in(&sheet, window, |this: &mut Self, _, _: &DismissEvent, window, cx| this.toggle_shortcut_sheet(window, cx));
            self.ui.shortcut_sheet = Some((sheet, sub));
        }
        cx.notify();
    }
}

// ---------------------------------------------------------------- Animate mate

/// Animate playback type (Onshape: Single, Reciprocate, Loop).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Playback {
    Single,
    Reciprocate,
    Loop,
}

/// Animate mate dialog state.
pub struct AnimState {
    pub mate: MateId,
    /// Driven free value (0 = Rz, 1..3 = Tx/Ty/Tz).
    pub dof: usize,
    pub start: Entity<crate::quantity_field::QuantityInput>,
    pub end: Entity<crate::quantity_field::QuantityInput>,
    pub steps: Entity<crate::quantity_field::QuantityInput>,
    /// Position along start..end (0..1) and direction.
    pub t: f64,
    pub dir: f64,
    pub playing: bool,
    /// ▶ was pressed once: Current value shows (live 53: empty before).
    pub played: bool,
    pub playback: Playback,
    pub busy: bool,
    pub current: f64,
    pub task: Option<Task<()>>,
    /// Preview from the mate dialog's Animate button (no Animate dialog).
    pub preview: bool,
    /// The DOF chip is filled (live: auto-filled for one DOF, empty for
    /// several until picked in "Select degree of freedom").
    pub chosen: bool,
    /// The "Select degree of freedom" chooser is open.
    pub chooser: bool,
}

impl AnimState {
    pub fn angular(&self) -> bool {
        self.dof == 0
    }
}

/// A mate's degrees of freedom in live's Animate chooser order (shared
/// with the agent API's assembly_animate).
pub(super) use noshape_ops::assembly::anim_dofs;

/// Full turns of the mate dialog's Animate preview (one degree per frame).
const PREVIEW_TURNS: f64 = 2.0;
/// Ascender of the mate value text (px; Onshape dimension text is 16 px, behaviours "Entity colours").
const VALUE_TEXT_ASC: f64 = 11.5;

/// Live's mate angle label number: up to 3 decimals, trailing zeros dropped
/// ("275.326", and "0" at 0°; qa5 MT7).
pub fn mate_angle_label(deg: f64) -> String {
    let t = noshape_units::to_fixed(deg, 3);
    if t.contains('.') { t.trim_end_matches('0').trim_end_matches('.').to_string() } else { t }
}
/// A mate's angle as the sweep of its value arc, in [0, 2π): an angle that
/// reads 0 or 360 at the label's 3 decimals is 0, no arc (MATE-ANGLE-360: a
/// Revolute at 0° solved to -1e-12 drew the full circle with "360°").
pub fn mate_angle_sweep(rad: f64) -> f64 {
    let s = rad.rem_euclid(std::f64::consts::TAU);
    let eps = 0.0005f64.to_radians();
    if s < eps || std::f64::consts::TAU - s < eps { 0.0 } else { s }
}

/// Onshape `PREVIEW_ANGULAR_DIMENSION_RADIUS_PIXEL_SIZE`.
const VALUE_ARC_RADIUS_PX: f64 = 80.0;

/// One mate value shown as a dimension (see [`AssemblyView::value_dims`]).
pub(super) struct ValueDim {
    pub mate: MateId,
    /// Value index as in `MateStatus::values` (0 rotation, 1..3 X/Y/Z).
    pub ix: usize,
    pub text: String,
    /// Text centre and in-plane axes; ascender (world units).
    pub center: DVec3,
    pub x: DVec3,
    pub y: DVec3,
    pub asc: f64,
    pub shape: ValueShape,
}

pub(super) enum ValueShape {
    Arc(crate::assembly_scene::ValueArc),
    Line(crate::assembly_scene::ValueLine),
}

/// A mate's values as Onshape shows them (bundle `calculateMateValues`): the
/// first connector in the second's frame, `[rotation about Z, x, y, z]`.
pub(super) fn onshape_mate_values(a: &noshape_model::assembly::solver::Frame, b: &noshape_model::assembly::solver::Frame) -> [f64; 4] {
    let (bx, by, bz) = (b.rotation * DVec3::X, b.rotation * DVec3::Y, b.rotation * DVec3::Z);
    let ax = a.rotation * DVec3::X;
    let d = a.origin - b.origin;
    [ax.dot(by).atan2(ax.dot(bx)), d.dot(bx), d.dot(by), d.dot(bz)]
}

/// Our solver values (`solver::mate_values`: the second connector in the
/// first's frame) that put Onshape's value `ix` at `v`, the other shown
/// values unchanged. Rotation: the solver's value is the one shown;
/// translations: the first connector's offset in the second's frame,
/// expressed back in the first's (negated).
pub(super) fn solver_values_for(a: &noshape_model::assembly::solver::Frame, b: &noshape_model::assembly::solver::Frame, ours: [f64; 4], ix: usize, v: f64) -> [f64; 4] {
    let mut on = onshape_mate_values(a, b);
    let mut out = ours;
    // The rotation shown is the solver's own value (live's rotationZ).
    if ix == 0 {
        out[0] = v;
        return out;
    }
    on[ix] = v;
    let (bx, by, bz) = (b.rotation * DVec3::X, b.rotation * DVec3::Y, b.rotation * DVec3::Z);
    let d = bx * on[1] + by * on[2] + bz * on[3];
    for (k, axis) in [DVec3::X, DVec3::Y, DVec3::Z].into_iter().enumerate() {
        out[k + 1] = -(a.rotation * axis).dot(d);
    }
    out
}

impl AssemblyView {
    fn anim_defaults(&self, m: &Mate, dof: usize, cx: &App) -> (String, String) {
        noshape_ops::assembly::anim_defaults(m, dof, &self.units(cx))
    }

    /// Mate (or mate indicator) context menu > Animate: open the Animate
    /// dialog for the selected mate (paused; Play starts it).
    pub(super) fn start_animation(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(asm) = self.assembly(cx) else { return };
        let animatable = |m: &&Mate| !noshape_model::assembly::eval::mate_dofs(m.mate_type).is_empty();
        let pick = self.selected_mate.and_then(|id| asm.mate(id)).filter(animatable).or_else(|| asm.mates.iter().filter(|m| !m.suppressed).find(animatable));
        let Some(m) = pick.cloned() else {
            self.message = Some((false, "Select a mate with degrees of freedom to animate.".into()));
            cx.notify();
            return;
        };
        self.stop_animation(window, cx);
        let dofs = anim_dofs(m.mate_type);
        let dof = dofs[0];
        let chosen = dofs.len() == 1;
        let (a, b) = if chosen { self.anim_defaults(&m, dof, cx) } else { (String::new(), String::new()) };
        let [start, end] = self.anim_value_inputs(dof == 0, a, b, window, cx);
        let steps = self.anim_steps_input("300".into(), window, cx);
        let current = self.result.solved.mate_values.get(&m.id).map(|v| v[dof]).unwrap_or(0.0);
        self.selected_mate = Some(m.id);
        self.anim = Some(AnimState { mate: m.id, dof, start, end, steps, t: 0.0, dir: 1.0, playing: false, played: false, playback: Playback::Reciprocate, busy: false, current, task: None, preview: false, chosen, chooser: false });
        cx.notify();
    }

    /// Mate dialog ▶ Animate (live 575): the mate's first degree of freedom
    /// runs once through full turns (a span for a slider) from its current
    /// value, then the preview returns to the start pose. ▶ stays ▶.
    pub(super) fn toggle_mate_preview(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.anim.as_ref().is_some_and(|a| a.preview) {
            return;
        }
        let Some(m) = self.mate.as_ref().and_then(|s| s.dialog.read(cx).to_mate(cx)) else {
            self.message = Some((false, "Select two mate connectors first.".into()));
            if let Some(d) = self.mate.as_ref().map(|s| s.dialog.clone()) {
                d.update(cx, |d, cx| {
                    d.animating = false;
                    cx.notify();
                });
            }
            cx.notify();
            return;
        };
        let Some(&(dof, _)) = noshape_model::assembly::eval::mate_dofs(m.mate_type).first() else { return };
        let now = self.result.solved.mate_values.get(&m.id).map(|v| v[dof]).unwrap_or(0.0);
        let (a, b, steps) = if dof == 0 {
            let deg = |v: f64| format!("{} deg", noshape_units::to_fixed(v.to_degrees(), 6));
            (deg(now), deg(now + PREVIEW_TURNS * std::f64::consts::TAU), PREVIEW_TURNS * 360.0)
        } else {
            let (a, b) = self.anim_defaults(&m, dof, cx);
            (a, b, 360.0)
        };
        let [start, end] = self.anim_value_inputs(dof == 0, a, b, window, cx);
        let steps = self.anim_steps_input(format!("{steps}"), window, cx);
        self.anim = Some(AnimState { mate: m.id, dof, start, end, steps, t: 0.0, dir: 1.0, playing: false, played: true, playback: Playback::Single, busy: false, current: now, task: None, preview: true, chosen: true, chooser: false });
        self.play_animation(cx);
    }

    /// The preview ran its course: back to the start pose, ▶ released.
    pub(super) fn finish_mate_preview(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.anim = None;
        if let Some(d) = self.mate.as_ref().map(|s| s.dialog.clone()) {
            d.update(cx, |d, cx| {
                d.animating = false;
                cx.notify();
            });
        }
        self.schedule_eval(window, cx);
    }

    /// Open the "Select degree of freedom" chooser (multi-DOF mates).
    pub(super) fn open_dof_chooser(&mut self, cx: &mut Context<Self>) {
        let Some(t) = self.anim.as_ref().filter(|a| !a.preview).and_then(|a| self.assembly(cx).and_then(|asm| asm.mate(a.mate)).map(|m| m.mate_type)) else { return };
        if anim_dofs(t).len() > 1
            && let Some(a) = &mut self.anim
        {
            a.chooser = true;
        }
        cx.notify();
    }

    fn set_anim_dof(&mut self, dof: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(mid) = self.anim.as_ref().map(|a| a.mate) else { return };
        let Some(m) = self.assembly(cx).and_then(|a| a.mate(mid).cloned()) else { return };
        let (a, b) = self.anim_defaults(&m, dof, cx);
        // The fields take the new degree of freedom's unit (deg / mm).
        let [start, end] = self.anim_value_inputs(dof == 0, a, b, window, cx);
        if let Some(an) = &mut self.anim {
            an.dof = dof;
            an.t = 0.0;
            an.chosen = true;
            an.chooser = false;
            an.start = start;
            an.end = end;
        }
        cx.notify();
    }

    /// The Animate dialog's Start / End value fields: Onshape's number
    /// field (live "Input behaviour outside feature dialogs"), in degrees for
    /// a rotation and lengths for a translation.
    fn anim_value_inputs(&self, angular: bool, start: String, end: String, window: &mut Window, cx: &mut Context<Self>) -> [Entity<crate::quantity_field::QuantityInput>; 2] {
        use crate::quantity_field::{QuantityInput, QuantitySpec};
        let units = self.units(cx);
        let variables = self.variable_scope(cx);
        let dim = if angular { noshape_units::Dim::ANGLE } else { noshape_units::Dim::LENGTH };
        let mut mk = |text: String, label: &'static str| {
            let vars = variables.clone();
            cx.new(|cx| {
                let mut q = QuantityInput::new(text, QuantitySpec::quantity(dim).label(label), units, window, cx);
                q.set_variables(vars, cx);
                q
            })
        };
        [mk(start, "Start value"), mk(end, "End value")]
    }

    /// The Animate dialog's Steps field (a count).
    fn anim_steps_input(&self, text: String, window: &mut Window, cx: &mut Context<Self>) -> Entity<crate::quantity_field::QuantityInput> {
        use crate::quantity_field::{QuantityInput, QuantitySpec};
        let units = self.units(cx);
        let variables = self.variable_scope(cx);
        cx.new(|cx| {
            let mut q = QuantityInput::new(text, QuantitySpec::count().label("Steps"), units, window, cx);
            q.set_variables(variables, cx);
            q
        })
    }

    /// Start / end values and step count from the dialog fields.
    fn anim_range(&self, cx: &App) -> Option<(f64, f64, u32)> {
        let a = self.anim.as_ref()?;
        let ev = |st: &Entity<crate::quantity_field::QuantityInput>| st.read(cx).value();
        let steps = a.steps.read(cx).value().map(|v| v.round().max(0.) as u32).unwrap_or(60).max(2);
        // Up to 100 revolutions.
        let lim = |v: f64| if a.angular() { v.clamp(-100.0 * std::f64::consts::TAU, 100.0 * std::f64::consts::TAU) } else { v };
        Some((lim(ev(&a.start)?), lim(ev(&a.end)?), steps))
    }

    pub(super) fn play_animation(&mut self, cx: &mut Context<Self>) {
        if self.anim.as_ref().is_some_and(|a| !a.chosen) {
            return;
        }
        let Some((start, end, steps)) = self.anim_range(cx) else {
            self.message = Some((true, "Enter valid start and end values.".into()));
            cx.notify();
            return;
        };
        let Some(a) = &mut self.anim else { return };
        if a.playback == Playback::Single && ((a.dir > 0.0 && a.t >= 1.0) || (a.dir < 0.0 && a.t <= 0.0)) {
            // Single playback finished: play again from the start.
            a.t = if a.dir > 0.0 { 0.0 } else { 1.0 };
        }
        a.playing = true;
        a.played = true;
        // 60 steps per second, so more steps play slower; time based, so a
        // slow frame (solve + redraw) skips steps instead of slowing down.
        // Live steps by (end − start) / Steps and stays on those values
        // (behaviours "Animate mate: Current value": 1.2° steps, 3.6, 4.8, 12).
        let step = 1.0 / steps.max(1) as f64;
        let mut last = std::time::Instant::now();
        a.task = Some(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_millis(16)).await;
                let job = this.update(cx, |this: &mut Self, cx| {
                    let a = this.anim.as_mut()?;
                    if !a.playing || a.busy {
                        return a.playing.then_some(None);
                    }
                    let now = std::time::Instant::now();
                    let frames = (now.duration_since(last).as_secs_f64() * 60.0).clamp(1.0, 30.0).round();
                    last = now;
                    a.t = ((a.t / step).round() + a.dir * frames) * step;
                    if a.t > 1.0 + 1e-9 || a.t < -1e-9 {
                        match a.playback {
                            Playback::Single => {
                                a.t = a.t.clamp(0.0, 1.0);
                                a.playing = false;
                                a.task = None;
                                let preview = a.preview;
                                this.ui.preview_done = preview;
                                cx.notify();
                                return None;
                            }
                            Playback::Loop => a.t = if a.dir > 0.0 { 0.0 } else { 1.0 },
                            Playback::Reciprocate => {
                                a.dir = -a.dir;
                                a.t = a.t.clamp(0.0, 1.0);
                            }
                        }
                    }
                    let v = start + (end - start) * a.t;
                    a.current = v;
                    a.busy = true;
                    let (mate, dof) = (a.mate, a.dof);
                    let start: HashMap<InstanceId, Placement> = this.result.solved.placements.iter().cloned().collect();
                    Some(Some((this.result.prepared.clone(), SolveOptions { hold_values: vec![(mate, dof, v)], start: Some(start), nested_start: Some(this.result.solved.nested.clone()), ..Default::default() })))
                });
                let Ok(Some(job)) = job else { break };
                let Some((prepared, opts)) = job else { continue };
                let solved = cx.background_spawn(async move { prepared.solve(&opts) }).await;
                let ok = this.update(cx, |this: &mut Self, cx| {
                    if let Some(a) = &mut this.anim {
                        a.busy = false;
                    }
                    this.apply_solved(solved, cx);
                    this.rebuild_scene(true, cx);
                    cx.notify();
                });
                if ok.is_err() {
                    break;
                }
            }
        }));
        cx.notify();
    }

    /// Stop: the assembly stays at the current value.
    pub(super) fn stop_animation(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        if let Some(a) = &mut self.anim {
            a.playing = false;
            a.task = None;
        }
        cx.notify();
    }

    pub(super) fn close_animation(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.anim = None;
        // The keyboard goes back to the view (its fields are gone; Alt+C's
        // tool search then got no typing).
        window.focus(&self.focus, cx);
        // Back to the document's positions (animation is not saved).
        self.schedule_eval(window, cx);
    }

    pub(super) fn render_anim_panel(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let Some(a) = &self.anim else { return div().into_any_element() };
        let Some(m) = self.assembly(cx).and_then(|x| x.mate(a.mate).cloned()) else { return div().into_any_element() };
        let units = self.units(cx);
        // Live: the needed decimals only, a space, the unit ("3.6 deg"; no °).
        let current = if !a.played {
            String::new()
        } else if a.angular() {
            format!("{} deg", trim_number(a.current.to_degrees()))
        } else {
            let f = units.si_factor(noshape_units::Dim::LENGTH);
            format!("{} {}", trim_number(a.current / f), units.length.symbol())
        };
        let (playing, pb, dof, reversed) = (a.playing, a.playback, a.dof, a.dir < 0.0);
        let dofs = anim_dofs(m.mate_type);
        let chosen = a.chosen;
        let chooser_open = a.chooser;
        // Onshape's "Animate mate" dialog: the DOF chip box, right-aligned
        // fields, Playback type ▾ + reverse, Current value, ▶ ■ and ?.
        use crate::os_ui as os;
        let field = |label: &'static str, st: &Entity<crate::quantity_field::QuantityInput>| os::param_row_plain(label, st.clone()).into_any_element();
        let axis_of = |ix: usize| match ix {
            0 => ("⟳", "z"),
            1 => ("↹", "x"),
            2 => ("↹", "y"),
            _ => ("↹", "z"),
        };
        let chip = |ix: usize| {
            let (g, ax) = axis_of(ix);
            format!("{g} - {ax} - {}", m.name)
        };
        let chip_rows: Vec<AnyElement> = chosen
            .then(|| {
                os::query_row("anim-dof-chip", chip(dof))
                    .child(
                        // Live (dom/dialog-animate): a 20 x 24 "×" span (5 px left pad), 8 x 7 px ink, #666.
                        div().id("anim-dof-x").w(px(20.)).h(px(24.)).pl(px(5.)).flex().items_center().child(crate::ui::glyph("ui-clear", 10., crate::theme::p().icon_tertiary)).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            cx.stop_propagation();
                            if let Some(a) = &mut this.anim {
                                (a.playing, a.task, a.chosen) = (false, None, false);
                            }
                            cx.notify();
                        })),
                    )
                    .into_any_element()
            })
            .into_iter()
            .collect();
        // Live dom dialog-animate: the chip box, 5 px, then 26 px rows.
        // Clicking it (or the mate's row) opens "Select degree of freedom".
        let dof_box = os::query_box("anim-dof", "Mate DOF to animate", true, chip_rows).on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.open_dof_chooser(cx)));
        let mut body = v_flex().px(px(3.)).pt(px(3.)).pb(px(2.)).child(dof_box).child(div().h(px(5.)));
        if chooser_open && dofs.len() > 1 {
            // Live (dom/animate-dof-chooser-cylindrical.txt): `div.mate-dof-chooser`,
            // one row per DOF.
            let mut list = v_flex()
                .id("anim-dof-chooser")
                .mx(px(3.))
                .mb(px(5.))
                .bg(crate::theme::p().panel)
                .border_1()
                .border_color(gpui_kit::rgb(0xd7d7d7))
                .shadow(vec![BoxShadow { color: hsla(0., 0., 0., 0.2), offset: point(px(0.), px(3.)), blur_radius: px(4.), spread_radius: px(0.), inset: false }])
                .child(div().px(px(6.)).py(px(4.)).text_size(px(12.)).font_weight(FontWeight::MEDIUM).text_color(pal.text).child("Select degree of freedom"));
            for ix in dofs {
                list = list.child(
                    div()
                        .id(SharedString::from(format!("anim-dof-{ix}")))
                        .h(px(24.))
                        .px(px(6.))
                        .flex()
                        .items_center()
                        .text_size(px(13.33))
                        .text_color(pal.text)
                        .cursor_pointer()
                        .hover(|d| d.bg(pal.row_hover))
                        .child(chip(ix))
                        .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| this.set_anim_dof(ix, window, cx))),
                );
            }
            body = body.child(list);
        }
        let view = cx.entity();
        let pb_label = match pb {
            Playback::Single => "Single",
            Playback::Reciprocate => "Reciprocate",
            Playback::Loop => "Loop",
        };
        // Live: 110 px, the text 6 px in, an 8x4 #333 caret 6 px from the right.
        let playback = Button::new("anim-playback")
            .ghost()
            .small()
            .w(px(110.))
            .px(px(0.))
            .child(h_flex().w_full().pl(px(6.)).pr(px(6.)).child(div().flex_1().text_size(px(14.)).text_color(pal.text).child(pb_label)).child(crate::ui::glyph("drawing-tool-caret", 12., gpui_kit::rgb(0x333333))))
            .os_dropdown_menu("anim-playback-dd", true, move |mut menu, _, _| {
                for (label, v) in [("Single", Playback::Single), ("Reciprocate", Playback::Reciprocate), ("Loop", Playback::Loop)] {
                    let view = view.clone();
                    menu = menu.item(crate::os_menu::OsMenuItem::new(label).checked(v == pb).on_click(move |_, _, cx| {
                        view.update(cx, |this, cx| {
                            if let Some(a) = &mut this.anim {
                                a.playback = v;
                            }
                            cx.notify();
                        })
                    }));
                }
                menu
            });
        let icon_btn = |id: &'static str, icon: &'static str, tip: &'static str, on: bool| {
            div()
                .id(id)
                .size(px(26.))
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .hover(|d| d.bg(pal.panel_hover))
                .when(on, |d| d.bg(pal.panel_active))
                .tooltip(crate::ui::tooltip(tip))
                .child(crate::ui::glyph(icon, 20., pal.text))
        };
        body = body
            .child(field("Start value", &a.start))
            .child(field("End value", &a.end))
            .child(field("Steps", &a.steps))
            .child(
                // Live (dom dialog-animate): the dialog's full 220 px: an 84 px
                // label column (5 px in), the 110 px select, the 26 px toggle.
                h_flex()
                    .h(px(28.))
                    .mx(px(-3.))
                    .child(div().w(px(84.)).flex_none().pl(px(5.)).whitespace_nowrap().text_size(px(12.)).text_color(pal.text_muted).child("Playback type"))
                    .child(div().w(px(110.)).flex_none().border_b_1().border_color(gpui_kit::rgb(0xd7d7d7)).child(playback))
                    .child(icon_btn("anim-reverse", "flip-circular-opposite", "Reverse", reversed).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        if let Some(a) = &mut this.anim {
                            a.dir = -a.dir;
                        }
                        cx.notify();
                    }))),
            )
            .child(os::param_row("Current value", div().h(px(25.)).flex().items_center().justify_end().text_size(px(13.33)).text_color(pal.text_muted).child(current)))
            .child(
                // Live: ▶ at the dialog's left edge, ■ 29 px on (26 px buttons).
                h_flex()
                    .h(px(30.))
                    .ml(px(-3.))
                    .gap(px(3.))
                    .child(icon_btn("anim-play", "anim-play", "Play", playing).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        if !this.anim.as_ref().is_some_and(|a| a.playing) {
                            this.play_animation(cx);
                        }
                    })))
                    .child(icon_btn("anim-stop", "anim-stop", "Stop", false).on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                        if this.anim.as_ref().is_some_and(|a| a.playing) {
                            this.stop_animation(window, cx);
                        }
                    })))
                    .child(div().flex_1())
                    .child(div().size(px(14.)).mr(px(4.)).rounded_full().flex().items_center().justify_center().text_size(px(10.)).font_weight(FontWeight::BOLD).child(crate::ui::glyph("help-dot", 15.6, crate::theme::help_dot(0x999999)))),
            );
        // Live dom dialog-animate: 220 px, the plain grey ×.
        Self::panel_frame_with("Animate mate", body, cx.listener(|this, _: &ClickEvent, window, cx| this.close_animation(window, cx)), "anim-close", true).w(px(220.)).into_any_element()
    }
}

// ---------------------------------------------------------------- display states, show mates, replace

impl AssemblyView {
    /// Relation dialog: a mate picked in the list or the graphics fills the
    /// first empty mate field (Screw: its single field).
    pub(super) fn pick_relation_mate(&mut self, id: MateId, cx: &mut Context<Self>) {
        let Some(r) = &mut self.relation else { return };
        // The Mate(s) query: a picked mate toggles; a screw takes one mate,
        // the others two (a third pick replaces the second).
        if r.b == Some(id) {
            r.b = None;
        } else if r.a == Some(id) {
            r.a = r.b.take();
        } else if r.a.is_none() || r.kind == RelationType::Screw {
            r.a = Some(id);
        } else {
            r.b = Some(id);
        }
        cx.notify();
    }

    /// h / toolbar: Show mates mode (hovered instance shows its mates, click pins).
    pub(super) fn toggle_show_mates_mode(&mut self, cx: &mut Context<Self>) {
        self.show_mates_mode = !self.show_mates_mode;
        self.mates_pinned = None;
        self.update_capture(cx);
        self.refresh_scene(false, cx);
        cx.notify();
    }

    /// Toolbar Replace instance: the Replace instance dialog with the
    /// selected instances (none: an empty, active field; live 833, no notice).
    pub(super) fn replace_selected(&mut self, cx: &mut Context<Self>) {
        let tops: Vec<InstanceId> = self.selected.iter().copied().filter(|id| self.assembly(cx).is_some_and(|a| a.instance(*id).is_some())).collect();
        self.open_replace_with(tops, cx);
    }

    pub(super) fn toggle_display_states(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.ui.display_states = !self.ui.display_states;
        if self.ui.ds_name.is_none() {
            self.ui.ds_name = Some(cx.new(|cx| InputState::new(window, cx).placeholder("New display state name")));
        }
        cx.notify();
    }

    fn add_display_state(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(st) = self.ui.ds_name.clone() else { return };
        let name = st.read(cx).value().trim().to_string();
        if name.is_empty() {
            return;
        }
        if self.assembly(cx).is_some_and(|a| a.display_states.iter().any(|d| d.name == name)) {
            self.message = Some((true, "There is already a display state with this name.".into()));
            cx.notify();
            return;
        }
        self.edit(&format!("Add display state : {name}"), cx, |a| a.save_display_state(&name));
        self.ui.ds_selected = self.assembly(cx).map(|a| a.display_states.len().saturating_sub(1));
        st.update(cx, |s, cx| s.set_value("", window, cx));
        cx.notify();
    }

    pub(super) fn render_display_states(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let states: Vec<String> = self.assembly(cx).map(|a| a.display_states.iter().map(|d| d.name.clone()).collect()).unwrap_or_default();
        let sel = self.ui.ds_selected.filter(|i| *i < states.len());
        let mut body = v_flex().p_2().gap_1();
        if states.is_empty() {
            body = body.child(div().px_1().text_xs().text_color(pal.text_faint).child("No display states created"));
        }
        for (i, name) in states.iter().enumerate() {
            let renaming = sel == Some(i) && self.ui.ds_renaming.is_some();
            body = body.child(
                h_flex()
                    .id(SharedString::from(format!("ds-{i}")))
                    .h(px(24.))
                    .px_1()
                    .gap_1()
                    .rounded_sm()
                    .text_sm()
                    .cursor_pointer()
                    .when(sel == Some(i), |d| d.bg(pal.accent_soft))
                    .hover(|d| d.bg(pal.panel_hover))
                    .child(match (renaming, &self.ui.ds_renaming) {
                        (true, Some(st)) => div().flex_1().child(Input::new(st).xsmall()).into_any_element(),
                        _ => div().flex_1().truncate().child(crate::ui::user_text(name.clone())).into_any_element(),
                    })
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        // Selecting a display state applies it.
                        this.ui.ds_selected = Some(i);
                        this.ui.ds_renaming = None;
                        let name = this.assembly(cx).and_then(|a| a.display_states.get(i).map(|d| d.name.clone())).unwrap_or_default();
                        this.edit(&format!("Apply display state : {name}"), cx, |a| a.apply_display_state(i));
                        cx.notify();
                    })),
            );
        }
        let actions = h_flex()
            .gap_1()
            .child(Button::new("ds-update").label("Update").xsmall().ghost().tooltip("Update this display state").disabled(sel.is_none()).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                let Some(i) = sel else { return };
                let name = this.assembly(cx).and_then(|a| a.display_states.get(i).map(|d| d.name.clone())).unwrap_or_default();
                this.edit(&format!("Update display state : {name}"), cx, |a| a.save_display_state(&name));
                this.message = Some((false, "Display state updated.".into()));
                cx.notify();
            })))
            .child(Button::new("ds-rename").icon(Icon::empty().path(icon_path("edit"))).xsmall().ghost().tooltip("Rename display state").disabled(sel.is_none()).on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                let Some(i) = sel else { return };
                let name = this.assembly(cx).and_then(|a| a.display_states.get(i).map(|d| d.name.clone())).unwrap_or_default();
                let st = cx.new(|cx| InputState::new(window, cx).default_value(name));
                st.update(cx, |s, cx| s.focus(window, cx));
                let sub = cx.subscribe_in(&st, window, move |this: &mut Self, st, ev: &InputEvent, window, cx| {
                    if matches!(ev, InputEvent::PressEnter { .. } | InputEvent::Blur) {
                        let new = st.read(cx).value().trim().to_string();
                        this.ui.ds_renaming = None;
                        if !new.is_empty() {
                            this.edit(&format!("Rename display state : {new}"), cx, |a| {
                                if let Some(d) = a.display_states.get_mut(i) {
                                    d.name = new.clone();
                                }
                            });
                        }
                        window.focus(&this.focus, cx);
                        cx.notify();
                    }
                });
                this.transient_sub = Some(sub);
                this.ui.ds_renaming = Some(st);
                cx.notify();
            })))
            .child(Button::new("ds-delete").icon(Icon::empty().path(icon_path("delete"))).xsmall().ghost().tooltip("Remove display state").disabled(sel.is_none()).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                let Some(i) = sel else { return };
                this.edit("Remove display state", cx, |a| {
                    if i < a.display_states.len() {
                        a.display_states.remove(i);
                    }
                });
                this.ui.ds_selected = None;
                this.message = Some((false, "Display state deleted.".into()));
                cx.notify();
            })));
        body = body.child(actions);
        if let Some(st) = self.ui.ds_name.clone() {
            body = body.child(
                h_flex()
                    .gap_1()
                    .child(div().flex_1().child(Input::new(&st).xsmall()))
                    // Live (assembly/nv-904): a bordered 28 px box with Onshape's 12 px "+",
                    // #aaa (disabled) until a name is typed.
                    .child({
                        let empty = st.read(cx).value().trim().is_empty();
                        div()
                            .id("ds-add")
                            .size(px(28.))
                            .flex_none()
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(3.))
                            .border_1()
                            .border_color(gpui_kit::rgb(0xdddddd))
                            .when(!empty, |d| d.cursor_pointer().hover(|d| d.bg(p().panel_hover)))
                            .child(div().opacity(if empty { 0.42 } else { 1. }).child(crate::assets::icon("add-button", 20., cx)))
                            .tooltip(crate::ui::tooltip("Add display state"))
                            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.add_display_state(window, cx)))
                    }),
            );
        }
        // Live (assembly/nv-904): the plain ns-dialog × (no ✓).
        Self::panel_frame_with("Display states", body, cx.listener(|this, _: &ClickEvent, _, cx| {
            this.ui.display_states = false;
            cx.notify();
        }), "ds-close", true)
        .into_any_element()
    }
}

// ---------------------------------------------------------------- Group dialog

impl AssemblyView {
    pub(super) fn open_group(&mut self, editing: Option<MateId>, ids: Vec<InstanceId>, cx: &mut Context<Self>) {
        self.ui.group = Some((editing, ids));
        self.insert_open = false;
        self.update_capture(cx);
        cx.notify();
    }

    pub(super) fn toggle_group_member(&mut self, id: InstanceId, cx: &mut Context<Self>) {
        if self.assembly(cx).is_some_and(|a| a.generated_by(id).is_some()) {
            return;
        }
        if let Some((_, ids)) = &mut self.ui.group {
            match ids.iter().position(|x| *x == id) {
                Some(i) => {
                    ids.remove(i);
                }
                None => ids.push(id),
            }
        }
        self.refresh_scene(false, cx);
        cx.notify();
    }

    pub(super) fn render_group_dialog(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let Some((editing, ids)) = self.ui.group.clone() else { return div().into_any_element() };
        let names: HashMap<InstanceId, String> = self.assembly(cx).map(|a| a.instances.iter().map(|i| (i.id, i.name.clone())).collect()).unwrap_or_default();
        let asm = self.assembly(cx);
        // Live (dom/dialog-assembly-group.txt): a new group is titled "Group 1".
        let title = match editing {
            Some(g) => asm.and_then(|a| a.groups.iter().find(|x| x.id == g).map(|x| x.name.clone())).unwrap_or_else(|| "Group".into()),
            None => asm.map(|a| a.next_feature_name("Group")).unwrap_or_else(|| "Group 1".into()),
        };
        // Live: empty → "Nothing selected."; members already mated together (E2) →
        // "Mate cannot be added between member of same group.".
        let mated = asm.is_some_and(|a| a.mates.iter().any(|m| !m.suppressed && ids.contains(&m.a.instance) && ids.contains(&m.b.instance)));
        let error = if ids.is_empty() {
            Some("Nothing selected.")
        } else if mated {
            Some("Mate cannot be added between member of same group.")
        } else {
            None
        };
        let rows: Vec<AnyElement> = ids
            .iter()
            .map(|&id| {
                crate::os_ui::query_row(SharedString::from(format!("grp-row-{}", id.0)), names.get(&id).cloned().unwrap_or_default())
                    .child(
                        div()
                            .id(SharedString::from(format!("grp-x-{}", id.0)))
                            .px(px(5.))
                            .text_size(px(16.67))
                            .text_color(crate::theme::p().icon_tertiary)
                            .cursor_pointer()
                            .child("×")
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.toggle_group_member(id, cx))),
                    )
                    .into_any_element()
            })
            .collect();
        let ok = !ids.is_empty();
        let body = v_flex()
            .pb(px(2.))
            // The Instances field is active while the dialog is open.
            .child(div().px(px(3.)).pt(px(5.)).child(crate::os_ui::query_box("grp-instances", "Instances", true, rows)))
            .child(h_flex().justify_end().px(px(4.)).pt(px(4.)).pb(px(2.)).child(div().size(px(14.)).rounded_full().flex().items_center().justify_center().text_size(px(10.)).font_weight(FontWeight::BOLD).child(crate::ui::glyph("help-dot", 15.6, crate::theme::help_dot(0x999999)))));
        v_flex()
            .id("group-dialog")
            .w(px(220.))
            .bg(pal.dialog_bg)
            .shadow(vec![BoxShadow { color: hsla(0., 0., 0., 0.2), offset: point(px(0.), px(5.)), blur_radius: px(10.), spread_radius: px(0.), inset: false }])
            .text_color(pal.text)
            .child(
                h_flex()
                    .h(px(29.))
                    .bg(crate::theme::p().panel)
                    .border_b_1()
                    .border_color(gpui_kit::rgb(0xd7d7d7))
                    .child(
                        div()
                            .id("grp-title")
                            .flex_1()
                            .pl(px(6.))
                            .text_size(px(13.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(if error.is_some() { pal.text_invalid } else { pal.text })
                            .truncate()
                            .when_some(error.map(|e| format!("{title} did not regenerate properly: {e}")), |d, t| d.tooltip(crate::ui::tooltip(t)))
                            .child(crate::ui::user_text(title.clone())),
                    )
                    .child(
                        // Live: the ✓ at opacity 0.2 while nothing is selected.
                        div()
                            .id("grp-ok")
                            .size(px(28.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .bg(pal.accept)
                            .when(!ok, |d| d.opacity(0.2))
                            .when(ok, |d| d.hover(|d| d.bg(gpui_kit::rgb(0x74ce3a))).cursor_pointer())
                            .child(crate::assets::icon("dialog-ok", 24., cx))
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                if ok {
                                    this.accept_group(cx);
                                }
                            })),
                    )
                    .child(
                        div()
                            .id("grp-close")
                            .size(px(28.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .hover(|d| d.bg(pal.panel_hover))
                            .child(crate::assets::icon("dialog-cancel", 24., cx))
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.ui.group = None;
                                this.update_capture(cx);
                                cx.notify();
                            })),
                    ),
            )
            .child(body)
            .into_any_element()
    }
}

// ---------------------------------------------------------------- Select other

impl AssemblyView {
    /// Instances under the cursor, nearest first.
    fn instances_under(&self, position: DVec2, cx: &App) -> Vec<InstanceId> {
        let (o, d) = self.viewport.read(cx).ray(position);
        noshape_ops::assembly::instances_under(&self.result, self.isolate.as_ref(), o, d)
    }

    /// ` (Select other), live (behaviours "qa (2026-10-28, P3) assembly.md
    /// K27"): the Part Studio's dialog listing the face under the pointer
    /// ("Face of Part 2 <1>") and the instances under it; the instance (what
    /// a click selects) is bold. ` again (Shift+` backwards) moves the bold
    /// row; hovering a row pre-highlights it, a click selects it.
    pub(super) fn select_other(&mut self, backwards: bool, cx: &mut Context<Self>) {
        let (rows, ix, at) = match self.ui.select_other.take() {
            Some((rows, ix, at)) if !rows.is_empty() => {
                let n = rows.len();
                let ix = if backwards { (ix + n - 1) % n } else { (ix + 1) % n };
                (rows, ix, at)
            }
            _ => {
                let mut rows: Vec<SelectOtherRow> = self.hover.clone().filter(|t| matches!(t.kind, PickKind::Face | PickKind::Edge | PickKind::Vertex)).map(SelectOtherRow::Entity).into_iter().collect();
                rows.extend(self.instances_under(self.pointer, cx).into_iter().map(SelectOtherRow::Instance));
                let ix = if backwards { rows.len().saturating_sub(1) } else { 1 % rows.len().max(1) };
                (rows, ix, self.pointer)
            }
        };
        self.ui.select_other = Some((rows, ix, at));
        cx.notify();
    }

    /// Select a Select other row (replacing the selection).
    fn choose_select_other(&mut self, row: SelectOtherRow, cx: &mut Context<Self>) {
        self.ui.select_other = None;
        self.ui.list_hover = None;
        self.clear_selection();
        match row {
            SelectOtherRow::Entity(t) => self.select_entity(t, false),
            SelectOtherRow::Instance(id) => self.selected = vec![id],
        }
        self.ui.scroll_to_selection = true;
        self.refresh_scene(false, cx);
        cx.notify();
    }

    pub(super) fn render_select_other(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let (list, ix, at) = self.ui.select_other.clone()?;
        let pal = p();
        let names: HashMap<InstanceId, String> = self.assembly(cx).map(|a| a.instances.iter().map(|i| (i.id, i.name.clone())).collect()).unwrap_or_default();
        let label = |r: &SelectOtherRow| match r {
            SelectOtherRow::Instance(id) => names.get(id).cloned().unwrap_or_default(),
            SelectOtherRow::Entity(t) => {
                let part = self.result.node(&t.path).map(|n| n.name.clone()).unwrap_or_default();
                match t.kind {
                    PickKind::Edge => format!("Edge of {part}"),
                    PickKind::Vertex => format!("Vertex of {part}"),
                    _ => format!("Face of {part}"),
                }
            }
        };
        let mut rows: Vec<AnyElement> = Vec::new();
        for (k, r) in list.iter().enumerate() {
            let (hover_row, pick_row) = (r.clone(), r.clone());
            rows.push(
                h_flex()
                    .id(SharedString::from(format!("so-{k}")))
                    .flex_none()
                    .h(px(24.))
                    .pl(px(6.))
                    .pr(px(20.))
                    .cursor_pointer()
                    .when(k % 2 == 1, |d| d.bg(gpui_kit::rgba(0xffffff59)))
                    .child(div().truncate().text_size(px(13.33)).text_color(pal.text).when(k == ix, |d| d.font_weight(FontWeight::BOLD)).child(label(r)))
                    // Hovering a row pre-highlights its entity or instance.
                    .on_hover(cx.listener(move |this, h: &bool, _, cx| {
                        match &hover_row {
                            SelectOtherRow::Entity(t) => this.hover = h.then(|| t.clone()),
                            SelectOtherRow::Instance(id) => this.ui.list_hover = h.then_some(*id),
                        }
                        this.refresh_scene(false, cx);
                        cx.notify();
                    }))
                    .on_mouse_down(MouseButton::Left, cx.listener(move |this, _: &MouseDownEvent, _, cx| {
                        cx.stop_propagation();
                        this.choose_select_other(pick_row.clone(), cx);
                    }))
                    .into_any_element(),
            );
        }
        if rows.is_empty() {
            rows.push(h_flex().id("so-none").h(px(24.)).pl(px(6.)).text_size(px(13.33)).child("(No selections found)").into_any_element());
        }
        let list_h = (rows.len() as f32 * 24. + 2.).min(92.);
        let size = self.viewport.read(cx).size();
        let (w, h) = (220., 29. + list_h + 28.);
        let left = (at.x as f32 + 100.).min(size.x as f32 - w).max(0.);
        let top = (at.y as f32 + 51.).min(size.y as f32 - h).max(0.);
        let close = cx.listener(|this, _: &ClickEvent, _, cx| {
            this.ui.select_other = None;
            this.ui.list_hover = None;
            this.refresh_scene(false, cx);
            cx.notify();
        });
        Some(
            v_flex()
                .id("asm-select-other")
                .occlude()
                .absolute()
                .left(px(left))
                .top(px(top))
                .w(px(w))
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
                .child(div().id("so-list").h(px(list_h)).flex_none().border_1().border_color(pal.border).rounded(px(2.)).hover(|d| d.border_color(gpui_kit::rgb(0x1651b0))).child(v_flex().id("so-rows").size_full().overflow_y_scroll().children(rows)))
                // Live: the help row, its "?" 14 px, 4 px from the right.
                .child(h_flex().h(px(28.)).flex_none().items_start().justify_end().pt(px(10.)).pr(px(4.)).child(crate::ui::glyph("help-dot", 14., crate::theme::help_dot(0xa0a0a0))))
                .into_any_element(),
        )
    }
}

/// A row of the assembly's Select other dialog.
#[derive(Clone, Debug)]
pub enum SelectOtherRow {
    Entity(AsmPickTarget),
    Instance(InstanceId),
}

// ---------------------------------------------------------------- Insert: configuration inputs

impl AssemblyView {
    /// The configuration inputs of a configured Part Studio or assembly.
    fn insert_config_inputs(&self, studio: TabId, cx: &App) -> Vec<noshape_model::configuration::ConfigInput> {
        match self.doc.read(cx).session.doc.tab(studio).map(|t| &t.content) {
            Some(TabContent::PartStudio(ps)) => ps.configuration.inputs.clone(),
            Some(TabContent::Assembly(a)) => a.configuration.inputs.clone(),
            _ => Vec::new(),
        }
    }

    /// A configured row's split button (live assembly/592–594, 57x30):
    /// "Set to last viewed configuration" and ▾ Set to last viewed | Reset
    /// to default.
    pub(super) fn insert_config_button(&mut self, studio: TabId, cx: &mut Context<Self>) -> Option<AnyElement> {
        if self.insert_config_inputs(studio, cx).is_empty() {
            return None;
        }
        let set_last = move |this: &mut Self, window: &mut Window, cx: &mut Context<Self>| {
            match crate::assembly_hooks::last_viewed_config(studio) {
                Some(v) => {
                    this.set_config_panel_values(&v, window, cx);
                    this.ui.insert_config.insert(studio, v);
                }
                None => this.show_cfg_toast("Could not find last viewed configuration.".into(), false, cx),
            }
            cx.notify();
        };
        let view = cx.entity();
        let caret: Rc<std::cell::Cell<Bounds<Pixels>>> = Default::default();
        let probe = caret.clone();
        Some(
            h_flex()
                .flex_none()
                .mr(px(6.))
                .h(px(30.))
                .rounded(px(3.))
                .border_1()
                .border_color(rgb(0xaaaaaa))
                .child(
                    div()
                        .id(SharedString::from(format!("inscfg-last-{studio}")))
                        .w(px(35.))
                        .h_full()
                        .flex()
                        .items_center()
                        .justify_center()
                        .border_r_1()
                        .border_color(rgb(0xaaaaaa))
                        .cursor_pointer()
                        .hover(|d| d.bg(rgb(0xeaeaea)))
                        .tooltip(crate::ui::tooltip("Set to last viewed configuration"))
                        .child(crate::assets::icon(&icon_path("configuration"), 20., cx))
                        .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                            cx.stop_propagation();
                            set_last(this, window, cx);
                        })),
                )
                .child(
                    div()
                        .id(SharedString::from(format!("inscfg-dd-{studio}")))
                        .relative()
                        .w(px(20.))
                        .h_full()
                        .flex()
                        .items_center()
                        .justify_center()
                        .cursor_pointer()
                        .hover(|d| d.bg(rgb(0xeaeaea)))
                        .child(crate::os_ui::glyph(crate::os_ui::GLYPH_CARET, 8., rgb(0x000000)))
                        .child(canvas(move |b, _, _| probe.set(b), |_, _, _, _| {}).absolute().top_0().left_0().size_full())
                        .on_click(move |_: &ClickEvent, window, cx| {
                            cx.stop_propagation();
                            let (v1, v2) = (view.clone(), view.clone());
                            let menu = crate::os_menu::OsMenu::build(window, cx, move |m, _, _| {
                                m.item(crate::os_menu::OsMenuItem::new("Set to last viewed").icon_name("cfg-last-viewed").on_click(move |_, window, cx| v1.update(cx, |this, cx| set_last(this, window, cx))))
                                    .item(crate::os_menu::OsMenuItem::new("Reset to default").icon_name("cfg-reset-default").on_click(move |_, window, cx| {
                                        v2.update(cx, |this, cx| {
                                            this.ui.insert_config.remove(&studio);
                                            // Change configuration (qa5 CA5): its inputs go
                                            // back to their defaults too.
                                            if let Some(inputs) = this.config_panel.as_ref().map(|s| s.inputs.clone()) {
                                                let d = noshape_model::configuration::Configuration { inputs, configured: Vec::new() }.defaults();
                                                this.set_config_panel_values(&d, window, cx);
                                            }
                                            cx.notify();
                                        })
                                    }))
                            });
                            // Live 594: the list hangs under the caret, right-aligned to it.
                            let b = caret.get();
                            crate::os_menu::open_anchored(point(b.right(), b.bottom()), Anchor::TopRight, menu, window, cx);
                        }),
                )
                .into_any_element(),
        )
    }

    /// The Change configuration panel takes `values` (its fields too);
    /// Generate turns on when they differ from the panel's.
    fn set_config_panel_values(&mut self, values: &ConfigValues, window: &mut Window, cx: &mut Context<Self>) {
        let Some(s) = &mut self.config_panel else { return };
        let mut changed = false;
        for (k, v) in &values.0 {
            if s.values.0.get(k) != Some(v) {
                s.values.0.insert(k.clone(), v.clone());
                changed = true;
            }
            if let Some((_, st)) = s.texts.iter().find(|(id, _)| id == k) {
                let v = v.clone();
                st.update(cx, |st, cx| st.set_value(v, window, cx));
            }
        }
        s.generate |= changed;
        cx.notify();
    }

    /// Configuration inputs of a configured Part Studio (shown when the
    /// row is expanded, live 595: "Size [Default ▾]" above the parts) or
    /// assembly: the values apply to the next insert.
    pub(super) fn render_insert_config(&mut self, studio: TabId, cx: &mut Context<Self>) -> Option<AnyElement> {
        use noshape_model::configuration::ConfigInputKind;
        let inputs = self.insert_config_inputs(studio, cx);
        if inputs.is_empty() {
            return None;
        }
        let pal = p();
        let values = self.ui.insert_config.get(&studio).cloned().unwrap_or_default();
        let mut col = v_flex().py(px(2.)).gap(px(2.));
        let label = |name: String| div().w(px(44.)).flex_none().truncate().text_size(px(12.)).text_color(pal.text_muted).child(name);
        // Visibility conditions: hidden inputs and options don't show.
        let (shown, effective) = noshape_model::configuration::Configuration { inputs: inputs.clone(), configured: Vec::new() }.visibility(&values);
        for (inp, vis) in inputs.iter().zip(&shown) {
            if !vis.visible {
                continue;
            }
            let id = inp.id.clone();
            match &inp.kind {
                ConfigInputKind::List { options, .. } => {
                    // Live 595: the feature-dialog select (underlined, the list
                    // hanging under it with the current row in blue).
                    let cur = effective.0.get(&inp.id).cloned().unwrap_or_default();
                    let items: Vec<(SharedString, SharedString)> = options.iter().filter(|o| vis.options.contains(&o.id)).map(|o| (SharedString::from(o.id.clone()), SharedString::from(o.name.clone()))).collect();
                    let key = (studio, inp.id.clone());
                    let sel = match self.ui.insert_cfg_selects.get(&key) {
                        Some(s) => s.clone(),
                        None => {
                            let s = cx.new(|_| crate::os_select::OsSelect::new(SharedString::from(format!("inscfg-{studio}-{}", inp.id))));
                            let id2 = id.clone();
                            let sub = cx.subscribe(&s, move |this: &mut Self, _, ev: &crate::os_select::OsSelectEvent, cx| {
                                let crate::os_select::OsSelectEvent::Picked(oid) = ev;
                                this.ui.insert_config.entry(studio).or_default().0.insert(id2.clone(), oid.to_string());
                                cx.notify();
                            });
                            self.ui.insert_cfg_subs.push(sub);
                            self.ui.insert_cfg_selects.insert(key, s.clone());
                            s
                        }
                    };
                    sel.update(cx, |s, _| s.set_items(items, cur));
                    col = col.child(h_flex().min_h(px(28.)).items_start().child(div().pl(px(5.)).pt(px(6.)).flex_none().text_size(px(12.)).text_color(pal.text_muted).child(crate::ui::user_text(inp.name.clone()))).child(div().id(SharedString::from(format!("inscfg-tt-{studio}-{}", id))).flex_1().min_w_0().tooltip(crate::ui::tooltip(inp.name.clone())).child(sel)));
                }
                ConfigInputKind::Checkbox { .. } => {
                    let cur = effective.0.get(&inp.id).is_some_and(|v| v == "true");
                    col = col.child(Checkbox::new(SharedString::from(format!("inscfg-{studio}-{}", inp.id))).label(inp.name.clone()).checked(cur).on_click(cx.listener(move |this, v: &bool, _, cx| {
                        this.ui.insert_config.entry(studio).or_default().0.insert(id.clone(), v.to_string());
                        cx.notify();
                    })));
                }
                ConfigInputKind::Variable { default, .. } => {
                    col = col.child(h_flex().h(px(26.)).gap(px(6.)).child(label(inp.name.clone())).child(div().text_size(px(12.)).text_color(pal.text).child(effective.0.get(&inp.id).cloned().unwrap_or_else(|| default.text.clone()))));
                }
                ConfigInputKind::Text { .. } => {}
            }
        }
        Some(col.into_any_element())
    }
}

// ---------------------------------------------------------------- live mate values

impl AssemblyView {
    /// Mates whose values show in the graphics: only the selected mate
    /// (live 300–318: nothing during or after a drag; the values stay while
    /// the mate is selected, Esc keeps them, a click in empty space clears).
    fn value_mates(&self, cx: &App) -> Vec<MateId> {
        let Some(asm) = self.assembly(cx) else { return vec![] };
        let shown = |m: &&Mate| !noshape_model::assembly::eval::mate_dofs(m.mate_type).is_empty();
        if let Some((id, ..)) = &self.ui.value_edit {
            return vec![*id];
        }
        self.selected_mate.and_then(|id| asm.mate(id)).filter(shown).map(|m| vec![m.id]).unwrap_or_default()
    }

    /// The selected mate's values (live 305z) or the mate dialog's ▶ preview
    /// rotation (live 575, whole degrees), as Onshape's mate value dimensions
    /// (bundle `calculateMateValues` / `calculateDimensionDisplayData`): the
    /// first connector measured in the second's frame; the rotation is an
    /// angular dimension around the moving connector (radius 80 px when first
    /// shown, `PREVIEW_ANGULAR_DIMENSION_RADIUS_PIXEL_SIZE`), translations are
    /// linear dimensions X: origin → x, Y: (x, 0) → (x, y), Z: (x, y, 0) →
    /// (x, y, z). Each value's text lies in its plane, centred on its line or
    /// arc, which breaks around it.
    pub(super) fn value_dims(&self, cx: &App) -> Vec<ValueDim> {
        let (id, preview) = match &self.anim {
            Some(a) if a.preview => (a.mate, true),
            Some(_) => return vec![],
            None if self.mate.is_some() => return vec![],
            None => match self.value_mates(cx).first() {
                Some(id) => (*id, false),
                None => return vec![],
            },
        };
        let Some(ms) = self.result.mates.iter().find(|m| m.id == id && m.ok && !m.inactive) else { return vec![] };
        let Some(t) = self.assembly(cx).and_then(|a| a.mate(id)).map(|m| m.mate_type) else { return vec![] };
        let (Some(a), Some(b)) = (ms.frame_a, ms.frame_b) else { return vec![] };
        let vp = self.viewport.read(cx);
        let size = vp.size();
        let wpp = vp.camera.world_per_pixel(size.y);
        let asc = VALUE_TEXT_ASC * wpp;
        let pad = 3.0 * wpp;
        let units = self.units(cx);
        let (bx, by, bz) = (b.rotation * DVec3::X, b.rotation * DVec3::Y, b.rotation * DVec3::Z);
        let on = onshape_mate_values(&a, &b);
        let width = |text: &str| noshape_sketch::text::layout_text(text, noshape_sketch::text::DEFAULT_FONT, false, false).width * asc;
        // Text axes turned so it never reads mirrored or upside down.
        let readable = |center: DVec3, mut x: DVec3, mut y: DVec3| {
            if let (Some(c), Some(px_), Some(py_)) = (vp.project(center), vp.project(center + x * wpp * 40.0), vp.project(center + y * wpp * 40.0)) {
                let (sx, sy) = (px_ - c, py_ - c);
                if sx.x * sy.y - sx.y * sy.x > 0.0 {
                    y = -y;
                }
                if sx.x < 0.0 {
                    x = -x;
                    y = -y;
                }
            }
            (x, y)
        };
        let mut out = Vec::new();
        for (ix, _) in noshape_model::assembly::eval::mate_dofs(t).iter().copied().filter(|(ix, _)| !preview || *ix == 0) {
            if ix == 0 {
                let radius = match self.ui.value_radius.get() {
                    Some((m, r)) if m == id => r,
                    _ => {
                        let r = VALUE_ARC_RADIUS_PX * wpp;
                        self.ui.value_radius.set(Some((id, r)));
                        r
                    }
                };
                // The value is live's rotationZ, our solver's (oracle_asm R2:
                // +45 with the pin turned -45° about the plate's Z; the limits
                // bound it); `on[0]`, the angle from the second connector's X
                // to the first's, reads the other way for connectors aligned
                // as usual (MATE-LIMITS-OPPOSITE-SIGN: the label read 270° at
                // a 0–90 limit's 90).
                let sweep = mate_angle_sweep(ms.values[0]);
                let text = if preview { format!("{}°", noshape_units::to_fixed(sweep.to_degrees(), 0)) } else { format!("{}°", mate_angle_label(sweep.to_degrees())) };
                // The arc still runs from the one X axis to the other: drawn
                // clockwise (its frame mirrored) when the value turns that way.
                let close = |x: f64, y: f64| (x - y).rem_euclid(std::f64::consts::TAU).min((y - x).rem_euclid(std::f64::consts::TAU)) < 1e-6;
                let cw = close(on[0], -ms.values[0]) && !close(on[0], ms.values[0]);
                let rot = if cw { b.rotation * glam::DQuat::from_rotation_x(std::f64::consts::PI) } else { b.rotation };
                let frame = noshape_model::assembly::solver::Frame { origin: a.origin, rotation: rot };
                let mid = sweep * 0.5;
                // Live (qa5 MT7): at 0° the label sits at the X axis's end.
                let center = if sweep < 1e-9 {
                    a.origin + bx * (self.triad_size() + width(&text) * 0.5 + pad)
                } else {
                    a.origin + rot * DVec3::new(mid.cos() * radius, mid.sin() * radius, 0.0)
                };
                let (x, y) = readable(center, bx, by);
                let (hw, hh) = (width(&text) * 0.5 + pad, asc * 0.65 + pad);
                let (tx, ty) = (mid.sin().abs(), mid.cos().abs());
                let along = (if tx > 1e-9 { hw / tx } else { f64::INFINITY }).min(if ty > 1e-9 { hh / ty } else { f64::INFINITY });
                let gap = along.is_finite().then(|| (mid, along / radius));
                out.push(ValueDim { mate: id, ix, text, center, x, y, asc, shape: ValueShape::Arc(crate::assembly_scene::ValueArc { frame, radius, sweep, gap }) });
                continue;
            }
            let (x, y, z) = (on[1], on[2], on[3]);
            let p = |u: f64, v: f64, w: f64| b.origin + bx * u + by * v + bz * w;
            let (from, to, dir, up) = match ix {
                1 => (p(0.0, 0.0, 0.0), p(x, 0.0, 0.0), bx, by),
                2 => (p(x, 0.0, 0.0), p(x, y, 0.0), by, bx),
                _ => (p(x, y, 0.0), p(x, y, z), bz, bx),
            };
            let text = noshape_units::format_quantity(noshape_units::Quantity::meters(on[ix]), &units);
            let center = (from + to) * 0.5;
            let (tx, ty) = readable(center, dir, up);
            let len = from.distance(to);
            let gap = (len > 1e-12).then(|| {
                let half = (width(&text) * 0.5 + pad) / len;
                (0.5 - half, 0.5 + half)
            });
            out.push(ValueDim { mate: id, ix, text, center, x: tx, y: ty, asc, shape: ValueShape::Line(crate::assembly_scene::ValueLine { a: from, b: to, up, gap, head: 10.0 * wpp }) });
        }
        out
    }

    /// Each value's text drawn in its plane, plus a hit box over it for
    /// double-click editing.
    fn render_value_text(&mut self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let dims = self.value_dims(cx);
        let vp = self.viewport.read(cx);
        let mut polys: Vec<Vec<Point<Pixels>>> = Vec::new();
        let mut hits: Vec<(MateId, usize, DVec2, DVec2)> = Vec::new();
        for d in &dims {
            let layout = noshape_sketch::text::layout_text(&d.text, noshape_sketch::text::DEFAULT_FONT, false, false);
            let base = d.center - d.x * (layout.width * d.asc * 0.5) - d.y * (d.asc * 0.5);
            let (mut lo, mut hi) = (DVec2::splat(f64::INFINITY), DVec2::splat(f64::NEG_INFINITY));
            for contour in layout.polylines(6) {
                let mut poly = Vec::with_capacity(contour.len());
                for q in contour {
                    let Some(s) = vp.project(base + d.x * (q.x * d.asc) + d.y * (q.y * d.asc)) else {
                        poly.clear();
                        break;
                    };
                    lo = lo.min(s);
                    hi = hi.max(s);
                    poly.push(point(px(s.x as f32), px(s.y as f32)));
                }
                if poly.len() >= 3 {
                    polys.push(poly);
                }
            }
            if lo.x.is_finite() {
                hits.push((d.mate, d.ix, lo, hi));
            }
        }
        if polys.is_empty() {
            return vec![];
        }
        let color: Hsla = rgb(0x1a35b8).into();
        let mut out = vec![canvas(
            |_, _, _| {},
            move |bounds, _, window, _| {
                let o = bounds.origin;
                let mut b = PathBuilder::fill();
                for poly in &polys {
                    b.move_to(point(o.x + poly[0].x, o.y + poly[0].y));
                    for q in &poly[1..] {
                        b.line_to(point(o.x + q.x, o.y + q.y));
                    }
                    b.close();
                }
                if let Ok(path) = b.build() {
                    window.paint_path(path, color);
                }
            },
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .into_any_element()];
        if self.anim.is_some() {
            return out;
        }
        for (id, ix, lo, hi) in hits {
            let editing = self.ui.value_edit.as_ref().filter(|(m, d, ..)| *m == id && *d == ix).map(|(_, _, st, _)| st.clone());
            let hit = div().id(SharedString::from(format!("mv-{}-{ix}", id.0))).absolute().left(px(lo.x as f32 - 3.)).top(px(lo.y as f32 - 3.)).w(px((hi.x - lo.x) as f32 + 6.)).h(px((hi.y - lo.y) as f32 + 6.));
            out.push(
                match editing {
                    Some(st) => hit.child(div().w(px(80.)).bg(rgb(0xffffff)).child(Input::new(&st).xsmall())),
                    None => hit.cursor_pointer().on_click(cx.listener(move |this, ev: &ClickEvent, window, cx| {
                        if ev.click_count() >= 2 {
                            this.start_value_edit(id, ix, window, cx);
                        }
                    })),
                }
                .occlude()
                .into_any_element(),
            );
        }
        out
    }

    pub(super) fn render_mate_values(&mut self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let mut out = self.render_value_text(cx);
        out.extend(self.render_triad_letters(cx));
        out
    }

    /// The selected mate's connector triad carries X / Y / Z letters in the
    /// axis colours just past the tips (live 305z, qa5 MT7).
    fn render_triad_letters(&self, cx: &App) -> Vec<AnyElement> {
        if self.mate.is_some() {
            return vec![];
        }
        let Some(ms) = self.selected_mate.and_then(|id| self.result.mates.iter().find(|m| m.id == id)) else { return vec![] };
        let Some(f) = ms.frame_b.or(ms.frame_a) else { return vec![] };
        let vp = self.viewport.read(cx);
        let size = self.triad_size();
        let Some(o) = vp.project(f.origin) else { return vec![] };
        let colors = {
            use crate::theme::{pick, viewport as l, viewport_dark as d};
            [pick(l::AXIS_X, d::AXIS_X), pick(l::AXIS_Y, d::AXIS_Y), pick(l::AXIS_Z, d::AXIS_Z)]
        };
        let mut out = Vec::new();
        for (k, (axis, letter)) in [(f.x(), "X"), (f.y(), "Y"), (f.z(), "Z")].into_iter().enumerate() {
            let Some(tip) = vp.project(f.origin + axis * size) else { continue };
            // 7 px beyond the tip along the axis on screen.
            let dir = (tip - o).normalize_or_zero();
            let at = tip + dir * 7.0;
            out.push(
                div()
                    .absolute()
                    .left(px(at.x as f32 - 5.))
                    .top(px(at.y as f32 - 7.))
                    .w(px(10.))
                    .h(px(14.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px(11.))
                    .text_color(gpui_kit::rgb_static(colors[k]))
                    .child(letter)
                    .into_any_element(),
            );
        }
        out
    }

    fn start_value_edit(&mut self, id: MateId, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        let units = self.units(cx);
        // The rotation as shown: our solver's value (live's rotationZ), 0 not 360.
        let v = self.result.mates.iter().find(|m| m.id == id).and_then(|m| Some(if ix == 0 { mate_angle_sweep(m.values[0]) } else { onshape_mate_values(&m.frame_a?, &m.frame_b?)[ix] })).unwrap_or(0.0);
        let q = if ix == 0 { noshape_units::Quantity::radians(v) } else { noshape_units::Quantity::meters(v) };
        let st = cx.new(|cx| InputState::new(window, cx).default_value(noshape_units::format_quantity(q, &units)));
        // Focus once the double-click is over (the viewport's mouse-up
        // takes focus back otherwise and the blur closes the box).
        let st2 = st.clone();
        cx.spawn_in(window, async move |_, cx| {
            cx.background_executor().timer(Duration::from_millis(40)).await;
            let _ = st2.update_in(cx, |s, window, cx| {
                s.focus(window, cx);
                s.select_all(window, cx);
            });
        })
        .detach();
        let sub = cx.subscribe_in(&st, window, move |this: &mut Self, st, ev: &InputEvent, window, cx| match ev {
            InputEvent::PressEnter { .. } => {
                let text = st.read(cx).value().to_string();
                this.ui.value_edit = None;
                this.apply_mate_value(id, ix, &text, cx);
                window.focus(&this.focus, cx);
            }
            InputEvent::Blur => {
                this.ui.value_edit = None;
                cx.notify();
            }
            _ => {}
        });
        self.ui.value_edit = Some((id, ix, st, sub));
        cx.notify();
    }

    /// Move the parts so one mate value becomes `text` (other mates enforced).
    pub(super) fn apply_mate_value(&mut self, id: MateId, ix: usize, text: &str, cx: &mut Context<Self>) {
        let units = self.units(cx);
        let dim = if ix == 0 { noshape_units::Dim::ANGLE } else { noshape_units::Dim::LENGTH };
        let v = match noshape_units::evaluate_as(text, dim, &units, &noshape_units::EmptyScope) {
            Ok(q) => q.value,
            Err(e) => {
                self.message = Some((true, format!("{e}")));
                cx.notify();
                return;
            }
        };
        // The typed value is Onshape's: the rotation our solver's own, the
        // translations `onshape_mate_values`'; hold the matching solver
        // values (all translations, so the others stay).
        let Some(ms) = self.result.mates.iter().find(|m| m.id == id) else { return };
        let (Some(fa), Some(fb)) = (ms.frame_a, ms.frame_b) else { return };
        let target = solver_values_for(&fa, &fb, ms.values, ix, v);
        let t = self.assembly(cx).and_then(|a| a.mate(id)).map(|m| m.mate_type);
        let holds: Vec<(MateId, usize, f64)> = t
            .map(|t| noshape_model::assembly::eval::mate_dofs(t).iter().map(|(k, _)| *k).filter(|k| (ix == 0) == (*k == 0)).map(|k| (id, k, target[k])).collect())
            .unwrap_or_default();
        let start: HashMap<InstanceId, Placement> = self.result.solved.placements.iter().cloned().collect();
        let solved = self.result.prepared.solve(&SolveOptions { hold_values: holds, start: Some(start), nested_start: Some(self.result.solved.nested.clone()), ..Default::default() });
        let name = self.assembly(cx).and_then(|a| a.mate(id).map(|m| m.name.clone())).unwrap_or_default();
        self.apply_solved(solved, cx);
        self.edit(&format!("Move : {name}"), cx, |_| {});
        cx.notify();
    }
}

// ---------------------------------------------------------------- snap insert

impl AssemblyView {
    /// Explicit mate connectors (from its Part Studio) of an instance.
    pub(super) fn explicit_connectors(&self, id: InstanceId) -> Vec<MateConnectorRef> {
        let mut out = Vec::new();
        let mut seen = HashSet::new();
        for o in self.result.occurrences.iter().filter(|o| o.top() == id) {
            if !seen.insert(o.path.clone()) {
                continue;
            }
            for c in &o.context.connectors {
                out.push(MateConnectorRef::on(id, o.path[1..].to_vec(), c.reference.clone()));
            }
        }
        out
    }

    /// Placing in snap mode: the chosen explicit connector of the inserted
    /// instance lands on the mate connector under the cursor. Returns false
    /// (free placement) without connectors or target.
    pub(super) fn update_insert_snap(&mut self, id: InstanceId, cx: &mut Context<Self>) -> bool {
        let sources = self.explicit_connectors(id);
        if sources.is_empty() {
            return false;
        }
        let mut src = sources[self.ui.place_src_ix % sources.len()].clone();
        src.flip = self.ui.std.flip;
        let Some(dst) = self.snap_target(self.pointer, id, cx) else {
            self.ui.place_snap = None;
            return false;
        };
        let units = self.units(cx);
        let (Some(fs), Some(fd), Some(pose)) = (self.result.connector_world(&src, &units), self.result.connector_world(&dst, &units), self.result.solved.placement(id)) else { return false };
        let np = noshape_model::assembly::eval::snap_placement(&pose, &fs, &fd);
        let mut solved = self.result.solved.clone();
        for (i, pl) in &mut solved.placements {
            if *i == id {
                *pl = np;
            }
        }
        self.apply_solved(solved, cx);
        self.placing_point = Some(fd.origin);
        self.ui.place_snap = Some((src, dst));
        true
    }

    /// Ctrl while snap-inserting: the next explicit connector is the source.
    pub(super) fn cycle_insert_source(&mut self, cx: &mut Context<Self>) -> bool {
        if self.placing.as_ref().is_none_or(|p| p.len() != 1) || (self.snap.is_none() && !self.placing.as_ref().is_some_and(|p| self.std_placing(p[0], cx))) {
            return false;
        }
        self.ui.place_src_ix += 1;
        if self.update_placing(cx) {
            self.rebuild_scene(true, cx);
        }
        true
    }
}

// ---------------------------------------------------------------- measure and section view

impl AssemblyView {
    /// `[`: Measure tool on / off. While on, clicked faces, edges and vertices
    /// are measured (click again to drop one).
    pub(super) fn toggle_measure(&mut self, cx: &mut Context<Self>) {
        self.ui.measure = if self.ui.measure.is_some() { None } else { Some(Vec::new()) };
        self.ui.measure_result.clear();
        self.update_capture(cx);
        self.refresh_scene(false, cx);
        cx.notify();
    }

    pub(super) fn measure_pick(&mut self, t: AsmPickTarget, cx: &mut Context<Self>) {
        let Some(list) = &mut self.ui.measure else { return };
        if !matches!(t.kind, PickKind::Face | PickKind::Edge | PickKind::Vertex) {
            return;
        }
        let key = (t.path.clone(), t.reference.clone());
        match list.iter().position(|x| *x == key) {
            Some(i) => {
                list.remove(i);
            }
            None => list.push(key),
        }
        let picks = list.clone();
        self.ui.measure_result = noshape_model::assembly::analysis::measure_entities(&self.result, &picks);
        self.refresh_scene(false, cx);
        cx.notify();
    }

    /// The Measure panel (`crate::measure_panel`, live part-studio/82 and
    /// icons2 Round 14): docked at the bottom right like the Part Studio's.
    pub(super) fn render_measure(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let picks = self.ui.measure.as_ref()?;
        let asm = self.assembly(cx)?;
        // The field's entries: "Face of Part 1 <2>".
        let selected: Vec<String> = picks
            .iter()
            .map(|(path, r)| {
                let inst = path.last().and_then(|id| asm.instance(*id)).map(|i| i.name.clone()).unwrap_or_default();
                let kind = match r {
                    Reference::Face(_) => "Face",
                    Reference::Edge(_) => "Edge",
                    Reference::Vertex(_) => "Vertex",
                    _ => "Entity",
                };
                format!("{kind} of {inst}")
            })
            .collect();
        let units = self.units(cx);
        let settings = self.ui.measure_settings;
        let rows = crate::measure_panel::rows(&self.ui.measure_result, &settings, &units);
        let (v1, v2) = (cx.entity(), cx.entity());
        let panel = crate::measure_panel::panel(
            selected,
            rows,
            settings,
            units,
            std::rc::Rc::new(move |s, cx| {
                v1.update(cx, |this, cx| {
                    this.ui.measure_settings = s;
                    cx.notify();
                })
            }),
            std::rc::Rc::new(move |_, cx| v2.update(cx, |this, cx| this.toggle_measure(cx))),
            cx,
        );
        Some(div().absolute().bottom(px(36.)).right(px(40.)).child(panel).into_any_element())
    }

    /// Shift+X / Section view…: open the dialog (from the hovered face or the
    /// selected mate connector, else empty: live part-studio/70); with the
    /// dialog open it cancels; with a section on and no dialog it turns it off.
    pub(super) fn toggle_section(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match &self.ui.section {
            Some(s) if s.dialog.is_some() => return self.close_section_dialog(false, window, cx),
            Some(s) if !s.planes.is_empty() => {
                self.ui.section = None;
                self.rebuild_scene(false, cx);
                cx.notify();
                return;
            }
            _ => {}
        }
        let saved = self.ui.section.take().map(Box::new);
        let mut planes = Vec::new();
        // The face under the pointer (shortcut), else the selected face the
        // menu was opened on (live: right-click a face › Section view… cuts
        // parallel to it, the manipulator at its centroid).
        let menu_face = self.ui.menu_face.take();
        let face = self.hover.clone().filter(|t| t.kind == PickKind::Face).or(menu_face).or_else(|| {
            self.ui.entity_sel.iter().rev().find(|(_, r)| matches!(r, Reference::Face(_))).and_then(|(path, r)| {
                let occ = self.result.occurrences.iter().position(|o| o.path == *path && o.face_refs.contains(r))?;
                Some(AsmPickTarget { path: path.clone(), reference: r.clone(), kind: PickKind::Face, occ, anchor: None })
            })
        });
        if let Some(t) = face {
            planes.extend(self.section_plane_of(&t, cx));
        }
        if planes.is_empty()
            && let Some(id) = self.selected_mate
            && let Some(f) = self.result.connector_frame(id)
        {
            planes.push(AsmSectionPlane::new(f.origin, f.z(), f.x()));
        }
        let active = planes.len().saturating_sub(1);
        self.ui.section = Some(AsmSection { planes, include: false, items: Vec::new(), dialog: Some(AsmSectionDialog { items_active: false, active, chosen: true, saved, popup: None, popup_plane: None }) });
        self.rebuild_scene(false, cx);
        cx.notify();
    }

    /// "Edit section view…": the dialog on the current section, where ✕
    /// turns the section off (live behaviours "Edit section view…": it does
    /// not restore it, even with nothing changed).
    pub fn edit_section(&mut self, cx: &mut Context<Self>) {
        let Some(s) = self.ui.section.as_mut().filter(|s| s.dialog.is_none() && !s.planes.is_empty()) else { return };
        s.dialog = Some(AsmSectionDialog { items_active: false, active: s.planes.len() - 1, chosen: false, saved: None, popup: None, popup_plane: None });
        self.rebuild_scene(false, cx);
        cx.notify();
    }

    /// "Turn off section view" (also with the dialog open).
    pub fn turn_off_section(&mut self, cx: &mut Context<Self>) {
        self.ui.section = None;
        self.rebuild_scene(false, cx);
        cx.notify();
    }

    /// The plane through a picked face (its centroid frame).
    fn section_plane_of(&self, t: &AsmPickTarget, cx: &App) -> Option<AsmSectionPlane> {
        let o = self.result.occurrences.get(t.occ)?;
        let r = Reference::Implicit { on: Box::new(t.reference.clone()), inference: Inference::Centroid };
        let f = noshape_model::assembly::eval::resolve_connector_in(&o.context, Some(&o.body), &r, &self.units(cx))?;
        let w = noshape_model::assembly::eval::matrix_frame(&o.transform).mul(&f);
        Some(AsmSectionPlane::new(w.origin, w.z(), w.x()))
    }

    /// ✓ keeps the section, ✕ / Esc restores what was there before.
    pub(super) fn close_section_dialog(&mut self, accept: bool, _window: &mut Window, cx: &mut Context<Self>) {
        let Some(s) = &mut self.ui.section else { return };
        let Some(d) = s.dialog.take() else { return };
        if !accept {
            self.ui.section = d.saved.map(|b| *b);
        } else if s.planes.is_empty() {
            self.ui.section = None;
        }
        self.rebuild_scene(false, cx);
        cx.notify();
    }

    /// A pick while the dialog is open: a face for "Section planes" (up to
    /// three), an instance for "Items".
    pub(super) fn section_pick(&mut self, t: AsmPickTarget, cx: &mut Context<Self>) {
        let Some(items_active) = self.ui.section.as_ref().and_then(|s| s.dialog.as_ref()).map(|d| d.items_active) else { return };
        if items_active {
            let top = t.path[0];
            if let Some(s) = &mut self.ui.section {
                match s.items.iter().position(|x| *x == top) {
                    Some(i) => {
                        s.items.remove(i);
                    }
                    None => s.items.push(top),
                }
            }
        } else if t.kind == PickKind::Face
            && let Some(pl) = self.section_plane_of(&t, cx)
            && let Some(s) = &mut self.ui.section
            && s.planes.len() < MAX_SECTION_PLANES
        {
            s.planes.push(pl);
            if let Some(d) = &mut s.dialog {
                d.active = s.planes.len() - 1;
                d.chosen = true;
            }
        }
        self.rebuild_scene(false, cx);
        cx.notify();
    }

    /// The plane the dialog's manipulator drives (live: only while the
    /// dialog is open).
    fn active_section_plane(&self) -> Option<&AsmSectionPlane> {
        let s = self.ui.section.as_ref()?;
        s.planes.get(s.dialog.as_ref()?.active)
    }

    fn active_section_plane_mut(&mut self) -> Option<&mut AsmSectionPlane> {
        let s = self.ui.section.as_mut()?;
        let i = s.dialog.as_ref()?.active;
        s.planes.get_mut(i)
    }

    /// A pixel's world size at a point.
    fn wpp_at(&self, _p: DVec3, cx: &App) -> f64 {
        let vp = self.viewport.read(cx);
        vp.camera.world_per_pixel(vp.size().y.max(1.0))
    }

    /// The manipulator glyph for the scene (live assembly/2368, 2370).
    pub(super) fn section_glyph(&self, cx: &App) -> Option<crate::assembly_scene::SectionGlyph> {
        let pl = self.active_section_plane()?;
        let base = pl.base();
        let d = self.ui.section_drag.as_ref().filter(|d| d.moved);
        Some(crate::assembly_scene::SectionGlyph {
            base,
            dir: pl.dir(),
            x: pl.x,
            y: pl.y(),
            wpp: self.wpp_at(base, cx),
            view: self.viewport.read(cx).camera.forward(),
            hovered: match self.hover_overlay {
                Some(OverlayPick::Section(h)) => Some(h),
                _ => None,
            },
            dragging: d.map(|d| d.handle),
            start: d.and_then(|d| match d.handle {
                SectionHandle::Arrow => Some((d.plane0.base(), false)),
                SectionHandle::Ring(_) => Some((d.plane0.dir(), true)),
                SectionHandle::Base => None,
            }),
        })
    }

    /// What a dragged section base snaps to under `position` (live
    /// behaviours "skb (10-24) section base snap + move to origin" (1)): a
    /// vertex within 22 px (also off the parts); on a part, the centre of a
    /// face bounded by one closed edge (a pin's top circle), an edge of the
    /// face under the pointer within 35 px (its nearest point), else the
    /// point on the face. Off the parts and away from vertices: nothing.
    fn section_base_snap(&self, position: glam::DVec2, cx: &App) -> Option<SectionSnap> {
        let vp = self.viewport.read(cx);
        let (ro, rd) = vp.ray(position);
        let to_d = |p: [f32; 3]| DVec3::new(p[0] as f64, p[1] as f64, p[2] as f64);
        // The nearest face hit: (ray parameter, occurrence, face).
        let mut hit: Option<(f64, usize, usize)> = None;
        for (oi, o) in self.result.occurrences.iter().enumerate().filter(|(_, o)| !o.hidden) {
            let inv = o.transform.inverse();
            let (lo, ld) = (inv.transform_point3(ro), inv.transform_vector3(rd));
            let m = &o.mesh;
            let pos = |i: u32| m.positions.get(i as usize).copied().map(to_d).unwrap_or_default();
            for (fi, &(first, count)) in m.face_ranges.iter().enumerate() {
                let Some(tris) = m.indices.get(first as usize..(first + count) as usize) else { continue };
                for t in tris.as_chunks::<3>().0 {
                    if let Some(tt) = panels::ray_triangle(lo, ld, pos(t[0]), pos(t[1]), pos(t[2]))
                        && hit.is_none_or(|h| tt < h.0)
                    {
                        hit = Some((tt, oi, fi));
                    }
                }
            }
        }
        // Vertices within 22 px: of the part under the pointer, or of any part.
        let mut best_v: Option<(f64, usize, usize)> = None;
        for (oi, o) in self.result.occurrences.iter().enumerate().filter(|(oi, o)| !o.hidden && hit.is_none_or(|h| h.1 == *oi)) {
            for (vi, v) in o.mesh.vertices.iter().enumerate() {
                let Some(sp) = vp.project(o.transform.transform_point3(to_d(*v))) else { continue };
                let dist = sp.distance(position);
                if dist <= SECTION_SNAP_VERTEX_PX && best_v.is_none_or(|b| dist < b.0) {
                    best_v = Some((dist, oi, vi));
                }
            }
        }
        if let Some((_, oi, vi)) = best_v {
            let o = &self.result.occurrences[oi];
            return Some(SectionSnap { point: o.transform.transform_point3(to_d(o.mesh.vertices[vi])), path: o.path.clone(), entity: o.vertex_refs.get(vi)?.clone(), dots: Vec::new() });
        }
        let (t, oi, fi) = hit?;
        let o = &self.result.occurrences[oi];
        let world = |p: [f32; 3]| o.transform.transform_point3(to_d(p));
        let edges: Vec<usize> = o.context.body(&o.body).and_then(|b| b.shape.edge_faces().ok()).map(|ef| ef.iter().enumerate().filter(|(_, fs)| fs.contains(&(fi as u32))).map(|(ei, _)| ei).collect()).unwrap_or_default();
        let face = o.face_refs.get(fi)?.clone();
        let dots: Vec<DVec3> = edges.iter().filter_map(|ei| o.mesh.edges.get(*ei)).flat_map(|pl| [pl.first(), pl.last()]).flatten().map(|p| world(*p)).collect();
        // A face bounded by one closed edge (a circle): its centre.
        if let [ei] = edges[..]
            && let Some(pl) = o.mesh.edges.get(ei)
            && pl.len() > 2
            && to_d(pl[0]).distance(to_d(pl[pl.len() - 1])) < 1e-9
        {
            let n = (pl.len() - 1) as f64;
            let c = pl[..pl.len() - 1].iter().map(|p| world(*p)).fold(DVec3::ZERO, |a, p| a + p) / n;
            return Some(SectionSnap { point: c, path: o.path.clone(), entity: face, dots: Vec::new() });
        }
        // The face's edges within 35 px: the nearest point on the nearest one.
        let mut best_e: Option<(f64, usize, DVec3)> = None;
        for &ei in &edges {
            let Some(pl) = o.mesh.edges.get(ei) else { continue };
            for w in pl.windows(2) {
                let (a, b) = (world(w[0]), world(w[1]));
                let (Some(sa), Some(sb)) = (vp.project(a), vp.project(b)) else { continue };
                let ab = sb - sa;
                let s = if ab.length_squared() > 1e-12 { ((position - sa).dot(ab) / ab.length_squared()).clamp(0.0, 1.0) } else { 0.0 };
                let dist = (sa + ab * s).distance(position);
                if dist <= SECTION_SNAP_EDGE_PX && best_e.is_none_or(|e| dist < e.0) {
                    best_e = Some((dist, ei, a + (b - a) * s));
                }
            }
        }
        if let Some((_, ei, p)) = best_e {
            return Some(SectionSnap { point: p, path: o.path.clone(), entity: o.edge_refs.get(ei)?.clone(), dots: Vec::new() });
        }
        Some(SectionSnap { point: ro + rd * t, path: o.path.clone(), entity: face, dots })
    }

    /// The arrow's tip and the rings' centre (screen-sized offsets from the base).
    fn section_tip(&self, cx: &App) -> Option<(DVec3, DVec3)> {
        let pl = self.active_section_plane()?;
        let w = self.wpp_at(pl.base(), cx);
        Some((pl.base() + pl.dir() * crate::assembly_scene::SECTION_TIP_PX * w, pl.base() + pl.dir() * crate::assembly_scene::SECTION_RINGS_PX * w))
    }

    /// Where the cursor ray passes closest to the line (point, dir): the
    /// parameter along the line.
    fn line_param(&self, position: glam::DVec2, point: DVec3, dir: DVec3, cx: &App) -> Option<f64> {
        let (o, r) = self.viewport.read(cx).ray(position);
        let w = point - o;
        let (a, b, c) = (dir.dot(dir), dir.dot(r), r.dot(r));
        let (dd, e) = (dir.dot(w), r.dot(w));
        let den = a * c - b * b;
        (den.abs() > 1e-12).then(|| (b * e - c * dd) / den)
    }

    /// Where the cursor ray meets the plane (point, normal).
    fn ray_plane_at(&self, position: glam::DVec2, point: DVec3, normal: DVec3, cx: &App) -> Option<DVec3> {
        let (o, r) = self.viewport.read(cx).ray(position);
        let den = r.dot(normal);
        (den.abs() > 1e-9).then(|| o + r * ((point - o).dot(normal) / den))
    }

    /// A ring's rotation axis (in the plane: ring 0 about Y, ring 1 about X).
    fn ring_axis(pl: &AsmSectionPlane, k: usize) -> DVec3 {
        if k == 0 { pl.y() } else { pl.x }
    }

    /// The plane after turning `plane0` by `angle` about a ring's axis
    /// through its manipulator base.
    fn rotated_plane(plane0: &AsmSectionPlane, k: usize, angle: f64) -> AsmSectionPlane {
        let q = glam::DQuat::from_axis_angle(Self::ring_axis(plane0, k), angle);
        let mut pl = plane0.clone();
        pl.origin = plane0.base();
        pl.offset = 0.0;
        pl.normal = (q * plane0.normal).normalize();
        pl.x = (q * plane0.x).normalize();
        pl
    }

    /// A press on a section handle starts its drag.
    pub(super) fn start_section_drag(&mut self, position: glam::DVec2, cx: &mut Context<Self>) -> bool {
        let Some(OverlayPick::Section(handle)) = self.hover_overlay else { return false };
        let Some(pl) = self.active_section_plane().cloned() else { return false };
        let (mut s0, mut v0, mut grab0) = (0.0, DVec3::ZERO, DVec3::ZERO);
        match handle {
            SectionHandle::Arrow => s0 = self.line_param(position, pl.base(), pl.dir(), cx).unwrap_or(0.0),
            SectionHandle::Ring(k) => {
                let (_, c) = self.section_tip(cx).unwrap_or((pl.base(), pl.base()));
                let axis = Self::ring_axis(&pl, k);
                v0 = self.ray_plane_at(position, c, axis, cx).and_then(|p| (p - c).try_normalize()).unwrap_or(pl.dir());
            }
            SectionHandle::Base => grab0 = self.ray_plane_at(position, pl.base(), pl.normal, cx).unwrap_or(pl.base()),
        }
        self.ui.section_value = None;
        self.ui.section_drag = Some(SectionDrag { handle, plane0: pl, s0, v0, grab0, value: 0.0, moved: false, snap: None });
        cx.notify();
        true
    }

    /// Dragging a handle: the arrow moves the plane along its normal (the
    /// value is the distance from the drag's start), a ring turns it
    /// (snapping to 5° near one), the base moves the manipulator over it.
    pub(super) fn drag_section(&mut self, position: glam::DVec2, shift: bool, cx: &mut Context<Self>) -> bool {
        let Some(d) = self.ui.section_drag.clone() else { return false };
        let pl0 = &d.plane0;
        let new_plane = match d.handle {
            SectionHandle::Arrow => {
                let Some(t) = self.line_param(position, pl0.base(), pl0.dir(), cx) else { return true };
                let v = t - d.s0;
                let mut pl = pl0.clone();
                pl.offset = pl0.offset + v * pl0.dir().dot(pl0.normal);
                Some((pl, v))
            }
            SectionHandle::Ring(k) => {
                let c = pl0.base() + pl0.dir() * crate::assembly_scene::SECTION_RINGS_PX * self.wpp_at(pl0.base(), cx);
                let axis = Self::ring_axis(pl0, k);
                let Some(p) = self.ray_plane_at(position, c, axis, cx) else { return true };
                let Some(v) = (p - c).try_normalize() else { return true };
                // Live's snap (5° / 1° / 0.1° by the pointer's distance).
                let radius_px = (p - c).length() / self.wpp_at(c, cx).max(1e-12);
                let deg = crate::viewport::angular_snap_deg(d.v0.cross(v).dot(axis).atan2(d.v0.dot(v)).to_degrees(), radius_px, shift);
                let angle = deg.to_radians();
                Some((Self::rotated_plane(pl0, k, angle), angle))
            }
            SectionHandle::Base => {
                // On geometry the base snaps (the cut stays until release);
                // off the parts it follows the pointer in the plane.
                let snap = self.section_base_snap(position, cx);
                let p = match &snap {
                    Some(s) => s.point,
                    None => match self.ray_plane_at(position, pl0.base(), pl0.normal, cx) {
                        Some(p) => p - (d.grab0 - pl0.base()),
                        None => return true,
                    },
                };
                if let Some(dd) = &mut self.ui.section_drag {
                    dd.snap = snap;
                }
                let mut pl = pl0.clone();
                let delta = p - pl0.base();
                pl.origin += delta - pl0.normal * delta.dot(pl0.normal);
                Some((pl, 0.0))
            }
        };
        if let Some((pl, v)) = new_plane {
            if let Some(slot) = self.active_section_plane_mut() {
                *slot = pl;
            }
            if let Some(dd) = &mut self.ui.section_drag {
                dd.value = v;
                dd.moved = true;
            }
            self.rebuild_scene(false, cx);
            cx.notify();
        }
        true
    }

    /// The end of a handle drag: an arrow or ring drag leaves its value box
    /// open, focused and selected (live).
    pub(super) fn end_section_drag(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let Some(d) = self.ui.section_drag.take() else { return false };
        // Live: on release the plane moves to pass through the snapped point
        // (an edge below the plane brings the cut down to it).
        if let Some(s) = d.snap.as_ref().filter(|_| d.handle == SectionHandle::Base)
            && let Some(slot) = self.active_section_plane_mut()
        {
            slot.origin = s.point - slot.normal * slot.offset;
            // The cut moves with it (the clip planes are scene data).
            self.rebuild_scene(false, cx);
        }
        if d.moved && d.handle != SectionHandle::Base {
            self.open_section_value(&d, window, cx);
        }
        self.refresh_scene(false, cx);
        cx.notify();
        true
    }

    /// A section drag's value as live shows it: lengths with 1 decimal,
    /// angles with up to 3, "deg".
    fn section_value_text(&self, handle: SectionHandle, v: f64, cx: &App) -> String {
        let units = self.units(cx);
        match handle {
            SectionHandle::Ring(_) => noshape_units::format_quantity_with(noshape_units::Quantity::radians(v), &units, 3).replace('°', " deg"),
            _ => noshape_units::format_quantity_with(noshape_units::Quantity::meters(v), &units, 1),
        }
    }

    fn open_section_value(&mut self, d: &SectionDrag, window: &mut Window, cx: &mut Context<Self>) {
        use crate::quantity_field::{CommitVia, QuantityEvent, QuantityInput, QuantitySpec};
        let text = self.section_value_text(d.handle, d.value, cx);
        let dim = if matches!(d.handle, SectionHandle::Ring(_)) { noshape_units::Dim::ANGLE } else { noshape_units::Dim::LENGTH };
        let units = self.units(cx);
        let input = cx.new(|cx| QuantityInput::new(text, QuantitySpec::quantity(dim).canvas(), units, window, cx));
        let sub = cx.subscribe_in(&input, window, |this: &mut Self, _, ev: &QuantityEvent, window, cx| match ev {
            QuantityEvent::Committed(CommitVia::Enter | CommitVia::Step) => this.apply_section_value(cx),
            // Live: Esc closes the box only (the plane stays).
            QuantityEvent::Escape => {
                this.ui.section_value = None;
                window.focus(&this.focus, cx);
                cx.notify();
            }
            _ => {}
        });
        let state = input.read(cx).state().clone();
        state.update(cx, |s, cx| {
            s.focus(window, cx);
            s.select_all(window, cx);
        });
        self.ui.section_value = Some(SectionValue { handle: d.handle, plane0: d.plane0.clone(), input, _sub: sub });
    }

    /// Enter in the value box: the plane at the typed value from where the
    /// drag started; the box stays open (live).
    fn apply_section_value(&mut self, cx: &mut Context<Self>) {
        let Some(sv) = &self.ui.section_value else { return };
        let Some(v) = sv.input.read(cx).value() else { return };
        let pl0 = sv.plane0.clone();
        let pl = match sv.handle {
            SectionHandle::Arrow => {
                let mut pl = pl0.clone();
                pl.offset = pl0.offset + v * pl0.dir().dot(pl0.normal);
                pl
            }
            SectionHandle::Ring(k) => Self::rotated_plane(&pl0, k, v),
            SectionHandle::Base => return,
        };
        if let Some(slot) = self.active_section_plane_mut() {
            *slot = pl;
        }
        self.rebuild_scene(false, cx);
        cx.notify();
    }

    /// The value box at the arrow's tip, upper right (live assembly/2370:
    /// 75 x 26, about 40 px right and 35 px above the tip): the dragged value
    /// while dragging, then the canvas field.
    pub(super) fn render_section_value(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let (tip, _) = self.section_tip(cx)?;
        let s = self.viewport.read(cx).project(tip)?;
        let at = div().absolute().left(px(s.x as f32 + 40.)).top(px(s.y as f32 - 35.)).w(px(75.)).h(px(26.));
        if let Some(d) = self.ui.section_drag.as_ref().filter(|d| d.moved && d.handle != SectionHandle::Base) {
            let text = self.section_value_text(d.handle, d.value, cx);
            let pal = p();
            return Some(at.px(px(3.)).flex().items_center().justify_end().border_1().border_color(pal.accent).bg(rgb(0xffffff)).text_size(px(13.33)).text_color(pal.text).child(text).into_any_element());
        }
        let sv = self.ui.section_value.as_ref()?;
        Some(at.id("asm-section-value").occlude().child(sv.input.clone()).into_any_element())
    }

    pub(super) fn render_section(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let s = self.ui.section.as_ref()?;
        let d = s.dialog.as_ref()?;
        let asm = self.assembly(cx);
        let items = s.items.iter().map(|id| asm.and_then(|a| a.instance(*id).map(|i| i.name.clone())).unwrap_or_default()).collect();
        let props = crate::section_dialog::Props { include: s.include, items_active: d.items_active, planes: s.planes.iter().map(|p| p.direction_name().to_string()).collect(), active: if d.chosen { d.active } else { usize::MAX }, items, popup: d.popup };
        Some(crate::section_dialog::render(props, "asm-section", &cx.entity(), Self::on_section_action, cx))
    }

    fn on_section_action(&mut self, a: crate::section_dialog::Action, w: &mut Window, cx: &mut Context<Self>) {
        use crate::section_dialog::Action;
        if matches!(a, Action::Accept | Action::Cancel) {
            return self.close_section_dialog(a == Action::Accept, w, cx);
        }
        let Some(s) = &mut self.ui.section else { return };
        match a {
            Action::Include(include) => s.include = include,
            Action::ItemsActive(items) => {
                if let Some(d) = &mut s.dialog {
                    d.items_active = items;
                }
            }
            Action::RemovePlane(i) => {
                if i < s.planes.len() {
                    s.planes.remove(i);
                }
                if let Some(d) = &mut s.dialog {
                    d.active = s.planes.len().saturating_sub(1);
                }
            }
            Action::RemoveItem(i) => {
                if i < s.items.len() {
                    s.items.remove(i);
                }
            }
            // Live ("Select default plane", as in the Part Studio): the
            // button opens the popup; a plane chosen there is added (and
            // replaced by the next choice) until ✓ commits it or ✕ takes it
            // away again.
            Action::PlaneButton => {
                if let Some(d) = &mut s.dialog {
                    d.popup = if d.popup.is_some() { None } else { Some(Default::default()) };
                    d.popup_plane = None;
                }
            }
            Action::PopupList => {
                if let Some(p) = s.dialog.as_mut().and_then(|d| d.popup.as_mut()) {
                    p.list_open = !p.list_open;
                }
            }
            Action::PopupPick(k) => {
                let Some(d) = &mut s.dialog else { return };
                let (normal, x) = [(DVec3::Z, DVec3::X), (DVec3::NEG_Y, DVec3::X), (DVec3::X, DVec3::Y)][k.min(2)];
                let pl = AsmSectionPlane::new(DVec3::ZERO, normal, x);
                let i = match d.popup_plane.filter(|i| *i < s.planes.len()) {
                    Some(i) => {
                        s.planes[i] = pl;
                        i
                    }
                    None if s.planes.len() >= MAX_SECTION_PLANES => return,
                    None => {
                        s.planes.push(pl);
                        s.planes.len() - 1
                    }
                };
                d.popup_plane = Some(i);
                d.active = i;
                d.chosen = true;
                d.items_active = false;
                if let Some(p) = &mut d.popup {
                    p.list_open = false;
                }
            }
            // Flip plane names the other side (live: "Section plane 2 (Back)").
            Action::PopupFlip => {
                let Some(d) = &s.dialog else { return };
                let i = d.popup_plane.unwrap_or(d.active);
                if let Some(pl) = s.planes.get_mut(i) {
                    pl.flipped = !pl.flipped;
                }
            }
            Action::PopupCommit => {
                if let Some(d) = &mut s.dialog {
                    d.popup = None;
                    d.popup_plane = None;
                }
            }
            Action::PopupCancel => {
                let Some(d) = &mut s.dialog else { return };
                d.popup = None;
                if let Some(i) = d.popup_plane.take().filter(|i| *i < s.planes.len()) {
                    s.planes.remove(i);
                    d.active = s.planes.len().saturating_sub(1);
                }
            }
            Action::Accept | Action::Cancel => {}
        }
        self.rebuild_scene(false, cx);
        cx.notify();
    }

    /// Isolate / Make transparent › Expand: switch distance / connectivity
    /// (back to no expansion) or add / remove steps.
    pub(super) fn set_iso_expand(&mut self, connectivity: Option<bool>, delta: i32, cx: &mut Context<Self>) {
        let transparent = self.isolate.is_none();
        let Some(current) = (if transparent { self.ui.transparent.clone() } else { self.isolate.clone() }) else { return };
        let (mut by_conn, mut steps, seeds) = self.ui.iso_expand.clone().unwrap_or_else(|| {
            let mut seeds: Vec<InstanceId> = current.iter().copied().collect();
            seeds.sort();
            // Live default: "Expand: connectivity".
            (true, 0, seeds)
        });
        if let Some(c) = connectivity {
            by_conn = c;
            steps = 0;
        }
        let Some(asm) = self.assembly(cx) else { return };
        let order = analysis::isolate_expansion(&self.result, asm, &seeds, by_conn);
        steps = (steps as i32 + delta).clamp(0, order.len() as i32) as usize;
        let mut set: HashSet<InstanceId> = seeds.iter().copied().collect();
        for s in order.iter().take(steps) {
            set.extend(s.iter().copied());
        }
        if transparent {
            self.ui.transparent = Some(set);
        } else {
            self.isolate = Some(set);
        }
        self.ui.iso_expand = Some((by_conn, steps, seeds));
        self.rebuild_scene(true, cx);
        cx.notify();
    }

}

// ---------------------------------------------------------------- export

impl AssemblyView {
    /// Export… (instance menu: those instances; top-level row: the whole
    /// assembly): Onshape's Export modal with the assembly format list
    /// (part_studio/export.rs); parts are written at their assembly positions.
    pub(super) fn open_asm_export(&mut self, ids: Option<Vec<InstanceId>>, window: &mut Window, cx: &mut Context<Self>) {
        let view = cx.entity();
        let base = match ids.as_deref() {
            Some([one]) => self.assembly(cx).and_then(|a| a.instance(*one).map(|i| i.name.clone())).unwrap_or_default(),
            _ => self.doc.read(cx).session.doc.tab(self.tab).map(|t| t.name.clone()).unwrap_or_else(|| "Assembly".into()),
        };
        // Through the user's export rules (Preferences › User export rules).
        let tab_name = self.doc.read(cx).session.doc.tab(self.tab).map(|t| t.name.clone()).unwrap_or_default();
        let values = crate::export_naming::values(&self.doc.read(cx).name(), &tab_name, &base, None);
        let base = crate::export_naming::default_name(noshape_model::export_rules::ExportElement::Assembly, noshape_ops::export::ExportSettings::default().format.label(), &values, base);
        let run: crate::assembly_hooks::RunExport = std::rc::Rc::new(move |s, name, window, cx| {
            let (v, ids) = (view.clone(), ids.clone());
            if let Some(msg) = noshape_ops::export::missing_writer(s.format) {
                v.update(cx, |this, cx| {
                    this.message = Some((true, msg));
                    cx.notify();
                });
                return;
            }
            // The saved file's own extension (OBJ downloads a .zip, as live).
            let ext = s.file_ext(true);
            let file = format!("{name}.{ext}");
            // Options › Store file in a new tab: no download.
            if s.options == 2 {
                v.update(cx, |this, cx| {
                    let asm = this.export_assembly(ids.as_deref(), s.hidden, cx);
                    this.message = Some(match crate::assembly_hooks::store_export(&this.doc, this.tab, &file, |p| noshape_ops::export::write_assembly_export(&s, p, &asm), cx) {
                        Ok(f) => (false, format!("Stored {f}")),
                        Err(e) => (true, format!("Export failed: {e:#}")),
                    });
                    cx.notify();
                });
                return;
            }
            crate::file_dialogs::request_save_path("Export", file, ext, window, cx, move |path, cx| {
                v.update(cx, |this, cx| {
                    let asm = this.export_assembly(ids.as_deref(), s.hidden, cx);
                    this.message = Some(match noshape_ops::export::write_assembly_export(&s, &path, &asm) {
                        Ok(p) => {
                            // Download and store file in a new tab.
                            if s.options == 1 {
                                crate::assembly_hooks::store_written(&this.doc, this.tab, &p, cx);
                            }
                            (false, format!("Exported {}", p.display()))
                        }
                        Err(e) => (true, format!("Export failed: {e:#}")),
                    });
                    cx.notify();
                });
            });
        });
        crate::assembly_hooks::open_export_modal(base, true, run, window, cx);
    }

    /// Tab › Export… of an Assembly tab: the whole assembly.
    pub fn open_export_all(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open_asm_export(None, window, cx);
    }

    /// What to export: instances `ids` (None: all), hidden ones only with
    /// "Include hidden instances in export"; STEP keeps the assembly
    /// (`noshape_ops::export::write_assembly_export`).
    fn export_assembly(&self, ids: Option<&[InstanceId]>, include_hidden: bool, cx: &App) -> noshape_ops::export::AssemblyExport {
        let root = self.doc.read(cx).session.doc.tab(self.tab).map(|t| t.name.clone()).unwrap_or_else(|| "Assembly".into());
        noshape_ops::export::AssemblyExport::of(&root, &self.result, ids, include_hidden)
    }
}

// ---------------------------------------------------------------- center of mass

impl AssemblyView {
    /// Show / Hide center of mass (instance menus: those instances; top-level
    /// row: the whole assembly). Parts are weighted by their density
    /// (volume when a part has no material).
    pub(super) fn toggle_center_of_mass(&mut self, ids: Vec<InstanceId>, cx: &mut Context<Self>) {
        if self.ui.com.as_ref().is_some_and(|(cur, _)| *cur == ids) {
            self.ui.com = None;
            cx.notify();
            return;
        }
        match self.center_of_mass(&ids, cx) {
            Some(p) => self.ui.com = Some((ids, p)),
            None => self.message = Some((false, "No solid parts to compute a center of mass for.".into())),
        }
        cx.notify();
    }

    /// Keep the shown center of mass at the solved positions.
    pub(super) fn refresh_center_of_mass(&mut self, cx: &App) {
        if let Some((ids, _)) = self.ui.com.clone() {
            self.ui.com = self.center_of_mass(&ids, cx).map(|p| (ids, p));
        }
    }

    fn center_of_mass(&self, ids: &[InstanceId], cx: &App) -> Option<DVec3> {
        let doc = &self.doc.read(cx).session.doc;
        let mut mass = 0.0;
        let mut moment = DVec3::ZERO;
        for o in self.result.occurrences.iter().filter(|o| !o.hidden && (ids.is_empty() || ids.contains(&o.top()))) {
            let density = |b: &BodyId| match doc.tab(o.studio).map(|t| &t.content) {
                Some(TabContent::PartStudio(ps)) => ps.part_properties.get(b).and_then(|p| p.density),
                _ => None,
            };
            let Some(mp) = noshape_model::regen::measure::mass_properties(&o.context, std::slice::from_ref(&o.body), &density) else { continue };
            let m = mp.mass.unwrap_or(mp.volume);
            mass += m;
            moment += o.transform.transform_point3(mp.center_of_mass) * m;
        }
        (mass > 0.0).then(|| moment / mass)
    }

    /// The center of mass indicator (Onshape's quartered circle).
    pub(super) fn render_center_of_mass(&self, cx: &App) -> Option<AnyElement> {
        let (_, at) = self.ui.com.as_ref()?;
        let vp = self.viewport.read(cx);
        let p = vp.project(*at)?;
        Some(div().absolute().left(px(p.x as f32 - 9.)).top(px(p.y as f32 - 9.)).child(crate::assets::icon(&icon_path("com-indicator"), 18., cx)).into_any_element())
    }
}

/// A value with up to three decimals and no trailing zeros ("3.6", "12").
fn trim_number(x: f64) -> String {
    let t = noshape_units::to_fixed(x, 3);
    let t = t.trim_end_matches('0').trim_end_matches('.');
    if t == "-0" { "0".to_string() } else { t.to_string() }
}

#[cfg(test)]
mod value_tests {
    use super::{onshape_mate_values, solver_values_for};
    use glam::{DQuat, DVec3};
    use noshape_model::assembly::solver::Frame;
    use std::prelude::v1::test;

    /// The second connector posed from our solver values relative to the
    /// first (rotation about Z, then the translation in the first's frame).
    fn second_from(a: &Frame, ours: [f64; 4]) -> Frame {
        Frame { origin: a.origin + a.rotation * DVec3::new(ours[1], ours[2], ours[3]), rotation: a.rotation * DQuat::from_rotation_z(ours[0]) }
    }

    #[test]
    fn typed_mate_values_round_trip_through_the_solver_convention() {
        let a = Frame { origin: DVec3::new(0.01, -0.02, 0.03), rotation: DQuat::from_rotation_x(0.4) * DQuat::from_rotation_y(-0.7) };
        let ours = [0.9, 0.004, -0.007, 0.012];
        let b = second_from(&a, ours);
        let on = onshape_mate_values(&a, &b);
        // Onshape measures the first connector in the second's frame.
        assert!((on[0] + ours[0]).abs() < 1e-12, "rotation reads the other way");
        for (ix, v) in [(0, 2.0_f64), (1, 0.02), (2, -0.015), (3, 0.05)] {
            let target = solver_values_for(&a, &b, ours, ix, v);
            if ix == 0 {
                assert_eq!(target[0], v, "the rotation shown is the solver's");
                continue;
            }
            let got = onshape_mate_values(&a, &second_from(&a, target));
            let want_rot = on[0];
            assert!((got[0] - want_rot).rem_euclid(std::f64::consts::TAU).min((want_rot - got[0]).rem_euclid(std::f64::consts::TAU)) < 1e-9, "ix {ix}: rotation {} vs {}", got[0], want_rot);
            if ix != 0 {
                for k in 1..4 {
                    let want = if k == ix { v } else { on[k] };
                    assert!((got[k] - want).abs() < 1e-9, "ix {ix}: value {k} {} vs {want}", got[k]);
                }
            }
        }
    }
}

/// The "Origin of new Part Studio" field. Live: opened from the toolbar it
/// starts empty (✓ then just closes, qa4 round 5 s2); from the Origin row's
/// menu it is prefilled with the Origin (assembly/542).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PsOrigin {
    Unset,
    Origin,
    Connector(MateId),
}

impl PsOrigin {
    /// The chip text (None: the empty field).
    pub fn chip(self, connector_name: impl FnOnce(MateId) -> Option<String>) -> Option<String> {
        match self {
            PsOrigin::Unset => None,
            PsOrigin::Origin => Some("Origin".into()),
            PsOrigin::Connector(id) => Some(connector_name(id).unwrap_or_else(|| "Origin".into())),
        }
    }
}

#[cfg(test)]
mod ps_origin_tests {
    use super::PsOrigin;
    use noshape_model::assembly::MateId;

    #[::core::prelude::v1::test]
    fn the_toolbar_field_starts_empty() {
        assert_eq!(PsOrigin::Unset.chip(|_| None), None);
        assert_eq!(PsOrigin::Origin.chip(|_| None).as_deref(), Some("Origin"));
        assert_eq!(PsOrigin::Connector(MateId(3)).chip(|_| Some("Mate connector 1".into())).as_deref(), Some("Mate connector 1"));
    }
}

/// The filter box's help (live, behaviours "qa (2026-10-27, P2) assembly
/// instance list: Filter by name finds pattern instances", request 1519).
fn filter_help() -> AnyElement {
    const ROWS: [(&str, &str); 10] = [
        (":part", "Filter by part contents"),
        (":assembly", "Filter by subassemblies"),
        (":mate", "Filter by mates and relations"),
        (":matelimits", "Filter by mates with limits"),
        (":folder", "Filter by assembly folders"),
        (":errors", "Filter by contents with errors"),
        (":warnings", "Filter by contents with warnings"),
        (":suppressed", "Filter by suppressed contents"),
        (":hidden", "Filter by hidden contents"),
        (":shown", "Filter by shown contents"),
    ];
    let pal = p();
    v_flex()
        .w(px(330.))
        .p(px(10.))
        .gap(px(4.))
        .bg(gpui_kit::rgb(0xffffff))
        .border_1()
        .border_color(gpui_kit::rgb(0xd7d7d7))
        .rounded(px(3.))
        .shadow_md()
        .text_size(px(12.))
        .text_color(pal.text)
        .child("Search with filters or a partial name.")
        .children(ROWS.iter().map(|(token, what)| {
            h_flex().child(div().font_weight(FontWeight::BOLD).child(*token)).child(div().pl(px(4.)).italic().child("name")).child(div().child(format!(":\u{a0}{what}")))
        }))
        .child(div().pt(px(4.)).child("To perform an exact search, use quotation marks, such as :part \"Part 1 <1>\". Filters may be combined. For example, :part :hidden will search for only parts which are hidden."))
        .into_any_element()
}
