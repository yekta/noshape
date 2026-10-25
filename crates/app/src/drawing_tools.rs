//! Drawing editor input: mouse/keyboard handling, selection and dragging,
//! and the view / dimension / annotation tools (with live previews).

use crate::drawing_scene::Overlay;
use crate::navigation as nav;
use crate::drawing_view::*;
use gpui_kit::*;
use noshape_model::drawing::build::PickTarget;
use noshape_model::drawing::views;
use noshape_model::drawing::*;

pub(crate) use noshape_scene::drawing_tools::geom::*;
use noshape_scene::drawing_tools as dt;

impl DrawingEditor {
    // ------------------------------------------------------------ tools

    pub fn set_tool(&mut self, tool: Tool, cx: &mut Context<Self>) {
        self.woken_centers.clear();
        self.woken_points.clear();
        self.datum_on = None;
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
        let sel_view = self.selection.iter().find_map(|s| if let Sel::View(v) = s { Some(*v) } else { None });
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
        // Edit detail (not redrawing its profile): Esc discards it.
        if self.detail_dlg.as_ref().is_some_and(|d| d.edit.is_some()) && !matches!(self.tool, Tool::Detail { .. }) {
            self.close_detail_edit(false, cx);
            return;
        }
        // Rename sheet / Rotation angle: Esc closes them unapplied (live).
        if matches!(self.panel, Some(Panel::SheetName { .. } | Panel::Rotation { .. })) {
            self.panel = None;
            cx.notify();
            return;
        }
        if self.zoom_window.take().is_some() || self.rotate_to_edge.take().is_some() || self.align_views.take().is_some() || self.sheet_props.take().is_some() || self.shortcut_toolbar.take().is_some() || self.measure.take().is_some() || self.align_centermark.take().is_some() || self.painter.take().is_some() || self.cancel_inspection(cx) || self.cancel_hatch(cx) {
            self.status = None;
            cx.notify();
            return;
        }
        if self.insert.chooser.take().is_some() {
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
        if self.gdt_dlg.is_some() {
            self.close_gdt_dlg(false, cx);
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
        if self.section_edit.is_some() {
            self.close_section_edit(false, cx);
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
        if self.move_sheet.take().is_some() {
            cx.notify();
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
        if !matches!(self.tool, Tool::Select) {
            // Esc ends a spline (keeping it) and exits the sketch tool.
            self.finish_sketch(cx);
            self.tool = Tool::Select;
            self.overlay = Overlay::default();
            self.note_leader = None;
            self.hover_snap = None;
            // Onshape: Esc ends the tool and nothing stays selected (the
            // base view of a projected-view chain is no longer orange).
            self.selection.clear();
            self.set_preview(None, cx);
            self.redraw(cx);
            return;
        }
        self.selection.clear();
        self.redraw(cx);
    }

    pub fn delete_selection(&mut self, cx: &mut Context<Self>) {
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
            Tool::EdgeCenterline { first: None } => "Select a cylinder silhouette edge, or the first of two edges",
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
            Tool::Gdt => "Click to place the geometric tolerance",
            Tool::Datum { anchor: None } => "Click the edge or point the datum is attached to",
            Tool::Datum { .. } => "Click to place the datum box",
            Tool::SurfaceFinish => "Click the edge the symbol sits on",
            Tool::Weld if self.weld_dlg.as_ref().is_some_and(|d| d.target.is_some()) => "Click to place the symbol",
            Tool::Weld => "Select an entity to associate the symbol",
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
            section: self.section_dlg.as_ref().map(|d| SectionOpts { cut_only: d.cut_only, flip: d.flip, depth_on: d.depth.is_some(), depth_value: d.depth_value, up_to: d.up_to, depth_ref: d.depth_ref.clone(), exclude: d.exclude.clone(), cut_from: d.cut_from }),
            detail: self.detail_dlg.as_ref().map(|d| DetailOpts { scale: d.scale, profile: d.profile.clone(), style: d.style, show_profile: d.show_profile, clip_border: d.clip_border, label: self.detail_label.clone() }),
        }
    }

    /// The dialog and tool state the previews read (see `ToolValues`).
    pub fn tool_values(&self) -> noshape_scene::drawing_tools::ToolValues {
        noshape_scene::drawing_tools::ToolValues {
            callout: self.callout_values_now(),
            datum_label: Some(self.datum_dialog_label()),
            datum_rows: self.datum_dlg.as_ref().map(|d| d.rows.iter().map(|r| r.ann.clone()).collect()).unwrap_or_default(),
            datum_on: self.datum_on,
            datum_target_letter: self.datum_target_letter,
            sketch_dim_pick: self.sketch_dim_pick,
        }
    }

    /// The view the active view tool would create at the cursor.
    pub fn pending_view(&self) -> Option<DrawingView> {
        self.pk().pending_view(&self.view_options())
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
            | Tool::Detail { center: None, .. }
            | Tool::JoggedSection { done: false, .. }
            | Tool::AlignedSection { b: None, .. }
            | Tool::Ordinate { .. }
            | Tool::Table { kind: TableTool::Hole, origin: None, .. }
            | Tool::Datum { anchor: None }
            | Tool::Note
            | Tool::Weld
            | Tool::Crop { .. } => self.snap_at(c),
            _ => None,
        };
        // Dimension tool: hovering a circular edge wakes its center mark,
        // which stays (an orange center snap with a small cross).
        if matches!(self.tool, Tool::Dimension { .. })
            && let Some((view, me)) = self.hovered_edge()
            && let views::EdgeGeom::Circle { center, .. } = me.geom
            && !self.woken_centers.contains(&(view, center))
        {
            self.woken_centers.push((view, center));
        }
        // Sketch tools: hovering an endpoint wakes it (SK8).
        if matches!(self.tool, Tool::Sketch { .. })
            && let Some((_, p)) = self.sketch_point_at(c)
            && !self.woken_points.iter().any(|w| (w[0] - p[0]).abs() < 1e-9 && (w[1] - p[1]).abs() < 1e-9)
        {
            self.woken_points.push(p);
        }
        if matches!(self.tool, Tool::Dimension { .. }) {
            let r = self.tol_mm(4.0);
            for (view, center) in self.woken_centers.clone() {
                let Some(pl) = self.placement(view) else { continue };
                let p = pl.to_sheet(center);
                overlay.marks.push((p, crate::drawing_scene::SnapMark::Circle, self.tol_mm(9.0)));
                overlay.lines.push((vec![[p[0] - r, p[1]], [p[0] + r, p[1]]], false));
                overlay.lines.push((vec![[p[0], p[1] - r], [p[0], p[1] + r]], false));
            }
        }
        self.hover_snap = snap.as_ref().and_then(|s| self.pick_sheet(s));
        if let (Some(p), Some(s)) = (self.hover_snap, &snap) {
            // Shape-coded orange snap marker (live: drawing/10).
            overlay.marks.push((p, crate::drawing_scene::SnapMark::of(s.r.as_ref()), self.tol_mm(9.0)));
        }
        self.section_edit_overlay(&mut overlay);
        self.table_tool_overlay(&mut overlay, cx);
        let mut preview: Option<Drawing> = None;
        match &self.tool {
            Tool::Dimension { picks, .. } => {
                if let [Pick::Point(a)] = picks.as_slice()
                    && let Some(sa) = self.pick_sheet(a)
                {
                    overlay.lines.push((vec![sa, self.hover_snap.unwrap_or(c)], true));
                }
                for p in picks {
                    match p {
                        Pick::Point(a) => overlay.points.extend(self.pick_sheet(a)),
                        Pick::Line { a, b } => {
                            if let (Some(sa), Some(sb)) = (self.pick_sheet(a), self.pick_sheet(b)) {
                                overlay.lines.push((vec![sa, sb], false));
                            }
                        }
                        Pick::Circle { view, center, radius, .. } | Pick::Arc { view, center, radius, .. } | Pick::CircleAt { view, center, radius, .. } => {
                            if let Some(pl) = self.placement(*view) {
                                overlay.circle(pl.to_sheet(*center), pl.len_to_sheet(*radius), false);
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
            Tool::EdgeCenterline { first: Some((a, b)) } | Tool::VirtualSharp { first: Some((a, b, _)) } => {
                if let (Some(sa), Some(sb)) = (self.pick_sheet(a), self.pick_sheet(b)) {
                    overlay.lines.push((vec![sa, sb], false));
                }
            }
            Tool::Section { parent: Some(pid), p0: Some(p0), line: None, .. } => {
                if let Some(pl) = self.placement(*pid) {
                    let a = pl.to_sheet(*p0);
                    let b = self.ortho_point(a, c);
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
            // Vertical / horizontal section: the cutting line follows the cursor across the view.
            Tool::Section { p0: None, partial: false, .. } if self.section_axis().is_some() => {
                let at = self.hover_snap.unwrap_or(c);
                if let (Some(e), Some(v)) = (self.section_axis(), self.view_at(at))
                    && let Some((la, lb)) = self.extend_cut(v, at, e)
                    && let Some(pl) = self.placement(v)
                {
                    overlay.lines.push((vec![pl.to_sheet(la), pl.to_sheet(lb)], true));
                }
            }
            Tool::Detail { parent: Some(pid), center: Some(ctr), radius: None } => {
                if let Some(pl) = self.placement(*pid) {
                    let cs = pl.to_sheet(*ctr);
                    overlay.circle(cs, len(sub(c, cs)), true);
                }
            }
            Tool::Sketch { .. } => self.sketch_overlay(&mut overlay),
            Tool::Weld => self.weld_overlay(&mut overlay),
            Tool::Note if self.note_leader.is_some() => {
                if let Some(a) = self.note_leader.as_ref().and_then(|l| Some(self.placement(l.view?)?.to_sheet(l.point))) {
                    overlay.lines.push((vec![a, self.cursor], true));
                }
            }
            Tool::PlaceImage { .. } => self.image_overlay(&mut overlay),
            Tool::Detail { center: None, .. } if self.detail_dlg.as_ref().is_some_and(|d| d.shape != 0) => self.detail_overlay(&mut overlay),
            _ => self.extra_overlay(&mut overlay),
        }
        self.crop_overlay(&mut overlay);
        // The Datum dialog's targets show until ✓ (under the pending one).
        let base = self.datum_rows_drawing();
        if let Some(v) = self.pending_view() {
            let mut d = self.drawing.clone();
            if let Some(s) = d.sheets.get_mut(self.sheet) {
                s.views.push(v);
            }
            preview = Some(d);
        } else if let Some(a) = self.pending_annotation() {
            let mut d = base.clone().unwrap_or_else(|| self.drawing.clone());
            if let Some(s) = d.sheets.get_mut(self.sheet) {
                s.annotations.push(a);
            }
            preview = Some(d);
        } else if base.is_some() {
            preview = base;
        }
        // Edit detail: the edited view as the dialog has it.
        if let Some(d) = self.detail_edit_drawing(cx) {
            preview = Some(d);
        }
        overlay.infer.extend(self.infer_lines.iter().cloned());
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

    fn tool_click(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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
        if self.weld_click(cx) || self.stack_click(cx) || self.table_dlg_click(cx) || self.custom_dlg_click(cx) || self.table_tool_click(cx) || self.gdt_click(cx) {
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
                    self.edit_sheet("Insert view", cx, |s| s.views.push(v));
                    // Placing a view on an out-of-date drawing updates it.
                    if self.out_of_date {
                        self.update_drawing(cx);
                    }
                    // Onshape activates the projected view tool next.
                    self.tool = Tool::Projected { parent: Some(id) };
                    self.selection = vec![Sel::View(id)];
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
            Tool::Section { p0: None, partial, .. } => {
                // A click just outside a view still cuts it (8 mm around it).
                let near = self.view_at(c).or_else(|| {
                    let b = self.built.as_ref()?;
                    b.placements.iter().filter(|p| p.contains(c, 8.0)).min_by(|x, y| x.dist_to(c).total_cmp(&y.dist_to(c))).map(|p| p.id)
                });
                let pick = self.snap_at(c).or_else(|| near.and_then(|v| self.placement(v).map(|pl| PickedPoint { view: v, p: pl.to_view(c), r: None })));
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
                    self.tool = Tool::Section { parent: Some(pp.view), p0: Some(pp.p), line: axis_line, partial, jogs: vec![] };
                }
            }
            Tool::Section { parent: Some(pid), p0: Some(p0), line: None, partial, .. } => {
                let Some(pl) = self.placement(pid) else { return };
                let a = pl.to_sheet(p0);
                let b = self.ortho_point(a, c);
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
            Tool::Section { line: Some(_), .. } | Tool::Detail { radius: Some(_), .. } => {
                if let Some(mut v) = self.pending_view() {
                    if let (ViewKind::Section { label, .. }, Some(l)) = (&mut v.kind, self.section_label(cx)) {
                        *label = l;
                    }
                    let id = v.id;
                    let name = if matches!(self.tool, Tool::Section { .. }) { "Section view" } else { "Detail view" };
                    self.edit_sheet(name, cx, |s| s.views.push(v));
                    self.tool = Tool::Select;
                    self.selection = vec![Sel::View(id)];
                }
            }
            Tool::Detail { center: None, .. } => {
                let pick = self.snap_at(c).or_else(|| self.view_at(c).and_then(|v| self.placement(v).map(|pl| PickedPoint { view: v, p: pl.to_view(c), r: None })));
                if let Some(pp) = pick {
                    self.tool = Tool::Detail { parent: Some(pp.view), center: Some(pp.p), radius: None };
                }
            }
            Tool::Detail { parent: Some(pid), center: Some(ctr), radius: None } => {
                if let Some(pl) = self.placement(pid) {
                    let r = len(sub(c, pl.to_sheet(ctr))) / (pl.scale * 1000.0);
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
                    self.edit_sheet(kind.name(), cx, |s| s.annotations.extend(anns));
                    self.tool = Tool::Dimension { kind, picks: vec![] };
                } else {
                    match kind {
                        DimTool::MaxMin | DimTool::Foreshorten => self.more_dimension_pick(kind, &mut picks),
                        _ => self.pk().dimension_click(kind, c, &mut picks),
                    }
                    self.tool = Tool::Dimension { kind, picks };
                }
            }
            Tool::HoleCallout { circle: None } => {
                if let Some(Pick::Circle { view, center, radius, r }) = self.circle_pick(c) {
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
                    None => match self.pk().silhouette_centerline(&a, &b) {
                        // A cylinder / cone silhouette: its axis, spanning the edge.
                        Some((p, q)) => {
                            let id = self.current_sheet().next_annotation_id();
                            self.edit_sheet("Edge to edge centerline", cx, |s| s.annotations.push(dt::edge_centerline(id, a.view, p, q, c)));
                        }
                        None => self.tool = Tool::EdgeCenterline { first: Some((a, b)) },
                    },
                    Some((a1, b1)) => {
                        if a.view != a1.view {
                            self.status = Some("Select an edge in the same view".into());
                            return;
                        }
                        // The middle line of the two edges (ends paired along the first edge).
                        let (p, q) = dt::edge_pair_centerline(a1.p, b1.p, a.p, b.p);
                        let id = self.current_sheet().next_annotation_id();
                        self.edit_sheet("Edge to edge centerline", cx, |s| s.annotations.push(dt::edge_centerline(id, a.view, p, q, c)));
                        self.tool = Tool::EdgeCenterline { first: None };
                    }
                }
            }
            Tool::CircleCenterline { three, mut points } => {
                if let Some(p) = self.snap_at(c).filter(|p| points.first().is_none_or(|f| f.view == p.view)) {
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
                // A click on a snap point anchors a leader; the next click
                // places the text.
                match (self.note_leader.is_some(), self.snap_at(c)) {
                    (false, Some(p)) => self.note_leader = Some(Leader { view: Some(p.view), point: p.p, nodes: Vec::new(), ..Default::default() }),
                    _ => self.open_note_panel(None, c, window, cx),
                }
            }
            Tool::Centerline { first: None } => {
                if let Some(p) = self.snap_at(c) {
                    self.tool = Tool::Centerline { first: Some(p) };
                }
            }
            Tool::Centerline { first: Some(a) } => {
                if let Some(b) = self.snap_at(c).filter(|b| b.view == a.view) {
                    let id = self.current_sheet().next_annotation_id();
                    self.edit_sheet("Centerline", cx, |s| s.annotations.push(dt::centerline(id, &a, &b, c)));
                    self.tool = Tool::Centerline { first: None };
                }
            }
            Tool::Balloon { anchor: None } => {
                if let Some(p) = self.any_point(c) {
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
                    self.edit_sheet("Callout", cx, |s| s.annotations.push(a));
                    self.tool = Tool::Balloon { anchor: None };
                }
            }
            _ => {}
        }
        self.update_preview(cx);
    }

    // ------------------------------------------------------------ mouse

    pub fn on_mouse_down(&mut self, e: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
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
        if e.button == MouseButton::Left && self.image_mouse_down() {
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
        if e.button == MouseButton::Left && self.align_centermark.is_some() && self.align_centermark_click(cx) {
            return;
        }
        if e.button == MouseButton::Left && self.rotate_to_edge.is_some() && self.rotate_to_edge_click(cx) {
            return;
        }
        if e.button == MouseButton::Left && self.align_views.is_some() && self.align_views_click(cx) {
            return;
        }
        if e.button == MouseButton::Left && e.click_count >= 2 && matches!(self.tool, Tool::Select) {
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
            && len(sub(lp, start_px)) > 3.0
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
                            self.start_drag(pick, start)
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
            Drag::MoveViews { start, orig, constraint } => {
                let mut d = sub(self.cursor, start);
                if let Some((axis, _)) = constraint {
                    let t = dot(d, axis);
                    d = [axis[0] * t, axis[1] * t];
                }
                let mut dr = self.drawing.clone();
                if let Some(s) = dr.sheets.get_mut(self.sheet) {
                    for (id, p) in &orig {
                        if let Some(v) = s.view_mut(*id) {
                            v.position = [p[0] + d[0], p[1] + d[1]];
                        }
                    }
                    // Notes and callouts grouped with a moved view go along.
                    for a in s.annotations.iter_mut().filter(|a| a.view_group.is_some_and(|v| orig.iter().any(|(o, _)| *o == v))) {
                        a.position = [a.position[0] + d[0], a.position[1] + d[1]];
                    }
                }
                self.set_preview(Some(dr), cx);
            }
            Drag::SelectCells { .. } => self.update_preview(cx),
            Drag::MoveTable { .. } | Drag::ResizeTable { .. } | Drag::TableDivider { .. } | Drag::MoveSectionLine { .. } | Drag::MoveDetailCircle { .. } | Drag::ResizeDetailCircle { .. } | Drag::MoveDetailLabel { .. } | Drag::MoveSectionPoint { .. } | Drag::AuxIndicator { .. } | Drag::NoteGrip { .. } | Drag::MoveViewLabel { .. } => {
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
            // A datum from the model moves along its face normal only.
            Drag::MoveAnnotation { start, orig } if orig.model.is_some() => self.drag_model_datum(&orig, start, cx),
            Drag::MoveAnnotation { start, orig } => {
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
            Drag::CentermarkArm { orig, k } => {
                let a = self.dragged_centermark(&orig, k, e.modifiers.shift);
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
                    (x.kind, x.refs) = (a.kind, a.refs);
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
                if !matches!(self.tool, Tool::Select) {
                    // Placing a dimension or annotation: align with the others
                    // (Shift turns inferencing off).
                    self.infer_lines.clear();
                    if !e.modifiers.shift && self.pending_annotation().is_some() {
                        self.infer_placement(None);
                    } else if !e.modifiers.shift && self.placing_table() {
                        self.infer_table_placement();
                    }
                    self.update_preview(cx);
                }
            }
        }
    }

    /// A table tool waiting for the placing click.
    pub fn placing_table(&self) -> bool {
        match &self.tool {
            Tool::Table { kind: TableTool::Custom, .. } => self.custom_dlg_placing(),
            Tool::Table { kind: TableTool::General | TableTool::Revision, .. } => true,
            Tool::Table { kind: TableTool::Hole, view: Some(_), origin, .. } => origin.is_some(),
            Tool::Table { view: Some(_), .. } => true,
            _ => false,
        }
    }

    /// Placing a table: the cursor (the table's fixed corner) snaps to the
    /// x / y of the border frame, the title block, views and other tables
    /// nearby, shown as dashed inference lines.
    pub fn infer_table_placement(&mut self) {
        let tol = self.tol_mm(6.0);
        let c = self.cursor;
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
    /// other dimensions and annotations nearby, shown as red dashed lines.
    pub fn infer_placement(&mut self, skip: Option<u64>) {
        let tol = self.tol_mm(6.0);
        let c = self.cursor;
        let sheet = self.current_sheet();
        let pts: Vec<[f64; 2]> = sheet.annotations.iter().filter(|a| Some(a.id) != skip).map(|a| a.position).chain(sheet.tables.iter().map(|t| t.position)).collect();
        let best = |axis: usize| pts.iter().filter(|p| (p[axis] - c[axis]).abs() < tol).min_by(|a, b| (a[axis] - c[axis]).abs().total_cmp(&(b[axis] - c[axis]).abs())).copied();
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
        if e.button == MouseButton::Left && !e.modifiers.shift && matches!(self.drag, Drag::Pending { .. }) && !matches!(self.tool, Tool::Select) && self.pending_annotation().is_some() {
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
            Drag::MoveViews { .. } => {
                if let Some(p) = self.preview.take() {
                    self.commit("Move view", p, cx);
                }
            }
            Drag::MoveAnnotation { orig, .. } => {
                if let Some(mut p) = self.preview.take() {
                    self.group_callout_on_drop(&orig, &mut p);
                    self.commit("Move annotation", p, cx);
                }
            }
            Drag::Jog { .. } | Drag::Reattach { .. } | Drag::CentermarkArm { .. } => {
                if let Some(p) = self.preview.take() {
                    let name = match drag {
                        Drag::Jog { .. } => "Move jog",
                        Drag::Reattach { .. } => "Re-attach dimension",
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
            Drag::MoveTable { .. } | Drag::ResizeTable { .. } | Drag::TableDivider { .. } | Drag::MoveSectionLine { .. } | Drag::MoveDetailCircle { .. } | Drag::ResizeDetailCircle { .. } | Drag::MoveDetailLabel { .. } | Drag::MoveSectionPoint { .. } | Drag::AuxIndicator { .. } | Drag::NoteGrip { .. } | Drag::MoveViewLabel { .. } | Drag::LeaderBend { .. } | Drag::LeaderEnd { .. } => {
                let name = match drag {
                    Drag::LeaderBend { .. } | Drag::LeaderEnd { .. } => "Move by grip",
                    Drag::MoveTable { .. } => "Move table",
                    Drag::TableDivider { .. } => "Resize cells",
                    Drag::ResizeTable { .. } => "Resize table",
                    Drag::MoveSectionLine { .. } => "Move cutting line",
                    Drag::MoveSectionPoint { .. } => "Edit cutting line",
                    Drag::AuxIndicator { .. } => "Edit view reference indicator",
                    Drag::MoveViewLabel { .. } => "Move view label",
                    Drag::NoteGrip { grip: 2, .. } => "Rotate note",
                    Drag::NoteGrip { .. } => "Resize note",
                    _ => "Move detail circle",
                };
                if let Some(p) = self.preview.take() {
                    self.commit(name, p, cx);
                }
            }
            _ => {}
        }
        cx.notify();
    }

    /// Wheel / two-finger scroll: zoom about the pointer with Onshape's amounts
    /// (navigation.rs); horizontal scrolls do nothing.
    pub fn on_scroll(&mut self, e: &ScrollWheelEvent, _window: &mut Window, cx: &mut Context<Self>) {
        let lp = self.local(e.position);
        let reverse = nav::MouseControls::current().reverse_scroll;
        let Some(amount) = nav::wheel_zoom_amount(e.delta, nav::Platform::current(), reverse, e.modifiers.shift) else { return };
        self.zoom_by(1.0 / nav::zoom_scale(amount), lp, cx);
    }

    /// Trackpad pinch: the sheet scales with the fingers about their centre.
    pub fn on_pinch(&mut self, e: &PinchEvent, _window: &mut Window, cx: &mut Context<Self>) {
        if e.delta == 0.0 {
            return;
        }
        let lp = self.local(e.position);
        self.zoom_by(nav::pinch_ratio(e.delta), lp, cx);
    }

    pub fn on_key(&mut self, e: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let k = &e.keystroke;
        // Ctrl+M: maximum / minimum dimension (Onshape).
        if (k.modifiers.control || k.modifiers.platform) && !k.modifiers.shift && k.key == "m" && self.panel.is_none() {
            self.set_tool(Tool::Dimension { kind: DimTool::MaxMin, picks: vec![] }, cx);
            cx.stop_propagation();
            return;
        }
        // Ctrl+S: Sheets panel (Onshape's "Display sheet menu").
        if (k.modifiers.control || k.modifiers.platform) && !k.modifiers.shift && k.key == "s" {
            self.toggle_sheets_panel(cx);
            cx.stop_propagation();
            return;
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
        if self.linestyle_click(cx) || self.hide_edges_click(cx) || self.section_edit_click(cx) || self.replicate_click(cx) {
            return;
        }
        // A click on the sheet keeps the palette's changes.
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
        let sel = self.sel_at(pick, self.cursor);
        if self.table_cell_click(sel, m.shift, cx) {
            return;
        }
        // Onshape: selection is additive with a plain click (clicking a
        // selected entity again deselects it); clicking empty space clears.
        match sel {
            Some(s) => {
                if let Some(i) = self.selection.iter().position(|x| *x == s) {
                    self.selection.remove(i);
                } else {
                    self.selection.push(s);
                }
            }
            None => self.selection.clear(),
        }
        self.status = None;
        self.redraw(cx);
    }

    /// Alt+drag (Onshape: copy the hovered entity): leave a duplicate in place.
    fn alt_duplicate(&mut self, pick: Option<u32>, start: [f64; 2], cx: &mut Context<Self>) {
        let Some(sel) = self.sel_at(pick, start) else { return };
        match sel {
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
            Sel::View(id) => {
                if let Ok(clip) = self.drawing.copy_views(self.sheet, &[id], Some(&self.sources)) {
                    self.edit_sheet("Copy view", cx, |s| {
                        // The copy keeps the original's place: paste around the same center.
                        let n = clip.sheet.views.len().max(1) as f64;
                        let c = clip.sheet.views.iter().fold([0.0, 0.0], |acc, v| [acc[0] + v.position[0] / n, acc[1] + v.position[1] / n]);
                        s.paste_views(&clip, Some(c));
                    });
                }
            }
            Sel::Break(..) | Sel::Sketch(_) | Sel::Hatch(_) | Sel::Image(_) | Sel::AutoCentermark(..) | Sel::ViewLabel(_) | Sel::AuxIndicator(_) => {}
        }
    }

    fn start_drag(&mut self, pick: Option<u32>, start: [f64; 2]) -> Drag {
        if let Some(d) = self.leader_bend_drag(start).or_else(|| self.leader_end_drag(start)) {
            return d;
        }
        if let Some(d) = self.note_grip_drag(start) {
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
            return Drag::CentermarkArm { orig, k };
        }
        let Some(sel) = self.sel_at(pick, start) else { return Drag::None };
        if !self.selection.contains(&sel) {
            self.selection = vec![sel];
        }
        let sheet = &self.drawing.sheets[self.sheet];
        match sel {
            Sel::View(id) => {
                // Move the view with all views derived from it; a projected
                // (or section/auxiliary) view slides along its alignment axis.
                // Views with "Suppress alignment with parent" stay where they are.
                let ids: Vec<ViewId> = sheet.descendants(id).into_iter().filter(|v| !v.is_ghost() && (*v == id || !sheet.view(*v).is_some_and(|x| x.unaligned))).collect();
                let orig = ids.iter().filter_map(|v| sheet.view(*v).map(|x| (*v, x.position))).collect();
                let prot = sheet.parent_of(id).map(|p| views::view_rotation(sheet, p)).unwrap_or(0.0);
                // A view linked to a parent on another sheet moves freely (live x (5)).
                let unaligned = sheet.view(id).is_some_and(|v| v.unaligned) || sheet.linked_parent(id).is_some();
                let constraint = match &sheet.view(id).map(|v| v.kind.clone()).filter(|_| !unaligned) {
                    Some(ViewKind::Projected { .. }) => views::projected_direction(sheet, id).filter(|d| d[0] == 0.0 || d[1] == 0.0).map(|d| views::rotate2(d, prot)).map(|d| (d, d)),
                    Some(ViewKind::Section { .. } | ViewKind::AlignedSection { .. }) => views::section_cut(sheet, id).map(|c| views::rotate2(c.toward_view, prot)).map(|d| (d, d)),
                    Some(ViewKind::Auxiliary { edge_dir, .. }) => {
                        let n = norm([-edge_dir[1], edge_dir[0]]);
                        Some((n, n))
                    }
                    _ => None,
                };
                Drag::MoveViews { start, orig, constraint }
            }
            Sel::Annotation(aid) => match self.built.as_ref().and_then(|b| b.resolved.annotations.iter().find(|a| a.id == aid).cloned()) {
                Some(a) if !matches!(a.kind, AnnotationKind::Centermark { .. } | AnnotationKind::Centerline { .. }) => Drag::MoveAnnotation { start, orig: a },
                _ => Drag::None,
            },
            // Drag a sketch entity (its points move, the sketch solves).
            Sel::Sketch(id) => Drag::MoveSketch { id, start, orig: sheet.sketch.clone() },
            Sel::Image(id) => match sheet.images.iter().find(|i| i.id == id) {
                Some(im) => Drag::MoveImage { id, start, orig: im.pos },
                None => Drag::None,
            },
            Sel::ViewLabel(v) => self.label_drag(v, start),
            // Dragging the indicator's line (off its dots) does nothing (live u-914).
            Sel::Table(_) | Sel::Break(..) | Sel::Hatch(_) | Sel::AutoCentermark(..) | Sel::AuxIndicator(_) => Drag::None,
        }
    }

    /// The annotation `orig` (resolved) dragged from `start` to the cursor.
    fn dragged_annotation(&self, orig: &Annotation, start: [f64; 2]) -> Annotation {
        self.pk().dragged_annotation(orig, start)
    }

    /// A double click opens what's under the pointer; a triple click on a
    /// note opens it with all its text selected (help H:note).
    fn double_click(&mut self, triple: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.update_hover(cx);
        let Some(h) = self.hover else { return };
        match self.built.as_ref().and_then(|b| b.picks.get(h as usize)).cloned() {
            Some(PickTarget::TitleBlock) => self.open_title_panel(window, cx),
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
                    Some(a) => self.open_annotation_panel(&a, window, cx),
                    None => {}
                }
            }
            Some(PickTarget::Table(id)) => {
                // On a divider: fit it to the text; in a cell: edit the cell;
                // a custom table's fixed corner: its properties (help).
                if self.custom_corner_at(id) {
                    self.open_custom_props(id, cx);
                } else if !self.fit_table_divider(id, cx) {
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
        if !matches!(self.tool, Tool::Select) {
            self.cancel(cx);
            return;
        }
        let sel = self.sel_at(pick, self.cursor);
        if let Some(s) = sel
            && !self.selection.contains(&s)
        {
            self.selection = vec![s];
            self.redraw(cx);
        }
        self.menu_at = self.local(position);
        // Exactly on a note or callout leader: its own menu (live drw2 10-21
        // g (3), (4), (6)).
        // On a detail view's circle in its parent: the circle's menu (live
        // asb 10-23 views (2)).
        let target = pick.and_then(|p| self.built.as_ref()?.picks.get(p as usize).cloned());
        let circle = target.as_ref().and_then(|t| if let PickTarget::DetailCircle(v) = t { Some(*v) } else { None });
        let cut_line = target.as_ref().and_then(|t| if let PickTarget::SectionLine(v) = t { Some(*v) } else { None });
        self.menu_broken_out = target.and_then(|t| if let PickTarget::BrokenOut { view, index } = t { Some((view, index)) } else { None });
        let entries = match (self.leader_hit(self.cursor), circle, cut_line) {
            (Some(hit), _, _) => self.leader_entries(hit, cx),
            (None, Some(v), _) => self.detail_circle_entries(v, cx),
            (None, None, Some(v)) => self.section_line_entries(v, cx),
            (None, None, None) => self.context_entries(sel, cx),
        };
        let ed = cx.entity();
        // Onshape's right-click menu (os_menu: 12 px, 22 px rows).
        let menu = crate::os_menu::OsMenu::build(window, cx, move |m, window, cx| fill_os_menu(m, &entries, &ed, window, cx));
        if !menu.read(cx).is_empty() {
            crate::os_menu::open(position, menu, window, cx);
        }
        cx.notify();
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
                t.start_paste_table(cx);
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

    fn context_entries(&self, sel: Option<Sel>, cx: &App) -> Vec<MenuEntry> {
        let mut out = Vec::new();
        // Several entities selected (live drawing/291).
        if self.selection.len() > 1 && sel.is_some_and(|s| self.selection.contains(&s)) {
            return self.multi_selection_entries(cx);
        }
        match sel {
            Some(Sel::View(id)) => out = self.onshape_view_entries(id, cx),
            Some(Sel::AuxIndicator(id)) => return Self::aux_indicator_entries(id, crate::drawing_props::paste_entry(cx)),
            Some(Sel::ViewLabel(id)) => out = self.label_entries(id, cx),
            Some(Sel::Annotation(id)) => {
                // Linear dimensions: Onshape's own menu.
                if let Some(entries) = self.dimension_entries(id).or_else(|| self.ordinate_entries(id, cx)) {
                    return entries;
                }
                // Centermarks (live drawing/487): Paste as table, Align
                // centermark | Clear selection, Zoom to fit.
                if self.current_sheet().annotations.iter().any(|a| a.id == id && matches!(a.kind, AnnotationKind::Centermark { .. })) {
                    return vec![
                        MenuEntry::item("Paste as table", |t, _, cx| {
                            t.start_paste_table(cx);
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
                // Notes: their own menu in live's order (drawing_leaders).
                if let Some(a) = a.as_ref().filter(|a| matches!(a.kind, AnnotationKind::Note { .. })) {
                    let mut out = self.inspection_entries(a);
                    out.extend(self.note_entries(a, cx));
                    return out;
                }
                if let Some(a) = &a {
                    out.extend(self.inspection_entries(a));
                    if let AnnotationKind::Note { .. } = a.kind {
                        let pos = a.position;
                        out.push(MenuEntry::item("Edit note...", move |t, w, cx| t.open_note_panel(Some(id), pos, w, cx)));
                    }
                    if let AnnotationKind::LinearDimension { orientation, .. } = a.kind {
                        for (label, o) in [("Horizontal", DimOrientation::Horizontal), ("Vertical", DimOrientation::Vertical), ("Aligned", DimOrientation::Aligned)] {
                            out.push(MenuEntry::check(label, orientation == o, move |t, _, cx| {
                                t.edit_sheet("Dimension orientation", cx, |s| {
                                    if let Some(AnnotationKind::LinearDimension { orientation, .. }) = s.annotations.iter_mut().find(|x| x.id == id).map(|x| &mut x.kind) {
                                        *orientation = o;
                                    }
                                })
                            }));
                        }
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
                        let sel_view = self.selection.iter().find_map(|s| if let Sel::View(v) = s { Some(*v) } else { None });
                        let anns: Vec<u64> = self.selection.iter().filter_map(|s| if let Sel::Annotation(x) = s { Some(*x) } else { None }).chain([id]).collect();
                        if a.view_group.is_some() {
                            out.push(MenuEntry::item("Remove from group", move |t, _, cx| t.edit_sheet("Remove from group", cx, |s| noshape_scene::drawing_tools::annotation_edit::group_with_view(s, &[id], None))));
                        } else if let Some(v) = sel_view {
                            let ids = anns.clone();
                            out.push(MenuEntry::item("Group", move |t, _, cx| t.edit_sheet("Group", cx, |s| noshape_scene::drawing_tools::annotation_edit::group_with_view(s, &ids, Some(v)))));
                        }
                        if a.ann_group.is_some() {
                            let ids = anns.clone();
                            out.push(MenuEntry::item("Remove annotation from group", move |t, _, cx| t.edit_sheet("Remove annotation from group", cx, |s| noshape_scene::drawing_tools::annotation_edit::group_annotations(s, &ids, false))));
                        } else if anns.iter().collect::<std::collections::HashSet<_>>().len() >= 2 {
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
                    // Move to ▸ a format layer (or back to the drawing).
                    if matches!(a.kind, AnnotationKind::Note { .. } | AnnotationKind::Balloon { .. } | AnnotationKind::Gdt { .. } | AnnotationKind::Datum { .. } | AnnotationKind::DatumTarget { .. } | AnnotationKind::SurfaceFinish { .. } | AnnotationKind::Weld { .. }) {
                        let cur = a.layer;
                        let layers = SheetLayer::FORMAT.iter().copied().chain((!cur.is_drawing()).then_some(SheetLayer::Drawing));
                        let items = layers
                            .filter(|l| *l != cur)
                            .map(|l| {
                                MenuEntry::item(l.name(), move |t, _, cx| {
                                    let ids: Vec<u64> = t.selection.iter().filter_map(|s| if let Sel::Annotation(x) = s { Some(*x) } else { None }).chain([id]).collect();
                                    t.edit_sheet("Move to", cx, |s| s.annotations.iter_mut().filter(|x| ids.contains(&x.id)).for_each(|x| x.layer = l));
                                })
                            })
                            .collect();
                        out.push(MenuEntry::Submenu("Move to".into(), items));
                    }
                    if !a.refs.is_empty() {
                        out.push(MenuEntry::item("Detach from geometry", move |t, _, cx| {
                            t.edit_sheet("Detach", cx, |s| noshape_scene::drawing_tools::annotation_edit::detach(s, id))
                        }));
                    }
                    // Paste as table is in every drawing menu (live drawing/290).
                    out.push(MenuEntry::item("Paste as table", |t, _, cx| {
                        t.start_paste_table(cx);
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
            Some(Sel::Sketch(id)) => out.extend(self.sketch_entries(id)),
            Some(Sel::Image(id)) => out.extend(self.image_entries(id)),
            // Live 483–493: an automatic centermark's menu.
            Some(Sel::AutoCentermark(..)) => {
                out.push(MenuEntry::item("Paste as table", |t, _, cx| {
                    t.start_paste_table(cx);
                }));
                out.push(MenuEntry::Separator);
                out.push(MenuEntry::item("Clear selection", |t, _, cx| {
                    t.selection.clear();
                    t.redraw(cx);
                }));
                out.push(MenuEntry::item("Zoom to fit", |t, _, cx| t.zoom_to_fit(cx)));
            }
            Some(Sel::Hatch(id)) => {
                out.push(MenuEntry::item("Edit hatch", move |t, w, cx| t.open_hatch_dialog(Some(id), w, cx)));
                out.push(MenuEntry::Separator);
                out.push(MenuEntry::item("Delete", move |t, _, cx| {
                    t.selection.clear();
                    t.edit_sheet("Delete hatch", cx, |s| s.sketch.hatches.retain(|h| h.id != id));
                }));
            }
            Some(Sel::Break(view, index)) => out.extend(self.break_entries(view, index)),
            None => {
                // Onshape's empty-sheet menu (live drawing/290): [Paste,]
                // Paste as table | Sheet properties…, Drawing properties… |
                // Zoom to fit, Zoom to window, Zoom to sheet.
                out.extend(crate::drawing_props::paste_entry(cx));
                out.push(MenuEntry::item("Paste as table", |t, _, cx| {
                    t.start_paste_table(cx);
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

