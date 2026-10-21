//! Drawing editor: Onshape's drawing sketch tools and constraints (live
//! toolbar order: Line (l), Center point circle (c) ▾ 3 point circle,
//! Tangent arc, Corner rectangle (g) ▾ Center point rectangle (r), Spline ▾
//! Spline point, Sketch point, Hatch region, Trim (m) | Horizontal (h) ▾
//! Vertical (v), Coincident (i), Perpendicular (Shift+l), Parallel (b),
//! Tangent (t)). Geometry lives in the sheet's [`SheetSketch`] (sheet mm) and
//! is solved by the Part Studio sketch engine; constraints can also use view
//! edges (copied in as fixed external geometry).

use crate::drawing_view::*;
use gpui_kit::*;
use noshape_model::drawing::build::PickTarget;
use noshape_model::drawing::Drawing;
use noshape_model::drawing::sheet_sketch::SheetSketch;
use noshape_sketch::{ConstraintKind, Curve2, DVec2, EntityId, Geometry};

pub use noshape_scene::drawing_tools::{SkCon, SkTool};

fn v2(p: [f64; 2]) -> DVec2 {
    DVec2::new(p[0], p[1])
}

/// The point entities defining a sketch entity (moved together when it is dragged).
pub fn entity_points(sk: &noshape_sketch::Sketch, id: EntityId) -> Vec<EntityId> {
    let Some(e) = sk.entity(id) else { return vec![] };
    match &e.geometry {
        Geometry::Point { .. } => vec![id],
        Geometry::Line { start, end } => vec![*start, *end],
        Geometry::Circle { center, .. } | Geometry::Ellipse { center, .. } => vec![*center],
        Geometry::Arc { center, start, end } | Geometry::EllipticalArc { center, start, end, .. } => vec![*center, *start, *end],
        Geometry::BSpline { control, .. } => control.clone(),
        Geometry::FitSpline { points, .. } => points.clone(),
        _ => vec![],
    }
}

/// A point aligned with woken endpoints, and the inference lines (from, to).
type Aligned = ([f64; 2], Vec<([f64; 2], [f64; 2])>);

impl DrawingEditor {
    pub fn sketch(&self) -> &SheetSketch {
        &self.drawing.sheets[self.sheet].sketch
    }

    /// The sketch point nearest the cursor (within 6 px), if any.
    pub(crate) fn sketch_point_at(&self, c: [f64; 2]) -> Option<(EntityId, [f64; 2])> {
        let tol = self.tol_mm(6.0);
        let sk = &self.sketch().sketch;
        sk.entities()
            .filter(|e| matches!(e.geometry, Geometry::Point { .. }) && !e.external)
            .filter_map(|e| sk.point(e.id).map(|p| (e.id, [p.x, p.y])))
            .filter(|(_, p)| ((p[0] - c[0]).powi(2) + (p[1] - c[1]).powi(2)).sqrt() < tol)
            .min_by(|a, b| {
                let d = |p: [f64; 2]| (p[0] - c[0]).powi(2) + (p[1] - c[1]).powi(2);
                d(a.1).total_cmp(&d(b.1))
            })
    }

    /// The hovered sketch entity (curves via their pick id).
    pub(crate) fn sketch_entity_at(&self) -> Option<EntityId> {
        let b = self.built.as_ref()?;
        match b.picks.get(self.hover? as usize)? {
            PickTarget::SketchEntity(id) => Some(EntityId(*id)),
            _ => None,
        }
    }

    /// Where a sketch click lands: an existing sketch point, a view snap
    /// point, or the cursor (with horizontal / vertical inference from
    /// `from`, within 1.5°). Returns (point, existing point id, inferred H/V).
    fn sketch_target(&self, from: Option<[f64; 2]>) -> ([f64; 2], Option<EntityId>, Option<SkCon>) {
        let c = self.cursor;
        if let Some((id, p)) = self.sketch_point_at(c) {
            return (p, Some(id), None);
        }
        if let Some(p) = self.snap_at(c).and_then(|s| self.pick_sheet(&s)) {
            return (p, None, None);
        }
        if let Some(f) = from {
            let d = [c[0] - f[0], c[1] - f[1]];
            let l = (d[0] * d[0] + d[1] * d[1]).sqrt();
            if l > 1e-6 {
                let a = d[1].atan2(d[0]).to_degrees().rem_euclid(180.0);
                if a < 1.5 || a > 178.5 {
                    return ([c[0], f[1]], None, Some(SkCon::Horizontal));
                }
                if (a - 90.0).abs() < 1.5 {
                    return ([f[0], c[1]], None, Some(SkCon::Vertical));
                }
            }
        }
        (self.sketch_aligned(c).0, None, None)
    }

