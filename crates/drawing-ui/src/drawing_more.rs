//! Drawing editor, third part: broken-out sections, partial / angular
//! section editing (cutting line points, angle), polygon and spline crops,
//! max/min and foreshortened dimensions, ordinate groups, datum targets,
//! GD&T leaders, table cell formatting and merging, moving views between
//! sheets, templates and title block layouts.

use crate::drawing_scene::Overlay;
use noshape_scene::drawing_tools::table_edit as te;
use crate::drawing_view::*;
use crate::theme::p;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::Input;
use crate::os_ui::OsDropdownExt as _;
use gpui_kit::component::{Sizable as _, h_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_model::drawing::views;
use noshape_model::drawing::*;
use noshape_model::drawing::format;

fn sub(a: [f64; 2], b: [f64; 2]) -> [f64; 2] {
    [a[0] - b[0], a[1] - b[1]]
}
fn dot(a: [f64; 2], b: [f64; 2]) -> f64 {
    a[0] * b[0] + a[1] * b[1]
}
fn len(a: [f64; 2]) -> f64 {
    dot(a, a).sqrt()
}

const AREAS: [&str; 3] = ["Point", "Circle", "Rectangle"];
const SCOPES: [&str; 4] = ["Cell", "Row", "Column", "Table"];

impl DrawingEditor {
    // ------------------------------------------------------------ prompts

    pub fn more_prompt(&self) -> Option<&'static str> {
        Some(match &self.tool {
            Tool::Dimension { kind: DimTool::MaxMin, picks } => match picks.len() {
                0 => "Select a circle or arc: click its near side for a minimum, its far side for a maximum",
                1 => "Select a point or another circle",
                _ => "Click to place the dimension",
            },
            Tool::Dimension { kind: DimTool::Foreshorten, picks } => match picks.len() {
                0 => "Select the axis: a centerline (or automatic centerline) or a straight edge",
                1 => "Select the cylinder's silhouette edge or a point on it",
                _ => "Click to place the dimension",
            },
            Tool::BrokenOut { done: false, .. } => "Click the points of the closed profile in a view; Enter or double-click to close it",
            Tool::BrokenOut { .. } => "Enter the depth in the panel, or click a point that sets the depth (up to entity)",
            Tool::CropPoly { .. } => "Click the points of the crop boundary in a view, then the first point again to close it",
            Tool::DatumTarget { anchor: None } => "Click the target point",
            Tool::DatumTarget { .. } => "Click to place the target callout",
            Tool::GdtLeader { .. } => "Click the edge or point the leader points to",
            _ => return None,
        })
    }

    // ------------------------------------------------------------ picking

    /// Clicks of the max/min and foreshortened dimension tools.
    pub fn more_dimension_pick(&mut self, kind: DimTool, picks: &mut Vec<Pick>) {
        self.pk().more_dimension_pick(kind, picks)
    }

    // ------------------------------------------------------------ previews

    /// Closed profile through view points (spline or polygon) on the sheet.
    pub fn profile_on_sheet(&self, view: ViewId, pts: &[[f64; 2]], spline: bool) -> Vec<[f64; 2]> {
        let Some(pl) = self.placement(view) else { return vec![] };
        let pts = if spline { views::closed_spline(pts, 12) } else { pts.to_vec() };
        let mut out: Vec<[f64; 2]> = pts.iter().map(|p| pl.to_sheet(*p)).collect();
        if let Some(f) = out.first().copied() {
            out.push(f);
        }
        out
    }

    pub fn more_overlay(&self, overlay: &mut Overlay) {
        // Table cells: the drag-select rubber band (tan) and the selected
        // block (blue, 2 px; help images table-select-cells-01, table-copy-01).
        let rect = |a: [f64; 2], b: [f64; 2]| vec![a, [b[0], a[1]], b, [a[0], b[1]], a];
        if let Drag::SelectCells { start, .. } = self.drag {
            overlay.tinted.push((rect(start, self.cursor), [249, 229, 193, 255], 2.0));
        } else if let Some((a, b)) = self.cell_sel_rect() {
            overlay.tinted.push((rect(a, b), [33, 96, 168, 255], 2.0));
        }
        let c = self.hover_snap.unwrap_or(self.cursor);
        match &self.tool {
            Tool::BrokenOut { parent: Some(pid), points, .. } | Tool::CropPoly { parent: Some(pid), points, .. } if !points.is_empty() => {
                let Some(pl) = self.placement(*pid) else { return };
                let spline = matches!(self.tool, Tool::BrokenOut { .. } | Tool::CropPoly { spline: true, .. });
                let done = matches!(self.tool, Tool::BrokenOut { done: true, .. });
                let mut pts = points.clone();
                if !done {
                    pts.push(pl.to_view(c));
                }
                overlay.lines.push((self.profile_on_sheet(*pid, &pts, spline), true));
                overlay.points.extend(points.iter().map(|p| pl.to_sheet(*p)));
            }
            Tool::DatumTarget { anchor: Some(a) } => {
                if let Some(s) = self.pick_sheet(a) {
                    overlay.lines.push((vec![s, self.cursor], true));
                }
            }
            Tool::GdtLeader { id } => {
                // A note's branch rubber-bands from its bend or node.
                let from = self.branch_point(*id).or_else(|| self.current_sheet().annotations.iter().find(|a| a.id == *id).map(|a| a.position));
                if let Some(from) = from {
                    overlay.lines.push((vec![from, c], true));
                    // Live (4): the rubber band ends in an arrowhead.
                    self.rubber_band_arrow(overlay, from, c);
                }
            }
            _ => {}
        }
    }

    // ------------------------------------------------------------ clicks

    /// Sheet point `c` is on a profile's first point (view coordinates), within
    /// 8 px on screen.
    fn on_first_point(&self, view: ViewId, points: &[[f64; 2]], c: [f64; 2]) -> bool {
        let (Some(pl), Some(first)) = (self.placement(view), points.first()) else { return false };
        let f = pl.to_sheet(*first);
        (f[0] - c[0]).hypot(f[1] - c[1]) * self.zoom <= 8.0
    }

    pub fn more_tool_click(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let c = self.cursor;
        match self.tool.clone() {
            Tool::BrokenOut { parent, mut points, done: false } => {
                if let Some(pp) = self.view_point(parent) {
                    if let Some(why) = noshape_scene::drawing_tools::broken_out_refused(self.current_sheet(), pp.view) {
                        self.status = Some(why.into());
                        cx.notify();
                        return true;
                    }
                    if parent.is_none() {
                        points.clear();
                    }
                    points.push(pp.p);
                    self.tool = Tool::BrokenOut { parent: Some(pp.view), points, done: false };
                }
            }
            Tool::BrokenOut { done: true, .. } => {
                // Up to entity: a snap point (in any view of the same model) sets the depth.
                if let Some(p) = self.snap_at(c)
                    && let Some(r) = p.r.clone()
                    && let Some(Panel::BrokenOut { depth_ref, .. }) = &mut self.panel
                {
                    *depth_ref = Some(r);
                    self.status = Some("Depth: up to the selected point".into());
                }
            }
            Tool::CropPoly { parent, mut points, spline } => {
                if self.crop_add_point_click(c, cx) {
                    return true;
                }
                // A click on the first point closes the profile (live).
                if let Some(view) = parent
                    && points.len() >= 3
                    && self.on_first_point(view, &points, c)
                {
                    let crop = if spline { Crop::Spline(points) } else { Crop::Polygon(points) };
                    self.crop_done(view, crop, cx);
                    return true;
                }
                // The first point: the view under it, snapping only to that
                // view's points (a neighbour's corner nearby took the crop).
                let target = parent.or_else(|| self.pk().view_containing(c));
                if let Some(pp) = self.view_point(target) {
                    if self.crop_refused(pp.view) {
                        return true;
                    }
                    if parent.is_none() {
                        points.clear();
                    }
                    points.push(pp.p);
                    self.tool = Tool::CropPoly { parent: Some(pp.view), points, spline };
                }
            }
            Tool::DatumTarget { anchor: None } => {
                if self.datum_origin_click(window, cx) {
                    return true;
                }
                if let Some(p) = self.any_point(c) {
                    self.tool = Tool::DatumTarget { anchor: Some(p) };
                }
            }
            Tool::DatumTarget { anchor: Some(_) } => {
                if let Some(a) = self.pending_annotation() {
                    if let AnnotationKind::DatumTarget { label, .. } = &a.kind {
                        self.datum_target_letter = label.chars().next();
                    }
                    self.tool = Tool::DatumTarget { anchor: None };
                    // The Datum dialog lists it until ✓.
                    if !self.datum_target_add(a.clone(), window, cx) {
                        self.edit_sheet("Datum target", cx, |s| s.annotations.push(a));
                    }
                }
            }
            Tool::GdtLeader { id } => {
                let leader = noshape_scene::drawing_tools::annotation_edit::leader_to(&self.pk(), c);
                let start = self.leader_from.take().unwrap_or_default();
                self.edit_sheet("Add leader", cx, |s| noshape_scene::drawing_tools::annotation_edit::add_leader(s, id, leader, start));
                self.tool = Tool::Select;
                self.selection = vec![Sel::Annotation(id)];
            }
            _ => return false,
        }
        let _ = window;
        true
    }

    /// Enter / double-click for the profile tools.
    pub fn more_confirm(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        match self.tool.clone() {
            Tool::BrokenOut { parent: Some(view), points, done: false } if points.len() >= 3 => {
                self.tool = Tool::BrokenOut { parent: Some(view), points: points.clone(), done: true };
                // Default depth: half the model's depth along the line of sight.
                let half = self.pk().broken_out_default_depth(view);
                let u = format::length_units(&self.drawing, &DimFormat::default());
                let depth = self.input(&format::format_number(half / u.meters(), None, 3), window, cx);
                Self::focus_input(&depth, window, cx);
                self.panel = Some(Panel::BrokenOut { view, edit: None, profile: points, spline: true, depth, depth_ref: None });
                self.update_preview(cx);
                true
            }
            Tool::CropPoly { parent: Some(view), points, spline } if points.len() >= 3 => {
                let crop = if spline { Crop::Spline(points) } else { Crop::Polygon(points) };
                // The Crop view dialog's ✓ applies it.
                self.crop_done(view, crop, cx);
                true
            }
            _ => false,
        }
    }

    // ------------------------------------------------------------ section line handles

    /// Sheet positions of the cutting line points of selected section views.
    pub fn selection_handles(&self) -> Vec<[f64; 2]> {
        if !matches!(self.tool, Tool::Select) {
            return vec![];
        }
        let sheet = self.current_sheet();
        let mut out = Vec::new();
        for s in &self.selection {
            let Sel::View(v) = s else { continue };
            let (Some(pts), Some(parent)) = (sheet.section_points(*v), sheet.parent_of(*v)) else { continue };
            let Some(pl) = self.placement(parent) else { continue };
            out.extend(pts.iter().map(|p| pl.to_sheet(*p)));
        }
        out
    }

    /// Index of the cutting line point of section view `view` near sheet point `s`.
    pub fn section_point_at(&self, view: ViewId, s: [f64; 2]) -> Option<usize> {
        let sheet = self.current_sheet();
        let pl = self.placement(sheet.parent_of(view)?)?;
        let tol = self.tol_mm(8.0);
        sheet
            .section_points(view)?
            .iter()
            .enumerate()
            .map(|(i, p)| (i, len(sub(pl.to_sheet(*p), s))))
            .filter(|(_, d)| *d < tol)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(i, _)| i)
    }

    // ------------------------------------------------------------ menus


    /// Move the selected views (with `id`) to sheet `to` (None = a new sheet).
    pub fn move_selected_views(&mut self, id: ViewId, to: Option<usize>, cx: &mut Context<Self>) {
        let mut ids: Vec<ViewId> = self.selection.iter().filter_map(|s| if let Sel::View(v) = s { Some(*v) } else { None }).collect();
        if !ids.contains(&id) {
            ids = vec![id];
        }
        let mut d = self.drawing.clone();
        let to = to.unwrap_or_else(|| d.add_sheet(self.sheet));
        // Live (behaviours "Views linked across sheets"): the views move alone, linked to their parents.
        match d.move_views_to_sheet_linked(self.sheet, &ids, to) {
            Ok(moved) => {
                let name = d.sheets[to].name.clone();
                self.commit("Move to sheet", d, cx);
                self.switch_sheet(to, cx);
                // Live drawing/286: the moved object stays selected.
                self.selection = moved.iter().map(|(_, n)| Sel::View(*n)).collect();
                self.status = Some(format!("Moved to {name}").into());
            }
            Err(e) => self.status = Some(e.into()),
        }
        cx.notify();
    }

    pub fn annotation_more_entries(&self, a: &Annotation) -> Vec<MenuEntry> {
        let mut out = Vec::new();
        let id = a.id;
        if matches!(a.kind, AnnotationKind::Centermark { .. }) {
            out.push(MenuEntry::item("Align centermark", move |t, _, cx| t.start_align_centermark(id, cx)));
        }
        let set_kind = move |t: &mut DrawingEditor, name: &str, kind: AnnotationKind, cx: &mut Context<DrawingEditor>| {
            t.edit_sheet(name, cx, |s| {
                if let Some(x) = s.annotations.iter_mut().find(|x| x.id == id) {
                    x.kind = kind;
                }
            })
        };
        match &a.kind {
            AnnotationKind::Gdt { leaders, .. } => {
                out.push(MenuEntry::item("Add leader", move |t, _, cx| t.set_tool(Tool::GdtLeader { id }, cx)));
                if !leaders.is_empty() {
                    out.push(MenuEntry::item("Remove leaders", move |t, _, cx| t.edit_sheet("Remove leaders", cx, |s| noshape_scene::drawing_tools::annotation_edit::remove_leaders(s, id))));
                }
            }
            // Callouts and surface finish symbols (help callout_balloon,
            // surface_finish): Add leader / Remove leaders.
            AnnotationKind::Balloon { callout, .. } => {
                out.push(MenuEntry::item("Add leader", move |t, _, cx| t.set_tool(Tool::GdtLeader { id }, cx)));
                if !callout.leaders.is_empty() {
                    out.push(MenuEntry::item("Remove leaders", move |t, _, cx| t.edit_sheet("Remove leaders", cx, |s| noshape_scene::drawing_tools::annotation_edit::remove_leaders(s, id))));
                }
            }
            AnnotationKind::SurfaceFinish { leaders, .. } => {
                out.push(MenuEntry::item("Add leader", move |t, _, cx| t.set_tool(Tool::GdtLeader { id }, cx)));
                if !leaders.is_empty() {
                    out.push(MenuEntry::item("Remove leaders", move |t, _, cx| t.edit_sheet("Remove leaders", cx, |s| noshape_scene::drawing_tools::annotation_edit::remove_leaders(s, id))));
                }
            }
            AnnotationKind::Weld { spec, .. } => {
                out.push(MenuEntry::item("Edit...", move |t, w, cx| t.open_weld_edit(id, w, cx)));
                out.push(MenuEntry::item("Add leader", move |t, _, cx| t.set_tool(Tool::GdtLeader { id }, cx)));
                if !spec.leaders.is_empty() {
                    out.push(MenuEntry::item("Remove leaders", move |t, _, cx| t.edit_sheet("Remove leaders", cx, |s| noshape_scene::drawing_tools::annotation_edit::remove_leaders(s, id))));
                }
            }
            // Live's radial menu has no Foreshorten (qa DWG-FORESHORTEN-JUMP,
            // behaviours D14: only the toolbar tool); one made before keeps
            // Remove foreshorten.
            AnnotationKind::ForeshortenedRadial { .. } => {
                if let Some(k) = noshape_scene::drawing_tools::annotation_edit::unforeshortened(&a.kind) {
                    out.push(MenuEntry::item("Remove foreshorten", move |t, _, cx| set_kind(t, "Remove foreshorten", k.clone(), cx)));
                }
            }
            &AnnotationKind::ForeshortenedDiameter { view, axis, edge, direct } => {
                let label = if direct { "Display as foreshorten diameter" } else { "Display as direct diameter" };
                out.push(MenuEntry::item(label, move |t, _, cx| set_kind(t, "Foreshortened diameter", AnnotationKind::ForeshortenedDiameter { view, axis, edge, direct: !direct }, cx)));
            }
            &AnnotationKind::OrdinateGroup { view, origin, horizontal, .. } => {
                let r = a.refs.iter().find(|x| x.slot == 0).map(|x| x.point.clone());
                out.push(MenuEntry::item("Add to ordinate group", move |t, _, cx| {
                    let o = PickedPoint { view, p: origin, r: r.clone() };
                    t.set_tool(Tool::Ordinate { origin: Some(o), horizontal: Some(horizontal), group: Some(id) }, cx)
                }));
            }
            AnnotationKind::MaxMinDimension { ra, rb, .. } => {
                for (end, r) in [(0usize, *ra), (1, *rb)] {
                    if r <= 0.0 {
                        continue;
                    }
                    let entries = [("Center", ArcCondition::Center), ("Minimum", ArcCondition::Min), ("Maximum", ArcCondition::Max)]
                        .into_iter()
                        .map(|(n, cnd)| {
                            let cur = match &a.kind {
                                AnnotationKind::MaxMinDimension { ca, cb, .. } => if end == 0 { *ca } else { *cb },
                                _ => ArcCondition::Center,
                            };
                            MenuEntry::check(n, cur == cnd, move |t, _, cx| {
                                t.edit_sheet("Arc condition", cx, |s| {
                                    noshape_scene::drawing_tools::annotation_edit::set_arc_condition(s, id, end, cnd);
                                })
                            })
                        })
                        .collect();
                    out.push(MenuEntry::Submenu(format!("Arc condition (end {})", end + 1).into(), entries));
                }
            }
            AnnotationKind::DatumTarget { view, anchor, .. } => {
                let a2 = a.clone();
                out.push(MenuEntry::item("Edit...", move |t, w, cx| t.open_more_panel(&a2, w, cx)));
                // Live (behaviours "A35 datum target"): the target's
                // dimension › Hide dimension; its label (or anywhere else on
                // it) › Show dimensions ▸ This target / All targets.
                let id = a.id;
                let opts = a.target.as_deref().cloned().unwrap_or_default();
                let set = |hide: bool, all: bool| {
                    move |t: &mut DrawingEditor, _: &mut Window, cx: &mut Context<DrawingEditor>| {
                        t.edit_sheet(if hide { "Hide dimension" } else { "Show dimensions" }, cx, |s| noshape_scene::drawing_tools::annotation_edit::set_target_dims_hidden(s, id, hide, all))
                    }
                };
                let on_dimension = opts.origin.is_some() && !opts.hide_dims && self.placement(*view).is_some_and(|pl| target_dimension_hit(a.position, pl.to_sheet(*anchor), self.cursor));
                if on_dimension {
                    out.push(MenuEntry::item("Hide dimension", set(true, false)));
                } else {
                    out.push(MenuEntry::Submenu("Show dimensions".into(), vec![MenuEntry::item("This target", set(false, false)), MenuEntry::item("All targets", set(false, true))]));
                }
            }
            _ => {}
        }
        out
    }

    pub fn table_more_entries(&self, t: &Table, cell: Option<(usize, usize)>) -> Vec<MenuEntry> {
        let mut out = Vec::new();
        let id = t.id;
        let Some((r, c)) = cell.map(|(r, c)| t.anchor_cell(r, c)) else { return out };
        out.push(MenuEntry::item("Format cell...", move |t, w, cx| t.open_cell_format_panel(id, r, c, w, cx)));
        let layout = self.built.as_ref().and_then(|b| b.table_layout(id)).cloned();
        let m = t.merge_at(r, c).unwrap_or(CellMerge { row: r, col: c, rows: 1, cols: 1 });
        let (nrows, ncols) = layout.as_ref().map(te::counts).unwrap_or((1, 1));
        if let Some(l) = layout.clone() {
            if m.col + m.cols < ncols {
                let l = l.clone();
                out.push(MenuEntry::item("Merge with cell right", move |t, _, cx| t.edit_table(id, "Merge cells", cx, |t| {
                    te::merge_right(t, &l, r, c);
                })));
            }
            if m.row + m.rows < nrows {
                out.push(MenuEntry::item("Merge with cell below", move |t, _, cx| t.edit_table(id, "Merge cells", cx, |t| {
                    te::merge_below(t, &l, r, c);
                })));
            }
        }
        if t.merge_at(r, c).is_some() {
            out.push(MenuEntry::item("Unmerge cells", move |t, _, cx| t.edit_table(id, "Unmerge cells", cx, |t| {
                t.unmerge(r, c);
            })));
        }
        // Size › Columns equally / Rows equally; Resize… (column width, row height).
        if let Some(l) = layout {
            let (l1, l2) = (l.clone(), l.clone());
            out.push(MenuEntry::Submenu(
                "Size".into(),
                vec![
                    MenuEntry::item("Columns equally", move |t, _, cx| t.edit_table(id, "Size columns equally", cx, |t| te::columns_equally(t, &l1))),
                    MenuEntry::item("Rows equally", move |t, _, cx| t.edit_table(id, "Size rows equally", cx, |t| te::rows_equally(t, &l2))),
                ],
            ));
            let (w, h) = (l.xs.get(c + 1).zip(l.xs.get(c)).map(|(b, a)| b - a).unwrap_or(20.0), t.row_height);
            out.push(MenuEntry::item("Resize...", move |t, win, cx| {
                let width = t.input(&format::format_number(w, None, 2), win, cx);
                let height = t.input(&format::format_number(h, None, 2), win, cx);
                t.panel = Some(Panel::CellResize { table: id, col: c, width, height });
                cx.notify();
            }));
        }
        out.push(MenuEntry::Separator);
        out
    }

    // ------------------------------------------------------------ panels

    pub fn open_more_panel(&mut self, a: &Annotation, window: &mut Window, cx: &mut Context<Self>) {
        if let AnnotationKind::DatumTarget { label, area, .. } = &a.kind {
            let u = format::length_units(&self.drawing, &DimFormat::default());
            let num = |m: f64| format::format_number(m / u.meters(), None, 4);
            let (ai, w, h) = match area {
                DatumTargetArea::Point | DatumTargetArea::Dot | DatumTargetArea::Custom => (0, String::new(), String::new()),
                DatumTargetArea::Circle { diameter } => (1, num(*diameter), String::new()),
                DatumTargetArea::Rectangle { width, height } => (2, num(*width), num(*height)),
            };
            let label = self.input(label, window, cx);
            let width = self.input(&w, window, cx);
            let height = self.input(&h, window, cx);
            Self::focus_input(&label, window, cx);
            self.panel = Some(Panel::DatumTarget { id: a.id, label, area: ai, width, height });
            cx.notify();
        }
    }

    pub fn open_broken_out_panel(&mut self, view: ViewId, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(b) = self.current_sheet().view(view).and_then(|v| v.broken_out.get(index)).cloned() else { return };
        let u = format::length_units(&self.drawing, &DimFormat::default());
        let depth = self.input(&format::format_number(b.depth / u.meters(), None, 4), window, cx);
        Self::focus_input(&depth, window, cx);
        self.panel = Some(Panel::BrokenOut { view, edit: Some(index), profile: b.profile, spline: b.spline, depth, depth_ref: b.depth_ref });
        cx.notify();
    }

    pub fn open_cut_angle_panel(&mut self, view: ViewId, window: &mut Window, cx: &mut Context<Self>) {
        let Some(p) = self.current_sheet().section_points(view).filter(|p| p.len() == 2) else { return };
        let ang = (p[1][1] - p[0][1]).atan2(p[1][0] - p[0][0]).to_degrees();
        let input = self.input(&format::format_number(ang, None, 3), window, cx);
        Self::focus_input(&input, window, cx);
        self.panel = Some(Panel::CutAngle { view, input });
        cx.notify();
    }

    pub fn open_cell_format_panel(&mut self, table: u64, row: usize, col: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(t) = self.current_sheet().table(table) else { return };
        let f = t.shown_cell_format(row, col);
        let height = self.input(&f.height.map(|h| format::format_number(h, None, 2)).unwrap_or_default(), window, cx);
        self.panel = Some(Panel::CellFormat { table, row, col, scope: 0, align: f.align, bold: f.bold, height });
        cx.notify();
    }

    pub fn more_panel_rows(&mut self, cx: &mut Context<Self>) -> Option<(&'static str, Vec<AnyElement>)> {
        let ed = cx.entity();
        let pal = p();
        let units = format::length_units(&self.drawing, &DimFormat::default()).symbol();
        let panel = self.panel.as_ref()?;
        Some(match panel {
            Panel::BrokenOut { depth, depth_ref, .. } => {
                let mut rows = vec![Self::label_row("Depth", h_flex().gap_2().child(Input::new(depth).xsmall().w(px(80.))).child(div().text_xs().text_color(pal.text_faint).child(units)).into_any_element())];
                rows.push(div().text_xs().text_color(pal.text_faint).child(if depth_ref.is_some() { "Up to entity: the depth follows the selected model point" } else { "Blind depth from the nearest model point; click a point to cut up to it" }).into_any_element());
                if depth_ref.is_some() {
                    rows.push(
                        Button::new("dr-bo-blind")
                            .label("Use blind depth")
                            .xsmall()
                            .ghost()
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                if let Some(Panel::BrokenOut { depth_ref, .. }) = &mut this.panel {
                                    *depth_ref = None;
                                }
                                cx.notify();
                            }))
                            .into_any_element(),
                    );
                }
                ("Broken-out section", rows)
            }
            Panel::CutAngle { input, .. } => ("Cutting line angle", vec![Self::label_row("Angle (°)", Input::new(input).small().w(px(120.)).into_any_element())]),
            Panel::CellFormat { scope, align, bold, height, .. } => {
                let (scope, align, bold) = (*scope, *align, *bold);
                let scope_entries: Vec<MenuEntry> = SCOPES
                    .iter()
                    .enumerate()
                    .map(|(i, n)| {
                        MenuEntry::check(*n, i == scope, move |t, _, cx| {
                            if let Some(Panel::CellFormat { scope, .. }) = &mut t.panel {
                                *scope = i;
                            }
                            cx.notify();
                        })
                    })
                    .collect();
                let align_btn = |id: &'static str, label: &'static str, a: CellAlign| {
                    Button::new(id).label(label).xsmall().when(align == a, |b| b.primary()).when(align != a, |b| b.outline()).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        if let Some(Panel::CellFormat { align, .. }) = &mut this.panel {
                            *align = a;
                        }
                        cx.notify();
                    }))
                };
                (
                    "Format cells",
                    vec![
                        Self::label_row("Apply to", Button::new("dr-cf-scope").label(SCOPES[scope]).small().outline().dropdown_caret(true).os_dropdown_menu("dr-cf-scope-dd", true, move |m, w, cx| fill_os_menu(m, &scope_entries, &ed, w, cx)).into_any_element()),
                        Self::label_row("Alignment", h_flex().gap_1().child(align_btn("dr-cf-l", "Left", CellAlign::Left)).child(align_btn("dr-cf-c", "Center", CellAlign::Center)).child(align_btn("dr-cf-r", "Right", CellAlign::Right)).into_any_element()),
                        Self::label_row("Text height", h_flex().gap_2().child(Input::new(height).text_size(px(13.33)).text_color(gpui_kit::rgb(0x555555)).xsmall().w(px(60.))).child(div().text_xs().text_color(pal.text_faint).child("mm (empty = table)")).into_any_element()),
                        crate::drawing_chrome::DrCheckbox::new("dr-cf-bold")
                            .label("Bold")
                            .checked(bold)
                            .on_change(cx.listener(|this, v: &bool, _, cx| {
                                if let Some(Panel::CellFormat { bold, .. }) = &mut this.panel {
                                    *bold = *v;
                                }
                                cx.notify();
                            }))
                            .into_any_element(),
                    ],
                )
            }
            Panel::CellResize { width, height, .. } => (
                "Resize",
                vec![
                    Self::label_row("Column width", h_flex().gap_2().child(Input::new(width).xsmall().w(px(80.))).child(div().text_xs().text_color(pal.text_faint).child("mm")).into_any_element()),
                    Self::label_row("Row height", h_flex().gap_2().child(Input::new(height).xsmall().w(px(80.))).child(div().text_xs().text_color(pal.text_faint).child("mm")).into_any_element()),
                ],
            ),
            Panel::DatumTarget { label, area, width, height, .. } => {
                let area = *area;
                let entries: Vec<MenuEntry> = AREAS
                    .iter()
                    .enumerate()
                    .map(|(i, n)| {
                        MenuEntry::check(*n, i == area, move |t, _, cx| {
                            if let Some(Panel::DatumTarget { area, .. }) = &mut t.panel {
                                *area = i;
                            }
                            cx.notify();
                        })
                    })
                    .collect();
                let mut rows = vec![
                    Self::label_row("Label", Input::new(label).text_size(px(13.33)).text_color(gpui_kit::rgb(0x555555)).small().w(px(100.)).into_any_element()),
                    Self::label_row("Target area", Button::new("dr-dt-area").label(AREAS[area]).small().outline().dropdown_caret(true).os_dropdown_menu("dr-dt-area-dd", true, move |m, w, cx| fill_os_menu(m, &entries, &ed, w, cx)).into_any_element()),
                ];
                match area {
                    1 => rows.push(Self::label_row("Diameter", h_flex().gap_2().child(Input::new(width).text_size(px(13.33)).text_color(gpui_kit::rgb(0x555555)).xsmall().w(px(70.))).child(div().text_xs().text_color(pal.text_faint).child(units)).into_any_element())),
                    2 => rows.push(Self::label_row("Width × height", h_flex().gap_1().child(Input::new(width).text_size(px(13.33)).text_color(gpui_kit::rgb(0x555555)).xsmall().w(px(60.))).child(Input::new(height).text_size(px(13.33)).text_color(gpui_kit::rgb(0x555555)).xsmall().w(px(60.))).child(div().text_xs().text_color(pal.text_faint).child(units)).into_any_element())),
                    _ => {}
                }
                ("Datum target", rows)
            }
            _ => return None,
        })
    }

    pub fn apply_more_panel(&mut self, panel: Panel, cx: &mut Context<Self>) {
        let val = |e: &Entity<gpui_kit::component::input::InputState>, cx: &App| e.read(cx).value().to_string();
        let u = format::length_units(&self.drawing, &DimFormat::default()).meters();
        let num = |s: String| s.trim().parse::<f64>().ok().filter(|v| v.is_finite());
        match panel {
            Panel::BrokenOut { view, edit, profile, spline, depth, depth_ref } => {
                let Some(b) = noshape_scene::drawing_tools::broken_out(profile, spline, num(val(&depth, cx)).map(|v| v * u), depth_ref) else {
                    self.status = Some("Invalid depth".into());
                    return;
                };
                self.edit_view("Broken-out section", view, cx, |v| noshape_scene::drawing_tools::set_broken_out(v, edit, b));
                self.tool = Tool::Select;
                self.selection = vec![Sel::View(view)];
            }
            Panel::CutAngle { view, input } => match num(val(&input, cx).trim_end_matches('°').to_string()) {
                Some(deg) => {
                    if self.current_sheet().section_points(view).filter(|p| p.len() == 2).is_none() {
                        return;
                    }
                    self.edit_sheet("Cutting line angle", cx, |sh| {
                        noshape_scene::drawing_tools::annotation_edit::set_cutting_line_angle(sh, view, deg);
                    });
                }
                None => self.status = Some("Invalid angle".into()),
            },
            Panel::CellFormat { table, row, col, scope, align, bold, height } => {
                let h = num(val(&height, cx)).filter(|h| *h > 0.5);
                let layout = self.built.as_ref().and_then(|b| b.table_layout(table)).cloned();
                let scope = [te::FormatScope::Cell, te::FormatScope::Row, te::FormatScope::Column, te::FormatScope::Table][scope.min(3)];
                self.edit_table(table, "Format cells", cx, |t| te::format_cells(t, layout.as_ref(), row, col, scope, te::FormatFields { align, bold, height: h }));
            }
            Panel::CellResize { table, col, width, height } => {
                // Can't exceed the sheet.
                let size = self.current_sheet().size.size_mm();
                let w = num(val(&width, cx).trim_end_matches("mm").to_string()).filter(|v| *v > 0.5);
                let h = num(val(&height, cx).trim_end_matches("mm").to_string()).filter(|v| *v > 0.5);
                let layout = self.built.as_ref().and_then(|b| b.table_layout(table)).cloned();
                self.edit_table(table, "Resize cell", cx, |t| te::resize_cell(t, layout.as_ref(), size, col, w, h));
            }
            Panel::DatumTarget { id, label, area, width, height } => {
                let (w, h) = (num(val(&width, cx)).unwrap_or(0.0) * u, num(val(&height, cx)).unwrap_or(0.0) * u);
                let area = match area {
                    1 => DatumTargetArea::Circle { diameter: w.max(1e-4) },
                    2 => DatumTargetArea::Rectangle { width: w.max(1e-4), height: h.max(1e-4) },
                    _ => DatumTargetArea::Point,
                };
                let label = val(&label, cx);
                self.edit_sheet("Edit datum target", cx, |s| {
                    if let Some(AnnotationKind::DatumTarget { label: l, area: a, .. }) = s.annotations.iter_mut().find(|a| a.id == id).map(|a| &mut a.kind) {
                        *l = label;
                        *a = area;
                    }
                });
            }
            _ => {}
        }
        cx.notify();
    }
}

