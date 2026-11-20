//! Drawing editor input: mouse/keyboard handling, selection and dragging,
//! and the view / dimension / annotation tools (with live previews).

use crate::drawing_scene::Overlay;
use crate::navigation as nav;
use crate::drawing_view::*;
use gpui_kit::*;
use noshape_model::drawing::build::PickTarget;
use noshape_model::drawing::views;
use noshape_model::drawing::*;

pub use noshape_scene::drawing_tools::geom::*;
use noshape_scene::drawing_tools as dt;

/// How near (px) a placed table's fixed corner snaps to the border frame's corner (live2 place-1118).
pub(crate) const TABLE_CORNER_REACH_PX: f64 = 10.0;

impl DrawingEditor {
    // ------------------------------------------------------------ tools

    pub fn set_tool(&mut self, tool: Tool, cx: &mut Context<Self>) {
        if !matches!(tool, Tool::Select) {
            let again = tool.clone();
            if self.wait_for_sheet(Box::new(move |this, cx| this.set_tool(again, cx)), cx) {
                return;
            }
        }
        self.projection_parent_lit = false;
        self.tool_points.clear();
        let sheet = self.drawing_sheet();
        self.tool_start = (sheet.sketch.sketch.entities().map(|e| e.id.0).max().unwrap_or(0), sheet.annotations.iter().map(|a| a.id).max().unwrap_or(0));
        // A constraint key on a selection that completes it: applied at once.
        if let Tool::Constrain { kind, picks } = &tool
            && picks.is_empty()
            && self.constrain_preselection(*kind, cx)
        {
            return;
        }
        // Live (request 1566, qa DWG-PANEL-ESC-STACK): starting a tool closes
        // View properties first, its pending edits discarded.
        if !matches!(tool, Tool::Select) {
            self.view_props = None;
        }
        self.woken_points.clear();
        self.woken_plus.clear();
        self.datum_on = None;
        self.datum_at = None;
        self.datum_edge = None;
        self.insert_continuation = false;
        self.sketch_dim_pick = None;
        // Choosing a tool keeps the palette's changes (like a click away).
        if self.palette.is_some() {
            self.close_palette(true, cx);
        }
        self.datum_target_letter = None;
        // Clicking the active tool's button again exits it.
        // Dimension tools exit only when the same kind is chosen again.
        let same_dim = matches!((&tool, &self.tool), (Tool::Dimension { kind: a, .. }, Tool::Dimension { kind: b, .. }) if a == b)
            || matches!((&tool, &self.tool), (Tool::Sketch { tool: a, .. }, Tool::Sketch { tool: b, .. }) if a == b)
            || matches!((&tool, &self.tool), (Tool::Constrain { kind: a, .. }, Tool::Constrain { kind: b, .. }) if a == b);
        if std::mem::discriminant(&tool) == std::mem::discriminant(&self.tool) && (same_dim || !matches!(tool, Tool::Select | Tool::InsertView { .. } | Tool::Dimension { .. } | Tool::Sketch { .. } | Tool::Constrain { .. })) {
            self.tool = Tool::Select;
        } else {
            // A new Insert view starts from the drawing's View simplification default.
            if matches!(tool, Tool::InsertView { .. }) && !matches!(self.tool, Tool::InsertView { .. }) {
                self.insert.simplification = self.drawing.props.view_defaults.simplification;
                self.insert.simp_value = None;
            }
            self.tool = tool;
        }
        // Tools that start from the selected view.
        let sel_view = self.selection.iter().find_map(|s| s.as_view());
        match &mut self.tool {
            Tool::Projected { parent }
            | Tool::Section { parent, .. }
            | Tool::Detail { parent, .. }
            | Tool::JoggedSection { parent, .. }
            | Tool::AlignedSection { parent, .. }
            | Tool::Crop { parent, .. }
            | Tool::Break { parent, .. }
            | Tool::BrokenOut { parent, .. }
            | Tool::CropPoly { parent, .. }
                if parent.is_none() =>
            {
                *parent = sel_view
            }
            Tool::Table { kind: TableTool::Bom | TableTool::Hole | TableTool::CutList | TableTool::Custom, view, .. } if view.is_none() => *view = sel_view,
            _ => {}
        }
        self.overlay = Overlay::default();
        self.note_leader = None;
        self.hover_snap = None;
        self.status = None;
        self.snap_mid_quad = false;
        self.ensure_sources(false, cx);
        self.update_preview(cx);
        cx.notify();
    }

    pub fn cancel(&mut self, cx: &mut Context<Self>) {
        // A BOM table just placed stays selected through Esc (live2 bomsel-1118).
        if self.placed_table.is_some_and(|id| self.selection == [Sel::Table(id)]) {
            return;
        }
        if matches!(self.drag, Drag::MoveViews { .. }) {
            self.view_drag_cancelled = true;
            return;
        }
        // Esc while a box is dragged does nothing: the release still selects (live drawing-feel-1118 F-esc).
        if matches!(self.drag, Drag::SelectBox { .. }) {
            return;
        }
        self.projection_parent_lit = false;
        self.aligned = None;
        // (Nor is the view under the pointer lit any more after a menu: live2 drw-centermark-align s8.)
        self.menu_closed_at = None;
        // A hole table following the pointer after ✓: Esc leaves it there (live 2131).
        if self.hole_table_placing() {
            self.place_hole_table(cx);
            return;
        }
        // Edit detail (not redrawing its profile, or picking a closed profile after its × : live2
        // drw-detail-id-profile-tips s19): Esc discards it.
        if self.detail_dlg.as_ref().is_some_and(|d| d.edit.is_some() && (d.pick_region || !matches!(self.tool, Tool::Detail { .. }))) {
            self.close_detail_edit(false, cx);
            return;
        }
        // Edit aligned section: Esc closes it unapplied, nothing left selected (live drw-aligned-section-edit).
        if self.aligned_dlg.as_ref().is_some_and(|d| d.edit.is_some()) {
            self.aligned_dlg = None;
            self.selection.clear();
            self.set_preview(None, cx);
            self.redraw(cx);
            return;
        }
        // Rename sheet / Rotation angle: Esc closes them unapplied (live).
        if self.row_rename.take().is_some() {
            cx.notify();
            return;
        }
        // The Reference manager (live doc-tab-menu-change-to-version: Escape closes it).
        if self.ref_manager.take().is_some() {
            cx.notify();
            return;
        }
        if matches!(self.panel, Some(Panel::SheetName { .. } | Panel::Rotation { .. })) {
            self.panel = None;
            cx.notify();
            return;
        }
        // Add revision callout: Esc ends it, the table kept selected (live2 drw-revision-callout-add).
        if matches!(self.tool, Tool::RevisionCallout { .. }) {
            self.tool = Tool::Select;
            self.status = None;
            cx.notify();
            return;
        }
        // Picked cells: Esc closes their Cell panel and drops them (live2 drw-revision-cell-format); a panel over it
        // (Resize…) goes first (drw-revision-cell-resize).
        if self.panel.is_none() && self.cell_sel.take().is_some() {
            self.selection.clear();
            cx.notify();
            return;
        }
        // Revision table properties…: Esc closes it, as a cancel, and drops the selection (live
        // drw-revision-table-properties).
        if self.revision_dlg.as_ref().is_some_and(|d| d.table.is_some()) {
            self.close_revision_dlg(false, cx);
            self.selection.clear();
            return;
        }
        // Align centermark: Esc ends it with nothing left selected (live2 drw-centermark-align s8).
        if self.align_centermark.take().is_some() || self.align_auto_centermark.take().is_some() {
            self.selection.clear();
            self.status = None;
            self.redraw(cx);
            return;
        }
        // Hatch region with a profile tool on (Corner rectangle …): the first Esc ends the tool, the next closes the
        // panel (live2 drw-hatch-panel-tips s14, behaviours "studios (11-17, low)").
        if self.hatch_dlg.is_some() && matches!(self.tool, Tool::Sketch { .. }) {
            self.tool = Tool::Select;
            self.status = None;
            self.redraw(cx);
            return;
        }
        if self.zoom_window.take().is_some() || self.rotate_to_edge.take().is_some() || self.align_views.take().is_some() || self.sheet_props.take().is_some() || self.shortcut_toolbar.take().is_some() || self.measure.take().is_some() || self.painter.take().is_some() || self.cancel_inspection(cx) || self.cancel_hatch(cx) {
            self.status = None;
            cx.notify();
            return;
        }
        // View properties: one Esc closes it as a cancel (live request 1566), the view no longer selected (live
        // drw-view-properties).
        if self.view_props.take().is_some() {
            self.selection.clear();
            self.redraw(cx);
            return;
        }
        // A hole table's properties: Esc closes them unchanged (live drw-hole-table-properties).
        if self.hole_dlg.as_ref().is_some_and(|d| d.table.is_some()) {
            self.close_hole_table_props(false, cx);
            return;
        }
        // The Table dialog: closed without placing.
        if self.table_dlg.is_some() {
            self.close_table_dlg(false, cx);
            return;
        }
        if self.insert.chooser.take().is_some() | std::mem::take(&mut self.insert.panel) {
            cx.notify();
            return;
        }
        // The Surface finish tool: Esc ends it, the symbols placed stay (live drw-surface-finish).
        if matches!(self.tool, Tool::SurfaceFinish) && matches!(self.panel, Some(Panel::SurfaceFinish { edit: None, .. })) {
            self.panel = None;
            self.note_leader = None;
            self.tool = Tool::Select;
            cx.notify();
            return;
        }
        // The note editor: Esc closes it and ends the Note tool (live2 drw-note-editor).
        if matches!(self.tool, Tool::Note) && matches!(self.panel, Some(Panel::Note { .. })) {
            self.panel = None;
            self.note_leader = None;
            self.tool = Tool::Select;
            cx.notify();
            return;
        }
        if self.panel.is_some() {
            self.panel = None;
            cx.notify();
            return;
        }
        if self.weld_dlg.is_some() {
            self.close_weld_dlg(false, cx);
            return;
        }
        if self.edge_condition_dlg.is_some() {
            self.close_edge_condition_dlg(false, cx);
            return;
        }
        if self.gdt_dlg.is_some() {
            self.close_gdt_dlg(false, cx);
            return;
        }
        if self.close_centermark_on_escape(cx) {
            return;
        }
        if self.custom_tables.dlg.as_ref().is_some_and(|d| d.picker.is_some()) {
            if let Some(d) = &mut self.custom_tables.dlg {
                d.picker = None;
            }
            cx.notify();
            return;
        }
        if self.linestyle.is_some() {
            self.close_linestyle(false, cx);
            return;
        }
        if self.hide_edges.is_some() {
            self.close_hide_edges(false, cx);
            return;
        }
        // Up to entity's field picking: Esc leaves the field, the panel stays (live2 drw-section-edit-depth-direction).
        if let Some(e) = self.section_edit.as_mut().filter(|e| e.picking) {
            e.picking = false;
            cx.notify();
            return;
        }
        if self.section_edit.is_some() {
            self.close_section_edit(false, cx);
            return;
        }
        // Edit auxiliary view: closed, nothing left selected (live drw-aux-edit).
        if self.aux_edit.take().is_some() {
            self.selection.clear();
            cx.notify();
            return;
        }
        // Region…'s first Esc puts its rectangle tool down (the field then asks for a closed profile); the next closes it.
        if let Some(crate::drawing_replicate::ReplicateDlg::Create { center, rect: None, select: select @ false, .. }) = &mut self.replicate_dlg {
            (*select, *center) = (true, None);
            cx.notify();
            return;
        }
        if self.replicate_dlg.is_some() {
            self.close_replicate(false, cx);
            return;
        }
        if self.palette.is_some() {
            self.close_palette(false, cx);
            return;
        }
        if self.datum_dlg.as_ref().is_some_and(|d| !d.rows.is_empty()) {
            self.close_datum_dlg(false, cx);
            return;
        }
        // Live: Esc closes Move to sheet and clears the selection.
        if self.move_sheet.take().is_some() {
            self.selection.clear();
            self.redraw(cx);
            return;
        }
        if self.table_props.is_some() {
            self.close_table_props(false, cx);
            return;
        }
        if self.table_dlg.as_ref().is_some_and(|d| d.table.is_some()) {
            self.close_table_dlg(false, cx);
            return;
        }
        if self.custom_tables.dlg.as_ref().is_some_and(|d| d.table.is_some()) {
            self.custom_tables.dlg = None;
            cx.notify();
            return;
        }
        if self.stack_dlg.take().is_some() {
            self.selection.clear();
            cx.notify();
            return;
        }
        if self.callout_dlg.as_ref().is_some_and(|d| d.edit.is_some()) {
            self.callout_dlg = None;
            cx.notify();
            return;
        }
        if self.detail_escape(cx) {
            return;
        }
        // Esc ends a spline being drawn (keeping it) and stays in the Spline tool; the next Esc ends the tool (live2
        // drw-sketch-spline).
        if self.finish_sketch(cx) {
            self.redraw(cx);
            return;
        }
        if !matches!(self.tool, Tool::Select) {
            let made = self.made_by_tool();
            self.tool = Tool::Select;
            self.overlay = Overlay::default();
            self.note_leader = None;
            self.hover_snap = None;
            // Onshape: Esc ends the tool and nothing stays selected (the
            // base view of a projected-view chain is no longer orange), but what a sketch tool or the maximum /
            // minimum dimension made.
            self.selection = made;
            self.set_preview(None, cx);
            self.redraw(cx);
            return;
        }
        // A selected hole table stays selected (live2 drw-hole-table-rmb s18, -split s16: its frame still read lit).
        let sheet = self.current_sheet();
        let hole = |s: &Sel| matches!(s, Sel::Table(id) if sheet.table(*id).is_some_and(|t| matches!(t.kind, TableKind::Hole { .. })));
        if !self.selection.is_empty() && self.selection.iter().all(hole) {
            return;
        }
        self.selection.clear();
        self.redraw(cx);
    }

    /// What the tool made that Esc leaves selected (live 2442: a sketch point turns orange at Esc; a center point
    /// circle drawn (drw-crop-existing-profile), a maximum / minimum dimension (drw-max-min-dimension), a line to line
    /// angular one (live2 selection-after-esc on drw-dimension-angular: its handles orange with the pointer away), a
    /// 3 point angular one and a radius (live2's 11-15 retakes of drw-dimension-3-point-angular, -foreshorten) placed
    /// too; not the other dimensions (drw-dimension-diameter, -line-to-line, -point-to-point read none).
    pub(crate) fn made_by_tool(&self) -> Vec<Sel> {
        let sheet = self.drawing_sheet();
        let (entities, annotations) = self.tool_start;
        match self.tool {
            Tool::Sketch { tool: dt::SkTool::SplinePoint | dt::SkTool::Trim, .. } => Vec::new(),
            Tool::Sketch { .. } => sheet.sketch.sketch.entities().map(|e| e.id.0).filter(|&id| id > entities).map(Sel::Sketch).collect(),
            // (And a hole callout: live2 drw-hcallout-override s7, its text read lit in Top after Esc.)
            // (And a note committed with ✓: live2 drw-tools-1119 rad-note, its grips after Esc.)
            Tool::Dimension { kind: DimTool::MaxMin | DimTool::Angle | DimTool::Angle3 | DimTool::Radius, .. } | Tool::HoleCallout { .. } => sheet.annotations.iter().filter(|a| a.id > annotations).map(|a| Sel::Annotation(a.id)).collect(),
            Tool::Note => sheet.annotations.iter().filter(|a| a.id > annotations && matches!(a.kind, AnnotationKind::Note { .. })).map(|a| Sel::Annotation(a.id)).collect(),
            _ => Vec::new(),
        }
    }

    pub fn delete_selection(&mut self, cx: &mut Context<Self>) {
        // A selected constraint glyph (help SK15: Delete removes it).
        if let Some((cid, _)) = self.glyph_selected.take() {
            self.glyph_hover = None;
            self.edit_sketch("Delete constraint", cx, |s| {
                let _ = s.sketch.remove_constraint(noshape_sketch::ConstraintId(cid));
            });
            return;
        }
        if self.selection.is_empty() {
            return;
        }
        let sel = std::mem::take(&mut self.selection);
        self.edit_sheet("Delete", cx, |s| dt::delete_selection(s, &sel));
    }

