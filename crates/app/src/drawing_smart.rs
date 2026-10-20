//! Drawing editor: Onshape's smart Dimension tool (d). "The type of curve
//! selected determines the type of dimension": a circle gives a diameter,
//! an arc a radius, a line its length; two points, a point and a line, two
//! parallel lines (distance) or two lines at an angle (angular), a circle
//! with anything (from its center), two concentric circles (wall
//! thickness). Clicking a picked edge again deselects it.

use crate::drawing_tools::{auto_orientation, dim_offset, intersect};
use crate::drawing_view::*;
use noshape_model::drawing::*;
use noshape_model::drawing::views;

fn sub(a: [f64; 2], b: [f64; 2]) -> [f64; 2] {
    [a[0] - b[0], a[1] - b[1]]
}
fn dot(a: [f64; 2], b: [f64; 2]) -> f64 {
    a[0] * b[0] + a[1] * b[1]
}
fn len(a: [f64; 2]) -> f64 {
    dot(a, a).sqrt()
}
fn norm(a: [f64; 2]) -> [f64; 2] {
    let l = len(a).max(1e-12);
    [a[0] / l, a[1] / l]
}

type Refs = Vec<(u8, Option<PointRef>)>;

impl DrawingEditor {
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
}

/// Same picked entity (same view and geometry).
fn same(a: &Pick, b: &Pick) -> bool {
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

fn view_of(p: &Pick) -> ViewId {
    match p {
        Pick::Point(p) => p.view,
        Pick::Line { a, .. } => a.view,
        Pick::Circle { view, .. } | Pick::Arc { view, .. } | Pick::CircleAt { view, .. } => *view,
    }
}

impl DrawingEditor {
    /// What the smart tool picks under the cursor: a vertex or center
    /// snap, else the hovered edge (line, arc or circle).
    pub fn smart_pick(&self, c: [f64; 2]) -> Option<Pick> {
        if let Some(p) = self.snap_at(c) {
            return Some(Pick::Point(p));
        }
        self.line_pick().or_else(|| self.arc_pick()).or_else(|| self.hovered_edge().and_then(|_| self.circle_pick(c)))
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
        Some((AnnotationKind::AngularDimension { view: a1.view, vertex: v, a: pa.p, b: pb.p, radius, three_point: false }, refs))
    }

    /// A linear dimension from `a` to `b`, oriented by the cursor (or
    /// `fixed`).
    fn linear(&self, a: &PickedPoint, b: &PickedPoint, fixed: Option<DimOrientation>) -> Option<(AnnotationKind, Refs)> {
        let (ps, qs) = (self.pick_sheet(a)?, self.pick_sheet(b)?);
        let c = self.cursor;
        let o = fixed.unwrap_or_else(|| auto_orientation(ps, qs, c));
        let offset = dim_offset(ps, qs, o, c);
        Some((AnnotationKind::LinearDimension { view: a.view, a: a.p, b: b.p, offset, orientation: o, override_text: None }, vec![(0, a.r.clone()), (1, b.r.clone())]))
    }

    /// Point to line: the perpendicular distance. Along a sheet axis the
    /// line's end point keeps it associative; otherwise it measures to the
    /// foot of the perpendicular.
    fn point_to_line(&self, p: &PickedPoint, a: &PickedPoint, b: &PickedPoint) -> Option<(AnnotationKind, Refs)> {
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
                Some((AnnotationKind::RadialDimension { view: *view, center: *center, radius: *radius, diameter, angle: d[1].atan2(d[0]) }, vec![(0, r.clone())]))
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
            // Two concentric circles / arcs: the wall thickness, toward the cursor.
            [Pick::Circle { view, center: c1, radius: r1, .. } | Pick::Arc { view, center: c1, radius: r1, .. }, Pick::Circle { center: c2, radius: r2, .. } | Pick::Arc { center: c2, radius: r2, .. }] if len(sub(*c1, *c2)) < 1e-9 => {
                let pl = self.placement(*view)?;
                let u = norm(sub(pl.to_view(c), *c1));
                let at = |r: f64| PickedPoint { view: *view, p: [c1[0] + u[0] * r, c1[1] + u[1] * r], r: None };
                self.linear(&at(*r1), &at(*r2), Some(DimOrientation::Aligned))
            }
            [x, y] => self.linear(&center(x)?, &center(y)?, None),
            _ => None,
        }
    }
}