    /// The cursor aligned with woken endpoints (SK8, live: hovering an
    /// endpoint shows a red dotted horizontal or vertical inference line;
    /// no constraint is added), with the inference lines (from, to).
    pub fn sketch_aligned(&self, c: [f64; 2]) -> Aligned {
        let tol = self.tol_mm(4.0);
        let mut p = c;
        let (mut vx, mut vy) = (None, None);
        for w in &self.woken_points {
            if vx.is_none() && (c[0] - w[0]).abs() < tol && (c[1] - w[1]).abs() > tol {
                p[0] = w[0];
                vx = Some(*w);
            }
            if vy.is_none() && (c[1] - w[1]).abs() < tol && (c[0] - w[0]).abs() > tol {
                p[1] = w[1];
                vy = Some(*w);
            }
        }
        let lines = [vx, vy].into_iter().flatten().map(|w| (w, p)).collect();
        (p, lines)
    }

    fn edit_sketch(&mut self, name: &str, cx: &mut Context<Self>, f: impl FnOnce(&mut SheetSketch)) {
        self.edit_sheet(name, cx, |s| {
            f(&mut s.sketch);
            let _ = s.sketch.sketch.solve();
        });
    }

    /// A click with a sketch tool. True when handled.
    pub fn sketch_click(&mut self, cx: &mut Context<Self>) -> bool {
        let Tool::Sketch { tool, mut pts, from } = self.tool.clone() else {
            if let Tool::Constrain { .. } = self.tool {
                self.constrain_click(cx);
                return true;
            }
            return false;
        };
        let last = pts.last().copied();
        let (p, existing, infer) = self.sketch_target(last);
        match tool {
            SkTool::Line => {
                if let (Some(a), Some(_)) = (last, pts.first()) {
                    if ((p[0] - a[0]).powi(2) + (p[1] - a[1]).powi(2)).sqrt() < 1e-6 {
                        return true;
                    }
                    let mut end_id = None;
                    self.edit_sketch("Line", cx, |s| {
                        let sk = &mut s.sketch;
                        let start = from.map(EntityId).unwrap_or_else(|| sk.add_point(v2(a)));
                        let end = existing.unwrap_or_else(|| sk.add_point(v2(p)));
                        if let Ok(line) = sk.add_line_between(start, end) {
                            match infer {
                                Some(SkCon::Horizontal) => {
                                    let _ = sk.add_constraint(ConstraintKind::Horizontal { a: line, b: None });
                                }
                                Some(SkCon::Vertical) => {
                                    let _ = sk.add_constraint(ConstraintKind::Vertical { a: line, b: None });
                                }
                                _ => {}
                            }
                        }
                        end_id = Some(end);
                    });
                    // The chain continues from the new end point.
                    self.tool = Tool::Sketch { tool, pts: vec![p], from: end_id.map(|e| e.0) };
                } else {
                    self.tool = Tool::Sketch { tool, pts: vec![p], from: existing.map(|e| e.0) };
                }
            }
            SkTool::Circle | SkTool::CornerRect | SkTool::CenterRect => {
                if let Some(a) = last {
                    let name = tool.name();
                    self.edit_sketch(name, cx, |s| {
                        let sk = &mut s.sketch;
                        match tool {
                            SkTool::Circle => {
                                let r = ((p[0] - a[0]).powi(2) + (p[1] - a[1]).powi(2)).sqrt();
                                if r > 1e-6 {
                                    match from {
                                        Some(c) => {
                                            let _ = sk.add_circle_at(EntityId(c), r);
                                        }
                                        None => {
                                            sk.center_circle(v2(a), r);
                                        }
                                    }
                                }
                            }
                            SkTool::CornerRect => {
                                sk.corner_rectangle(v2(a), v2(p));
                            }
                            _ => {
                                sk.center_rectangle(v2(a), v2(p));
                            }
                        }
                    });
                    self.tool = Tool::Sketch { tool, pts: vec![], from: None };
                } else {
                    self.tool = Tool::Sketch { tool, pts: vec![p], from: existing.map(|e| e.0) };
                }
            }
            SkTool::Circle3 => {
                pts.push(p);
                if pts.len() == 3 {
                    self.edit_sketch("3 point circle", cx, |s| {
                        let _ = s.sketch.three_point_circle(v2(pts[0]), v2(pts[1]), v2(pts[2]));
                    });
                    pts.clear();
                }
                self.tool = Tool::Sketch { tool, pts, from: None };
            }
            SkTool::TangentArc => match (last, from) {
                (Some(_), Some(start)) => {
                    self.edit_sketch("Tangent arc", cx, |s| {
                        let _ = s.sketch.tangent_arc(EntityId(start), v2(p));
                    });
                    self.tool = Tool::Sketch { tool, pts: vec![], from: None };
                }
                _ => match existing {
                    // Start on a line / arc end point.
                    Some(id) if !self.sketch().sketch.users_of_point(id).is_empty() => self.tool = Tool::Sketch { tool, pts: vec![p], from: Some(id.0) },
                    _ => self.status = Some("Click the end of a line or arc".into()),
                },
            },
            SkTool::Spline => {
                pts.push(p);
                self.tool = Tool::Sketch { tool, pts, from: None };
            }
            SkTool::SplinePoint => {
                if let Some(id) = self.sketch_entity_at() {
                    self.edit_sketch("Spline point", cx, |s| {
                        let _ = s.sketch.insert_spline_point(id, v2(p));
                    });
                }
            }
            SkTool::Point => self.edit_sketch("Sketch point", cx, |s| {
                s.sketch.add_point(v2(p));
            }),
            SkTool::Trim => {
                let target = self.sketch_entity_at().or_else(|| self.sketch_point_at(self.cursor).map(|x| x.0));
                if let Some(id) = target {
                    let c = self.cursor;
                    self.edit_sketch("Trim", cx, |s| {
                        let point = matches!(s.sketch.entity(id).map(|e| &e.geometry), Some(Geometry::Point { .. }));
                        if point {
                            s.sketch.remove_entity(id);
                        } else {
                            let _ = s.sketch.trim(id, v2(c));
                        }
                    });
                }
            }
        }
        self.update_preview(cx);
        true
    }