/// A right-click at sheet point `at` on a datum target with its dimensions
/// shown: on a dimension, not on the label bubble (center `label`, the
/// drawn 5.5 mm radius) or its leader to the target point `point`.
fn target_dimension_hit(label: [f64; 2], point: [f64; 2], at: [f64; 2]) -> bool {
    let dist = |a: [f64; 2], b: [f64; 2]| ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt();
    let (d, l) = ([point[0] - label[0], point[1] - label[1]], dist(label, point));
    let t = if l > 1e-9 { (((at[0] - label[0]) * d[0] + (at[1] - label[1]) * d[1]) / (l * l)).clamp(0.0, 1.0) } else { 0.0 };
    let on_leader = dist(at, [label[0] + d[0] * t, label[1] + d[1] * t]) < 1.5;
    dist(at, label) > 6.5 && !on_leader
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_target_right_click_is_on_its_dimension_away_from_label_and_leader() {
        let (label, point) = ([100.0, 100.0], [80.0, 80.0]);
        let hit = |at| super::target_dimension_hit(label, point, at);
        assert!(!hit([101.0, 99.0]), "the label bubble");
        assert!(!hit([90.0, 90.5]), "on the leader");
        assert!(hit([70.0, 88.0]), "a dimension beside the point");
        assert!(hit([90.0, 72.0]));
    }
}