    /// Prompt shown in the status bar for the active tool.
    pub fn prompt(&self) -> Option<&'static str> {
        if let Some(p) = self.more_prompt() {
            return Some(p);
        }
        Some(match &self.tool {
            Tool::Select => return None,
            Tool::Sketch { tool, pts, .. } => tool.prompt(pts.len()),
            Tool::Constrain { kind: crate::drawing_sketch::SkCon::Horizontal | crate::drawing_sketch::SkCon::Vertical, .. } => "Select a line, or two points",
            Tool::Constrain { picks, .. } if picks.is_empty() => "Select the first entity (sketch geometry or a view edge)",
            Tool::Constrain { .. } => "Select the second entity",
            Tool::PlaceImage { .. } => "Click and drag to place the image",
            Tool::InsertView { .. } => "Click to place the view",
            Tool::Projected { parent: None } => "Select the view to project from",
            Tool::Projected { .. } => "Move away from the parent view and click to place (Esc to finish)",
            Tool::Auxiliary { parent: None } => "Select a straight edge to fold about",
            Tool::Auxiliary { .. } => "Click to place the auxiliary view",
            Tool::Section { p0: None, partial: false, .. } if self.section_axis().is_some() => "Click in a view to place the cutting line",
            Tool::Section { parent: None, .. } | Tool::Section { p0: None, .. } => "Click the first point of the cutting line in a view",
            Tool::Section { line: None, .. } => "Click the second point of the cutting line",
            Tool::Section { .. } => "Drag to the side to view from and click to place",
            Tool::Detail { center: None, .. } => match self.detail_dlg.as_ref().map(|d| (d.pick_region, d.shape, d.pts.len())) {
                Some((true, ..)) => "Click a closed sketch region for the detail profile",
                Some((_, 1, 0)) => "Click the first corner of the rectangle (Esc: pick a closed region)",
                Some((_, 1, _)) => "Click the opposite corner",
                Some((_, 2 | 3, 0)) => "Click the first profile point (Esc: pick a closed region)",
                Some((_, 2 | 3, _)) => "Click the next point; click the first point or double-click to close",
                _ => "Click the center of the detail circle (Esc: pick a closed region)",
            },
            Tool::Detail { radius: None, .. } => "Click to set the detail radius",
            Tool::Detail { .. } => "Click to place the detail view",
            Tool::HoleCallout { circle: None } => "Select a circle of a hole or thread",
            Tool::HoleCallout { .. } => "Drag to place the callout and click",
            Tool::EdgeCenterline { first: None } => "Select two lines or two concentric arcs",
            Tool::EdgeCenterline { .. } => "Select the second edge",
            Tool::CircleCenterline { three: true, points } if points.len() < 3 => "Select three points on the circle (hole centers, end, mid or quad points)",
            Tool::CircleCenterline { points, .. } if points.is_empty() => "Select the center of the circle",
            Tool::CircleCenterline { .. } => "Select a point on the circle",
            Tool::VirtualSharp { first: None } => "Select the first straight edge",
            Tool::VirtualSharp { .. } => "Select the second straight edge",
            Tool::Dimension { kind: DimTool::Point2, picks } if picks.len() < 2 => "Select two points",
            Tool::Dimension { kind: DimTool::PointLine, picks } if picks.len() < 2 => "Select a point and a line",
            Tool::Dimension { kind: DimTool::LineLine, picks } if picks.len() < 2 => "Select two parallel lines",
            Tool::Dimension { kind: DimTool::Chamfer, picks } if picks.is_empty() => "Select the chamfer edge",
            Tool::Dimension { kind: DimTool::Chamfer, picks } if picks.len() == 1 => "Select the adjacent edge",
            Tool::Dimension { kind: DimTool::ArcLength, picks } if picks.is_empty() => "Select an arc",
            Tool::Dimension { kind: DimTool::Radius | DimTool::Diameter, picks } if picks.is_empty() => "Select a circle or arc",
            Tool::Dimension { kind: DimTool::Angle, picks } if picks.len() < 2 => "Select two straight edges",
            Tool::Dimension { kind: DimTool::Angle3, picks } if picks.is_empty() => "Select the vertex of the angle",
            Tool::Dimension { kind: DimTool::Angle3, picks } if picks.len() < 3 => "Select a point on each side of the angle",
            Tool::Dimension { picks, .. } if picks.is_empty() => "Select a circle, arc, line, circle center or point",
            Tool::Dimension { picks, .. } if picks.len() == 1 && matches!(picks[0], Pick::Point(_)) => "Select a second entity",
            Tool::Dimension { kind: DimTool::Linear, picks } if picks.len() == 1 => "Click to place the dimension, or select a second entity",
            Tool::Dimension { .. } => "Click to place the dimension",
            Tool::Note if self.note_leader.is_some() => "Click to place the note text",
            Tool::Note => "Click a point for the leader, or click to place the note",
            Tool::Centermark => "Select a circle or arc",
            Tool::Centerline { first: None } => "Select the first point",
            Tool::Centerline { .. } => "Select the second point",
            Tool::Balloon { anchor: None } => "Select the point the callout points to (in an assembly view: a part, for its BOM item)",
            Tool::Balloon { .. } => "Click to place the callout",
            Tool::JoggedSection { parent: None, .. } | Tool::JoggedSection { done: false, .. } => "Click the points of the cutting line (orthogonal jogs); Enter or double-click to finish",
            Tool::JoggedSection { .. } => "Drag to the side to view from and click to place",
            Tool::AlignedSection { center: None, .. } => "Click the center of the cutting line (e.g. a hole center)",
            Tool::AlignedSection { a: None, .. } => "Click a point on the leg the view looks at",
            Tool::AlignedSection { b: None, .. } => "Click a point on the leg revolved into the view plane",
            Tool::AlignedSection { .. } => "Drag to the side to view from and click to place",
            Tool::Crop { first: None, circle: false, .. } => "Click the first corner of the crop rectangle in a view",
            Tool::Crop { first: None, circle: true, .. } => "Click the center of the crop circle in a view",
            Tool::Crop { circle: false, .. } => "Click the opposite corner",
            Tool::Crop { .. } => "Click to set the crop radius",
            Tool::Break { first: None, .. } => "Click the first break line in a view",
            Tool::Break { .. } => "Click the second break line",
            Tool::Ordinate { origin: None, .. } => "Select the datum (zero) point",
            Tool::Ordinate { horizontal: None, .. } => "Move up/down (x values) or sideways (y values) and click to place the zero",
            Tool::Ordinate { .. } => "Select points to dimension (Esc to finish)",
            Tool::Table { kind: TableTool::General | TableTool::Revision, .. } => "Click to place the table (top-right corner)",
            Tool::Table { kind: TableTool::Bom | TableTool::CutList | TableTool::Custom, .. } => "Click to place the table",
            Tool::Table { view: None, .. } => "Select the view the table is for",
            Tool::Table { kind: TableTool::Hole, origin: None, .. } => "Select the origin (a vertex or circle center)",
            Tool::Table { .. } => "Click to place the table (top-right corner)",
            Tool::Gdt if self.note_leader.is_some() => "Click to place the geometric tolerance",
            Tool::Gdt => "Click a dimension, an edge, a point or a face for the leader, or click to place the geometric tolerance",
            Tool::Datum { anchor: None } => "Click the edge or point the datum is attached to",
            Tool::Datum { .. } => "Click to place the datum box",
            Tool::SurfaceFinish if self.note_leader.is_some() => "Click to place the surface finish symbol",
            Tool::SurfaceFinish => "Click an edge or an extension line the symbol sits on, or a face for a leader",
            Tool::Weld if self.weld_dlg.as_ref().is_some_and(|d| d.target.is_some()) => "Click to place the symbol",
            Tool::Weld => "Select an entity to associate the symbol",
            Tool::EdgeCondition if self.edge_condition_dlg.as_ref().is_some_and(|d| d.target.is_some()) => "Click to place the symbol",
            Tool::EdgeCondition => "Select an edge",
            Tool::RevisionCallout { .. } => "Click to place the revision callout",
            _ => return None,
        })
    }

    // ------------------------------------------------------------ picking

    /// The entity under the cursor: the picked geometry/annotation, else the
    /// view whose rectangle contains the point.
    pub fn sel_at(&self, pick: Option<u32>, s: [f64; 2]) -> Option<Sel> {
        self.pk().sel_at(pick, s)
    }

    /// What a Select click or right-click at `s` selects: a view's edge selects the view (live: a right-click on a
    /// detail view at the part's corner, on its edges, reads VIEW Detail, drw-detail-id-profile-tips s6; no take shows
    /// one edge selected by a click).
    fn select_target(&self, pick: Option<u32>, s: [f64; 2]) -> Option<Sel> {
        match self.sel_at(pick, s) {
            Some(Sel::Edge(v, _)) => Some(Sel::View(v)),
            sel => sel,
        }
    }

    /// Snap point (vertex / circle center) of a view near a sheet point.
    pub fn snap_at(&self, s: [f64; 2]) -> Option<PickedPoint> {
        self.pk().snap_at(s)
    }

    /// The hovered projected edge: (view, model edge).
    /// A snap point of view `pid` under `c` off the current cutting line: a
    /// jog for the Section view tool.
    fn section_jog_at(&self, pid: ViewId, c: [f64; 2]) -> Option<[f64; 2]> {
        self.pk().section_jog_at(pid, c)
    }

    pub fn hovered_edge(&self) -> Option<(ViewId, views::ModelEdge)> {
        self.pk().hovered_edge()
    }

    pub fn line_pick(&self) -> Option<Pick> {
        self.pk().line_pick()
    }

    pub fn circle_pick(&self, s: [f64; 2]) -> Option<Pick> {
        self.pk().circle_pick(s)
    }

    /// A point in a view: snap, else the point on a hovered edge, else free.
    /// The centermark (placed or automatic) under `s`: its center.
    fn centermark_point(&self, s: [f64; 2]) -> Option<PickedPoint> {
        let b = self.built.as_ref()?;
        let h = b.hit(s, self.pk().tol_mm(noshape_scene::drawing_tools::PICKBOX_PX))?;
        match b.picks.get(h as usize)? {
            PickTarget::AutoCentermark { view, center } => Some(PickedPoint { view: *view, p: [center[0] as f64 / 1e9, center[1] as f64 / 1e9], r: None }),
            PickTarget::Annotation(id) => match self.current_sheet().annotations.iter().find(|a| a.id == *id) {
                Some(Annotation { kind: AnnotationKind::Centermark { view, center, .. }, refs, .. }) => Some(PickedPoint { view: *view, p: *center, r: refs.first().map(|r| r.point.clone()) }),
                _ => None,
            },
            _ => None,
        }
    }

    pub fn any_point(&self, s: [f64; 2]) -> Option<PickedPoint> {
        self.pk().any_point(s)
    }

    // ------------------------------------------------------------ previews

    pub fn pick_sheet(&self, p: &PickedPoint) -> Option<[f64; 2]> {
        self.pk().pick_sheet(p)
    }

    /// The annotation the active dimension tool would create at the cursor.
    pub fn pending_annotation(&self) -> Option<Annotation> {
        self.pk().pending_annotation(&self.tool_values())
    }

    /// The view dialogs' options a new view takes (see `ViewOptions`).
    pub fn view_options(&self) -> noshape_scene::drawing_tools::ViewOptions {
        use noshape_scene::drawing_tools::{AuxOpts, DetailOpts, InsertOpts, SectionOpts, ViewOptions};
        let i = &self.insert;
        ViewOptions {
            insert: InsertOpts { scale: i.scale, simplification: i.simplification, flat: i.flat, sketch_appearances: i.sketch_appearances, model_annotations: i.model_annotations, asm_position: i.asm_position, display_state: i.display_state.clone(), named: i.named.clone(), sketch: i.sketch, pinned: i.pinned.clone() },
            aux: self.aux_dlg.as_ref().map(|d| AuxOpts { reverse: d.reverse, show_ref: d.show_ref, anchor: d.anchor, letter: self.aux_letter.clone() }),
            section: self.section_dlg.as_ref().map(|d| SectionOpts { cut_only: d.cut_only, flip: d.flip, depth_on: d.depth.is_some() && !matches!(&self.tool, Tool::Section { jogs, .. } if !jogs.is_empty()), depth_value: d.depth_value, up_to: d.up_to, depth_ref: d.depth_ref.clone(), offset: d.offset.as_ref().and(d.offset_value), offset_flip: d.offset_flip, exclude: d.exclude.clone(), cut_from: d.cut_from, anchor: d.anchor.clone() }),
            detail: self.detail_dlg.as_ref().map(|d| DetailOpts { scale: d.scale, profile: d.profile.clone(), style: d.style, show_profile: d.show_profile, clip_border: d.clip_border, label: self.detail_label.clone() }),
        }
    }

    /// Live (request 1558, qa DWG-TOOL-HINTS-PREVIEW): before its first pick
    /// the Datum tool shows its flag at the cursor, and the Geometric
    /// tolerance tool its frame (an empty position frame until typed).
    fn cursor_tool_preview(&self, cx: &App) -> Option<Annotation> {
        let c = self.cursor;
        match self.tool {
            Tool::Datum { anchor: None } => Some(Annotation { kind: AnnotationKind::Datum { anchor: c, label: self.datum_dialog_label(), on: None, at: None }, position: [c[0] + 8.0, c[1]], ..Default::default() }),
            // The next symbol at the pointer, on the edge or extension line under it (live2 ann-1118).
            Tool::SurfaceFinish if self.note_leader.is_none() && matches!(self.panel, Some(Panel::SurfaceFinish { edit: None, .. })) => Some(self.surface_finish_symbol(c, cx)),
            // With its leader once the first click anchored one (live2 ann-1118: the arrow at the point clicked).
            Tool::Gdt if self.gdt_dlg.as_ref().is_some_and(|d| d.edit.is_none()) => {
                let (frame, extra) = self.gdt_values(cx).unwrap_or_else(|| (vec![noshape_scene::drawing_tools::GDT_TYPES[0].0.to_string(), String::new()], Default::default()));
                let mut a = noshape_scene::drawing_tools::gdt(0, c, frame, extra);
                if let (Some(l), AnnotationKind::Gdt { leaders, .. }) = (self.note_leader.clone(), &mut a.kind) {
                    leaders.push(l);
                }
                Some(a)
            }
            _ => None,
        }
    }

    /// The dialog and tool state the previews read (see `ToolValues`).
    pub fn tool_values(&self) -> noshape_scene::drawing_tools::ToolValues {
        noshape_scene::drawing_tools::ToolValues {
            callout: self.callout_values_now(),
            datum_label: Some(self.datum_dialog_label()),
            datum_rows: self.datum_dlg.as_ref().map(|d| d.rows.iter().map(|r| r.ann.clone()).collect()).unwrap_or_default(),
            datum_on: self.datum_on,
            datum_at: self.datum_at,
            datum_edge: self.datum_edge,
            datum_target_letter: self.datum_target_letter,
            sketch_dim_pick: self.sketch_dim_pick,
        }
    }

    /// Places the pending section or detail view (its tool's last click, or the depth entity picked for a section
    /// view placed before it).
    pub fn place_section(&mut self, cx: &mut Context<Self>) {
        let Some(mut v) = self.pending_view() else { return };
        if let (ViewKind::Section { label, .. }, Some(l)) = (&mut v.kind, self.section_label(cx)) {
            *label = l;
        }
        let name = if matches!(self.tool, Tool::Section { .. }) { "Section view" } else { "Detail view" };
        let picked = if matches!(self.tool, Tool::Detail { .. }) { self.detail_dlg.as_ref().map(|d| d.picked.clone()).unwrap_or_default() } else { Vec::new() };
        self.edit_sheet(name, cx, |s| {
            s.views.push(v);
            if !picked.is_empty() {
                s.sketch.sketch.remove_entities(&picked);
                let _ = s.sketch.sketch.solve();
            }
        });
        self.tool = Tool::Select;
        // A newly placed view is not selected (live 2068).
        self.selection.clear();
    }

    /// The view the active view tool would create at the cursor (a section view placed before its depth entity:
    /// where it was placed).
    pub fn pending_view(&self) -> Option<DrawingView> {
        let mut pk = self.pk();
        if let (Tool::Section { .. }, Some(at)) = (&self.tool, self.section_dlg.as_ref().and_then(|d| d.placed)) {
            pk.cursor = at;
        }
        let mut v = pk.pending_view(&self.view_options())?;
        // Insert view's continuation places along the parent's axes only: a diagonal click places nothing (qa
        // session-1118; a P-started projected view does go isometric, live2 place-1118).
        if self.insert_continuation && matches!(v.kind, ViewKind::Projected { direction: Some(d), .. } if d[0] != 0.0 && d[1] != 0.0) {
            return None;
        }
        // An assembly's view: no automatic centermarks / centerlines (live: they're the parts' defaults).
        if let ViewKind::Base { source, .. } = &v.kind
            && self.sources.iter().any(|(k, s)| k.tab == *source && s.assembly)
        {
            v.show_centerlines = false;
            v.auto_centerlines = Some(false);
        }
        // The Aligned section view panel's flip and cut-only show in the preview.
        if let (Tool::AlignedSection { .. }, Some(d)) = (&self.tool, &self.aligned_dlg) {
            v.flip = d.flip;
            v.cut_only = d.cut_only;
            v.exclude_from_cut = d.exclude.clone();
            v.cut_from_section = d.cut_from;
        }
        Some(v)
    }

    /// Rebuild the preview drawing and the overlay for the current tool state.
    pub fn update_preview(&mut self, cx: &mut Context<Self>) {
        let mut overlay = Overlay::default();
        let c = self.cursor;
        let snap = match &self.tool {
            Tool::Dimension { kind: DimTool::Linear | DimTool::Point2 | DimTool::PointLine | DimTool::Angle3 | DimTool::MaxMin | DimTool::Foreshorten, .. }
            | Tool::BrokenOut { .. }
            | Tool::CropPoly { .. }
            | Tool::DatumTarget { anchor: None }
            | Tool::GdtLeader { .. }
            | Tool::Centerline { .. }
            | Tool::CircleCenterline { .. }
            | Tool::Balloon { anchor: None }
            | Tool::Section { p0: None, .. }
            | Tool::Detail { radius: None, .. }
            | Tool::JoggedSection { done: false, .. }
            | Tool::AlignedSection { b: None, .. }
            | Tool::Ordinate { .. }
            | Tool::Table { kind: TableTool::Hole, origin: None, .. }
            | Tool::Datum { anchor: None }
            | Tool::Note
            | Tool::Weld
            | Tool::EdgeCondition
            | Tool::Crop { .. } => self.snap_at(c),
            _ => None,
        };
        // Approaching a straight edge with Shift+Q's midpoints on (live request 1662 (b)): "+" marks at its near end and
        // its midpoint, kept while the tool is active; without Shift+Q nothing (live2 drw-tools-1119).
        if matches!(self.tool, Tool::Dimension { .. })
            && self.snap_mid_quad
            && let Some((view, me)) = self.hovered_edge()
            && let views::EdgeGeom::Line { a, b } = me.geom
            && let Some(pl) = self.placement(view)
        {
            let (sa, sb) = (pl.to_sheet(a), pl.to_sheet(b));
            let near = if (sa[0] - c[0]).hypot(sa[1] - c[1]) <= (sb[0] - c[0]).hypot(sb[1] - c[1]) { sa } else { sb };
            for p in [near, [(sa[0] + sb[0]) / 2.0, (sa[1] + sb[1]) / 2.0]] {
                if !self.woken_plus.iter().any(|w| (w[0] - p[0]).abs() < 1e-9 && (w[1] - p[1]).abs() < 1e-9) {
                    self.woken_plus.push(p);
                }
            }
        }
        // A thread line under the pointer, in the hover tan (live 1644 d-7).
        if let Some(Pick::Line { a, b }) = self.pk().thread_line_near(self.hover)
            && let Some(pl) = self.placement(a.view)
        {
            overlay.tinted.push((vec![pl.to_sheet(a.p), pl.to_sheet(b.p)], [0xcc, 0xb2, 0x92, 255], 3.0));
        }
        if matches!(self.tool, Tool::Dimension { .. }) {
            // Orange, about 11 px tall, a red centre (live 1662 (b)).
            const ORANGE: [u8; 4] = [255, 130, 63, 255];
            let h = self.tol_mm(5.5);
            for p in self.woken_plus.iter().chain(self.dimension_pick_marks().iter()) {
                overlay.tinted.push((vec![[p[0] - h * 0.7, p[1]], [p[0] + h * 0.7, p[1]]], ORANGE, 2.0));
                overlay.tinted.push((vec![[p[0], p[1] - h], [p[0], p[1] + h]], ORANGE, 2.0));
                overlay.tinted.push((vec![[p[0] - h * 0.1, p[1]], [p[0] + h * 0.1, p[1]]], [255, 0, 0, 255], 2.0));
            }
        }
        // Sketch tools: hovering an endpoint wakes it (SK8).
        if matches!(self.tool, Tool::Sketch { .. })
            && let Some((_, p)) = self.sketch_point_at(c)
            && !self.woken_points.iter().any(|w| (w[0] - p[0]).abs() < 1e-9 && (w[1] - p[1]).abs() < 1e-9)
        {
            self.woken_points.push(p);
        }
        self.hover_snap = snap.as_ref().and_then(|s| self.pick_sheet(s));
        if let Some(p) = self.hover_snap {
            // The Note tool and the 2 point centerline frame the point (live C11 (a), live2 ann-1118); the dimension tools
            // and others put the disc on it (D21).
            let mark = if matches!(self.tool, Tool::Note | Tool::Centerline { .. }) { crate::drawing_scene::SnapMark::Box } else { crate::drawing_scene::SnapMark::Disc };
            overlay.marks.push((p, mark, self.tol_mm(15.0)));
        }
        self.section_edit_overlay(&mut overlay);
        self.table_tool_overlay(&mut overlay, cx);
        self.table_corner_mark(&mut overlay);
        let mut preview: Option<Drawing> = None;
        match &self.tool {
            Tool::Dimension { picks, .. } => {
                if let [Pick::Point(a)] = picks.as_slice()
                    && let Some(sa) = self.pick_sheet(a)
                {
                    overlay.lines.push((vec![sa, self.hover_snap.unwrap_or(c)], true));
                }
                // A circle's pick spot: a red-orange hourglass, 13 px (live2 drw-tools-1119 rad-note r-picked).
                if let Some(p) = self.dim_circle_spot.filter(|_| picks.iter().any(|p| matches!(p, Pick::Circle { .. } | Pick::CircleAt { .. }))) {
                    const HOURGLASS: [u8; 4] = [255, 89, 0, 255];
                    let h = self.tol_mm(6.5);
                    overlay.tinted.push((vec![[p[0] - h, p[1] + h], [p[0] + h, p[1] + h], [p[0] - h, p[1] - h], [p[0] + h, p[1] - h], [p[0] - h, p[1] + h]], HOURGLASS, 1.5));
                }
                // Picked points keep the snap disc, picked edges the selection orange (live2 drw-tools-1119: p2p, dimg).
                const PICKED: [u8; 4] = [0xcc, 0x92, 0x5c, 255];
                let lit = LIT_LINE_PX;
                for p in picks {
                    match p {
                        Pick::Point(a) => overlay.marks.extend(self.pick_sheet(a).map(|p| (p, crate::drawing_scene::SnapMark::Disc, 0.0))),
                        Pick::Line { a, b } => {
                            if let (Some(sa), Some(sb)) = (self.pick_sheet(a), self.pick_sheet(b)) {
                                overlay.tinted.push((vec![sa, sb], PICKED, lit));
                            }
                        }
                        // A picked arc shows as itself, not its whole circle
                        // (qa DWG-ARC-REF-FULL-CIRCLE).
                        Pick::Arc { view, center, radius, a, b, mid, .. } => {
                            if let Some(pl) = self.placement(*view) {
                                let (c, r) = (pl.to_sheet(*center), pl.len_to_sheet(*radius));
                                let ang = |p: [f64; 2]| (p[1] - c[1]).atan2(p[0] - c[0]);
                                let tau = std::f64::consts::TAU;
                                let (t0, t1, tm) = (ang(pl.to_sheet(a.p)), ang(pl.to_sheet(b.p)), ang(pl.to_sheet(*mid)));
                                let ccw = (t1 - t0).rem_euclid(tau);
                                let sweep = if (tm - t0).rem_euclid(tau) <= ccw { ccw } else { ccw - tau };
                                let pts = (0..=48).map(|i| {
                                    let t = t0 + sweep * i as f64 / 48.0;
                                    [c[0] + r * t.cos(), c[1] + r * t.sin()]
                                });
                                overlay.tinted.push((pts.collect(), PICKED, lit));
                            }
                        }
                        Pick::Circle { view, center, radius, .. } | Pick::CircleAt { view, center, radius, .. } => {
                            if let Some(pl) = self.placement(*view) {
                                let (c, r) = (pl.to_sheet(*center), pl.len_to_sheet(*radius));
                                let pts = (0..=72).map(|i| {
                                    let t = i as f64 / 72.0 * std::f64::consts::TAU;
                                    [c[0] + r * t.cos(), c[1] + r * t.sin()]
                                });
                                overlay.tinted.push((pts.collect(), PICKED, lit));
                            }
                        }
                    }
                }
            }
            Tool::Centerline { first: Some(a) } => {
                if let Some(sa) = self.pick_sheet(a) {
                    overlay.lines.push((vec![sa, self.hover_snap.unwrap_or(c)], true));
                }
            }
            Tool::CircleCenterline { three, points } if !points.is_empty() => {
                let at = self.hover_snap.unwrap_or(c);
                let mut pts: Vec<[f64; 2]> = points.iter().filter_map(|p| self.pick_sheet(p)).collect();
                overlay.points.extend(pts.iter().copied());
                pts.push(at);
                if (*three && pts.len() == 3) || (!*three && pts.len() == 2) {
                    if let Some((cc, r)) = AnnotationKind::circle_centerline(&pts) {
                        overlay.circle(cc, r, true);
                    }
                } else if pts.len() == 2 {
                    overlay.lines.push((pts, true));
                }
            }
            // The first edge stays lit tan, no centerline previewed (live2 drw-tools-1119 hc.png).
            Tool::EdgeCenterline { first: Some((a, b)) } => {
                if let (Some(sa), Some(sb)) = (self.pick_sheet(a), self.pick_sheet(b)) {
                    overlay.tinted.push((vec![sa, sb], [0xcc, 0xb2, 0x92, 255], LIT_LINE_PX));
                }
            }
            Tool::VirtualSharp { first: Some((a, b, _)) } => {
                if let (Some(sa), Some(sb)) = (self.pick_sheet(a), self.pick_sheet(b)) {
                    overlay.lines.push((vec![sa, sb], false));
                }
            }
            Tool::Section { parent: Some(pid), p0: Some(p0), line: None, .. } => {
                if let Some(pl) = self.placement(*pid) {
                    let a = pl.to_sheet(*p0);
                    let b = if self.section_free_angle() { self.hover_snap.unwrap_or(c) } else { self.ortho_point(a, c) };
                    overlay.lines.push((vec![a, b], true));
                }
            }
            // A placed cutting line: hovering another snap point of the view
            // previews the jog through it (help drawing_section_view step 7).
            Tool::Section { parent: Some(pid), line: Some((a, b)), partial: false, jogs, .. } => {
                if let (Some(q), Some(pl)) = (self.section_jog_at(*pid, c), self.placement(*pid)) {
                    let mut js = jogs.clone();
                    js.push(q);
                    let pts = noshape_scene::drawing_tools::jogged_line(*a, *b, &js);
                    overlay.lines.push((pts.iter().map(|p| pl.to_sheet(*p)).collect(), true));
                }
            }
            // Vertical / horizontal section: once a view is picked the cutting line follows the pointer 1:1 across it,
            // jumping to an inference point in reach, marked (live2 place-1118); none before.
            Tool::Section { parent: Some(pid), p0: None, partial: false, .. } if self.section_axis().is_some() => {
                let inference = self.section_inference(c);
                let (v, at) = inference.as_ref().and_then(|(pp, _)| Some((pp.view, self.pick_sheet(pp)?))).unwrap_or((*pid, c));
                if let Some(e) = self.section_axis()
                    && let Some((la, lb)) = self.extend_cut(v, at, e)
                    && let Some(pl) = self.placement(v)
                {
                    // Live2 place-1118 (slc): grey (127,127,127), a long dash and two short ones (83 / 15 / 15 px, 16 px gaps).
                    let pattern = [83.0, 16.0, 15.0, 17.0, 15.0, 17.0].map(|px| self.tol_mm(px));
                    for piece in dash_pieces(pl.to_sheet(la), pl.to_sheet(lb), &pattern) {
                        overlay.tinted.push((piece, [127, 127, 127, 255], 2.0));
                    }
                }
                if let Some((_, Some(size))) = inference {
                    overlay.marks.push((at, crate::drawing_scene::SnapMark::Disc, size));
                }
            }
            Tool::Detail { parent: Some(pid), center: Some(ctr), radius: None } => {
                if let Some(pl) = self.placement(*pid) {
                    let cs = pl.to_sheet(*ctr);
                    let at = self.snap_at(c).filter(|pp| pp.view == *pid).map(|pp| pl.to_sheet(pp.p)).unwrap_or(c);
                    // Solid black while sized (live2 place-1118).
                    overlay.tinted.push((circle_points(cs, len(sub(at, cs))), [0, 0, 0, 255], 1.0));
                }
            }
            // Placing it: the parent keeps the plain circle until the view is made (live2 place-1118).
            Tool::Detail { parent: Some(pid), center: Some(ctr), radius: Some(r) } if self.detail_dlg.as_ref().is_none_or(|d| d.shape == 0) => {
                if let Some(pl) = self.placement(*pid) {
                    overlay.tinted.push((circle_points(pl.to_sheet(*ctr), r * pl.scale * 1000.0), [0, 0, 0, 255], 1.0));
                }
            }
            Tool::Sketch { .. } => self.sketch_overlay(&mut overlay),
            Tool::Weld => self.weld_overlay(&mut overlay),
            Tool::EdgeCondition => self.edge_condition_overlay(&mut overlay),
            Tool::Note | Tool::SurfaceFinish if self.note_leader.is_some() => {
                if let Some(a) = self.note_leader.as_ref().and_then(|l| Some(self.placement(l.view?)?.to_sheet(l.point))) {
                    overlay.lines.push((vec![a, self.cursor], true));
                }
            }
            Tool::PlaceImage { .. } => self.image_overlay(&mut overlay),
            Tool::Detail { center: None, .. } if self.detail_dlg.as_ref().is_some_and(|d| d.shape != 0) => self.detail_overlay(&mut overlay),
            _ => self.extra_overlay(&mut overlay),
        }
        self.crop_overlay(&mut overlay);
        self.closed_loop_overlay(&mut overlay);
        // The Datum dialog's targets show until ✓ (under the pending one).
        let base = self.datum_rows_drawing();
        if let Some(mut v) = self.pending_view() {
            let mut d = self.drawing.clone();
            let insert = matches!(self.tool, Tool::InsertView { .. });
            // A section, detail or auxiliary view's preview is grey with no label, a detail's profile not yet on its
            // parent, an auxiliary's arrows without letters (live2 place-1118).
            if matches!(self.tool, Tool::Section { .. } | Tool::Detail { .. } | Tool::Auxiliary { .. }) {
                v.hide_label = true;
            }
            if let Some(a) = v.aux_ref.as_mut() {
                a.letter.clear();
            }
            if matches!(self.tool, Tool::Detail { .. }) {
                v.detail_hide_profile = true;
            }
            if let Some(s) = d.sheets.get_mut(self.sheet) {
                if insert {
                    noshape_model::drawing::create::push_inserted_view(s, v);
                } else {
                    let id = v.id;
                    s.views.push(v);
                    if let Some(off) = self.aux_fold_offset(s, id)
                        && let Some(a) = s.view_mut(id).and_then(|v| v.aux_ref.as_mut())
                    {
                        a.offset = Some(off);
                    }
                }
            }
            preview = Some(d);
        } else if let Some(a) = self.pending_annotation() {
            let mut d = base.clone().unwrap_or_else(|| self.drawing.clone());
            if let Some(s) = d.sheets.get_mut(self.sheet) {
                s.annotations.push(a);
            }
            preview = Some(d);
        } else if matches!(self.tool, Tool::Note) && !matches!(self.panel, Some(Panel::Note { .. })) {
            // Live (behaviours 1414, drawing/220-222): a horizontal "NOTE"
            // preview follows the pointer until the click (qa5 P2-D3).
            let mut d = base.clone().unwrap_or_else(|| self.drawing.clone());
            if let Some(s) = d.sheets.get_mut(self.sheet) {
                let a = note_tool_preview(&self.drawing, s, c);
                s.annotations.push(a);
            }
            preview = Some(d);
        } else if let Some(mut a) = self.cursor_tool_preview(cx) {
            let mut d = base.clone().unwrap_or_else(|| self.drawing.clone());
            if let Some(s) = d.sheets.get_mut(self.sheet) {
                a.id = s.next_annotation_id();
                s.annotations.push(a);
            }
            preview = Some(d);
        } else if let Some(t) = self.hole_table_preview().or_else(|| self.table_dlg_preview()) {
            let mut d = self.drawing.clone();
            if let Some(s) = d.sheets.get_mut(self.sheet) {
                s.tables.push(t);
            }
            preview = Some(d);
        } else if base.is_some() {
            preview = base;
        }
        // Edit detail / Edit section: the edited view as the dialog has it.
        if let Some(d) = self.detail_edit_drawing(cx).or_else(|| self.section_edited()).or_else(|| self.foreshorten_preview()).or_else(|| self.break_preview()) {
            preview = Some(d);
        }
        overlay.infer.extend(self.infer_lines.iter().cloned());
        // The points an annotation tool woke keep their red ✕ (live2 wake-1118).
        if !matches!(self.tool, Tool::Select | Tool::Sketch { .. } | Tool::Constrain { .. }) {
            let h = self.tol_mm(5.0);
            overlay.crosses.extend(self.woken_points.iter().map(|p| (*p, h)));
        }
        // The pointer on an inference line: a red ✕ (live request 1662).
        if !self.infer_lines.is_empty() {
            overlay.crosses.push((self.cursor, self.tol_mm(5.0)));
        }
        for (c, r) in self.centermark_overlay() {
            overlay.circle(c, r, false);
        }
        self.overlay = overlay;
        let had_preview = self.preview.is_some();
        if preview.is_some() || had_preview {
            self.set_preview(preview, cx);
        }
        self.redraw(cx);
    }

    /// Snap a second point to horizontal/vertical from `a` when close.
    pub fn ortho_point(&self, a: [f64; 2], c: [f64; 2]) -> [f64; 2] {
        noshape_scene::drawing_tools::ortho_point(a, c)
    }

    // ------------------------------------------------------------ clicks

    /// A cutting line through sheet point `a` along sheet direction `e`,
    /// extended across view `pid` (view coordinates).
    pub fn extend_cut(&self, pid: ViewId, a: [f64; 2], e: [f64; 2]) -> Option<([f64; 2], [f64; 2])> {
        self.pk().extend_cut(pid, a, e)
    }

    pub(crate) fn tool_click(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // Centermark dialog: Single places at once, Circular / Linear collect circles.
        if matches!(self.tool, Tool::Centermark) && self.centermark_click(cx) {
            return;
        }
        let c = self.cursor;
        if self.sketch_click(cx) {
            return;
        }
        if matches!(self.tool, Tool::Section { .. }) && self.section_field_click(cx) {
            return;
        }
        if matches!(self.tool, Tool::Detail { .. }) && self.detail_click(cx) {
            return;
        }
        // Edit detail: a redrawn profile stays on the edited view (✓ applies).
        if matches!(self.tool, Tool::Detail { radius: Some(_), .. }) && self.detail_dlg.as_ref().is_some_and(|d| d.edit.is_some()) {
            return;
        }
        if self.weld_click(cx) || self.edge_condition_click(cx) || self.stack_click(cx) || self.table_dlg_click(cx) || self.custom_dlg_click(cx) || self.table_tool_click(cx) || self.gdt_click(cx) {
            return;
        }
        if self.extra_tool_click(window, cx) {
            self.update_preview(cx);
            return;
        }
        let tool = self.tool.clone();
        match tool {
            Tool::Select => {}
            Tool::InsertView { .. } => {
                if let Some(v) = self.pending_view() {
                    let id = v.id;
                    // The first view gives the sheet its scale; later
                    // views default to it.
                    if self.drawing_sheet().views.is_empty() {
                        self.insert.scale = None;
                        self.insert_scale_was_auto = false;
                    }
                    self.edit_sheet("Insert view", cx, |s| noshape_model::drawing::create::push_inserted_view(s, v));
                    // Placing a view on an out-of-date drawing updates it.
                    if self.out_of_date {
                        self.update_drawing(cx);
                    }
                    // Onshape activates the projected view tool next, the new view its parent drawn picked, not selected
                    // (live 2068).
                    self.tool = Tool::Projected { parent: Some(id) };
                    self.selection.clear();
                    self.projection_parent_lit = true;
                    self.insert_continuation = true;
                }
            }
            Tool::Projected { parent: None } => {
                if let Some(v) = self.view_at(c) {
                    // Help H:projected_view: not from detail views.
                    if self.current_sheet().view(v).is_some_and(|x| matches!(x.kind, ViewKind::Detail { .. })) {
                        self.status = Some("Projected views can't be created from detail views".into());
                    } else {
                        self.status = None;
                        self.tool = Tool::Projected { parent: Some(v) };
                        // The clicked parent shows selected (live 2068).
                        self.selection = vec![Sel::View(v)];
                    }
                }
            }
            Tool::Projected { parent: Some(_) } | Tool::Auxiliary { parent: Some(_) } => {
                if let Some(mut v) = self.pending_view() {
                    // Shift+click: the projected view keeps the parent section's cut.
                    if matches!(self.tool, Tool::Projected { .. }) && window.modifiers().shift {
                        v.cut_from_section = true;
                    }
                    let name = if matches!(self.tool, Tool::Auxiliary { .. }) { "Auxiliary view" } else { "Projected view" };
                    self.edit_sheet(name, cx, |s| s.views.push(v));
                    // A newly placed view is not selected, and the parent no longer lit (live 2068).
                    self.selection.clear();
                    self.projection_parent_lit = false;
                    if matches!(self.tool, Tool::Auxiliary { .. }) {
                        self.tool = Tool::Select;
                    }
                }
            }
            Tool::Auxiliary { parent: None } => {
                // Auxiliary view dialog: Normal / In line / Axis (drawing_aux).
                if let Some((v, d, anchor)) = self.aux_pick() {
                    let kind = self.current_sheet().view(v).map(|x| (matches!(x.kind, ViewKind::Detail { .. }), !x.breaks.is_empty()));
                    if kind.is_some_and(|(det, brk)| det || brk) {
                        self.status = Some("Auxiliary views can't be created from detail or break views".into());
                    } else {
                        if let Some(dl) = &mut self.aux_dlg {
                            dl.anchor = Some(anchor);
                        }
                        self.status = None;
                        self.tool = Tool::Auxiliary { parent: Some((v, d)) };
                    }
                } else {
                    self.status = Some(match self.aux_dlg.as_ref().map(|d| d.mode) {
                        Some(1) => "Select a sketch line or centerline",
                        Some(2) => "Select a hole axis",
                        _ => "Select a straight edge of a view",
                    }
                    .into());
                }
            }
            Tool::Section { parent, p0: None, partial, .. } => {
                // A click just outside a view still cuts it (8 mm around it).
                let near = self.view_at(c).or_else(|| {
                    let b = self.built.as_ref()?;
                    b.placements.iter().filter(|p| p.contains(c, 8.0)).min_by(|x, y| x.dist_to(c).total_cmp(&y.dist_to(c))).map(|p| p.id)
                });
                // Vertical / Horizontal (live2 place-1118): a click fixes the cut only on an inference point; elsewhere it
                // picks the view (the line then follows the pointer) or, once picked, does nothing.
                if self.section_axis().is_some() && !partial {
                    match (self.section_inference(c), parent) {
                        (Some(_), _) => {}
                        (None, None) => {
                            match near {
                                Some(v) => self.tool = Tool::Section { parent: Some(v), p0: None, line: None, partial, jogs: vec![] },
                                None => self.status = Some("Select a view to section".into()),
                            }
                            return;
                        }
                        (None, Some(_)) => return,
                    }
                }
                let pick = self.section_inference(c).map(|(pp, _)| pp).filter(|_| self.section_axis().is_some() && !partial).or_else(|| self.snap_at(c)).or_else(|| near.and_then(|v| self.placement(v).map(|pl| PickedPoint { view: v, p: pl.to_view(c), r: None })));
                if pick.is_none() {
                    self.status = Some("Select a view to section".into());
                }
                // Help drawing_section_view: no angular sections on views with breaks.
                let angular = self.section_dlg.as_ref().is_some_and(|d| d.mode == 2);
                if angular && pick.as_ref().and_then(|pp| self.current_sheet().view(pp.view)).is_some_and(|v| !v.breaks.is_empty()) {
                    self.status = Some("Angular section views can't be created from views with breaks".into());
                    return;
                }
                if let Some(pp) = pick {
                    // Section view dialog › Vertical / Horizontal: one click places the whole line.
                    let axis_line = self.section_axis().filter(|_| !partial).and_then(|e| self.placement(pp.view).and_then(|pl| self.extend_cut(pp.view, pl.to_sheet(pp.p), e)));
                    if let Some(d) = &mut self.section_dlg {
                        d.anchor = pp.r.clone();
                    }
                    self.tool = Tool::Section { parent: Some(pp.view), p0: Some(pp.p), line: axis_line, partial, jogs: vec![] };
                }
            }
            Tool::Section { parent: Some(pid), p0: Some(p0), line: None, partial, .. } => {
                let Some(pl) = self.placement(pid) else { return };
                let a = pl.to_sheet(p0);
                // Angular, Cut angle unticked: through the second click (its snap point), at any angle.
                let free = self.section_free_angle() && !partial;
                let b = if free { self.snap_at(c).filter(|q| q.view == pid).map(|q| pl.to_sheet(q.p)).unwrap_or(c) } else { self.ortho_point(a, c) };
                if len(sub(b, a)) < 2.0 {
                    return;
                }
                if partial {
                    // A partial section uses the line as clicked (snapped).
                    let q = self.snap_at(c).filter(|q| q.view == pid).map(|q| q.p).unwrap_or(pl.to_view(b));
                    self.tool = Tool::Section { parent: Some(pid), p0: Some(p0), line: Some((p0, q)), partial, jogs: vec![] };
                    self.update_preview(cx);
                    return;
                }
                // Extend the cutting line across the whole view.
                let Some((la, lb)) = self.extend_cut(pid, a, norm(sub(b, a))) else { return };
                self.tool = Tool::Section { parent: Some(pid), p0: Some(p0), line: Some((la, lb)), partial, jogs: vec![] };
            }
            // Another snap point of the view jogs the cutting line through it
            // (help drawing_section_view step 7); any other click places.
            Tool::Section { parent: Some(pid), line: Some(_), partial: false, .. } if self.section_jog_at(pid, c).is_some() => {
                if let (Some(q), Tool::Section { jogs, .. }) = (self.section_jog_at(pid, c), &mut self.tool) {
                    jogs.push(q);
                }
                self.update_preview(cx);
            }
            Tool::Section { line: Some(_), .. } | Tool::Detail { radius: Some(_), .. } => self.place_section(cx),
            Tool::Detail { center: None, .. } => {
                let pick = self.snap_at(c).or_else(|| self.view_at(c).and_then(|v| self.placement(v).map(|pl| PickedPoint { view: v, p: pl.to_view(c), r: None })));
                if let Some(pp) = pick {
                    self.tool = Tool::Detail { parent: Some(pp.view), center: Some(pp.p), radius: None };
                }
            }
            Tool::Detail { parent: Some(pid), center: Some(ctr), radius: None } => {
                if let Some(pl) = self.placement(pid) {
                    // The radius to a snapped point of the view (a vertex, an edge), else to the click.
                    let at = self.snap_at(c).filter(|pp| pp.view == pid).map(|pp| pl.to_sheet(pp.p)).unwrap_or(c);
                    let r = len(sub(at, pl.to_sheet(ctr))) / (pl.scale * 1000.0);
                    if r > 1e-6 {
                        self.tool = Tool::Detail { parent: Some(pid), center: Some(ctr), radius: Some(r) };
                    }
                }
            }
            Tool::Dimension { kind: DimTool::Linear, ref picks } if picks.is_empty() && (self.sketch_dim_pick.is_some() || self.sketch_entity_at().is_some()) => {
                // Drawing sketch geometry (SK7): pick, then place + value box.
                self.tool = Tool::Dimension { kind: DimTool::Linear, picks: vec![] };
                self.sketch_dim_click(c, window, cx);
            }
            Tool::Dimension { kind, mut picks } => {
                // Smart Dimension: a click on another entity adds it (a second
                // pick) instead of placing; on a picked edge it deselects it.
                let more = self.pk().dimension_takes_more(kind, c, &picks);
                if !more && let Some(a) = self.pending_annotation() {
                    // Chamfer, Dimensions style: the two legs as linear dimensions.
                    let anns = self.chamfer_as_dimensions(&a).unwrap_or_else(|| vec![a]);
                    // The new dimension stays selected, bold, its "±0.1" launcher above it (live2 drw-tools-1119).
                    let placed: Vec<Sel> = anns.iter().map(|a| Sel::Annotation(a.id)).collect();
                    self.edit_sheet(kind.name(), cx, |s| s.annotations.extend(anns));
                    self.selection = placed;
                    self.tool = Tool::Dimension { kind, picks: vec![] };
                } else {
                    // The next dimension started: the one placed before is let go.
                    if picks.is_empty() {
                        self.selection.retain(|s| !matches!(s, Sel::Annotation(_)));
                    }
                    let had = picks.len();
                    match kind {
                        DimTool::MaxMin | DimTool::Foreshorten => self.more_dimension_pick(kind, &mut picks),
                        _ => self.pk().dimension_click(kind, c, &mut picks),
                    }
                    if picks.len() > had && matches!(picks.last(), Some(Pick::Circle { .. } | Pick::CircleAt { .. })) {
                        self.dim_circle_spot = Some(c);
                    }
                    self.tool = Tool::Dimension { kind, picks };
                }
            }
            Tool::HoleCallout { circle: None } => {
                if let Some(Pick::Circle { view, center, radius, r }) = self.circle_pick(c)
                    && self.pk().is_hole_circle(view, r.as_ref())
                {
                    self.tool = Tool::HoleCallout { circle: Some((view, center, radius, r)) };
                }
            }
            Tool::HoleCallout { circle: Some(_) } => {
                if let Some(a) = self.pending_annotation() {
                    self.edit_sheet("Hole callout", cx, |s| s.annotations.push(a));
                    self.tool = Tool::HoleCallout { circle: None };
                }
            }
            Tool::EdgeCenterline { first } => {
                let Some(Pick::Line { a, b }) = self.line_pick() else {
                    self.status = Some("Select a straight edge".into());
                    return;
                };
                match first {
                    // Two edges always (live: "Place a centerline by selecting two edges"; a cylinder's silhouette too).
                    None => self.tool = Tool::EdgeCenterline { first: Some((a, b)) },
                    Some((a1, b1)) => {
                        if a.view != a1.view {
                            self.status = Some("Select an edge in the same view".into());
                            return;
                        }
                        // The middle line of the two edges (ends paired along the first edge).
                        let (p, q) = dt::edge_pair_centerline(a1.p, b1.p, a.p, b.p);
                        let id = self.current_sheet().next_annotation_id();
                        self.edit_sheet("Edge to edge centerline", cx, |s| s.annotations.push(dt::edge_centerline(id, a.view, p, q, c)));
                        // The new centerline stays selected, its grips blue (live2 drw-tools-1119 hc.png).
                        self.selection = vec![Sel::Annotation(id)];
                        self.tool = Tool::EdgeCenterline { first: None };
                    }
                }
            }
            Tool::CircleCenterline { three, mut points } => {
                // A snap point, else the point on the edge under the pointer (live drw-centerline-circle-3-point).
                if let Some(p) = self.pk().snap_or_edge_point(c).filter(|p| points.first().is_none_or(|f| f.view == p.view)) {
                    points.push(p);
                }
                let need = if three { 3 } else { 2 };
                if points.len() >= need {
                    let id = self.current_sheet().next_annotation_id();
                    if let Some(a) = dt::circle_centerline(id, &points, c) {
                        let name = if three { "3 point circle centerline" } else { "2 point circle centerline" };
                        self.edit_sheet(name, cx, |s| s.annotations.push(a));
                    }
                    points.clear();
                }
                self.tool = Tool::CircleCenterline { three, points };
            }
            Tool::VirtualSharp { first } => {
                let Some((view, me)) = self.hovered_edge() else { return };
                let Some(Pick::Line { a, b }) = self.line_pick() else {
                    self.status = Some("Select a straight edge".into());
                    return;
                };
                let edge = Some((me.body.clone(), (*me.name).clone()));
                match first {
                    None => self.tool = Tool::VirtualSharp { first: Some((a, b, edge)) },
                    Some((a1, b1, e1)) if a1.view == view => {
                        let id = self.current_sheet().next_annotation_id();
                        match dt::virtual_sharp(id, view, (&a1, &b1, &e1), (&a, &b, &edge), c) {
                            Ok(ann) => {
                                self.edit_sheet("Virtual sharp", cx, |s| s.annotations.push(ann));
                                self.tool = Tool::VirtualSharp { first: None };
                            }
                            Err(why) => {
                                self.status = Some(why.into());
                                return;
                            }
                        }
                    }
                    Some(_) => self.status = Some("Select an edge in the same view".into()),
                }
            }
            Tool::Note => {
                // A click on a snap point or an edge anchors a leader; the
                // next click places the text.
                match (self.note_leader.is_some(), self.pk().snap_or_edge_point(c)) {
                    (false, Some(p)) => self.note_leader = Some(Leader { view: Some(p.view), point: p.p, nodes: Vec::new(), ..Default::default() }),
                    _ => self.open_note_panel(None, c, window, cx),
                }
            }
            Tool::Centerline { first: None } => {
                if let Some(p) = self.pk().centerline_point(c, None) {
                    self.tool = Tool::Centerline { first: Some(p) };
                }
            }
            Tool::Centerline { first: Some(a) } => {
                if let Some(b) = self.pk().centerline_point(c, Some(a.view)) {
                    let id = self.current_sheet().next_annotation_id();
                    self.edit_sheet("Centerline", cx, |s| s.annotations.push(dt::centerline(id, &a, &b, c)));
                    self.tool = Tool::Centerline { first: None };
                }
            }
            Tool::Balloon { anchor: None } => {
                // On a dimension (help callout_balloon; qa
                // CALLOUT-ON-DIMENSION), else on the geometry.
                let on = self.pk().dimension_anchor(c);
                self.datum_on = on.as_ref().map(|o| o.1);
                // On a centermark: its center (help callout_balloon, drawing.md A22).
                let mark = if on.is_none() { self.centermark_point(c) } else { None };
                if let Some(p) = on.map(|o| o.0).or(mark).or_else(|| self.any_point(c)) {
                    self.tool = Tool::Balloon { anchor: Some(p) };
                } else {
                    // Empty space: a callout without a leader, right there.
                    self.refresh_callout_values(cx);
                    let (text, style) = self.callout_values_now().unwrap_or_default();
                    let id = self.current_sheet().next_annotation_id();
                    self.edit_sheet("Callout", cx, |s| s.annotations.push(dt::free_callout(id, c, text, style)));
                }
            }
            Tool::Balloon { anchor: Some(anchor) } => {
                self.refresh_callout_values(cx);
                if let Some(mut a) = self.pending_annotation() {
                    // Clicked on a face or a silhouette: the part on the line of sight (qa5 D8).
                    self.pk().finish_callout(&mut a, &anchor);
                    // Placed on a dimension, callout, weld or surface finish
                    // symbol or datum: grouped with it (help callout_balloon, A27).
                    let mut d = self.drawing.clone();
                    if let Some(s) = d.sheets.get_mut(self.sheet) {
                        s.annotations.push(a.clone());
                    }
                    self.group_callout_on_drop(&a, &mut d);
                    self.commit("Callout", d, cx);
                    self.tool = Tool::Balloon { anchor: None };
                    self.datum_on = None;
                }
            }
            _ => {}
        }
        self.update_preview(cx);
    }

    // ------------------------------------------------------------ mouse

    pub fn on_mouse_down(&mut self, e: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        self.glyph_selected = None;
        if self.placed_table.take().is_some() {
            cx.notify();
        }
        let quick = match (e.button, self.last_press) {
            (MouseButton::Left, Some((t, at))) => quick_double_click(t.elapsed(), (e.position - at).magnitude() as f32),
            _ => false,
        };
        if e.button == MouseButton::Left {
            self.last_press = Some((std::time::Instant::now(), e.position));
        }
        let lp = self.local(e.position);
        self.cursor = self.to_sheet(lp);
        window.focus(&self.focus, cx);
        // A note edited in place: a click away commits it (live).
        if e.button == MouseButton::Left && (matches!(self.panel, Some(Panel::Note { .. })) || self.cell_inplace_box().is_some()) {
            self.apply_panel(cx);
            cx.stop_propagation();
            return;
        }
        if self.shortcut_toolbar.take().is_some() {
            cx.notify();
        }
        // The cached sheet is view-only: selecting and dragging wait for the real one.
        if e.button == MouseButton::Left && self.cached_view() {
            self.say_waiting(cx);
            cx.stop_propagation();
            return;
        }
        // Double middle click: zoom to fit (live navigation/45-46).
        if e.button == MouseButton::Middle && e.click_count >= 2 {
            self.drag = Drag::None;
            self.zoom_to_fit(cx);
            cx.stop_propagation();
            return;
        }
        if e.button == MouseButton::Left && self.painter.is_some() && self.painter_click(cx) {
            cx.stop_propagation();
            return;
        }
        if e.button == MouseButton::Left && self.image_mouse_down(cx) {
            cx.stop_propagation();
            return;
        }
        if e.button == MouseButton::Left && self.hide_parts_click(cx) {
            cx.stop_propagation();
            return;
        }
        if e.button == MouseButton::Left && self.hatch_click(window, cx) {
            cx.stop_propagation();
            return;
        }
        if e.button == MouseButton::Left && self.inspection.is_some() && self.inspection_click(window, cx) {
            cx.stop_propagation();
            return;
        }
        if e.button == MouseButton::Left && self.measure_click(cx) {
            cx.stop_propagation();
            return;
        }
        if e.button == MouseButton::Left && self.zoom_window_down(lp, cx) {
            cx.stop_propagation();
            return;
        }
        if e.button == MouseButton::Left && (self.align_centermark.is_some() || self.align_auto_centermark.is_some()) && self.align_centermark_click(cx) {
            return;
        }
        if e.button == MouseButton::Left && self.rotate_to_edge.is_some() && self.rotate_to_edge_click(cx) {
            return;
        }
        if e.button == MouseButton::Left && self.align_views.is_some() && self.align_views_click(cx) {
            return;
        }
        if e.button == MouseButton::Left && self.aligned_dlg.as_ref().is_some_and(|d| d.repick.is_some()) && self.aligned_repick_click(cx) {
            return;
        }
        if e.button == MouseButton::Left && self.aligned_dlg.as_ref().is_some_and(|d| d.picking) && self.aligned_exclude_click(cx) {
            return;
        }
        // Stacked callouts panel: picks add instances (the panel leaves the
        // Select tool on, so they don't reach tool_click).
        if e.button == MouseButton::Left && self.stack_dlg.is_some() && matches!(self.tool, Tool::Select) && self.stack_click(cx) {
            return;
        }
        // Hole table properties… › Holes to exclude taking picks.
        if e.button == MouseButton::Left && self.hole_dlg.as_ref().is_some_and(|d| d.picking && d.table.is_some()) && self.hole_exclude_click(cx) {
            return;
        }
        if e.button == MouseButton::Left && self.hole_dlg.as_ref().is_some_and(|d| (d.x_picking || d.y_picking) && d.table.is_some()) && self.hole_x_dir_click(cx) {
            return;
        }
        // A dimension's palette and a view's View properties open on a double-click within 200 ms and 3 px (live2
        // 11-13; live drawing-feel-1118: 200 ms opens, 250 doesn't), not the OS's 400 ms.
        if e.button == MouseButton::Left && e.click_count >= 2 && matches!(self.tool, Tool::Select) && (quick || !self.timed_double_click_target(cx)) {
            self.double_click(e.click_count >= 3, window, cx);
            return;
        }
        if e.button == MouseButton::Left && matches!(self.tool, Tool::Select) && self.flip_node_click(cx) {
            cx.stop_propagation();
            return;
        }
        if e.button == MouseButton::Left && e.click_count >= 2 && self.finish_sketch(cx) {
            return;
        }
        if e.button == MouseButton::Left && e.click_count >= 2 && matches!(self.tool, Tool::Detail { .. }) && self.finish_detail_profile(cx) {
            return;
        }
        if e.button == MouseButton::Left && e.click_count >= 2 && matches!(self.tool, Tool::JoggedSection { done: false, .. } | Tool::BrokenOut { done: false, .. } | Tool::CropPoly { .. }) {
            self.confirm(window, cx);
            return;
        }
        self.update_hover(cx);
        self.drag = Drag::Pending { button: drag_button(e.button), start: self.cursor, start_px: lp, pick: self.hover };
        cx.stop_propagation();
    }

    pub fn on_mouse_move(&mut self, e: &MouseMoveEvent, window: &mut Window, cx: &mut Context<Self>) {
        let lp = self.local(e.position);
        self.cursor = self.to_sheet(lp);
        if self.zoom_window_move(lp, cx) {
            return;
        }
        if let Drag::Pending { button, start, start_px, pick } = self.drag.clone()
            && len(sub(lp, start_px)) > self.drag_threshold_px(button, pick, start)
        {
            self.drag = match button {
                Button::Right | Button::Middle => {
                    // Preferences › Mouse controls, "2D Drawing" column
                    // (default: right or middle drag, with or without Ctrl, pans).
                    let mut buttons = nav::Buttons::default();
                    buttons.set(mouse_button(button), true);
                    let r = nav::ModeResolver::new(nav::MouseControls::current(), nav::Space::Drawing);
                    match r.resolve(buttons, r.input_key(e.modifiers.into()), nav::Mode::None) {
                        nav::Mode::Pan => Drag::Pan { last_px: start_px },
                        nav::Mode::Zoom => Drag::Zoom { anchor: start_px, last_px: start_px },
                        _ => Drag::None,
                    }
                }
                Button::Left if matches!(self.tool, Tool::Select) => {
                    // Alt+drag copies: a duplicate stays, the dragged one moves
                    // (a view label: a Note copy moves, live u-952).
                    match e.modifiers.alt.then(|| self.alt_copy_label(pick, start, cx)).flatten() {
                        Some(d) => d,
                        None => {
                            if e.modifiers.alt {
                                self.alt_duplicate(pick, start, cx);
                            }
                            // Alt moves the copied thing whole: a selected note's side grip or a leader's handle under
                            // the press doesn't take it (ui-parity drw-copy-annotation-drag: the press on the note's
                            // left end resized it).
                            match self.start_drag(pick, start, !e.modifiers.alt) {
                                Drag::SelectBox { .. } if len(sub(lp, start_px)) <= BOX_START_PX => self.drag.clone(),
                                d => d,
                            }
                        }
                    }
                }
                _ => Drag::None,
            };
        }
        match self.drag.clone() {
            Drag::Zoom { anchor, last_px } => {
                // Drag zoom: device pixels / DRAG_ZOOM_FACTOR, up zooms in.
                let amount = -(lp[1] - last_px[1]) * window.scale_factor() as f64 / nav::DRAG_ZOOM_FACTOR;
                self.zoom_by(1.0 / nav::zoom_scale(amount), anchor, cx);
                self.drag = Drag::Zoom { anchor, last_px: lp };
            }
            Drag::Pan { last_px } => {
                self.center[0] -= (lp[0] - last_px[0]) / self.zoom;
                self.center[1] += (lp[1] - last_px[1]) / self.zoom;
                self.drag = Drag::Pan { last_px: lp };
                self.redraw(cx);
            }
            // Only the dragged view moves while the button is held (with what moves with it); the views aligned with it
            // follow on release (live drawing-feel-1118 drag2: Top stays while Front is dragged, then jumps).
            Drag::MoveViews { .. } if self.view_drag_cancelled => {}
            Drag::MoveViews { start, view, .. } => {
                let mut dr = self.drawing.clone();
                if let Some(s) = dr.sheets.get_mut(self.sheet)
                    && let Some((_, p)) = noshape_model::drawing::dragged_view_positions(s, view, sub(self.cursor, start)).into_iter().find(|(v, _)| *v == view)
                    && let Some(old) = s.view(view).map(|v| v.position)
                {
                    let m = sub(p, old);
                    if let Some(x) = s.view_mut(view) {
                        x.position = p;
                    }
                    for a in s.annotations.iter_mut().filter(|a| a.moves_with_view() == Some(view)) {
                        a.position = [a.position[0] + m[0], a.position[1] + m[1]];
                    }
                }
                self.set_preview(Some(dr), cx);
            }
            Drag::SelectCells { .. } => self.update_preview(cx),
            Drag::SelectBox { .. } => cx.notify(),
            Drag::MoveTable { .. } | Drag::ResizeTable { .. } | Drag::TableDivider { .. } | Drag::MoveBreak { .. } | Drag::MoveSectionLine { .. } | Drag::MoveDetailCircle { .. } | Drag::ResizeDetailCircle { .. } | Drag::MoveDetailLabel { .. } | Drag::MoveSectionPoint { .. } | Drag::AuxIndicator { .. } | Drag::NoteGrip { .. } | Drag::MoveViewLabel { .. } => {
                let drag = self.drag.clone();
                self.extra_drag_move(&drag, cx);
            }
            Drag::LeaderBend { .. } => {
                let drag = self.drag.clone();
                self.leader_bend_move(&drag, cx);
            }
            Drag::LeaderEnd { .. } => {
                let drag = self.drag.clone();
                self.leader_end_move(&drag, cx);
            }
            Drag::LeaderNode { .. } => {
                let drag = self.drag.clone();
                self.leader_node_move(&drag, cx);
            }
            // A datum from the model moves along its face normal only.
            Drag::MoveAnnotation { start, orig, .. } if orig.model.is_some() => self.drag_model_datum(&orig, start, cx),
            Drag::MoveAnnotation { start, orig, .. } => {
                self.infer_lines.clear();
                if !e.modifiers.shift {
                    // Inference on the dragged annotation's own position.
                    let offset = sub(orig.position, start);
                    let raw = self.cursor;
                    self.cursor = [raw[0] + offset[0], raw[1] + offset[1]];
                    self.infer_placement(Some(orig.id));
                    self.cursor = [self.cursor[0] - offset[0], self.cursor[1] - offset[1]];
                }
                let a = self.dragged_annotation(&orig, start);
                let mut dr = self.drawing.clone();
                // Group annotation: the other members follow (not with Shift).
                if let (Some(g), false) = (orig.ann_group, e.modifiers.shift)
                    && let Some(s) = dr.sheets.get_mut(self.sheet)
                {
                    let d = sub(a.position, orig.position);
                    for x in s.annotations.iter_mut().filter(|x| x.ann_group == Some(g) && x.id != orig.id) {
                        x.position = [x.position[0] + d[0], x.position[1] + d[1]];
                    }
                }
                if let Some(s) = dr.sheets.get_mut(self.sheet)
                    && let Some(x) = s.annotations.iter_mut().find(|x| x.id == a.id)
                {
                    *x = a;
                }
                self.set_preview(Some(dr), cx);
            }
            Drag::CenterlineEnd { orig, k } => {
                let a = self.dragged_centerline(&orig, k);
                let mut dr = self.drawing.clone();
                if let Some(x) = dr.sheets.get_mut(self.sheet).and_then(|s| s.annotations.iter_mut().find(|x| x.id == a.id)) {
                    x.kind = a.kind;
                }
                self.set_preview(Some(dr), cx);
            }
            Drag::AutoCentermarkArm { arm, .. } => {
                let ext = self.pk().dragged_auto_centermark(&arm);
                self.drag = Drag::AutoCentermarkArm { arm, refused: ext.is_none() };
                let preview = ext.map(|ext| {
                    let mut dr = self.drawing.clone();
                    if let Some(v) = dr.sheets.get_mut(self.sheet).and_then(|s| s.view_mut(arm.view)) {
                        set_centermark_extension(v, arm.key, ext);
                    }
                    dr
                });
                self.set_preview(preview, cx);
            }
            Drag::CentermarkArm { orig, k, .. } => {
                let snap = if e.modifiers.shift { None } else { self.centermark_edge_snap(&orig, k).map(|s| s.1) };
                let a = self.dragged_centermark(&orig, k, e.modifiers.shift);
                self.drag = Drag::CentermarkArm { orig, k, snap };
                let mut dr = self.drawing.clone();
                if let Some(x) = dr.sheets.get_mut(self.sheet).and_then(|s| s.annotations.iter_mut().find(|x| x.id == a.id)) {
                    x.kind = a.kind;
                }
                self.set_preview(Some(dr), cx);
            }
            Drag::Reattach { orig, k } => {
                let a = self.reattached(&orig, k);
                let mut dr = self.drawing.clone();
                if let Some(x) = dr.sheets.get_mut(self.sheet).and_then(|s| s.annotations.iter_mut().find(|x| x.id == a.id)) {
                    (x.kind, x.refs, x.loose) = (a.kind, a.refs, a.loose);
                }
                self.set_preview(Some(dr), cx);
            }
            Drag::Jog { orig, k, far } => {
                let a = self.dragged_jog(&orig, k, far);
                let mut dr = self.drawing.clone();
                if let Some(x) = dr.sheets.get_mut(self.sheet).and_then(|s| s.annotations.iter_mut().find(|x| x.id == a.id)) {
                    x.jogs = a.jogs;
                }
                self.set_preview(Some(dr), cx);
            }
            Drag::MoveInspection(_) => self.update_hover(cx),
            Drag::MoveImage { id, start, orig } => {
                let d = sub(self.cursor, start);
                let mut dr = self.drawing.clone();
                if let Some(im) = dr.sheets[self.sheet].images.iter_mut().find(|i| i.id == id) {
                    im.pos = [orig[0] + d[0], orig[1] + d[1]];
                }
                self.set_preview(Some(dr), cx);
            }
            Drag::MoveSketch { id, start, orig } => {
                let dr = self.drag_sketch(id, &orig, sub(self.cursor, start));
                self.set_preview(Some(dr), cx);
            }
            _ => {
                self.update_hover(cx);
                self.inspection_hover(cx);
                // Edit section: the preview jog follows the pointer (qa
                // DWG-EDIT-SECTION-DIALOG); so does Paste as table's grid.
                if self.section_edit.is_some() || (self.paste_table.is_some() && matches!(self.tool, Tool::Select)) {
                    self.update_preview(cx);
                }
                if !matches!(self.tool, Tool::Select) {
                    // Placing a dimension or annotation: align with the others
                    // (Shift turns inferencing off).
                    self.infer_lines.clear();
                    if !e.modifiers.shift && ((self.pending_annotation().is_some() && !self.dimension_picking()) || self.placing_note() || self.placing_gdt()) {
                        // (A dimension being placed wakes no point: live2 drw-tools-1119, no ✕.)
                        if self.placing_note() || self.placing_gdt() {
                            self.wake_point_at_cursor();
                        }
                        self.infer_placement(None);
                    } else if !e.modifiers.shift && self.placing_table() {
                        self.infer_table_placement();
                    }
                    self.update_preview(cx);
                }
            }
        }
    }

    /// The border frame's corners (sheet mm).
    fn frame_corners(&self) -> [[f64; 2]; 4] {
        let [x0, y0, x1, y1] = self.current_sheet().size.frame_rect();
        [[x0, y0], [x1, y0], [x1, y1], [x0, y1]]
    }

    /// Placing a table with its fixed corner snapped on the frame's corner: the hollow orange square there, 14 px
    /// (live2 place-1118).
    fn table_corner_mark(&self, overlay: &mut Overlay) {
        if self.placing_table() && self.frame_corners().contains(&self.cursor) {
            overlay.marks.push((self.cursor, crate::drawing_scene::SnapMark::Box, self.tol_mm(14.0)));
        }
    }

    /// A table tool waiting for the placing click.
    pub fn placing_table(&self) -> bool {
        match &self.tool {
            Tool::Table { kind: TableTool::Custom, .. } => self.custom_dlg_placing(),
            Tool::Table { kind: TableTool::General | TableTool::Revision, .. } => true,
            Tool::Table { kind: TableTool::Hole, view: Some(_), .. } => self.hole_ready_to_place(),
            Tool::Table { view: Some(_), .. } => true,
            // A BOM or cut list from the dialog's source (no view picked).
            Tool::Table { kind: TableTool::Bom | TableTool::CutList, .. } => self.table_dlg_preview().is_some(),
            _ => false,
        }
    }

    /// Placing a table: the cursor (the table's fixed corner) snaps to the
    /// x / y of the border frame, the title block, views and other tables
    /// nearby, shown as dashed inference lines.
    pub fn infer_table_placement(&mut self) {
        let tol = self.tol_mm(6.0);
        let c = self.cursor;
        // The border frame's inner corners take the fixed corner first, within 10 px (live2 place-1118: at 5 and
        // 8.6 px, not at 12.8), marked by a hollow orange square (`table_corner_mark`).
        if let Some(k) = self.frame_corners().into_iter().filter(|k| len(sub(*k, c)) <= self.tol_mm(TABLE_CORNER_REACH_PX)).min_by(|a, b| len(sub(*a, c)).total_cmp(&len(sub(*b, c)))) {
            self.cursor = k;
            return;
        }
        let mut xs: Vec<f64> = Vec::new();
        let mut ys: Vec<f64> = Vec::new();
        if let Some(b) = &self.built {
            // The border frame's sides.
            let [x0, y0, x1, y1] = self.current_sheet().size.frame_rect();
            xs.extend([x0, x1]);
            ys.extend([y0, y1]);
            if let Some(r) = b.title_rect {
                xs.extend([r[0], r[2]]);
                ys.extend([r[1], r[3]]);
            }
            for p in &b.placements {
                xs.extend([p.min[0], p.max[0]]);
                ys.extend([p.min[1], p.max[1]]);
            }
            for (_, l) in &b.tables {
                xs.extend([l.min()[0], l.max()[0]]);
                ys.extend([l.min()[1], l.max()[1]]);
            }
        }
        let best = |v: &[f64], x: f64| v.iter().copied().filter(|a| (a - x).abs() < tol).min_by(|a, b| (a - x).abs().total_cmp(&(b - x).abs()));
        let mut snapped = c;
        if let Some(x) = best(&xs, c[0]) {
            snapped[0] = x;
        }
        if let Some(y) = best(&ys, c[1]) {
            snapped[1] = y;
        }
        if snapped[0] != c[0] {
            self.infer_lines.push(([snapped[0], snapped[1] + 40.0], snapped));
        }
        if snapped[1] != c[1] {
            self.infer_lines.push(([snapped[0] - 40.0, snapped[1]], snapped));
        }
        self.cursor = snapped;
    }

    /// Onshape's placement inferencing: the cursor snaps to the x / y of
    /// other dimensions and annotations nearby and of the views' vertices
    /// and circle centers (help D23: lines near other entities), shown as
    /// red dashed lines.
    /// The Note tool with its "NOTE" preview at the pointer (no leader
    /// pending, no note being typed), and Insert image's corners: they infer
    /// placement too (live request 1662; EX17).
    fn placing_note(&self) -> bool {
        (matches!(self.tool, Tool::Note) && self.note_leader.is_none() && !matches!(self.panel, Some(Panel::Note { .. }))) || matches!(self.tool, Tool::PlaceImage { .. })
    }

    /// How far (screen px) the pointer moves before a press drags (live2 feel/drawing.json D5: a view or a
    /// dimension's text from 5 px, a note's text from 8; 4 and 6 are clicks).
    fn drag_threshold_px(&self, button: Button, pick: Option<u32>, start: [f64; 2]) -> f64 {
        if !matches!(button, Button::Left) || !matches!(self.tool, Tool::Select) {
            return 3.0;
        }
        let note = |id: u64| self.current_sheet().annotations.iter().any(|a| a.id == id && matches!(a.kind, AnnotationKind::Note { .. }));
        match self.sel_at(pick, start) {
            Some(Sel::Annotation(id)) if note(id) => 7.0,
            _ => 4.0,
        }
    }

    /// After a dimension pick (a circle or an edge, not a point) every dimension of the sheet shows small orange +
    /// marks, snap points for the placement: a linear one at its value's centre and at its dimension line's end on the
    /// second extension line, the others at the value's centre; none on hover (live2 rad-note plus-*.png).
    pub(crate) fn dimension_pick_marks(&self) -> Vec<[f64; 2]> {
        let Tool::Dimension { picks, .. } = &self.tool else { return Vec::new() };
        if picks.first().is_none_or(|p| matches!(p, Pick::Point(_))) {
            return Vec::new();
        }
        let (Some(b), Some(sheet)) = (&self.built, self.drawing.sheets.get(self.sheet)) else { return Vec::new() };
        let mut out = Vec::new();
        for a in sheet.annotations.iter().filter(|a| a.kind.is_dimension()) {
            let mine = self.picks_of(Sel::Annotation(a.id));
            if let Some(t) = b.geometry.texts.iter().find(|t| mine.contains(&t.pick)) {
                // The box's middle, raised from the descender room to the capitals' middle.
                let q = noshape_model::drawing::font::text_box(t);
                let up = 0.15 * t.height;
                out.push([(q[0][0] + q[2][0]) / 2.0 - t.angle.sin() * up, (q[0][1] + q[2][1]) / 2.0 + t.angle.cos() * up]);
            }
            if matches!(a.kind, AnnotationKind::LinearDimension { .. })
                && let Some(g) = b.geometry.grips.iter().filter(|g| mine.contains(&g.pick) && g.kind == noshape_model::drawing::sheet::GripKind::End).nth(1)
            {
                out.push(g.pos);
            }
        }
        out
    }

    /// The Dimension tool still taking picks at the pointer (the click adds one, not places): no placement alignment.
    fn dimension_picking(&self) -> bool {
        match &self.tool {
            Tool::Dimension { kind, picks } => self.pk().dimension_takes_more(*kind, self.cursor, picks),
            _ => false,
        }
    }

    /// A new geometric tolerance's frame on the pointer (its panel open, not editing one).
    fn placing_gdt(&self) -> bool {
        matches!(self.tool, Tool::Gdt) && self.gdt_dlg.as_ref().is_some_and(|d| d.edit.is_none())
    }

    /// Placing an annotation: a view's point under the pointer wakes (live2 wake-1118: a red ✕ stays on it, and the
    /// placement lines up with it from then on); a view's empty inside wakes nothing.
    fn wake_point_at_cursor(&mut self) {
        let Some(p) = self.snap_at(self.cursor).and_then(|s| self.placement(s.view).map(|pl| pl.to_sheet(s.p))) else { return };
        if !self.woken_points.iter().any(|w| (w[0] - p[0]).abs() < 1e-9 && (w[1] - p[1]).abs() < 1e-9) {
            self.woken_points.push(p);
        }
    }

    pub fn infer_placement(&mut self, skip: Option<u64>) {
        let tol = self.tol_mm(INFER_REACH_PX);
        let c = self.cursor;
        // The committed sheet: the preview holds the annotation being
        // placed, at the cursor, which would align with itself.
        let Some(sheet) = self.drawing.sheets.get(self.sheet) else { return };
        let mut pts: Vec<[f64; 2]> = sheet.annotations.iter().filter(|a| Some(a.id) != skip).map(|a| a.position).chain(sheet.tables.iter().map(|t| t.position)).collect();
        // A tool placing lines up with the views' points it woke (live2 wake-1118: a hovered vertex, not every one); a
        // dragged annotation with all of them.
        if matches!(self.tool, Tool::Select) {
            for (view, g) in &self.geoms {
                if let Some(pl) = self.placement(*view) {
                    pts.extend(g.snaps.iter().map(|s| pl.to_sheet(s.p)));
                }
            }
        } else {
            pts.extend(self.woken_points.iter().copied());
            pts.extend(self.dimension_pick_marks());
        }
        let best = |axis: usize| pts.iter().filter(|p| (p[axis] - c[axis]).abs() <= tol).min_by(|a, b| (a[axis] - c[axis]).abs().total_cmp(&(b[axis] - c[axis]).abs())).copied();
        let (bx, by) = (best(0), best(1));
        let mut snapped = c;
        if let Some(p) = bx {
            snapped[0] = p[0];
        }
        if let Some(p) = by {
            snapped[1] = p[1];
        }
        if let Some(p) = bx {
            self.infer_lines.push((p, snapped));
        }
        if let Some(p) = by {
            self.infer_lines.push((p, snapped));
        }
        self.cursor = snapped;
    }

    pub fn on_mouse_up(&mut self, e: &MouseUpEvent, window: &mut Window, cx: &mut Context<Self>) {
        let lp = self.local(e.position);
        self.cursor = self.to_sheet(lp);
        if e.button == MouseButton::Left && self.zoom_window_up(cx) {
            return;
        }
        if e.button == MouseButton::Left && self.image_mouse_up(cx) {
            return;
        }
        // Placement inference also applies to the click that places.
        self.infer_lines.clear();
        if e.button == MouseButton::Left && !e.modifiers.shift && matches!(self.drag, Drag::Pending { .. }) && !matches!(self.tool, Tool::Select) && ((self.pending_annotation().is_some() && !self.dimension_picking()) || self.placing_note() || self.placing_gdt()) {
            self.infer_placement(None);
            self.infer_lines.clear();
        } else if e.button == MouseButton::Left && !e.modifiers.shift && matches!(self.drag, Drag::Pending { .. }) && self.placing_table() {
            self.infer_table_placement();
            self.infer_lines.clear();
        }
        let drag = std::mem::replace(&mut self.drag, Drag::None);
        match drag {
            Drag::Pending { button: Button::Left, pick, .. } => {
                if matches!(self.tool, Tool::Select) {
                    self.select_click(pick, e.modifiers, cx);
                } else {
                    self.tool_click(window, cx);
                }
            }
            Drag::Pending { button: Button::Right, pick, .. } => self.open_context_menu(pick, e.position, window, cx),
            Drag::MoveViews { .. } if std::mem::take(&mut self.view_drag_cancelled) => {
                self.set_preview(None, cx);
                self.selection.clear();
                self.redraw(cx);
            }
            Drag::MoveViews { start, view, fresh } => {
                if self.preview.take().is_some() {
                    let mut dr = self.drawing.clone();
                    if let Some(s) = dr.sheets.get_mut(self.sheet) {
                        noshape_model::drawing::drag_view(s, view, sub(self.cursor, start));
                    }
                    self.commit("Move view", dr, cx);
                }
                if fresh {
                    self.selection.clear();
                    self.redraw(cx);
                }
            }
            Drag::MoveAnnotation { orig, fresh, .. } => {
                if let Some(mut p) = self.preview.take() {
                    self.group_callout_on_drop(&orig, &mut p);
                    self.commit("Move annotation", p, cx);
                    if fresh {
                        self.selection.clear();
                        self.redraw(cx);
                    }
                }
            }
            Drag::Jog { .. } | Drag::Reattach { .. } | Drag::CentermarkArm { .. } | Drag::AutoCentermarkArm { .. } | Drag::CenterlineEnd { .. } => {
                if let Some(p) = self.preview.take() {
                    let name = match drag {
                        Drag::Jog { .. } => "Move jog",
                        Drag::Reattach { .. } => "Re-attach dimension",
                        Drag::CenterlineEnd { .. } => "Resize centerline",
                        _ => "Resize centermark",
                    };
                    self.commit(name, p, cx);
                }
            }
            Drag::MoveInspection(id) => self.drop_inspection(id, cx),
            Drag::MoveSketch { .. } | Drag::MoveImage { .. } => {
                if let Some(p) = self.preview.take() {
                    self.commit(if matches!(drag, Drag::MoveImage { .. }) { "Move image" } else { "Move sketch" }, p, cx);
                }
            }
            Drag::SelectCells { id, start } => self.select_cells_in(id, start, self.cursor, cx),
            // A box adds to the selection; Shift+drag makes none (live2 feel/drawing.json F3).
            Drag::SelectBox { start } if !e.modifiers.shift => self.select_in_box(start, self.cursor, true, cx),
            Drag::SelectBox { .. } => {}
            Drag::MoveTable { .. } | Drag::ResizeTable { .. } | Drag::TableDivider { .. } | Drag::MoveBreak { .. } | Drag::MoveSectionLine { .. } | Drag::MoveDetailCircle { .. } | Drag::ResizeDetailCircle { .. } | Drag::MoveDetailLabel { .. } | Drag::MoveSectionPoint { .. } | Drag::AuxIndicator { .. } | Drag::NoteGrip { .. } | Drag::MoveViewLabel { .. } | Drag::LeaderBend { .. } | Drag::LeaderEnd { .. } | Drag::LeaderNode { .. } => {
                let name = match drag {
                    Drag::LeaderBend { .. } | Drag::LeaderEnd { .. } | Drag::LeaderNode { .. } => "Move by grip",
                    Drag::MoveTable { .. } => "Move table",
                    Drag::TableDivider { .. } => "Resize cells",
                    Drag::ResizeTable { .. } => "Resize table",
                    Drag::MoveSectionLine { .. } => "Move cutting line",
                    Drag::MoveBreak { .. } => "Move break",
                    Drag::MoveSectionPoint { .. } => "Edit cutting line",
                    Drag::AuxIndicator { .. } => "Edit view reference indicator",
                    Drag::MoveViewLabel { .. } => "Move view label",
                    Drag::NoteGrip { grip: 2, .. } => "Rotate note",
                    Drag::NoteGrip { .. } => "Resize note",
                    _ => "Move detail circle",
                };
                if let Some(mut p) = self.preview.take() {
                    // A section line's end pulled inside the part makes a
                    // partial section, pulled out again a full one (help V21).
                    if let Drag::MoveSectionPoint { view, .. } = drag {
                        self.section_partial_by_ends(&mut p, view);
                    }
                    self.commit(name, p, cx);
                }
            }
            _ => {}
        }
        cx.notify();
    }

    /// A line following its model point was dragged where it is drawn: its
    /// anchor moves with it; a lost one dropped on a model point of the
    /// parent re-attaches there (live request 1625).
    pub(crate) fn rebase_cut_anchor(&self, d: &mut Drawing, view: ViewId) {
        // As the line was when the drag started (the build since is the
        // drag's preview).
        let Some((_, lost, drawn)) = self.cut_drag.clone().filter(|c| c.0 == view) else { return };
        let stored = self.current_sheet().section_points(view);
        let parent = self.current_sheet().parent_of(view);
        let snap = self.snap_at(self.cursor).filter(|p| Some(p.view) == parent && p.r.is_some());
        let Some(v) = d.sheets[self.sheet].view_mut(view) else { return };
        if lost {
            if let Some(p) = snap {
                v.cut_anchor = p.r.map(|r| (r, p.p));
            }
            return;
        }
        if let (Some((_, at)), Some(s)) = (&mut v.cut_anchor, stored)
            && let (Some(s0), Some(r0)) = (s.first(), drawn.first())
        {
            *at = [at[0] + r0[0] - s0[0], at[1] + r0[1] - s0[1]];
        }
    }

    /// A straight section view's `partial` from its cutting line's ends in
    /// `d`: partial when an end lies inside the part (a closed region of the
    /// parent view's visible edges).
    fn section_partial_by_ends(&self, d: &mut Drawing, view: ViewId) {
        let sheet = &d.sheets[self.sheet];
        let Some(v) = sheet.view(view) else { return };
        let ViewKind::Section { parent, line, .. } = &v.kind else { return };
        let (Some(a), Some(b)) = (line.first(), line.last()) else { return };
        let Some(pl) = self.placement(*parent) else { return };
        let pk = self.pk();
        let inside = |p: &[f64; 2]| pk.view_region_at(*parent, pl.to_sheet(*p)).is_some();
        let partial = inside(a) || inside(b);
        if let Some(v) = d.sheets[self.sheet].view_mut(view) {
            v.partial = partial;
        }
    }

    /// Wheel / two-finger scroll: zoom about the pointer with Onshape's amounts
    /// (navigation.rs); horizontal scrolls do nothing.
    pub fn on_scroll(&mut self, e: &ScrollWheelEvent, _window: &mut Window, cx: &mut Context<Self>) {
        let lp = self.local(e.position);
        let reverse = nav::MouseControls::current().reverse_scroll;
        let Some(amount) = nav::drawing_wheel_zoom_amount(e, nav::Platform::current(), reverse) else { return };
        self.zoom_by(1.0 / nav::zoom_scale(amount), lp, cx);
    }

    /// Trackpad pinch: one wheel notch per pinch update about the pointer,
    /// as in 3D (`nav::pinch_zoom_amount`).
    pub fn on_pinch(&mut self, e: &PinchEvent, window: &mut Window, cx: &mut Context<Self>) {
        let reverse = nav::MouseControls::current().reverse_scroll;
        let Some(amount) = nav::pinch_zoom_amount(e, nav::Platform::current(), nav::is_x11(window), reverse) else { return };
        let lp = self.local(e.position);
        self.zoom_by(1.0 / nav::zoom_scale(amount), lp, cx);
    }

    pub fn on_key(&mut self, e: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let k = &e.keystroke;
        match crate::drawing_keys::control_key(&k.modifiers, &k.key) {
            Some(crate::drawing_keys::ControlKey::MaxMin) if self.panel.is_none() => {
                self.set_tool(Tool::Dimension { kind: DimTool::MaxMin, picks: vec![] }, cx);
                cx.stop_propagation();
                return;
            }
            Some(crate::drawing_keys::ControlKey::Sheets) => {
                self.toggle_sheets_panel(cx);
                cx.stop_propagation();
                return;
            }
            _ => {}
        }
        // Typing in a dialog's text box (focus inside the editor, not on it)
        // never triggers drawing shortcuts.
        if k.modifiers.control || k.modifiers.platform || k.modifiers.alt || self.panel.is_some() || (self.focus.contains_focused(window, cx) && !self.focus.is_focused(window)) {
            return;
        }
        // Shift+Q: midpoints and quad points on / off for the current dimension command.
        if k.modifiers.shift && k.key == "q" && matches!(self.tool, Tool::Dimension { .. }) {
            self.snap_mid_quad = !self.snap_mid_quad;
            self.status = Some(if self.snap_mid_quad { "Midpoints and quad points on".into() } else { "Midpoints and quad points off".into() });
            self.update_preview(cx);
            cx.stop_propagation();
            return;
        }
        match k.key.as_str() {
            "enter" => {
                // Crop view: Enter closes a profile being drawn, else applies
                // the pending one (✓).
                if !matches!(self.tool, Tool::CropPoly { parent: Some(_), .. }) && self.crop_enter(cx) {
                    cx.stop_propagation();
                    return;
                }
                self.confirm(window, cx);
                cx.stop_propagation();
                return;
            }
            "pagedown" | "pageup" | "home" | "end" => {
                let n = self.drawing.sheets.len();
                let i = match k.key.as_str() {
                    "pagedown" => (self.sheet + 1).min(n - 1),
                    "pageup" => self.sheet.saturating_sub(1),
                    "home" => 0,
                    _ => n - 1,
                };
                self.switch_sheet(i, cx);
                cx.stop_propagation();
                return;
            }
            _ => {}
        }
        let tool = match (k.key.as_str(), k.modifiers.shift) {
            ("d", false) => Tool::Dimension { kind: DimTool::Linear, picks: vec![] },
            ("d", true) => Tool::Dimension { kind: DimTool::Diameter, picks: vec![] },
            ("r", true) => Tool::Dimension { kind: DimTool::Radius, picks: vec![] },
            ("h", true) => Tool::Dimension { kind: DimTool::Foreshorten, picks: vec![] },
            // Drawing sketch tools and constraints (Onshape's drawing shortcuts).
            ("l", false) => Tool::Sketch { tool: crate::drawing_sketch::SkTool::Line, pts: vec![], from: None },
            ("c", false) => Tool::Sketch { tool: crate::drawing_sketch::SkTool::Circle, pts: vec![], from: None },
            ("g", false) => Tool::Sketch { tool: crate::drawing_sketch::SkTool::CornerRect, pts: vec![], from: None },
            ("r", false) => Tool::Sketch { tool: crate::drawing_sketch::SkTool::CenterRect, pts: vec![], from: None },
            ("m", false) => Tool::Sketch { tool: crate::drawing_sketch::SkTool::Trim, pts: vec![], from: None },
            ("h", false) => Tool::Constrain { kind: crate::drawing_sketch::SkCon::Horizontal, picks: vec![] },
            ("v", false) => Tool::Constrain { kind: crate::drawing_sketch::SkCon::Vertical, picks: vec![] },
            ("i", false) => Tool::Constrain { kind: crate::drawing_sketch::SkCon::Coincident, picks: vec![] },
            ("l", true) => Tool::Constrain { kind: crate::drawing_sketch::SkCon::Perpendicular, picks: vec![] },
            ("b", false) => Tool::Constrain { kind: crate::drawing_sketch::SkCon::Parallel, picks: vec![] },
            ("t", false) => Tool::Constrain { kind: crate::drawing_sketch::SkCon::Tangent, picks: vec![] },
            _ => return,
        };
        self.set_tool(tool, cx);
        cx.stop_propagation();
    }

    fn select_click(&mut self, pick: Option<u32>, m: Modifiers, cx: &mut Context<Self>) {
        let armed = std::mem::take(&mut self.copy_armed);
        if self.linestyle_click(cx) || self.hide_edges_click(cx) || self.section_edit_click(cx) || self.replicate_click(cx) {
            return;
        }
        // A click away (on the empty sheet too) closes the palette, keeping its changes (live2 REQUESTS 11-14 00:43,
        // drawing.json dimensionPalette.clickOutside_1114).
        if self.palette.is_some() {
            self.close_palette(true, cx);
            return;
        }
        if self.place_paste_table(cx) {
            return;
        }
        // Exactly on a note or callout leader: it, with that leader selected.
        self.leader_sel = None;
        if let Some(crate::drawing_leaders::LeaderHit::Stroke { id, leader, .. } | crate::drawing_leaders::LeaderHit::Node { id, leader, .. }) = self.leader_hit(self.cursor)
            && !m.shift
        {
            self.selection = vec![Sel::Annotation(id)];
            self.leader_sel = Some((id, leader));
            self.redraw(cx);
            return;
        }
        let sel = self.select_target(pick, self.cursor);
        if self.table_cell_click(sel, m.shift, cx) {
            return;
        }
        // Onshape: selection is additive with a plain click (clicking a
        // selected entity again deselects it); clicking empty space clears, but not the first click after Ctrl+C.
        match sel {
            Some(s) => {
                if let Some(i) = self.selection.iter().position(|x| *x == s) {
                    self.selection.remove(i);
                } else {
                    self.selection.push(s);
                }
            }
            None if armed => {}
            None => self.selection.clear(),
        }
        self.status = None;
        self.redraw(cx);
    }

    /// Alt+drag (Onshape: copy the hovered entity): leave a duplicate in place.
    fn alt_duplicate(&mut self, pick: Option<u32>, start: [f64; 2], cx: &mut Context<Self>) {
        let Some(sel) = self.sel_at(pick, start) else { return };
        match sel {
            // A dimension is only moved (live 2313: Alt+drag copies a note, not a dimension).
            Sel::Annotation(id) if self.current_sheet().annotations.iter().any(|a| a.id == id && a.kind.is_dimension()) => {}
            Sel::Annotation(id) => self.edit_sheet("Copy annotation", cx, |s| {
                if let Some(mut a) = s.annotations.iter().find(|a| a.id == id).cloned() {
                    a.id = s.next_annotation_id();
                    s.annotations.push(a);
                }
            }),
            Sel::Table(id) => self.edit_sheet("Copy table", cx, |s| {
                if let Some(mut t) = s.tables.iter().find(|t| t.id == id).cloned() {
                    t.id = s.next_table_id();
                    s.tables.push(t);
                }
            }),
            Sel::View(id) | Sel::Edge(id, _) => {
                if let Ok(clip) = self.drawing.copy_views(self.sheet, &[id], Some(&self.sources)) {
                    self.edit_sheet("Copy view", cx, |s| {
                        // The copy keeps the original's place: paste around the same center.
                        let n = clip.sheet.views.len().max(1) as f64;
                        let c = clip.sheet.views.iter().fold([0.0, 0.0], |acc, v| [acc[0] + v.position[0] / n, acc[1] + v.position[1] / n]);
                        s.paste_views(&clip, Some(c));
                    });
                }
            }
            Sel::Break(..) | Sel::Sketch(_) | Sel::Hatch(_) | Sel::Image(_) | Sel::AutoCentermark(..) | Sel::ViewLabel(_) | Sel::AuxIndicator(_) | Sel::CutLine(_) | Sel::HoleAxis(_) | Sel::FormatLine(_) => {}
        }
    }

    /// The drag a press at `start` begins; `handles`: a selected note's grips and leaders' handles can take it.
    fn start_drag(&mut self, pick: Option<u32>, start: [f64; 2], handles: bool) -> Drag {
        if handles && let Some(d) = self.leader_bend_drag(start).or_else(|| self.leader_node_drag(start)).or_else(|| self.leader_end_drag(start)) {
            return d;
        }
        if handles && let Some(d) = self.note_grip_drag(start) {
            return d;
        }
        if let Some(d) = self.extra_start_drag(pick, start) {
            return d;
        }
        if let Some(id) = self.inspection_at(pick) {
            return Drag::MoveInspection(id);
        }
        if let Some((orig, k, far)) = self.jog_grip_at(start) {
            return Drag::Jog { orig, k, far };
        }
        if let Some((orig, k)) = self.attach_grip_at(start) {
            return Drag::Reattach { orig, k };
        }
        if let Some((orig, k)) = self.centermark_grip_at(start) {
            return Drag::CentermarkArm { orig, k, snap: None };
        }
        if let Some(arm) = self.pk().auto_centermark_grip_at(start) {
            return Drag::AutoCentermarkArm { arm, refused: false };
        }
        if let Some((orig, k)) = self.centerline_grip_at(start) {
            return Drag::CenterlineEnd { orig, k };
        }
        if let Some(orig) = self.centerline_middle_at(start) {
            return Drag::MoveAnnotation { start, orig, fresh: false };
        }
        // A hole's circle lights its centre mark but a press on it drags the view (live2 drw-centermark t5).
        if matches!(self.tool, Tool::Select)
            && let Some(view) = self.pk().hole_press_view(start)
        {
            let fresh = !self.selection.contains(&Sel::View(view));
            if fresh {
                self.selection = vec![Sel::View(view)];
            }
            return Drag::MoveViews { start, view, fresh };
        }
        // Nothing there: a box select (qa DRAWING-BOX-SELECT).
        let Some(sel) = self.sel_at(pick, start) else { return Drag::SelectBox { start } };
        let fresh = !self.selection.contains(&sel);
        if fresh {
            self.selection = vec![sel];
        }
        let sheet = &self.drawing.sheets[self.sheet];
        match sel {
            // (Live: a view moves by its edges too; the views aligned with it follow: `dragged_view_positions`.)
            Sel::View(id) | Sel::Edge(id, _) => Drag::MoveViews { start, view: id, fresh },
            Sel::Annotation(aid) => match self.built.as_ref().and_then(|b| b.resolved.annotations.iter().find(|a| a.id == aid).cloned()) {
                Some(a) if !matches!(a.kind, AnnotationKind::Centermark { .. } | AnnotationKind::Centerline { .. }) => Drag::MoveAnnotation { start, fresh: fresh && a.kind.is_dimension(), orig: a },
                _ => Drag::None,
            },
            // Drag a sketch entity (its points move, the sketch solves); on a
            // spline's own point only that point moves, reshaping it (help
            // drawing_sketch_tools: "Drag points to reshape").
            Sel::Sketch(id) => {
                let sk = &sheet.sketch.sketch;
                let spline = sk.entity(noshape_sketch::EntityId(id)).is_some_and(|e| matches!(e.geometry, noshape_sketch::Geometry::FitSpline { .. } | noshape_sketch::Geometry::BSpline { .. }));
                let point = self.sketch_point_at(start).map(|p| p.0).filter(|p| spline && crate::drawing_sketch::entity_points(sk, noshape_sketch::EntityId(id)).contains(p));
                Drag::MoveSketch { id: point.map(|p| p.0).unwrap_or(id), start, orig: sheet.sketch.clone() }
            }
            Sel::Image(id) => match sheet.images.iter().find(|i| i.id == id) {
                Some(im) => Drag::MoveImage { id, start, orig: im.pos },
                None => Drag::None,
            },
            Sel::ViewLabel(v) => self.label_drag(v, start),
            // Dragging the indicator's line (off its dots) does nothing (live u-914).
            Sel::Table(_) | Sel::Break(..) | Sel::Hatch(_) | Sel::AutoCentermark(..) | Sel::AuxIndicator(_) | Sel::CutLine(_) | Sel::HoleAxis(_) | Sel::FormatLine(_) => Drag::None,
        }
    }

    /// The annotation `orig` (resolved) dragged from `start` to the cursor.
    fn dragged_annotation(&self, orig: &Annotation, start: [f64; 2]) -> Annotation {
        self.pk().dragged_annotation(orig, start)
    }

    /// The pointer is over a dimension.
    /// A dimension or a view under the pointer: their double-click is live's 200 ms one.
    fn timed_double_click_target(&mut self, cx: &mut Context<Self>) -> bool {
        self.update_hover(cx);
        match self.hover.and_then(|h| self.built.as_ref().and_then(|b| b.picks.get(h as usize)).cloned()) {
            Some(PickTarget::Annotation(id)) => self.current_sheet().annotations.iter().any(|a| a.id == id && a.kind.is_dimension()),
            Some(PickTarget::View(_) | PickTarget::Edge { .. }) => true,
            _ => false,
        }
    }

    /// A double click opens what's under the pointer; a triple click on a
    /// note opens it with all its text selected (help H:note).
    fn double_click(&mut self, triple: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.update_hover(cx);
        let Some(h) = self.hover else { return };
        match self.built.as_ref().and_then(|b| b.picks.get(h as usize)).cloned() {
            Some(PickTarget::TitleBlock) => self.open_title_panel(window, cx),
            Some(PickTarget::TitleField(key)) => self.open_title_field(key, window, cx),
            Some(PickTarget::Inspection(id)) => self.edit_inspection(id, window, cx),
            Some(PickTarget::Annotation(id)) => {
                let note = self.drawing.sheets[self.sheet].annotations.iter().find(|a| a.id == id).cloned();
                match note {
                    Some(Annotation { kind: AnnotationKind::Note { .. }, position, .. }) => {
                        self.open_note_panel(Some(id), position, window, cx);
                        // After the editor takes the focus (deferred in `textarea`).
                        if triple && let Some(Panel::Note { input, .. }) = &self.panel {
                            let input = input.downgrade();
                            window.defer(cx, move |window, cx| {
                                if let Some(input) = input.upgrade() {
                                    input.update(cx, |s, cx| s.select_all(window, cx));
                                }
                            });
                        }
                    }
                    Some(Annotation { kind: AnnotationKind::Weld { .. }, .. }) => self.open_weld_edit(id, window, cx),
                    Some(Annotation { kind: AnnotationKind::EdgeCondition { .. }, .. }) => self.open_edge_condition_edit(id, window, cx),
                    Some(a) => self.open_annotation_panel(&a, window, cx),
                    None => {}
                }
            }
            Some(PickTarget::Table(id)) => {
                // On a divider: fit it to the text; in a cell: edit the cell;
                // on a corner: the table's properties (help H:table: "or by
                // double-clicking a corner"), the dialog its menu opens.
                let kind = self.current_sheet().table(id).map(|t| t.kind.clone());
                if self.table_corner_at(id) && !matches!(kind, Some(noshape_model::drawing::TableKind::Revision { .. }) | None) {
                    use noshape_model::drawing::TableKind as K;
                    match kind {
                        Some(K::Custom { .. }) => self.open_custom_props(id, cx),
                        Some(K::Bom { .. } | K::CutList { .. }) => self.open_table_props(id, cx),
                        Some(K::Hole { .. }) => self.open_hole_table_props(id, cx),
                        _ => self.open_table_props_corner(id, cx),
                    }
                // A cell border opens the Cell panel like a cell (live 2009);
                // the resize grips fit (live 1607 (b)).
                } else if !self.fit_table_grip(id, cx) {
                    self.open_cell_panel(id, window, cx)
                }
            }
            Some(t) => {
                // Onshape: double-clicking a view opens View properties (with
                // several selected: just that view's, live drw2 10-21 j).
                if let Some(v) = t.view() {
                    self.selection = vec![Sel::View(v)];
                    self.open_view_props(v, window, cx);
                }
            }
            None => {}
        }
    }

    // ------------------------------------------------------------ context menu

    fn open_context_menu(&mut self, pick: Option<u32>, position: Point<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        // Live (request 1596, qa DWG-SHEET-PROPS-ESC): no context menu while
        // Sheet properties is open.
        if self.sheet_props.is_some() {
            return;
        }
        if !matches!(self.tool, Tool::Select) {
            self.cancel(cx);
            return;
        }
        self.menu_at = self.local(position);
        let entries = self.context_menu_entries(pick, cx);
        // The pointer is on the menu now: the canvas hover drops, so a selected leader shows orange, not tan (live2
        // drw-leader-add-node s10).
        if self.hover.take().is_some() {
            self.redraw(cx);
        }
        let ed = cx.entity();
        // Onshape's right-click menu (os_menu: 12 px text, a drawing's 23.5 px rows).
        // Esc on a table cell's menu closes the Cell panel too and clears the selection (live2 drw-hole-table-cell-rmb
        // s20: nothing read lit after it).
        let cell = self.cell_sel.is_some().then(|| ed.downgrade());
        let menu = crate::os_menu::OsMenu::build(window, cx, move |m, window, cx| {
            let m = match cell {
                Some(ed) => m.after_escape(move |_, cx| {
                    ed.update(cx, |t, cx| {
                        t.cell_sel = None;
                        t.selection.clear();
                        t.redraw(cx);
                    })
                    .ok();
                }),
                None => m,
            };
            fill_os_menu(m.drawing_context(), &entries, &ed, window, cx)
        });
        if !menu.read(cx).is_empty() {
            self.frame_menu = Some(menu.downgrade());
            crate::drawing_view::open_canvas_menu(position, menu, window, cx);
        }
        cx.notify();
    }

    /// The right-click menu's entries for what `pick` (at the cursor) is, the selection set to it as live does.
    pub(crate) fn context_menu_entries(&mut self, pick: Option<u32>, cx: &mut Context<Self>) -> Vec<MenuEntry> {
        let mut entries = self.context_menu_rows(pick, cx);
        // Paste is listed while drawing content is on the clipboard (live's menus from a fresh session have none,
        // dom/context-menus-drawing-2.txt; with a view copied every one starts so, behaviours 1736).
        if crate::drawing_props::view_clipboard(cx).is_none() {
            entries.retain(|e| !matches!(e, MenuEntry::Item { label, .. } if label.as_ref() == "Paste"));
        }
        // No separator at either end or twice in a row (live: a centerline's menu starts with Clear selection).
        let mut out: Vec<MenuEntry> = Vec::with_capacity(entries.len());
        for e in entries {
            if matches!(e, MenuEntry::Separator) && out.last().is_none_or(|l| matches!(l, MenuEntry::Separator)) {
                continue;
            }
            out.push(e);
        }
        while out.last().is_some_and(|l| matches!(l, MenuEntry::Separator)) {
            out.pop();
        }
        out
    }

    fn context_menu_rows(&mut self, pick: Option<u32>, cx: &mut Context<Self>) -> Vec<MenuEntry> {
        self.placed_table = None;
        let mut sel = self.select_target(pick, self.cursor);
        // The empty sheet with one note selected: the note's menu (live2 drw-note-copy-paste s8).
        if sel.is_none()
            && let [Sel::Annotation(id)] = self.selection[..]
            && self.current_sheet().annotations.iter().any(|a| a.id == id && matches!(a.kind, AnnotationKind::Note { .. }))
        {
            sel = Some(Sel::Annotation(id));
        }
        // A section line's menu leaves the selection as it was (live drw-section-flip-direction: nothing orange).
        let on_cut_line = pick.and_then(|p| self.built.as_ref()?.picks.get(p as usize)).is_some_and(|t| matches!(t, PickTarget::SectionLine(_)));
        if let Some(s) = sel
            && !on_cut_line
            && !self.selection.contains(&s)
        {
            self.selection = vec![s];
            self.redraw(cx);
        }
        // Exactly on a note or callout leader: its own menu (live drw2 10-21
        // g (3), (4), (6)).
        // On a detail view's circle in its parent: the circle's menu (live
        // asb 10-23 views (2)).
        let target = pick.and_then(|p| self.built.as_ref()?.picks.get(p as usize).cloned());
        let circle = target.as_ref().and_then(|t| if let PickTarget::DetailCircle(v) = t { Some(*v) } else { None });
        let cut_line = target.as_ref().and_then(|t| if let PickTarget::SectionLine(v) = t { Some(*v) } else { None });
        // A title block field is a note (live qa4 round 5 s11 (2)): its menu
        // starts with Edit note….
        let field = target.as_ref().and_then(|t| if let PickTarget::TitleField(k) = t { Some(k.clone()) } else { None });
        // Live 2226: an inspection bubble has its own menu; the inspected annotation's has none of its rows.
        let bubble = target.as_ref().and_then(|t| if let PickTarget::Inspection(id) = t { Some(*id) } else { None });
        let bubble = bubble.and_then(|id| self.drawing.sheets[self.sheet].annotations.iter().find(|a| a.id == id).cloned());
        self.menu_broken_out = target.and_then(|t| if let PickTarget::BrokenOut { view, index } = t { Some((view, index)) } else { None });
        // Live: a broken-out section's hatch has its menu too (anywhere inside its profile).
        if self.menu_broken_out.is_none() {
            let at = self.cursor;
            self.menu_broken_out = self.current_sheet().views.iter().find_map(|v| {
                let q = self.placement(v.id)?.to_view(at);
                let index = v.broken_out.iter().position(|b| noshape_model::drawing::views::point_in_polygon(q, &noshape_model::drawing::views::broken_out_outline(b)))?;
                Some((v.id, index))
            });
        }
        match (self.leader_hit(self.cursor), circle, cut_line, field) {
            _ if bubble.is_some() => bubble.map(|a| self.inspection_entries(&a)).unwrap_or_default(),
            (Some(hit), ..) => self.leader_entries(hit, cx),
            (None, Some(v), ..) => self.detail_circle_entries(v, cx),
            (None, None, Some(v), _) => self.section_line_entries(v, cx),
            (None, None, None, Some(key)) => {
                let (k2, k3, k4, k5, k6, k7) = (key.clone(), key.clone(), key.clone(), key.clone(), key.clone(), key.clone());
                // Live 1992: a field is a note, with the note's menu. Copy is
                // a note with the field's text; Delete leaves the cell empty;
                // Explode note, Add leader, Move to and the order make the
                // field a note of its own first (S18).
                // (Its own layer, Title block, listed greyed: live2 title-line-edit.)
                let layers: Vec<MenuEntry> = noshape_model::drawing::SheetLayer::FORMAT
                    .into_iter()
                    .chain([noshape_model::drawing::SheetLayer::Drawing])
                    .map(|l| {
                        let k = key.clone();
                        MenuEntry::item(l.name(), move |t, _, cx| {
                            t.with_field_note(&k, "Move to", cx, |s, id| s.annotations.iter_mut().filter(|a| a.id == id).for_each(|a| a.layer = l));
                        })
                        .disabled(l == noshape_model::drawing::SheetLayer::TitleBlock)
                    })
                    .collect();
                // With something else selected the field joins it: the rows both have, the field's Add leader, Move to
                // ▸ only with notes (live2 rmb-probe 11-15: a datum, note, dimension or view selected).
                if !self.selection.is_empty() {
                    let sheet = self.current_sheet();
                    let kind = |s: &Sel| match s {
                        Sel::Annotation(id) => sheet.annotations.iter().find(|a| a.id == *id).map(|a| &a.kind),
                        _ => None,
                    };
                    let dims = self.selection.iter().any(|s| kind(s).is_some_and(|k| k.is_dimension()));
                    let notes = self.selection.iter().all(|s| matches!(kind(s), Some(AnnotationKind::Note { .. })));
                    let mut out = vec![crate::drawing_props::paste_always()];
                    if !dims {
                        out.push(MenuEntry::item("Copy", |t, _, cx| t.copy_entities(cx)));
                    }
                    out.push(MenuEntry::item("Paste as table", |t, _, cx| {
                        t.paste_as_table(cx);
                    }));
                    out.push(MenuEntry::item("Add leader", move |t, _, cx| {
                        if let Some(id) = t.with_field_note(&k5, "Edit note", cx, |_, _| {}) {
                            t.note_add_leader(id, cx);
                        }
                    }));
                    if notes {
                        out.push(MenuEntry::Submenu("Move to".into(), layers));
                    }
                    out.extend([
                        MenuEntry::Separator,
                        MenuEntry::item("Clear selection", |t, _, cx| {
                            t.selection.clear();
                            t.redraw(cx);
                        }),
                        MenuEntry::item("Zoom to fit", |t, _, cx| t.zoom_to_fit(cx)),
                        MenuEntry::item("Delete", |t, _, cx| t.delete_selection(cx)),
                    ]);
                    return out;
                }
                vec![
                    MenuEntry::item("Edit note...", move |t, w, cx| t.open_title_field(key.clone(), w, cx)),
                    MenuEntry::item("Explode note", move |t, _, cx| {
                        t.with_field_note(&k4, "Explode note", cx, |s, id| {
                            if let Some(AnnotationKind::Note { style, .. }) = s.annotations.iter_mut().find(|a| a.id == id).map(|a| &mut a.kind) {
                                style.explode = true;
                            }
                        });
                    }),
                    crate::drawing_props::paste_always(),
                    MenuEntry::item("Copy", move |t, _, cx| t.copy_title_field(&k2, cx)),
                    MenuEntry::item("Paste as table", |t, _, cx| {
                        t.paste_as_table(cx);
                    }),
                    MenuEntry::item("Add leader", move |t, _, cx| {
                        if let Some(id) = t.with_field_note(&k5, "Edit note", cx, |_, _| {}) {
                            t.note_add_leader(id, cx);
                        }
                    }),
                    MenuEntry::Submenu("Move to".into(), layers),
                    MenuEntry::item("Bring to front", move |t, _, cx| {
                        t.with_field_note(&k6, "Bring to front", cx, |s, id| {
                            if let Some(i) = s.annotations.iter().position(|a| a.id == id) {
                                let a = s.annotations.remove(i);
                                s.annotations.push(a);
                            }
                        });
                    }),
                    MenuEntry::item("Send to back", move |t, _, cx| {
                        t.with_field_note(&k7, "Send to back", cx, |s, id| {
                            if let Some(i) = s.annotations.iter().position(|a| a.id == id) {
                                let a = s.annotations.remove(i);
                                s.annotations.insert(0, a);
                            }
                        });
                    }),
                    MenuEntry::Separator,
                    MenuEntry::item("Clear selection", |t, _, cx| {
                        t.selection.clear();
                        t.redraw(cx);
                    }),
                    MenuEntry::item("Zoom to fit", |t, _, cx| t.zoom_to_fit(cx)),
                    MenuEntry::item("Delete", move |t, _, cx| t.edit_sheet("Edit title block", cx, |s| noshape_scene::drawing_tools::set_title_field(s, &k3, ""))),
                ]
            }
            (None, None, None, None) => self.context_entries(sel, cx),
        }
    }

    /// The auxiliary view reference indicator's own menu (live u-906).
    fn aux_indicator_entries(id: ViewId, paste: Option<MenuEntry>) -> Vec<MenuEntry> {
        let mut out = vec![
            MenuEntry::item("Edit auxiliary view...", move |t, w, cx| t.edit_aux_view(id, w, cx)),
            MenuEntry::item("Flip direction", move |t, _, cx| t.edit_view("Flip direction", id, cx, |v| v.flip = !v.flip)),
        ];
        out.extend(paste);
        out.extend([
            MenuEntry::item("Paste as table", |t, _, cx| {
                t.paste_as_table(cx);
            }),
            MenuEntry::Separator,
            MenuEntry::item("Clear selection", |t, _, cx| {
                t.selection.clear();
                t.redraw(cx);
            }),
            MenuEntry::item("Zoom to fit", |t, _, cx| t.zoom_to_fit(cx)),
            MenuEntry::item("Delete", move |t, _, cx| {
                t.selection.clear();
                t.edit_sheet("Delete view", cx, |s| s.remove_view(id));
            }),
        ]);
        out
    }

    /// Move to ▸ a format layer (or back to the drawing), for annotation `a`
    /// with the rest of the selection.
    /// Live's order for an annotation with entries of its own (`own`):
    /// `edit` · Paste · Copy · Paste as table · `own` (· Move to ▸) | Clear
    /// selection · Zoom to fit · Delete.
    fn live_order_entries(&self, a: &Annotation, edit: MenuEntry, own: Vec<MenuEntry>, move_to: bool, cx: &App) -> Vec<MenuEntry> {
        let id = a.id;
        let mut out = vec![edit];
        out.extend(crate::drawing_props::paste_entry(cx));
        out.push(MenuEntry::item("Copy", move |t, _, cx| {
            if !t.selection.contains(&Sel::Annotation(id)) {
                t.selection = vec![Sel::Annotation(id)];
            }
            t.copy_entities(cx);
        }));
        out.push(MenuEntry::item("Paste as table", |t, _, cx| {
            t.paste_as_table(cx);
        }));
        out.extend(own);
        if move_to {
            out.push(Self::move_to_entry(a));
        }
        out.extend([
            MenuEntry::Separator,
            MenuEntry::item("Clear selection", |t, _, cx| {
                t.selection.clear();
                t.redraw(cx);
            }),
            MenuEntry::item("Zoom to fit", |t, _, cx| t.zoom_to_fit(cx)),
            MenuEntry::item("Delete", |t, _, cx| t.delete_selection(cx)),
        ]);
        out
    }

    /// A format line (live behaviours "Hole table cell / title block clicks"
    /// (2)): Paste · Copy · Paste as table · Move to ▸ | Clear selection ·
    /// Zoom to fit · Delete. Copy puts it on the clipboard as a sketch line;
    /// Move to ▸ the other format layers.
    fn format_line_entries(&self, k: [i64; 4]) -> Vec<MenuEntry> {
        let current = self.built.as_ref().and_then(|b| {
            let pick = b.picks.iter().position(|t| *t == PickTarget::FormatLine(k))?;
            let line = b.geometry.lines.iter().position(|l| l.pick as usize == pick)?;
            b.geometry.layers.iter().find(|s| s.lines.contains(&line)).map(|s| s.layer)
        });
        let layers = SheetLayer::FORMAT
            .iter()
            .copied()
            .filter(|l| Some(*l) != current)
            .map(|l| MenuEntry::item(l.name(), move |t, _, cx| t.edit_sheet("Move to", cx, |s| s.edit_format_line(k, |e| e.layer = Some(l)))))
            .collect();
        let mut out = vec![
            crate::drawing_props::paste_always(),
            MenuEntry::item("Copy", move |t, _, cx| {
                let mut d = t.drawing.clone();
                let s = &mut d.sheets[t.sheet];
                let p = |i: usize| noshape_sketch::DVec2::new(k[i] as f64 / 100.0, k[i + 1] as f64 / 100.0);
                let e = s.sketch.sketch.add_line(p(0), p(2));
                if let Ok(c) = d.copy_selection(t.sheet, &[], &[], &[e.0], &[]) {
                    cx.write_to_clipboard(ClipboardItem::new_string(c.to_json()));
                }
            }),
            MenuEntry::item("Paste as table", |t, _, cx| {
                t.paste_as_table(cx);
            }),
            MenuEntry::Submenu("Move to".into(), layers),
        ];
        out.extend(Self::selection_tail());
        out
    }

    /// | Clear selection · Zoom to fit · Delete: the end of every
    /// annotation menu.
    pub(crate) fn selection_tail() -> [MenuEntry; 4] {
        [
            MenuEntry::Separator,
            MenuEntry::item("Clear selection", |t, _, cx| {
                t.selection.clear();
                t.redraw(cx);
            }),
            MenuEntry::item("Zoom to fit", |t, _, cx| t.zoom_to_fit(cx)),
            MenuEntry::item("Delete", |t, _, cx| t.delete_selection(cx)),
        ]
    }

    pub(crate) fn move_to_entry(a: &Annotation) -> MenuEntry {
        let (id, cur) = (a.id, a.layer);
        let layers = SheetLayer::FORMAT.iter().copied().chain([SheetLayer::Drawing]);
        let items = layers
            .map(|l| {
                MenuEntry::item(l.name(), move |t, _, cx| {
                    let ids: Vec<u64> = t.selection.iter().filter_map(|s| if let Sel::Annotation(x) = s { Some(*x) } else { None }).chain([id]).collect();
                    t.edit_sheet("Move to", cx, |s| s.annotations.iter_mut().filter(|x| ids.contains(&x.id)).for_each(|x| x.layer = l));
                })
                .disabled(l == cur)
            })
            .collect();
        MenuEntry::Submenu("Move to".into(), items)
    }

    /// A surface finish symbol, free or on an edge (live2 dom/context-menus-drawing-2.txt, 11-15): Edit... · (Paste
    /// while drawing content is copied) · Copy · Paste as table · Add leader · Move to ▸ | Clear selection · Zoom to
    /// fit · Delete.
    fn surface_finish_entries(&self, a: &Annotation, cx: &App) -> Vec<MenuEntry> {
        let id = a.id;
        let mut out = Vec::new();
        let a2 = a.clone();
        out.push(MenuEntry::item("Edit...", move |t, w, cx| t.open_annotation_panel(&a2, w, cx)));
        out.push(crate::drawing_props::paste_always());
        out.push(MenuEntry::item("Copy", move |t, _, cx| {
            if !t.selection.contains(&Sel::Annotation(id)) {
                t.selection = vec![Sel::Annotation(id)];
            }
            t.copy_entities(cx);
        }));
        out.push(MenuEntry::item("Paste as table", |t, _, cx| {
            t.paste_as_table(cx);
        }));
        // Standing on an edge without a leader: Flip (to the edge's other side) where a free or led one has Add
        // leader (live2 drw-surface-finish-flip retake 11-15: Edit… · Flip · Copy · Paste as table · Move to).
        if matches!(&a.kind, AnnotationKind::SurfaceFinish { on: Some(_), leaders, .. } if leaders.is_empty()) {
            out.insert(1, MenuEntry::item("Flip", move |t, _, cx| {
                t.edit_sheet("Flip", cx, |s| {
                    if let Some(AnnotationKind::SurfaceFinish { on: Some(on), .. }) = s.annotations.iter_mut().find(|x| x.id == id).map(|x| &mut x.kind) {
                        on.3 = !on.3;
                    }
                })
            }));
        } else {
            out.push(MenuEntry::item("Add leader", move |t, _, cx| t.set_tool(Tool::GdtLeader { id }, cx)));
        }
        out.push(Self::move_to_entry(a));
        out.extend([
            MenuEntry::Separator,
            MenuEntry::item("Clear selection", |t, _, cx| {
                t.selection.clear();
                t.redraw(cx);
            }),
            MenuEntry::item("Zoom to fit", |t, _, cx| t.zoom_to_fit(cx)),
            MenuEntry::item("Delete", |t, _, cx| t.delete_selection(cx)),
        ]);
        let _ = cx;
        out
    }

    fn context_entries(&self, sel: Option<Sel>, cx: &App) -> Vec<MenuEntry> {
        let mut out = Vec::new();
        // Several entities selected (live drawing/291).
        if self.selection.len() > 1 && sel.is_some_and(|s| self.selection.contains(&s)) {
            // Several callouts: Align horizontally / vertically first (qa
            // CALLOUT-ALIGN-MISSING).
            let clicked = match sel {
                Some(Sel::Annotation(id)) => self.current_sheet().annotations.iter().find(|a| a.id == id).cloned(),
                _ => None,
            };
            let mut out = clicked.map(|a| self.callout_align_entries(&a)).unwrap_or_default();
            out.extend(self.multi_selection_entries(cx));
            return out;
        }
        match sel {
            Some(Sel::View(id) | Sel::Edge(id, _) | Sel::CutLine(id)) => out = self.onshape_view_entries(id, cx),
            Some(Sel::AuxIndicator(id)) => return Self::aux_indicator_entries(id, crate::drawing_props::paste_entry(cx)),
            // A hole table's tag axis (help drawing_hole_table): Hole table
            // properties, Flip X direction, Flip Y direction | Clear
            // selection, Zoom to fit.
            Some(Sel::HoleAxis(id)) => {
                use noshape_model::drawing::TableKind;
                return vec![
                    MenuEntry::item("Hole table properties...", move |t, _, cx| t.open_hole_table_props(id, cx)),
                    MenuEntry::item("Flip X direction", move |t, _, cx| t.edit_table(id, "Flip X direction", cx, |t| if let TableKind::Hole { flip_x, .. } = &mut t.kind { *flip_x = !*flip_x })),
                    MenuEntry::item("Flip Y direction", move |t, _, cx| t.edit_table(id, "Flip Y direction", cx, |t| if let TableKind::Hole { flip_y, .. } = &mut t.kind { *flip_y = !*flip_y })),
                    MenuEntry::Separator,
                    MenuEntry::item("Clear selection", |t, _, cx| {
                        t.selection.clear();
                        t.redraw(cx);
                    }),
                    MenuEntry::item("Zoom to fit", |t, _, cx| t.zoom_to_fit(cx)),
                ];
            }
            Some(Sel::ViewLabel(id)) => out = self.label_entries(id, cx),
            Some(Sel::Annotation(id)) => {
                // Linear dimensions: Onshape's own menu.
                if let Some(entries) = self.dimension_entries(id).or_else(|| self.radial_entries(id)).or_else(|| self.ordinate_entries(id, cx)) {
                    return entries;
                }
                // Centermarks (live drawing/487): Paste as table, Align
                // centermark | Clear selection, Zoom to fit.
                if self.current_sheet().annotations.iter().any(|a| a.id == id && matches!(a.kind, AnnotationKind::Centermark { .. })) {
                    return vec![
                        MenuEntry::item("Paste as table", |t, _, cx| {
                            t.paste_as_table(cx);
                        }),
                        MenuEntry::item("Align centermark", move |t, _, cx| t.start_align_centermark(id, cx)),
                        MenuEntry::Separator,
                        MenuEntry::item("Clear selection", |t, _, cx| {
                            t.selection.clear();
                            t.redraw(cx);
                        }),
                        MenuEntry::item("Zoom to fit", |t, _, cx| t.zoom_to_fit(cx)),
                    ];
                }
                let a = self.drawing.sheets[self.sheet].annotations.iter().find(|a| a.id == id).cloned();
                if let Some(a) = a.as_ref().filter(|a| matches!(a.kind, AnnotationKind::SurfaceFinish { .. })) {
                    return self.surface_finish_entries(a, cx);
                }
                // Hole callouts (live behaviours A56): Edit... · Paste · Copy
                // · Paste as table · Add leader (· Remove leaders) | Clear
                // selection · Zoom to fit · Delete.
                if let Some(a) = a.as_ref().filter(|a| matches!(a.kind, AnnotationKind::HoleCallout { .. })) {
                    let a2 = a.clone();
                    let mut out = vec![MenuEntry::item("Edit...", move |t, w, cx| t.open_annotation_panel(&a2, w, cx))];
                    out.extend(crate::drawing_props::paste_entry(cx));
                    out.push(MenuEntry::item("Copy", move |t, _, cx| {
                        if !t.selection.contains(&Sel::Annotation(id)) {
                            t.selection = vec![Sel::Annotation(id)];
                        }
                        t.copy_entities(cx);
                    }));
                    out.push(MenuEntry::item("Paste as table", |t, _, cx| {
                        t.paste_as_table(cx);
                    }));
                    out.push(MenuEntry::item("Add leader", move |t, _, cx| t.set_tool(Tool::GdtLeader { id }, cx)));
                    if matches!(&a.kind, AnnotationKind::HoleCallout { leaders, .. } if !leaders.is_empty()) {
                        out.push(MenuEntry::item("Remove leaders", move |t, _, cx| t.edit_sheet("Remove leaders", cx, |s| noshape_scene::drawing_tools::annotation_edit::remove_leaders(s, id))));
                    }
                    out.extend([
                        MenuEntry::Separator,
                        MenuEntry::item("Clear selection", |t, _, cx| {
                            t.selection.clear();
                            t.redraw(cx);
                        }),
                        MenuEntry::item("Zoom to fit", |t, _, cx| t.zoom_to_fit(cx)),
                        MenuEntry::item("Delete", |t, _, cx| t.delete_selection(cx)),
                    ]);
                    return out;
                }
                // Callouts (help callout_balloon, drawing.md A25), as notes and
                // GD&T frames: Edit... · Paste · Copy · Paste as table · Add
                // leader (· Remove leaders) · grouping · Move to ▸ | Clear
                // selection · Zoom to fit · Delete.
                if let Some(a) = a.as_ref().filter(|a| matches!(a.kind, AnnotationKind::Balloon { .. })) {
                    use noshape_scene::drawing_tools::annotation_edit::{group_annotations, group_with_view};
                    let a2 = a.clone();
                    let edit = MenuEntry::item("Edit...", move |t, w, cx| t.open_annotation_panel(&a2, w, cx));
                    let mut own = self.annotation_more_entries(a);
                    // (No Group with a selected view: live's menu for a callout with its view selected has none, live2
                    // 2442; an older document's group can still be left.)
                    if a.view_group.is_some() {
                        own.push(MenuEntry::item("Remove from group", move |t, _, cx| t.edit_sheet("Remove from group", cx, |s| group_with_view(s, &[id], None))));
                    }
                    if a.ann_group.is_some() {
                        own.push(MenuEntry::item("Remove from grouped annotation", move |t, _, cx| t.edit_sheet("Remove from grouped annotation", cx, |s| group_annotations(s, &[id], false))));
                    }
                    // With a BOM table in the drawing: Add stacked callouts... (live drawing/191; live2 2240's menu without
                    // it was of two selected annotations).
                    if self.drawing.sheets.iter().flat_map(|s| &s.tables).any(|t| matches!(t.kind, TableKind::Bom { .. })) {
                        own.push(MenuEntry::item("Add stacked callouts...", move |t, _, cx| t.open_stack_dlg(id, cx)));
                    }
                    return self.live_order_entries(a, edit, own, true, cx);
                }
                // GD&T frames (help drawing.md A42, qa DWG-GDT-MENU-ORDER):
                // Edit... · Paste · Copy · Paste as table · Add leader (·
                // Remove leaders) · Move to ▸ | Clear selection · Zoom to fit · Delete.
                if let Some(a) = a.as_ref().filter(|a| matches!(a.kind, AnnotationKind::Gdt { .. })) {
                    let a2 = a.clone();
                    let edit = MenuEntry::item("Edit...", move |t, w, cx| t.open_annotation_panel(&a2, w, cx));
                    let own = self.annotation_more_entries(a);
                    return self.live_order_entries(a, edit, own, true, cx);
                }
                // Datum targets (live A35, qa DWG-DATUM-TARGET-MENU): Edit... ·
                // Paste · Copy · Paste as table · Show dimensions ▸ (on its
                // dimension: Hide dimension) | Clear selection · Zoom to fit · Delete.
                if let Some(a) = a.as_ref().filter(|a| matches!(a.kind, AnnotationKind::DatumTarget { .. })) {
                    let mut own = self.annotation_more_entries(a);
                    let edit = own.remove(0);
                    return self.live_order_entries(a, edit, own, false, cx);
                }
                // Weld symbols, always on an entity (live2 dom/context-menus-drawing-2.txt, 11-15): Edit... · Flip ·
                // (Paste while drawing content is copied) · Copy · Paste as table · Move to ▸ | Clear selection · Zoom
                // to fit · Delete.
                if let Some(a) = a.as_ref().filter(|a| matches!(a.kind, AnnotationKind::Weld { .. })) {
                    let mut out = vec![MenuEntry::item("Edit...", move |t, w, cx| t.open_weld_edit(id, w, cx))];
                    out.push(MenuEntry::item("Flip", move |t, _, cx| {
                        t.edit_sheet("Flip", cx, |s| {
                            if let Some(AnnotationKind::Weld { spec, .. }) = s.annotations.iter_mut().find(|x| x.id == id).map(|x| &mut x.kind) {
                                spec.flip = !spec.flip;
                            }
                        })
                    }));
                    out.push(crate::drawing_props::paste_always());
                    out.push(MenuEntry::item("Copy", move |t, _, cx| {
                        if !t.selection.contains(&Sel::Annotation(id)) {
                            t.selection = vec![Sel::Annotation(id)];
                        }
                        t.copy_entities(cx);
                    }));
                    out.push(MenuEntry::item("Paste as table", |t, _, cx| {
                        t.paste_as_table(cx);
                    }));
                    out.push(Self::move_to_entry(a));
                    out.extend(Self::selection_tail());
                    return out;
                }
                // Edge condition symbols (live 1955 f-rclick): Edit… · Copy · Paste as table · Add leader · Remove
                // leaders (even with none added) · Move to ▸ | Clear selection · Zoom to fit · Delete; no Paste.
                if let Some(a) = a.as_ref().filter(|a| matches!(a.kind, AnnotationKind::EdgeCondition { .. })) {
                    let mut out = vec![MenuEntry::item("Edit...", move |t, w, cx| t.open_edge_condition_edit(id, w, cx))];
                    out.push(MenuEntry::item("Copy", move |t, _, cx| {
                        if !t.selection.contains(&Sel::Annotation(id)) {
                            t.selection = vec![Sel::Annotation(id)];
                        }
                        t.copy_entities(cx);
                    }));
                    out.push(MenuEntry::item("Paste as table", |t, _, cx| t.paste_as_table(cx)));
                    out.push(MenuEntry::item("Add leader", move |t, _, cx| t.set_tool(Tool::GdtLeader { id }, cx)));
                    out.push(MenuEntry::item("Remove leaders", move |t, _, cx| t.edit_sheet("Remove leaders", cx, |s| noshape_scene::drawing_tools::annotation_edit::remove_leaders(s, id))));
                    out.push(Self::move_to_entry(a));
                    out.extend(Self::selection_tail());
                    return out;
                }
                // A 2 point / edge to edge centerline (live request 1736):
                // Paste | Clear selection · Zoom to fit · Delete.
                if a.as_ref().is_some_and(|a| matches!(a.kind, AnnotationKind::Centerline { .. })) {
                    let mut out = vec![crate::drawing_props::paste_always()];
                    out.extend(Self::selection_tail());
                    return out;
                }
                // Notes: their own menu in live's order (drawing_leaders).
                if let Some(a) = a.as_ref().filter(|a| matches!(a.kind, AnnotationKind::Note { .. })) {
                    let mut out = Vec::new();
                    out.extend(self.note_entries(a, cx));
                    return out;
                }
                if let Some(a) = &a {
                    if let AnnotationKind::Note { .. } = a.kind {
                        let pos = a.position;
                        out.push(MenuEntry::item("Edit note...", move |t, w, cx| t.open_note_panel(Some(id), pos, w, cx)));
                    }
                    out.extend(self.annotation_more_entries(a));
                    if let AnnotationKind::RadialDimension { diameter, .. } = a.kind {
                        out.push(MenuEntry::item(if diameter { "Show as radius" } else { "Show as diameter" }, move |t, _, cx| {
                            t.edit_sheet("Radial dimension", cx, |s| {
                                if let Some(AnnotationKind::RadialDimension { diameter, .. }) = s.annotations.iter_mut().find(|x| x.id == id).map(|x| &mut x.kind) {
                                    *diameter = !*diameter;
                                }
                            })
                        }));
                    }
                    out.extend(self.annotation_extra_entries(a));
                    // Group / Remove from group (with a selected view), Group
                    // annotation / Remove annotation from group (notes, callouts).
                    if matches!(a.kind, AnnotationKind::Note { .. } | AnnotationKind::Balloon { .. }) {
                        let sel_view = self.selection.iter().find_map(|s| s.as_view());
                        let anns: Vec<u64> = self.selection.iter().filter_map(|s| if let Sel::Annotation(x) = s { Some(*x) } else { None }).chain([id]).collect();
                        if a.view_group.is_some() {
                            out.push(MenuEntry::item("Remove from group", move |t, _, cx| t.edit_sheet("Remove from group", cx, |s| noshape_scene::drawing_tools::annotation_edit::group_with_view(s, &[id], None))));
                        } else if let Some(v) = sel_view.filter(|_| matches!(a.kind, AnnotationKind::Note { .. })) {
                            // (A callout has none: live2 2442.)
                            let ids = anns.clone();
                            out.push(MenuEntry::item("Group", move |t, _, cx| t.edit_sheet("Group", cx, |s| noshape_scene::drawing_tools::annotation_edit::group_with_view(s, &ids, Some(v)))));
                        }
                        // Live (request 1590): one grouped member alone leaves the
                        // group; grouping needs a callout among the annotations.
                        if a.ann_group.is_some() {
                            out.push(MenuEntry::item("Remove from grouped annotation", move |t, _, cx| t.edit_sheet("Remove from grouped annotation", cx, |s| noshape_scene::drawing_tools::annotation_edit::group_annotations(s, &[id], false))));
                        } else if anns.iter().collect::<std::collections::HashSet<_>>().len() >= 2 && matches!(a.kind, AnnotationKind::Balloon { .. }) {
                            let ids = anns.clone();
                            out.push(MenuEntry::item("Group annotation", move |t, _, cx| t.edit_sheet("Group annotation", cx, |s| noshape_scene::drawing_tools::annotation_edit::group_annotations(s, &ids, true))));
                        }
                    }
                    // Copy (with the rest of the selection).
                    if matches!(a.kind, AnnotationKind::Note { .. } | AnnotationKind::Balloon { .. } | AnnotationKind::Gdt { .. } | AnnotationKind::Datum { .. } | AnnotationKind::DatumTarget { .. } | AnnotationKind::SurfaceFinish { .. } | AnnotationKind::Weld { .. } | AnnotationKind::HoleCallout { .. }) {
                        out.push(MenuEntry::item("Copy", move |t, _, cx| {
                            if !t.selection.contains(&Sel::Annotation(id)) {
                                t.selection = vec![Sel::Annotation(id)];
                            }
                            t.copy_entities(cx);
                        }));
                    }
                    // Move to ▸ a format layer (or back to the drawing); not
                    // on datums (live C9: Edit... · Paste · Copy · Paste as
                    // table | Add comment | Clear selection · Zoom to fit ·
                    // Delete).
                    if matches!(a.kind, AnnotationKind::Note { .. } | AnnotationKind::Balloon { .. } | AnnotationKind::Gdt { .. } | AnnotationKind::DatumTarget { .. } | AnnotationKind::SurfaceFinish { .. } | AnnotationKind::Weld { .. }) {
                        out.push(Self::move_to_entry(a));
                    }
                    // Paste as table is in every drawing menu (live drawing/290).
                    out.push(MenuEntry::item("Paste as table", |t, _, cx| {
                        t.paste_as_table(cx);
                    }));
                    out.push(MenuEntry::Separator);
                }
                // Onshape's menus end with Clear selection, Zoom to fit, Delete.
                out.push(MenuEntry::item("Clear selection", |t, _, cx| {
                    t.selection.clear();
                    t.redraw(cx);
                }));
                out.push(MenuEntry::item("Zoom to fit", |t, _, cx| t.zoom_to_fit(cx)));
                out.push(MenuEntry::item("Delete", move |t, _, cx| {
                    t.selection.clear();
                    t.edit_sheet("Delete annotation", cx, |s| s.annotations.retain(|x| x.id != id));
                }));
            }
            Some(Sel::Table(id)) => out.extend(self.table_entries(id, cx)),
            Some(Sel::FormatLine(k)) => return self.format_line_entries(k),
            Some(Sel::Sketch(id)) => out.extend(self.sketch_entries(id)),
            Some(Sel::Image(id)) => out.extend(self.image_entries(id, cx)),
            // Live 483–493, ~1490: an automatic centermark's menu.
            Some(Sel::AutoCentermark(view, key)) => {
                out.push(MenuEntry::item("Paste as table", |t, _, cx| {
                    t.paste_as_table(cx);
                }));
                out.push(MenuEntry::item("Align centermark", move |t, _, cx| {
                    t.align_auto_centermark = Some((view, key));
                    t.status = Some("Align centermark: select a line or edge".into());
                    cx.notify();
                }));
                out.push(MenuEntry::Separator);
                out.push(MenuEntry::item("Clear selection", |t, _, cx| {
                    t.selection.clear();
                    t.redraw(cx);
                }));
                out.push(MenuEntry::item("Zoom to fit", |t, _, cx| t.zoom_to_fit(cx)));
            }
            // Live (live2 2186): Copy · Paste as table · Edit hatch region... | Clear selection · Zoom to fit · Delete.
            Some(Sel::Hatch(id)) => {
                out.push(MenuEntry::item("Copy", |t, _, cx| t.copy_views(None, cx)));
                out.push(MenuEntry::item("Paste as table", |t, _, cx| {
                    t.paste_as_table(cx);
                }));
                out.push(MenuEntry::item("Edit hatch region...", move |t, w, cx| t.open_hatch_dialog(Some(id), w, cx)));
                out.push(MenuEntry::Separator);
                out.push(MenuEntry::item("Clear selection", |t, _, cx| {
                    t.selection.clear();
                    t.redraw(cx);
                }));
                out.push(MenuEntry::item("Zoom to fit", |t, _, cx| t.zoom_to_fit(cx)));
                out.push(MenuEntry::item("Delete", move |t, _, cx| {
                    t.selection.clear();
                    t.edit_sheet("Delete hatch", cx, |s| s.sketch.hatches.retain(|h| h.id != id));
                }));
            }
            Some(Sel::Break(view, index)) => out.extend(self.break_entries(view, index)),
            // The empty sheet with something selected (live behaviours "Replicate annotations" set-up): Paste · Copy ·
            // Paste as table | Clear selection · Zoom to fit (Add comment is a cloud feature).
            None if !self.selection.is_empty() => {
                out.push(crate::drawing_props::paste_always());
                out.push(MenuEntry::item("Copy", |t, _, cx| t.copy_views(None, cx)));
                out.push(MenuEntry::item("Paste as table", |t, _, cx| {
                    t.paste_as_table(cx);
                }));
                out.push(MenuEntry::Separator);
                out.push(MenuEntry::item("Clear selection", |t, _, cx| {
                    t.selection.clear();
                    t.redraw(cx);
                }));
                out.push(MenuEntry::item("Zoom to fit", |t, _, cx| t.zoom_to_fit(cx)));
                // An annotation selected: Delete too (live2 drw-datum-copy-paste s6).
                if self.selection.iter().any(|s| matches!(s, Sel::Annotation(_))) {
                    out.push(MenuEntry::item("Delete", |t, _, cx| t.delete_selection(cx)));
                }
            }
            None => {
                // Onshape's empty-sheet menu (live drawing/290): [Paste,]
                // Paste as table | Sheet properties…, Drawing properties… |
                // Zoom to fit, Zoom to window, Zoom to sheet.
                // Live (A45): Paste is always there, also with nothing copied.
                out.push(crate::drawing_props::paste_always());
                out.push(MenuEntry::item("Paste as table", |t, _, cx| {
                    t.paste_as_table(cx);
                }));
                let si = self.sheet;
                out.push(MenuEntry::item("Sheet properties...", move |t, w, cx| t.open_sheet_props(si, w, cx)));
                out.push(MenuEntry::item("Drawing properties...", |t, _, cx| t.open_drawing_props(cx)));
                out.push(MenuEntry::Separator);
                out.push(MenuEntry::item("Zoom to fit", |t, _, cx| t.zoom_to_fit(cx)));
                out.push(MenuEntry::item("Zoom to window", |t, _, cx| t.start_zoom_window(cx)));
                out.push(MenuEntry::item("Zoom to sheet", |t, _, cx| t.zoom_to_fit(cx)));
            }
        }
        out
    }

    /// `edit_view` on several views, one undo step.
    pub fn edit_views(&mut self, description: &str, ids: &[ViewId], cx: &mut Context<Self>, f: impl Fn(&mut DrawingView)) {
        let targets: Vec<(usize, ViewId)> = ids.iter().map(|id| self.drawing.real_view_of(self.sheet, *id)).collect();
        self.edit(description, cx, |d| {
            for (si, id) in targets {
                if let Some(v) = d.sheets.get_mut(si).and_then(|s| s.view_mut(id)) {
                    f(v);
                }
            }
        });
    }

    /// Turn view `id` to `rotation` (radians); its edge dimensions turn
    /// with it (`set_view_rotation`).
    pub fn rotate_view(&mut self, description: &str, id: ViewId, rotation: f64, cx: &mut Context<Self>) {
        let (si, id) = self.drawing.real_view_of(self.sheet, id);
        self.edit(description, cx, |d| {
            if let Some(s) = d.sheets.get_mut(si) {
                noshape_scene::drawing_tools::view_edit::set_view_rotation(s, id, rotation);
            }
        });
    }

    pub fn edit_view(&mut self, description: &str, id: ViewId, cx: &mut Context<Self>, f: impl FnOnce(&mut DrawingView)) {
        // A mirror of a view on another sheet (a linked section's cutting
        // line on its parent): edit the real view.
        let (si, id) = self.drawing.real_view_of(self.sheet, id);
        self.edit(description, cx, |d| {
            if let Some(v) = d.sheets.get_mut(si).and_then(|s| s.view_mut(id)) {
                f(v);
            }
        });
    }
}