    /// Spline: double-click or Esc ends it (two or more points). True when committed.
    pub fn finish_sketch(&mut self, cx: &mut Context<Self>) -> bool {
        let Tool::Sketch { tool: SkTool::Spline, pts, .. } = self.tool.clone() else { return false };
        let mut pts = pts;
        pts.dedup_by(|a, b| ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt() < 1e-6);
        if pts.len() < 2 {
            return false;
        }
        self.edit_sketch("Spline", cx, |s| {
            let p: Vec<DVec2> = pts.iter().map(|p| v2(*p)).collect();
            let _ = s.sketch.add_fit_spline(&p);
        });
        self.tool = Tool::Sketch { tool: SkTool::Spline, pts: vec![], from: None };
        self.update_preview(cx);
        true
    }

    /// A constraint pick: a sketch point, sketch curve or view edge (copied
    /// into the sketch as fixed external geometry).
    fn constrain_click(&mut self, cx: &mut Context<Self>) {
        let Tool::Constrain { kind, mut picks } = self.tool.clone() else { return };
        let pick: Option<Result<u32, Curve2>> = if let Some((id, _)) = self.sketch_point_at(self.cursor) {
            Some(Ok(id.0))
        } else if let Some(id) = self.sketch_entity_at() {
            Some(Ok(id.0))
        } else if let Some(Pick::Line { a, b }) = self.line_pick() {
            match (self.pick_sheet(&a), self.pick_sheet(&b)) {
                (Some(pa), Some(pb)) => Some(Err(Curve2::Line { a: v2(pa), b: v2(pb) })),
                _ => None,
            }
        } else if let Some(Pick::Circle { view, center, radius, .. }) = self.circle_pick(self.cursor) {
            self.placement(view).map(|pl| Err(Curve2::Circle { center: v2(pl.to_sheet(center)), radius: pl.len_to_sheet(radius) }))
        } else {
            None
        };
        let Some(pick) = pick else { return };
        let id = match pick {
            Ok(id) => id,
            Err(curve) => {
                // View geometry: a fixed external copy the constraint can use.
                let mut ext = None;
                self.edit_sheet("Use view geometry", cx, |s| {
                    if let Ok(r) = s.sketch.sketch.use_external(&curve) {
                        ext = r.created.first().copied();
                    }
                });
                match ext {
                    Some(e) => e.0,
                    None => return,
                }
            }
        };
        if !picks.contains(&id) {
            picks.push(id);
        }
        // Horizontal / Vertical take one line, or two points.
        let one_line = picks.len() == 1 && matches!(self.sketch().sketch.entity(EntityId(picks[0])).map(|e| &e.geometry), Some(Geometry::Line { .. }));
        if (kind.needs() == 1 && one_line) || picks.len() >= 2 {
            let (a, b) = (EntityId(picks[0]), picks.get(1).map(|x| EntityId(*x)));
            let ck = match (kind, b) {
                (SkCon::Horizontal, b) => Some(ConstraintKind::Horizontal { a, b }),
                (SkCon::Vertical, b) => Some(ConstraintKind::Vertical { a, b }),
                (SkCon::Coincident, Some(b)) => Some(ConstraintKind::Coincident { a, b }),
                (SkCon::Perpendicular, Some(b)) => Some(ConstraintKind::Perpendicular { a, b }),
                (SkCon::Parallel, Some(b)) => Some(ConstraintKind::Parallel { a, b }),
                (SkCon::Tangent, Some(b)) => Some(ConstraintKind::Tangent { a, b }),
                _ => None,
            };
            if let Some(ck) = ck {
                let mut err = None;
                self.edit_sketch(kind.name(), cx, |s| {
                    if let Err(e) = s.sketch.add_constraint(ck) {
                        err = Some(e.to_string());
                    }
                });
                self.status = err.map(|e| format!("{}: {e}", kind.name()).into());
            }
            picks.clear();
        }
        self.tool = Tool::Constrain { kind, picks };
        self.redraw(cx);
    }

