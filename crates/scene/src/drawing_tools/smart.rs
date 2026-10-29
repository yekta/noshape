//! Drawing slice 3: Onshape's smart Dimension tool (d), from
//! `crates/app/src/drawing_smart.rs`. "The type of curve selected determines
//! the type of dimension": a circle gives a diameter, an arc a radius, a line
//! its length; two points, a point and a line, two parallel lines (distance)
//! or two lines at an angle (angular), a circle with anything (from its
//! center), two concentric circles (wall thickness). Clicking a picked edge
//! again deselects it.

use super::geom::*;
use super::{DimTool, Pick, PickedPoint, SheetPick};
use noshape_model::drawing::views;
use noshape_model::drawing::*;

/// Annotation reference slots and their model points.
pub type Refs = Vec<(u8, Option<PointRef>)>;

/// Same picked entity (same view and geometry).
pub fn same(a: &Pick, b: &Pick) -> bool {
    let close = |p: [f64; 2], q: [f64; 2]| len(sub(p, q)) < 1e-9;
    match (a, b) {
        (Pick::Point(p), Pick::Point(q)) => p.view == q.view && close(p.p, q.p),
        (Pick::Line { a: a1, b: b1 }, Pick::Line { a: a2, b: b2 }) => a1.view == a2.view && ((close(a1.p, a2.p) && close(b1.p, b2.p)) || (close(a1.p, b2.p) && close(b1.p, a2.p))),
        (Pick::Circle { view: v1, center: c1, radius: r1, .. } | Pick::Arc { view: v1, center: c1, radius: r1, .. }, Pick::Circle { view: v2, center: c2, radius: r2, .. } | Pick::Arc { view: v2, center: c2, radius: r2, .. }) => {
            v1 == v2 && close(*c1, *c2) && (r1 - r2).abs() < 1e-9
        }
        _ => false,
    }
}

/// The view a pick is in.
pub fn view_of(p: &Pick) -> ViewId {
    match p {
        Pick::Point(p) => p.view,
        Pick::Line { a, .. } => a.view,
        Pick::Circle { view, .. } | Pick::Arc { view, .. } | Pick::CircleAt { view, .. } => *view,
    }
}