impl DrawingEditor {
    /// A chamfer dimension under Dimensions style (Drawing properties ›
    /// Dimensions › Chamfer, 45° or not): its two legs as aligned linear
    /// dimensions (live drawing/255–259); None for Note style.
    fn chamfer_as_dimensions(&self, a: &Annotation) -> Option<Vec<Annotation>> {
        self.pk().chamfer_as_dimensions(a)
    }
}


/// The Note tool's preview at sheet point `at`: the text "NOTE" in the
/// height a new note gets (Drawing properties › Notes, else all
/// annotations'), where the click would put it.
fn note_tool_preview(d: &Drawing, sheet: &Sheet, at: [f64; 2]) -> Annotation {
    let h = d.props.ann_type(noshape_model::drawing::sheet::AnnType::Notes).height.unwrap_or(d.props.ann_text_height);
    Annotation { id: sheet.next_annotation_id(), kind: AnnotationKind::note("NOTE", h), position: at, ..Default::default() }
}

/// Two presses close enough in time and place to open a dimension's palette (live2 11-13: about 200 ms and 3 px).
pub fn quick_double_click(gap: std::time::Duration, dist_px: f32) -> bool {
    gap <= std::time::Duration::from_millis(200) && dist_px <= 3.
}

/// How far (screen px) placement inference holds the pointer on another item's x or y (live2 feel/drawing.json D4: a
/// note within 7 px of a dimension text's line snaps, 9 doesn't; wake-1118: a woken vertex at 8 px, not 12).
const INFER_REACH_PX: f64 = 8.5;
/// A box select shows once the pointer is more than this far from the press (live2 drw-feel-q (a): none at 7 px
/// along an axis or 5.7 px diagonally, a box at 8 px and at 7.07 px diagonally).
const BOX_START_PX: f64 = 7.0;