    /// The tangent direction leaving the curve that ends at point `from`.
    fn end_tangent(&self, from: EntityId) -> Option<DVec2> {
        let sk = &self.sketch().sketch;
        let p = sk.point(from)?;
        let owner = sk.entities().find(|e| e.endpoints().is_some_and(|(s, en)| s == from || en == from))?;
        let c = sk.curve(owner.id)?;
        // Sample just inside the end and step out through it.
        let pts = c.tessellate(0.01);
        let (first, last) = (*pts.first()?, *pts.last()?);
        let near = if (first - p).length() < (last - p).length() { pts.get(1).copied()? } else { pts.get(pts.len().checked_sub(2)?).copied()? };
        Some((p - near).normalize_or_zero())
    }

    /// Rubber bands of the active sketch tool.
    pub fn sketch_overlay(&self, overlay: &mut crate::drawing_scene::Overlay) {
        let Tool::Sketch { tool, pts, from } = &self.tool else { return };
        let (p, _, infer) = self.sketch_target(pts.last().copied());
        if infer.is_none() {
            overlay.infer.extend(self.sketch_aligned(self.cursor).1);
        }
        let Some(a) = pts.last().copied() else { return };
        match tool {
            SkTool::TangentArc => {
                // The arc tangent to the start curve through the cursor.
                let arc = from.and_then(|f| self.end_tangent(EntityId(f))).and_then(|d| {
                    let (p0, p1) = (v2(a), v2(p));
                    let ch = p1 - p0;
                    let n = DVec2::new(-d.y, d.x);
                    let k = ch.dot(n);
                    if k.abs() < 1e-9 {
                        return None;
                    }
                    let r = ch.length_squared() / (2.0 * k);
                    let c = p0 + n * r;
                    let (a0, a1) = ((p0 - c).y.atan2((p0 - c).x), (p1 - c).y.atan2((p1 - c).x));
                    // Sweep in the tangent's sense.
                    let ccw = r > 0.0;
                    let mut sweep = a1 - a0;
                    if ccw && sweep < 0.0 {
                        sweep += std::f64::consts::TAU;
                    }
                    if !ccw && sweep > 0.0 {
                        sweep -= std::f64::consts::TAU;
                    }
                    Some((0..=48).map(|i| {
                        let t = a0 + sweep * i as f64 / 48.0;
                        [c.x + r.abs() * t.cos(), c.y + r.abs() * t.sin()]
                    }).collect::<Vec<_>>())
                });
                overlay.lines.push((arc.unwrap_or_else(|| vec![a, p]), false));
            }
            SkTool::Line => {
                overlay.lines.push((vec![a, p], false));
                if infer.is_some() {
                    overlay.infer.push((a, p));
                }
            }
            SkTool::Circle => overlay.circle(a, ((p[0] - a[0]).powi(2) + (p[1] - a[1]).powi(2)).sqrt(), false),
            SkTool::CornerRect => overlay.rect(a, p, false),
            SkTool::CenterRect => overlay.rect([2.0 * a[0] - p[0], 2.0 * a[1] - p[1]], p, false),
            SkTool::Circle3 => {
                if pts.len() == 2
                    && let Some((c, r)) = noshape_sketch::circle_through(v2(pts[0]), v2(pts[1]), v2(p))
                {
                    overlay.circle([c.x, c.y], r, false);
                } else {
                    overlay.lines.push((vec![a, p], true));
                }
            }
            SkTool::Spline => {
                let mut all = pts.clone();
                all.push(p);
                overlay.lines.push((all, false));
            }
            _ => {}
        }
        for q in pts {
            overlay.points.push(*q);
        }
    }