impl SheetPick<'_> {
    /// Two picked parallel lines that are the silhouette of one cylinder in
    /// the view (a circle seen edge-on, centered between them, diameter =
    /// their distance): Onshape gives the dimension an automatic ⌀ (help
    /// drawing_-_dimension; also in section views).
    pub fn cylinder_silhouettes(&self, picks: &[Pick]) -> bool {
        let [Pick::Line { a: a1, b: b1 }, Pick::Line { a: a2, .. }] = picks else { return false };
        if a1.view != a2.view {
            return false;
        }
        let u = norm(sub(b1.p, a1.p));
        let side = |p: [f64; 2]| (p[0] - a1.p[0]) * u[1] - (p[1] - a1.p[1]) * u[0];
        let d = side(a2.p);
        if d.abs() < 1e-12 {
            return false;
        }
        let Some(g) = self.geoms.get(&a1.view) else { return false };
        let tol = d.abs() * 1e-3;
        g.model_edges.iter().any(|me| match me.geom {
            views::EdgeGeom::Circle { center, radius, face_on: false, .. } => (2.0 * radius - d.abs()).abs() < tol && (side(center) - d / 2.0).abs() < tol,
            _ => false,
        })
    }

    /// The hovered straight edge as one silhouette of a cylinder seen side
    /// on (an edge-on circle of the view with its center at the radius from
    /// the line), with the opposite silhouette: the line mirrored through the
    /// cylinder's axis.
    pub fn silhouette_pair(&self) -> Option<Vec<Pick>> {
        let Some(Pick::Line { a, b }) = self.line_pick() else { return None };
        let g = self.geoms.get(&a.view)?;
        let u = norm(sub(b.p, a.p));
        let off = |p: [f64; 2]| {
            let t = dot(sub(p, a.p), u);
            sub(p, [a.p[0] + u[0] * t, a.p[1] + u[1] * t])
        };
        let n = g.model_edges.iter().find_map(|me| match me.geom {
            views::EdgeGeom::Circle { center, radius, face_on: false, .. } => {
                let n = off(center);
                ((len(n) - radius).abs() < radius * 1e-3).then_some(n)
            }
            _ => None,
        })?;
        let mirror = |p: &PickedPoint| PickedPoint { view: p.view, p: [p.p[0] + 2.0 * n[0], p.p[1] + 2.0 * n[1]], r: None };
        Some(vec![Pick::Line { a: a.clone(), b: b.clone() }, Pick::Line { a: mirror(&a), b: mirror(&b) }])
    }

    /// 2 point linear, Point to line and Line to line: the click adds only
    /// the entity type the tool takes (points; a point and a line; two
    /// parallel lines), in the first pick's view; a picked entity clicked
    /// again is deselected.
    pub fn typed_linear_click(&self, kind: DimTool, c: [f64; 2], picks: &mut Vec<Pick>) {
        let point = || self.snap_at(c).map(Pick::Point);
        let line = || self.line_pick();
        let has_point = picks.iter().any(|p| matches!(p, Pick::Point(_)));
        let p = match kind {
            DimTool::Point2 => point(),
            DimTool::PointLine if picks.is_empty() => point().or_else(line),
            DimTool::PointLine if has_point => line(),
            DimTool::PointLine => point(),
            _ => line().filter(|l| match (picks.first(), l) {
                // The second line must be parallel to the first.
                (Some(Pick::Line { a: a1, b: b1 }), Pick::Line { a: a2, b: b2 }) => {
                    let (u, v) = (norm(sub(b1.p, a1.p)), norm(sub(b2.p, a2.p)));
                    (u[0] * v[1] - u[1] * v[0]).abs() < 1e-6
                }
                _ => true,
            }),
        };
        let Some(p) = p else { return };
        if let Some(i) = picks.iter().position(|q| same(q, &p)) {
            picks.remove(i);
            return;
        }
        if picks.first().is_some_and(|f| view_of(f) != view_of(&p)) {
            return;
        }
        if picks.len() < 2 {
            picks.push(p);
        }
    }

    /// With one pick: does a click at `c` add (or deselect) a pick rather
    /// than place the dimension?
    pub fn smart_takes(&self, c: [f64; 2], first: &Pick) -> bool {
        self.smart_pick(c).is_some_and(|p| same(&p, first) || view_of(&p) == view_of(first))
    }

    pub fn smart_click(&self, c: [f64; 2], picks: &mut Vec<Pick>) {
        let Some(p) = self.smart_pick(c) else { return };
        if let Some(i) = picks.iter().position(|q| same(q, &p)) {
            picks.remove(i);
            return;
        }
        if picks.len() >= 2 || picks.first().is_some_and(|f| view_of(f) != view_of(&p)) {
            picks.clear();
        }
        picks.push(p);
    }

    /// Angular dimension between two lines (smaller angle, vertex at their
    /// intersection; associative when they share the vertex).
    pub fn angular_from_lines(&self, a1: &PickedPoint, b1: &PickedPoint, a2: &PickedPoint, b2: &PickedPoint) -> Option<(AnnotationKind, Refs)> {
        if a1.view != a2.view {
            return None;
        }
        let c = self.cursor;
        let v = intersect(a1.p, b1.p, a2.p, b2.p)?;
        let far = |a: &PickedPoint, b: &PickedPoint| if len(sub(a.p, v)) > len(sub(b.p, v)) { a.clone() } else { b.clone() };
        let near = |a: &PickedPoint, b: &PickedPoint| if len(sub(a.p, v)) > len(sub(b.p, v)) { b.clone() } else { a.clone() };
        let (mut pa, mut pb) = (far(a1, b1), far(a2, b2));
        let ang = |p: [f64; 2]| (p[1] - v[1]).atan2(p[0] - v[0]);
        let mut sweep = ang(pb.p) - ang(pa.p);
        while sweep < 0.0 {
            sweep += std::f64::consts::TAU;
        }
        if sweep > std::f64::consts::PI {
            std::mem::swap(&mut pa, &mut pb);
        }
        let pl = self.placement(a1.view)?;
        let radius = len(sub(c, pl.to_sheet(v))).max(3.0);
        let (n1, n2) = (near(a1, b1), near(a2, b2));
        let vref = match (&n1.r, &n2.r) {
            (Some(x), Some(y)) if x == y && len(sub(n1.p, v)) < 1e-7 => Some(x.clone()),
            _ => None,
        };
        let refs = if vref.is_some() { vec![(0, vref), (1, pa.r.clone()), (2, pb.r.clone())] } else { vec![] };
        let mut kind = AnnotationKind::AngularDimension { view: a1.view, vertex: v, a: pa.p, b: pb.p, radius, three_point: false, flip_a: false, flip_b: false };
        // The angle of the sector the pointer is in (inner, supplementary
        // or vertical).
        let mut refs = refs;
        if super::geom::angular_to_sector(&mut kind, &pl, c) {
            for r in &mut refs {
                r.0 = match r.0 {
                    1 => 2,
                    2 => 1,
                    s => s,
                };
            }
        }
        Some((kind, refs))
    }

    /// A linear dimension from `a` to `b`, oriented by the cursor (or
    /// `fixed`).
    pub fn linear(&self, a: &PickedPoint, b: &PickedPoint, fixed: Option<DimOrientation>) -> Option<(AnnotationKind, Refs)> {
        let (ps, qs) = (self.pick_sheet(a)?, self.pick_sheet(b)?);
        let c = self.cursor;
        let o = fixed.unwrap_or_else(|| auto_orientation(ps, qs, c));
        let offset = dim_offset(ps, qs, o, c);
        Some((AnnotationKind::LinearDimension { view: a.view, a: a.p, b: b.p, offset, orientation: o, override_text: None }, vec![(0, a.r.clone()), (1, b.r.clone())]))
    }

    /// Point to line: the perpendicular distance. Along a sheet axis the
    /// line's end point keeps it associative; otherwise it measures to the
    /// foot of the perpendicular.
    pub fn point_to_line(&self, p: &PickedPoint, a: &PickedPoint, b: &PickedPoint) -> Option<(AnnotationKind, Refs)> {
        let (sa, sb) = (self.pick_sheet(a)?, self.pick_sheet(b)?);
        let d = norm(sub(sb, sa));
        if d[1].abs() < 1e-9 {
            return self.linear(p, a, Some(DimOrientation::Vertical));
        }
        if d[0].abs() < 1e-9 {
            return self.linear(p, a, Some(DimOrientation::Horizontal));
        }
        let u = norm(sub(b.p, a.p));
        let t = dot(sub(p.p, a.p), u);
        let foot = PickedPoint { view: a.view, p: [a.p[0] + u[0] * t, a.p[1] + u[1] * t], r: None };
        self.linear(p, &foot, Some(DimOrientation::Aligned))
    }

    /// The smart Dimension's annotation for `picks` (beyond one line or
    /// two points, which the linear dimension handles).
    pub fn smart_dimension(&self, picks: &[Pick]) -> Option<(AnnotationKind, Refs)> {
        let c = self.cursor;
        let center = |p: &Pick| -> Option<PickedPoint> {
            match p {
                Pick::Point(p) => Some(p.clone()),
                Pick::Circle { view, center, r, .. } | Pick::Arc { view, center, r, .. } | Pick::CircleAt { view, center, r, .. } => Some(PickedPoint { view: *view, p: *center, r: r.clone() }),
                Pick::Line { .. } => None,
            }
        };
        match picks {
            // A circle: diameter; an arc: radius (the leader follows the cursor).
            [Pick::Circle { view, center, radius, r }] | [Pick::Arc { view, center, radius, r, .. }] => {
                let diameter = matches!(picks[0], Pick::Circle { .. });
                let cs = self.placement(*view)?.to_sheet(*center);
                let d = sub(c, cs);
                Some((AnnotationKind::RadialDimension { view: *view, center: *center, radius: *radius, diameter, angle: d[1].atan2(d[0]), dim_line: diameter && self.drawing.radial_dim_lines(), knee: self.radial_knee(*view, *center, *radius, c) }, vec![(0, r.clone())]))
            }
            [Pick::Line { a: a1, b: b1 }, Pick::Line { a: a2, b: b2 }] => {
                let (u, w) = (norm(sub(b1.p, a1.p)), norm(sub(b2.p, a2.p)));
                if (u[0] * w[1] - u[1] * w[0]).abs() < 1e-6 {
                    // Parallel: the distance between them.
                    self.point_to_line(a1, a2, b2)
                } else {
                    self.angular_from_lines(a1, b1, a2, b2)
                }
            }
            [Pick::Line { a, b }, x] | [x, Pick::Line { a, b }] => self.point_to_line(&center(x)?, a, b),
            // Two concentric circles / arcs: the wall thickness, toward the
            // cursor. Each end follows its circle (slots 10 / 11), so the
            // dimension turns red when a circle goes (qa DWG-DANGLING-WALL).
            [Pick::Circle { view, center: c1, radius: r1, r: q1 } | Pick::Arc { view, center: c1, radius: r1, r: q1, .. }, Pick::Circle { center: c2, radius: r2, r: q2, .. } | Pick::Arc { center: c2, radius: r2, r: q2, .. }] if len(sub(*c1, *c2)) < 1e-9 => {
                let pl = self.placement(*view)?;
                let u = norm(sub(pl.to_view(c), *c1));
                let at = |r: f64| PickedPoint { view: *view, p: [c1[0] + u[0] * r, c1[1] + u[1] * r], r: None };
                let (kind, _) = self.linear(&at(*r1), &at(*r2), Some(DimOrientation::Aligned))?;
                let refs = match (q1, q2) {
                    (Some(x), Some(y)) => vec![(10, Some(x.clone())), (11, Some(y.clone()))],
                    _ => Vec::new(),
                };
                Some((kind, refs))
            }
            [x, y] => self.linear(&center(x)?, &center(y)?, None),
            _ => None,
        }
    }

    /// A click of a dimension tool that picks rather than places (the
    /// editor's tool click): what each tool adds to `picks`. Maximum /
    /// minimum and foreshortened dimensions pick with the editor's
    /// `more_dimension_pick`.
    pub fn dimension_click(&self, kind: DimTool, c: [f64; 2], picks: &mut Vec<Pick>) {
        match kind {
            DimTool::Radius | DimTool::Diameter => {
                if let Some(p) = self.circle_pick(c) {
                    *picks = vec![p];
                } else if kind == DimTool::Diameter
                    && let Some(pair) = self.silhouette_pair()
                {
                    // A cylinder's silhouette: its diameter (help D15).
                    *picks = pair;
                }
            }
            DimTool::Angle | DimTool::Chamfer => {
                if let Some(p) = self.line_pick() {
                    picks.push(p);
                }
            }
            DimTool::ArcLength => {
                if let Some(p) = self.arc_pick() {
                    *picks = vec![p];
                }
            }
            DimTool::MaxMin | DimTool::Foreshorten => {}
            DimTool::Angle3 => {
                if let Some(p) = self.snap_at(c).filter(|p| picks.first().is_none_or(|f| matches!(f, Pick::Point(v) if v.view == p.view))) {
                    picks.push(Pick::Point(p));
                }
            }
            DimTool::Point2 | DimTool::PointLine | DimTool::LineLine => self.typed_linear_click(kind, c, picks),
            DimTool::Linear => self.smart_click(c, picks),
        }
    }

    /// With `picks` made, does a click at `c` add a pick (the smart
    /// Dimension's second entity; linear tools until two) rather than place
    /// the dimension?
    pub fn dimension_takes_more(&self, kind: DimTool, c: [f64; 2], picks: &[Pick]) -> bool {
        match kind {
            DimTool::Linear => picks.len() == 1 && self.smart_takes(c, &picks[0]),
            k if k.linear() => picks.len() < 2,
            _ => false,
        }
    }

}