/// An automatic centermark's arms `ext` sheet mm longer than its default (0: as made).
pub(crate) fn set_centermark_extension(v: &mut DrawingView, key: [i64; 2], ext: f64) {
    v.centermark_extensions.retain(|(k, _)| *k != key);
    if ext > 1e-9 {
        v.centermark_extensions.push((key, ext));
    }
}

#[cfg(test)]
mod note_preview_tests {
    use super::*;

    /// Live2 11-13: a dimension's palette opens on two presses within about 200 ms and 3 px.
    #[::core::prelude::v1::test]
    fn a_dimensions_palette_needs_a_quick_close_double_click() {
        let ms = std::time::Duration::from_millis;
        assert!(quick_double_click(ms(150), 2.0));
        assert!(quick_double_click(ms(200), 3.0));
        assert!(!quick_double_click(ms(300), 0.0), "slower: two clicks");
        assert!(!quick_double_click(ms(100), 4.0), "further: two clicks");
    }

    /// qa5 P2-D3: the Note tool previews "NOTE" at the pointer in the new
    /// note's height.
    #[::core::prelude::v1::test]
    fn note_tool_previews_note_at_the_pointer() {
        let d = Drawing::default();
        let s = Sheet::default();
        let a = note_tool_preview(&d, &s, [100.0, 50.0]);
        assert_eq!(a.position, [100.0, 50.0]);
        match a.kind {
            AnnotationKind::Note { text, height_mm, .. } => {
                assert_eq!(text, "NOTE");
                assert!(height_mm > 0.0);
            }
            k => panic!("{k:?}"),
        }
    }
}