    /// Move a sketch entity (all its points) by `d` from `orig`, then solve.
    pub fn drag_sketch(&mut self, id: u32, orig: &SheetSketch, d: [f64; 2]) -> Drawing {
        let mut dr = self.drawing.clone();
        let mut sk = orig.clone();
        for p in entity_points(&sk.sketch, EntityId(id)) {
            if let Some(q) = orig.sketch.point(p) {
                let _ = sk.sketch.set_point(p, q + v2(d));
            }
        }
        let _ = sk.sketch.solve();
        dr.sheets[self.sheet].sketch = sk;
        dr
    }
}

impl DrawingEditor {
    /// Context menu of a sketch entity: Linestyle ▸, Delete.
    pub fn sketch_entries(&self, id: u32) -> Vec<MenuEntry> {
        use noshape_model::drawing::sheet_sketch::SketchLine;
        let cur = self.sketch().style(EntityId(id));
        let styles = SketchLine::ALL
            .iter()
            .map(|&l| {
                MenuEntry::check(l.name(), cur.line == l, move |t, _, cx| {
                    t.edit_sheet("Linestyle", cx, |s| {
                        let mut st = s.sketch.style(EntityId(id));
                        st.line = l;
                        s.sketch.styles.insert(id, st);
                    })
                })
            })
            .collect();
        vec![
            MenuEntry::Submenu("Linestyle".into(), styles),
            MenuEntry::Separator,
            MenuEntry::item("Delete", move |t, _, cx| {
                t.selection.clear();
                t.edit_sketch("Delete", cx, |s| {
                    s.sketch.remove_entity(EntityId(id));
                });
            }),
        ]
    }
}

impl DrawingEditor {
    /// Toolbar › Insert DXF or DWG: the file's model-space wireframe becomes
    /// drawing sketch geometry at its own coordinates (drawing units; blocks
    /// and polylines exploded, colors dropped).
    pub fn insert_cad_file(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let this = cx.entity().downgrade();
        let doc = self.doc.clone();
        use noshape_model::file_tab::FileKind;
        crate::file_tab_view::pick_file(doc, "Insert DXF or DWG", &[FileKind::Drawing2d], &["dxf", "dwg"], window, cx, move |file, _, cx| {
            let parsed = file.bytes().ok_or_else(|| "the file data is damaged".to_string()).and_then(|b| noshape_sketch::cad::parse_cad(&b).map_err(|e| e.to_string()));
            this.update(cx, |this, cx| match parsed {
                Ok(d) => {
                    // To sheet mm: the file's unit, else the drawing's.
                    let scale = d.unit_scale.map(|m| m * 1000.0).unwrap_or(if this.drawing.units_mm { 1.0 } else { 25.4 });
                    let mut n = 0;
                    this.edit_sketch("Insert DXF or DWG", cx, |s| n = s.sketch.insert_drawing(&d, scale, DVec2::ZERO).len());
                    this.status = Some(format!("Inserted {n} entities from {}", file.file_name).into());
                    cx.notify();
                }
                Err(e) => {
                    this.status = Some(format!("Insert DXF or DWG failed: {e}").into());
                    cx.notify();
                }
            })
            .ok();
        });
    }
}

