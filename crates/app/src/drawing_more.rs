//! Drawing editor, third part: broken-out sections, partial / angular
//! section editing (cutting line points, angle), polygon and spline crops,
//! max/min and foreshortened dimensions, ordinate groups, datum targets,
//! GD&T leaders, table cell formatting and merging, moving views between
//! sheets, templates and title block layouts.

use crate::drawing_scene::Overlay;
use crate::drawing_tools::{auto_orientation, dim_offset};
use crate::drawing_view::*;
use crate::theme::p;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::input::Input;
use gpui_kit::component::menu::DropdownMenu as _;
use gpui_kit::component::{Sizable as _, h_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_model::drawing::views;
use noshape_model::drawing::*;
use noshape_model::drawing::{format, templates};

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
            Tool::CropPoly { .. } => "Click the points of the crop boundary in a view; Enter or double-click to finish",
            Tool::DatumTarget { anchor: None } => "Click the target point",
            Tool::DatumTarget { .. } => "Click to place the target callout",
            Tool::GdtLeader { .. } => "Click the edge or point the leader points to",
            _ => return None,
        })
    }

    // ------------------------------------------------------------ picking

    /// The axis under the cursor: a centerline annotation, an automatic
    /// centerline of a view, else a straight edge.
    fn axis_pick(&self) -> Option<Pick> {
        let c = self.cursor;
        let built = self.built.as_ref()?;
        let tol = self.tol_mm(6.0);
        for a in &built.resolved.annotations {
            if let AnnotationKind::Centerline { view, a: pa, b: pb } = &a.kind
                && let Some(pl) = self.placement(*view)
                && views::dist_to_segment(c, pl.to_sheet(*pa), pl.to_sheet(*pb)) < tol
            {
                let r = |slot: u8| a.refs.iter().find(|x| x.slot == slot).map(|x| x.point.clone());
                return Some(Pick::Line { a: PickedPoint { view: *view, p: *pa, r: r(0) }, b: PickedPoint { view: *view, p: *pb, r: r(1) } });
            }
        }
        for pl in &built.placements {
            let Some(g) = self.geoms.get(&pl.id) else { continue };
            for ax in &g.axes {
                if views::dist_to_segment(c, pl.to_sheet(ax[0]), pl.to_sheet(ax[1])) < tol {
                    return Some(Pick::Line { a: PickedPoint { view: pl.id, p: ax[0], r: None }, b: PickedPoint { view: pl.id, p: ax[1], r: None } });
                }
            }
        }
        self.line_pick()
    }

    /// The hovered drawn line of a view (also silhouettes and cut edges
    /// without a model edge), as its end points.
    fn raw_line_pick(&self) -> Option<Pick> {
        let h = self.hover?;
        let noshape_model::drawing::build::PickTarget::Edge { view, edge } = self.built.as_ref()?.picks.get(h as usize)?.clone() else { return None };
        let pts = &self.geoms.get(&view)?.edges.get(edge as usize)?.points;
        let (a, b) = (*pts.first()?, *pts.last()?);
        let l = len(sub(b, a));
        if l < 1e-9 || pts.iter().any(|p| ((p[0] - a[0]) * (b[1] - a[1]) - (p[1] - a[1]) * (b[0] - a[0])).abs() / l > l * 1e-3) {
            return None;
        }
        Some(Pick::Line { a: PickedPoint { view, p: a, r: None }, b: PickedPoint { view, p: b, r: None } })
    }

    fn circle_at(&self) -> Option<Pick> {
        let Some(Pick::Circle { view, center, radius, r }) = self.circle_pick(self.cursor) else { return None };
        let at = self.placement(view)?.to_view(self.cursor);
        Some(Pick::CircleAt { view, center, radius, r, at })
    }

    /// Clicks of the max/min and foreshortened dimension tools.
    pub fn more_dimension_pick(&mut self, kind: DimTool, picks: &mut Vec<Pick>) {
        match (kind, picks.len()) {
            (DimTool::MaxMin, 0) => picks.extend(self.circle_at()),
            (DimTool::MaxMin, 1) => {
                let Some(Pick::CircleAt { view, center, .. }) = picks.first().cloned() else { return };
                let second = match self.circle_at() {
                    Some(Pick::CircleAt { view: v, center: c2, .. }) if v == view && len(sub(c2, center)) < 1e-9 => None,
                    Some(p @ Pick::CircleAt { view: v, .. }) if v == view => Some(p),
                    _ => self.snap_at(self.cursor).filter(|p| p.view == view).map(Pick::Point),
                };
                picks.extend(second);
            }
            (DimTool::Foreshorten, 0) => picks.extend(self.axis_pick()),
            (DimTool::Foreshorten, 1) => {
                let view = match &picks[0] {
                    Pick::Line { a, .. } => a.view,
                    _ => return,
                };
                // A snap point, a model edge, any drawn line (silhouettes have no
                // model edge), else the clicked point itself.
                let e = self
                    .snap_at(self.cursor)
                    .filter(|p| p.view == view)
                    .map(Pick::Point)
                    .or_else(|| self.line_pick().filter(|l| matches!(l, Pick::Line { a, .. } if a.view == view)))
                    .or_else(|| self.raw_line_pick().filter(|l| matches!(l, Pick::Line { a, .. } if a.view == view)))
                    .or_else(|| self.placement(view).filter(|pl| pl.contains(self.cursor, 2.0)).map(|pl| Pick::Point(PickedPoint { view, p: pl.to_view(self.cursor), r: None })));
                picks.extend(e);
            }
            _ => {}
        }
    }

    // ------------------------------------------------------------ previews

    pub fn more_pending_annotation(&self, id: u64) -> Option<Annotation> {
        let c = self.cursor;
        let refs = |v: Vec<(u8, Option<PointRef>)>| v.into_iter().filter_map(|(slot, r)| r.map(|point| AnnotationRef { slot, point })).collect::<Vec<_>>();
        match &self.tool {
            Tool::Dimension { kind: DimTool::MaxMin, picks } if picks.len() == 2 => {
                let Pick::CircleAt { view, center, radius, r, at } = &picks[0] else { return None };
                let (b, rb, cb, rref) = match &picks[1] {
                    Pick::Point(p) => (p.p, 0.0, ArcCondition::Center, p.r.clone()),
                    Pick::CircleAt { center: c2, radius: r2, r, at: at2, .. } => {
                        let near = dot(sub(*at2, *c2), sub(*center, *c2)) > 0.0;
                        (*c2, *r2, if near { ArcCondition::Min } else { ArcCondition::Max }, r.clone())
                    }
                    _ => return None,
                };
                let ca = if dot(sub(*at, *center), sub(b, *center)) > 0.0 { ArcCondition::Min } else { ArcCondition::Max };
                let pl = self.placement(*view)?;
                let (ps, qs) = (pl.to_sheet(*center), pl.to_sheet(b));
                let o = auto_orientation(ps, qs, c);
                let offset = dim_offset(ps, qs, o, c);
                Some(Annotation {
                    id,
                    kind: AnnotationKind::MaxMinDimension { view: *view, a: *center, ra: *radius, ca, b, rb, cb, offset, orientation: o },
                    position: c,
                    refs: refs(vec![(0, r.clone()), (1, rref)]),
                    ..Default::default()
                })
            }
            Tool::Dimension { kind: DimTool::Foreshorten, picks } if picks.len() == 2 => {
                let Pick::Line { a, b } = &picks[0] else { return None };
                let e = match &picks[1] {
                    Pick::Point(p) => p.clone(),
                    Pick::Line { a, .. } => a.clone(),
                    _ => return None,
                };
                Some(Annotation {
                    id,
                    kind: AnnotationKind::ForeshortenedDiameter { view: a.view, axis: [a.p, b.p], edge: e.p, direct: false },
                    position: c,
                    refs: refs(vec![(0, a.r.clone()), (1, b.r.clone()), (2, e.r.clone())]),
                    ..Default::default()
                })
            }
            Tool::DatumTarget { anchor: Some(a) } => {
                let s = self.pick_sheet(a)?;
                if len(sub(c, s)) < 4.0 {
                    return None;
                }
                let n = self.current_sheet().annotations.iter().filter(|x| matches!(&x.kind, AnnotationKind::DatumTarget { label, .. } if label.starts_with('A'))).count() + 1;
                Some(Annotation { id, kind: AnnotationKind::DatumTarget { view: a.view, anchor: a.p, label: format!("A{n}"), area: DatumTargetArea::Point }, position: c, refs: refs(vec![(0, a.r.clone())]), ..Default::default() })
            }
            _ => None,
        }
    }

    /// Closed profile through view points (spline or polygon) on the sheet.
    fn profile_on_sheet(&self, view: ViewId, pts: &[[f64; 2]], spline: bool) -> Vec<[f64; 2]> {
        let Some(pl) = self.placement(view) else { return vec![] };
        let pts = if spline { views::closed_spline(pts, 12) } else { pts.to_vec() };
        let mut out: Vec<[f64; 2]> = pts.iter().map(|p| pl.to_sheet(*p)).collect();
        if let Some(f) = out.first().copied() {
            out.push(f);
        }
        out
    }

    pub fn more_overlay(&self, overlay: &mut Overlay) {
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
                if let Some(a) = self.current_sheet().annotations.iter().find(|a| a.id == *id) {
                    overlay.lines.push((vec![a.position, c], true));
                }
            }
            _ => {}
        }
    }

    // ------------------------------------------------------------ clicks

    pub fn more_tool_click(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let c = self.cursor;
        match self.tool.clone() {
            Tool::BrokenOut { parent, mut points, done: false } => {
                if let Some(pp) = self.view_point(parent) {
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
                if let Some(pp) = self.view_point(parent) {
                    if parent.is_none() {
                        points.clear();
                    }
                    points.push(pp.p);
                    self.tool = Tool::CropPoly { parent: Some(pp.view), points, spline };
                }
            }
            Tool::DatumTarget { anchor: None } => {
                if let Some(p) = self.any_point(c) {
                    self.tool = Tool::DatumTarget { anchor: Some(p) };
                }
            }
            Tool::DatumTarget { anchor: Some(_) } => {
                if let Some(a) = self.pending_annotation() {
                    self.edit_sheet("Datum target", cx, |s| s.annotations.push(a));
                    self.tool = Tool::DatumTarget { anchor: None };
                }
            }
            Tool::GdtLeader { id } => {
                let leader = match self.snap_at(c).or_else(|| self.any_point(c)) {
                    Some(p) => Leader { view: Some(p.view), point: p.p },
                    None => Leader { view: None, point: c },
                };
                self.edit_sheet("Add leader", cx, |s| {
                    if let Some(AnnotationKind::Gdt { leaders, .. }) = s.annotations.iter_mut().find(|a| a.id == id).map(|a| &mut a.kind) {
                        leaders.push(leader);
                    }
                });
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
                let half = (|| {
                    let g = self.geoms.get(&view)?;
                    let (key, part) = views::view_source(self.current_sheet(), view)?;
                    let src = self.sources.get(&key)?;
                    Some(views::model_depth(&src.context, part.as_ref(), g.frame.as_ref()?) / 2.0)
                })()
                .unwrap_or(0.01);
                let u = format::length_units(&self.drawing, &DimFormat::default());
                let depth = self.input(&format::format_number(half / u.meters(), None, 3), window, cx);
                Self::focus_input(&depth, window, cx);
                self.panel = Some(Panel::BrokenOut { view, edit: None, profile: points, spline: true, depth, depth_ref: None });
                self.update_preview(cx);
                true
            }
            Tool::CropPoly { parent: Some(view), points, spline } if points.len() >= 3 => {
                let crop = if spline { Crop::Spline(points) } else { Crop::Polygon(points) };
                self.edit_view("Crop view", view, cx, |v| v.crop = Some(crop));
                self.tool = Tool::Select;
                self.update_preview(cx);
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

    pub fn view_more_entries(&self, id: ViewId) -> Vec<MenuEntry> {
        let mut out = Vec::new();
        let sheet = self.current_sheet();
        let Some(v) = sheet.view(id).cloned() else { return out };
        if matches!(v.kind, ViewKind::Section { .. }) {
            let partial = v.partial;
            out.push(MenuEntry::check("Partial section", partial, move |t, _, cx| t.edit_view("Partial section", id, cx, |v| v.partial = !partial)));
            if sheet.section_points(id).is_some_and(|p| p.len() == 2) {
                out.push(MenuEntry::item("Cutting line angle…", move |t, w, cx| t.open_cut_angle_panel(id, w, cx)));
            }
        }
        if !v.broken_out.is_empty() {
            for i in 0..v.broken_out.len() {
                out.push(MenuEntry::item(format!("Edit broken-out section {}…", i + 1), move |t, w, cx| t.open_broken_out_panel(id, i, w, cx)));
            }
            out.push(MenuEntry::item("Remove broken-out sections", move |t, _, cx| t.edit_view("Remove broken-out sections", id, cx, |v| v.broken_out.clear())));
        }
        let mut sheets: Vec<MenuEntry> = self
            .drawing
            .sheets
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != self.sheet)
            .map(|(i, s)| MenuEntry::item(s.name.clone(), move |t, _, cx| t.move_selected_views(id, Some(i), cx)))
            .collect();
        if !sheets.is_empty() {
            sheets.push(MenuEntry::Separator);
        }
        sheets.push(MenuEntry::item("New sheet", move |t, _, cx| t.move_selected_views(id, None, cx)));
        out.push(MenuEntry::Submenu("Move to sheet".into(), sheets));
        out
    }

    /// Move the selected views (with `id`) to sheet `to` (None = a new sheet).
    fn move_selected_views(&mut self, id: ViewId, to: Option<usize>, cx: &mut Context<Self>) {
        let mut ids: Vec<ViewId> = self.selection.iter().filter_map(|s| if let Sel::View(v) = s { Some(*v) } else { None }).collect();
        if !ids.contains(&id) {
            ids = vec![id];
        }
        let mut d = self.drawing.clone();
        let to = to.unwrap_or_else(|| d.add_sheet(self.sheet));
        match d.move_views_to_sheet(self.sheet, &ids, to, Some(&self.sources)) {
            Ok(_) => {
                let name = d.sheets[to].name.clone();
                self.selection.clear();
                self.commit("Move to sheet", d, cx);
                self.switch_sheet(to, cx);
                self.status = Some(format!("Moved to {name}").into());
            }
            Err(e) => self.status = Some(e.into()),
        }
        cx.notify();
    }

    pub fn annotation_more_entries(&self, a: &Annotation) -> Vec<MenuEntry> {
        let mut out = Vec::new();
        let id = a.id;
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
                    out.push(MenuEntry::item("Remove leaders", move |t, _, cx| {
                        t.edit_sheet("Remove leaders", cx, |s| {
                            if let Some(AnnotationKind::Gdt { leaders, .. }) = s.annotations.iter_mut().find(|x| x.id == id).map(|x| &mut x.kind) {
                                leaders.clear();
                            }
                        })
                    }));
                }
            }
            &AnnotationKind::RadialDimension { view, center, radius, angle, .. } => {
                let (d, w) = ([angle.cos(), angle.sin()], [-angle.sin(), angle.cos()]);
                let jog = [center[0] + d[0] * radius * 0.55 + w[0] * radius * 0.2, center[1] + d[1] * radius * 0.55 + w[1] * radius * 0.2];
                out.push(MenuEntry::item("Foreshorten", move |t, _, cx| set_kind(t, "Foreshorten", AnnotationKind::ForeshortenedRadial { view, center, radius, angle, jog }, cx)));
            }
            &AnnotationKind::ForeshortenedRadial { view, center, radius, angle, .. } => {
                out.push(MenuEntry::item("Remove foreshorten", move |t, _, cx| set_kind(t, "Remove foreshorten", AnnotationKind::RadialDimension { view, center, radius, diameter: false, angle }, cx)));
            }
            &AnnotationKind::ForeshortenedDiameter { view, axis, edge, direct } => {
                let label = if direct { "Display as foreshortened diameter" } else { "Display as direct diameter" };
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
                                    if let Some(AnnotationKind::MaxMinDimension { ca, cb, .. }) = s.annotations.iter_mut().find(|x| x.id == id).map(|x| &mut x.kind) {
                                        if end == 0 { *ca = cnd } else { *cb = cnd }
                                    }
                                })
                            })
                        })
                        .collect();
                    out.push(MenuEntry::Submenu(format!("Arc condition (end {})", end + 1).into(), entries));
                }
            }
            AnnotationKind::DatumTarget { .. } => {
                let a2 = a.clone();
                out.push(MenuEntry::item("Edit…", move |t, w, cx| t.open_more_panel(&a2, w, cx)));
            }
            _ => {}
        }
        out
    }

    pub fn template_entries(&self) -> Vec<MenuEntry> {
        let cur = self.current_sheet();
        templates::standard_templates()
            .into_iter()
            .map(|tp| {
                let checked = cur.size == tp.size && cur.title_style == tp.style && self.drawing.units_mm != tp.inches;
                let (w, h) = tp.size.size_mm();
                let label = format!("{}  ({} × {} {})", tp.name, if tp.inches { format::format_number(w / 25.4, Some(1), 1) } else { format!("{w}") }, if tp.inches { format::format_number(h / 25.4, Some(1), 1) } else { format!("{h}") }, if tp.inches { "in" } else { "mm" });
                MenuEntry::check(label, checked, move |t, _, cx| {
                    let si = t.sheet;
                    t.edit(&format!("Template {}", tp.name), cx, |d| templates::apply_template(d, si, &tp));
                    t.fitted = false;
                })
            })
            .collect()
    }

    pub fn title_style_entries(&self) -> Vec<MenuEntry> {
        let cur = self.current_sheet().title_style;
        TitleBlockStyle::ALL
            .into_iter()
            .map(|st| {
                MenuEntry::check(st.name(), cur == st, move |t, _, cx| {
                    let si = t.sheet;
                    t.edit("Title block layout", cx, |d| templates::set_title_style(d, si, st));
                })
            })
            .collect()
    }

    pub fn table_more_entries(&self, t: &Table, cell: Option<(usize, usize)>) -> Vec<MenuEntry> {
        let mut out = Vec::new();
        let id = t.id;
        let Some((r, c)) = cell.map(|(r, c)| t.anchor_cell(r, c)) else { return out };
        out.push(MenuEntry::item("Format cell…", move |t, w, cx| t.open_cell_format_panel(id, r, c, w, cx)));
        let m = t.merge_at(r, c).unwrap_or(CellMerge { row: r, col: c, rows: 1, cols: 1 });
        let ncols = self.built.as_ref().and_then(|b| b.table_layout(id)).map(|l| l.xs.len() - 1).unwrap_or(1);
        let nrows = self.built.as_ref().and_then(|b| b.table_layout(id)).map(|l| l.ys.len() - 1 - l.title as usize).unwrap_or(1);
        if m.col + m.cols < ncols {
            out.push(MenuEntry::item("Merge with cell right", move |t, _, cx| t.edit_table(id, "Merge cells", cx, |t| {
                t.merge_cells((m.row, m.col), (m.row + m.rows - 1, m.col + m.cols));
            })));
        }
        if m.row + m.rows < nrows {
            out.push(MenuEntry::item("Merge with cell below", move |t, _, cx| t.edit_table(id, "Merge cells", cx, |t| {
                t.merge_cells((m.row, m.col), (m.row + m.rows, m.col + m.cols - 1));
            })));
        }
        if t.merge_at(r, c).is_some() {
            out.push(MenuEntry::item("Unmerge cells", move |t, _, cx| t.edit_table(id, "Unmerge cells", cx, |t| {
                t.unmerge(r, c);
            })));
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
                DatumTargetArea::Point => (0, String::new(), String::new()),
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

    fn open_broken_out_panel(&mut self, view: ViewId, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(b) = self.current_sheet().view(view).and_then(|v| v.broken_out.get(index)).cloned() else { return };
        let u = format::length_units(&self.drawing, &DimFormat::default());
        let depth = self.input(&format::format_number(b.depth / u.meters(), None, 4), window, cx);
        Self::focus_input(&depth, window, cx);
        self.panel = Some(Panel::BrokenOut { view, edit: Some(index), profile: b.profile, spline: b.spline, depth, depth_ref: b.depth_ref });
        cx.notify();
    }

    fn open_cut_angle_panel(&mut self, view: ViewId, window: &mut Window, cx: &mut Context<Self>) {
        let Some(p) = self.current_sheet().section_points(view).filter(|p| p.len() == 2) else { return };
        let ang = (p[1][1] - p[0][1]).atan2(p[1][0] - p[0][0]).to_degrees();
        let input = self.input(&format::format_number(ang, None, 3), window, cx);
        Self::focus_input(&input, window, cx);
        self.panel = Some(Panel::CutAngle { view, input });
        cx.notify();
    }

    fn open_cell_format_panel(&mut self, table: u64, row: usize, col: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(t) = self.current_sheet().table(table) else { return };
        let f = t.cell_format(row, col);
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
                        Self::label_row("Apply to", Button::new("dr-cf-scope").label(SCOPES[scope]).small().outline().dropdown_caret(true).dropdown_menu(move |m, w, cx| fill_menu(m, &scope_entries, &ed, w, cx)).into_any_element()),
                        Self::label_row("Alignment", h_flex().gap_1().child(align_btn("dr-cf-l", "Left", CellAlign::Left)).child(align_btn("dr-cf-c", "Center", CellAlign::Center)).child(align_btn("dr-cf-r", "Right", CellAlign::Right)).into_any_element()),
                        Self::label_row("Text height", h_flex().gap_2().child(Input::new(height).xsmall().w(px(60.))).child(div().text_xs().text_color(pal.text_faint).child("mm (empty = table)")).into_any_element()),
                        Checkbox::new("dr-cf-bold")
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
                    Self::label_row("Label", Input::new(label).small().w(px(100.)).into_any_element()),
                    Self::label_row("Target area", Button::new("dr-dt-area").label(AREAS[area]).small().outline().dropdown_caret(true).dropdown_menu(move |m, w, cx| fill_menu(m, &entries, &ed, w, cx)).into_any_element()),
                ];
                match area {
                    1 => rows.push(Self::label_row("Diameter", h_flex().gap_2().child(Input::new(width).xsmall().w(px(70.))).child(div().text_xs().text_color(pal.text_faint).child(units)).into_any_element())),
                    2 => rows.push(Self::label_row("Width × height", h_flex().gap_1().child(Input::new(width).xsmall().w(px(60.))).child(Input::new(height).xsmall().w(px(60.))).child(div().text_xs().text_color(pal.text_faint).child(units)).into_any_element())),
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
                let Some(d) = num(val(&depth, cx)).map(|v| v * u).filter(|v| *v > 0.0).or(depth_ref.as_ref().map(|_| 0.0)) else {
                    self.status = Some("Invalid depth".into());
                    return;
                };
                let b = BrokenOut { profile, spline, depth: d, depth_ref };
                self.edit_view("Broken-out section", view, cx, |v| match edit {
                    Some(i) if i < v.broken_out.len() => v.broken_out[i] = b,
                    _ => v.broken_out.push(b),
                });
                self.tool = Tool::Select;
                self.selection = vec![Sel::View(view)];
            }
            Panel::CutAngle { view, input } => match num(val(&input, cx).trim_end_matches('°').to_string()) {
                Some(deg) => {
                    let Some(p) = self.current_sheet().section_points(view).filter(|p| p.len() == 2) else { return };
                    let m = [(p[0][0] + p[1][0]) / 2.0, (p[0][1] + p[1][1]) / 2.0];
                    let h = len(sub(p[1], p[0])) / 2.0;
                    let (s, c) = deg.to_radians().sin_cos();
                    let pts = vec![[m[0] - c * h, m[1] - s * h], [m[0] + c * h, m[1] + s * h]];
                    self.edit_sheet("Cutting line angle", cx, |sh| {
                        sh.set_section_points(view, pts);
                    });
                }
                None => self.status = Some("Invalid angle".into()),
            },
            Panel::CellFormat { table, row, col, scope, align, bold, height } => {
                let h = num(val(&height, cx)).filter(|h| *h > 0.5);
                let layout = self.built.as_ref().and_then(|b| b.table_layout(table)).cloned();
                let (nrows, ncols) = layout.map(|l| (l.ys.len() - 1 - l.title as usize, l.xs.len() - 1)).unwrap_or((row + 1, col + 1));
                let cells: Vec<(usize, usize)> = match scope {
                    1 => (0..ncols).map(|c| (row, c)).collect(),
                    2 => (0..nrows).map(|r| (r, col)).collect(),
                    3 => (0..nrows).flat_map(|r| (0..ncols).map(move |c| (r, c))).collect(),
                    _ => vec![(row, col)],
                };
                self.edit_table(table, "Format cells", cx, |t| {
                    for (r, c) in cells {
                        t.set_cell_format(r, c, CellFormat { align, height: h, bold });
                    }
                    t.col_widths.clear();
                });
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