/// The line `a`–`b` cut into a dash pattern (lengths in sheet mm: dash, gap, dash, gap, …): the dashes.
fn dash_pieces(a: [f64; 2], b: [f64; 2], pattern: &[f64]) -> Vec<Vec<[f64; 2]>> {
    let len = (b[0] - a[0]).hypot(b[1] - a[1]);
    let period: f64 = pattern.iter().sum();
    if len <= 0.0 || period <= 0.0 {
        return Vec::new();
    }
    let at = |t: f64| [a[0] + (b[0] - a[0]) * t / len, a[1] + (b[1] - a[1]) * t / len];
    let mut out = Vec::new();
    let mut t = 0.0;
    'outer: loop {
        for (i, l) in pattern.iter().enumerate() {
            let end = (t + l).min(len);
            if i % 2 == 0 && end > t {
                out.push(vec![at(t), at(end)]);
            }
            t += l;
            if t >= len {
                break 'outer;
            }
        }
    }
    out
}

#[cfg(test)]
mod dash_tests {
    /// A dash pattern cuts the line into its dashes, the last one cut short at the end.
    #[test]
    fn a_line_is_cut_into_its_dashes() {
        let pieces = super::dash_pieces([0.0, 0.0], [0.0, 100.0], &[40.0, 10.0, 5.0, 10.0]);
        let spans: Vec<(f64, f64)> = pieces.iter().map(|p| (p[0][1], p[1][1])).collect();
        assert_eq!(spans, vec![(0.0, 40.0), (50.0, 55.0), (65.0, 100.0)]);
    }
}

/// A circle as a closed polyline (sheet mm).
fn circle_points(c: [f64; 2], r: f64) -> Vec<[f64; 2]> {
    (0..=72)
        .map(|i| {
            let t = i as f64 / 72.0 * std::f64::consts::TAU;
            [c[0] + r * t.cos(), c[1] + r * t.sin()]
        })
        .collect()
}