impl DrawingEditor {
    /// Where an entity's constraint glyphs start (sheet mm).
    fn glyph_anchor(&self, id: EntityId) -> Option<[f64; 2]> {
        let sk = &self.sketch().sketch;
        if let Some(p) = sk.point(id).filter(|_| matches!(sk.entity(id).map(|e| &e.geometry), Some(Geometry::Point { .. }))) {
            return Some([p.x, p.y]);
        }
        // The point halfway along the curve.
        let pts = sk.curve(id)?.tessellate(0.05);
        let total: f64 = pts.windows(2).map(|w| (w[1] - w[0]).length()).sum();
        let mut run = 0.0;
        for w in pts.windows(2) {
            let l = (w[1] - w[0]).length();
            if run + l >= total / 2.0 && l > 0.0 {
                let p = w[0] + (w[1] - w[0]) * ((total / 2.0 - run) / l);
                return Some([p.x, p.y]);
            }
            run += l;
        }
        pts.first().map(|p| [p.x, p.y])
    }

    /// SK15: constraint glyphs of the hovered sketch entity (all of them
    /// while Shift is held). Hovering a glyph gives it a yellow background
    /// and highlights its entities; right-click › Delete removes it.
    pub fn render_sketch_glyphs(&mut self, window: &Window, cx: &mut Context<Self>) -> Vec<AnyElement> {
        if self.sketch().sketch.constraint_count() == 0 {
            return vec![];
        }
        let all = window.modifiers().shift;
        // The hovered entity stays while the cursor heads for its glyphs.
        let hovered = match self.sketch_entity_at() {
            Some(e) => Some(e),
            None => self.glyph_entity.map(EntityId).filter(|e| {
                self.glyph_hover.is_some()
                    || self.glyph_anchor(*e).is_some_and(|a| {
                        let (p, c) = (self.sheet_to_local(a), self.sheet_to_local(self.cursor));
                        ((p[0] - c[0]).powi(2) + (p[1] - c[1]).powi(2)).sqrt() < 70.0
                    })
            }),
        };
        self.glyph_entity = hovered.map(|e| e.0);
        let sk = &self.sketch().sketch;
        let mut by_entity: Vec<(EntityId, Vec<(noshape_sketch::ConstraintId, &'static str)>)> = Vec::new();
        for c in sk.constraints() {
            if c.kind.is_dimension() {
                continue;
            }
            let Some(icon) = crate::sketch_mode::constraint_icon(&c.kind) else { continue };
            for e in c.kind.entities() {
                if sk.entity(e).is_some_and(|x| x.external) || !(all || hovered == Some(e)) {
                    continue;
                }
                match by_entity.iter_mut().find(|x| x.0 == e) {
                    Some(x) => x.1.push((c.id, icon)),
                    None => by_entity.push((e, vec![(c.id, icon)])),
                }
            }
        }
        let mut out = Vec::new();
        for (e, glyphs) in by_entity {
            let Some(a) = self.glyph_anchor(e) else { continue };
            let base = self.sheet_to_local(a);
            for (k, (cid, icon)) in glyphs.into_iter().enumerate() {
                let on = self.glyph_hover == Some(cid.0);
                out.push(
                    div()
                        .id(SharedString::from(format!("sk-glyph-{}-{}", cid.0, e.0)))
                        .absolute()
                        .left(px((base[0] + 8.0 + k as f64 * 19.0) as f32))
                        .top(px((base[1] + 6.0) as f32))
                        .size(px(17.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(2.))
                        .bg(if on { gpui_kit::rgb(0xfff27a) } else { gpui_kit::rgba(0xffffffcc) })
                        .child(crate::assets::icon(icon, 15., cx))
                        .on_hover(cx.listener(move |this, h: &bool, _, cx| {
                            this.glyph_hover = if *h { Some(cid.0) } else if this.glyph_hover == Some(cid.0) { None } else { this.glyph_hover };
                            // Its entities highlight.
                            this.panel_highlight = (*h).then_some(Sel::Sketch(e.0));
                            this.redraw(cx);
                        }))
                        .on_mouse_down(
                            MouseButton::Right,
                            cx.listener(move |_, ev: &MouseDownEvent, window, cx| {
                                cx.stop_propagation();
                                let entries = vec![MenuEntry::item("Delete", move |t, _, cx| {
                                    t.glyph_hover = None;
                                    t.edit_sketch("Delete constraint", cx, |s| {
                                        let _ = s.sketch.remove_constraint(cid);
                                    });
                                })];
                                let ed = cx.entity();
                                let menu = crate::os_menu::OsMenu::build(window, cx, move |m, window, cx| fill_os_menu(m, &entries, &ed, window, cx));
                                crate::os_menu::open(ev.position, menu, window, cx);
                            }),
                        )
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .into_any_element(),
                );
            }
        }
        out
    }
}
