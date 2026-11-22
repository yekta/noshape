//! The drawing Measure tool (`[`): what a click picks and the panel's rows for the picks.

use super::{Pick, PickedPoint, SheetPick, seg_dist};
use noshape_model::drawing::ViewId;

/// The Measure panel's units (live2 wake-1118, in live's order): name, meters per unit, abbreviation.
pub const MEASURE_UNITS: [(&str, f64, &str); 6] = [("Inches", 0.0254, "in"), ("Millimeters", 0.001, "mm"), ("Centimeters", 0.01, "cm"), ("Feet", 0.3048, "ft"), ("Meters", 1.0, "m"), ("Yards", 0.9144, "yd")];

/// The Measure panel's unit list: live's fixed order, the drawing's own unit suffixed "(Drawing)" in its place (live2
/// wake-1118 mu-mm-units.png).
/// What the Measure tool (`[`) picked.
#[derive(Clone, Debug)]
pub enum MeasurePick {
    Point(PickedPoint),
    Line(PickedPoint, PickedPoint),
    Circle { view: ViewId, center: [f64; 2], radius: f64 },
}

impl MeasurePick {
    pub fn view(&self) -> ViewId {
        match self {
            MeasurePick::Point(p) | MeasurePick::Line(p, _) => p.view,
            MeasurePick::Circle { view, .. } => *view,
        }
    }
}

/// A Measure panel value: a length (model meters) or a projected angle (degrees).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Measured {
    Length(f64),
    Angle(f64),
}

/// The Measure panel's rows and its relation for the picks (live measure-1119): a point alone shows nothing; a line
/// its Length; a circle its Diameter and Length (circumference); two points Min distance, dx, dy; a point and a line
/// Min distance and the line's Length; a point and a circle Min / Max / Center distance and the circle's Length; a line
/// and a circle Min / Max / Center distance; two circles Min / Center distance; two lines (Parallel distance or an
/// oblique pair's Angle,) Min distance; pairs with curves sum the curves' Lengths; three picks or more only that sum.
pub fn measure_rows(picks: &[MeasurePick]) -> (Vec<(&'static str, Measured)>, Option<&'static str>) {
    let (rows, relation) = measure_lengths(picks);
    let angle = |r: &(&'static str, f64)| if r.0 == "Angle" { Measured::Angle(r.1) } else { Measured::Length(r.1) };
    (rows.iter().map(|r| (r.0, angle(r))).collect(), relation)
}

fn measure_lengths(picks: &[MeasurePick]) -> (Vec<(&'static str, f64)>, Option<&'static str>) {
    let d2 = |a: [f64; 2], b: [f64; 2]| (b[0] - a[0]).hypot(b[1] - a[1]);
    let circ = |r: f64| 2.0 * std::f64::consts::PI * r;
    let curve = |m: &MeasurePick| match m {
        MeasurePick::Point(_) => 0.0,
        MeasurePick::Line(a, b) => d2(a.p, b.p),
        MeasurePick::Circle { radius, .. } => circ(*radius),
    };
    if picks.len() > 2 {
        let curves: Vec<&MeasurePick> = picks.iter().filter(|m| !matches!(m, MeasurePick::Point(_))).collect();
        return (if curves.is_empty() { Vec::new() } else { vec![("Length", curves.iter().map(|m| curve(m)).sum())] }, None);
    }
    let mut sorted: Vec<&MeasurePick> = picks.iter().collect();
    sorted.sort_by_key(|m| match m {
        MeasurePick::Point(_) => 0,
        MeasurePick::Line(..) => 1,
        MeasurePick::Circle { .. } => 2,
    });
    match sorted.as_slice() {
        [MeasurePick::Line(a, b)] => (vec![("Length", d2(a.p, b.p))], Some("Line")),
        [MeasurePick::Circle { radius, .. }] => (vec![("Diameter", radius * 2.0), ("Length", circ(*radius))], Some("Circle")),
        [MeasurePick::Point(p), MeasurePick::Point(q)] => {
            (vec![("Min distance", d2(p.p, q.p)), ("dx", (q.p[0] - p.p[0]).abs()), ("dy", (q.p[1] - p.p[1]).abs())], Some("Point-Point"))
        }
        [MeasurePick::Point(p), MeasurePick::Line(a, b)] => (vec![("Min distance", seg_dist(p.p, a.p, b.p)), ("Length", d2(a.p, b.p))], Some("Point-Line")),
        [MeasurePick::Point(p), MeasurePick::Circle { center, radius, .. }] => {
            let c = d2(p.p, *center);
            (vec![("Min distance", (c - radius).abs()), ("Max distance", c + radius), ("Center distance", c), ("Length", circ(*radius))], Some("Point-Circle"))
        }
        [MeasurePick::Line(a1, b1), MeasurePick::Line(a2, b2)] => {
            let (u, v) = ([b1.p[0] - a1.p[0], b1.p[1] - a1.p[1]], [b2.p[0] - a2.p[0], b2.p[1] - a2.p[1]]);
            let (lu, lv) = (u[0].hypot(u[1]), v[0].hypot(v[1]));
            let sin = (u[0] * v[1] - u[1] * v[0]) / (lu * lv).max(1e-300);
            let cos = (u[0] * v[0] + u[1] * v[1]) / (lu * lv).max(1e-300);
            let crosses = {
                let side = |a: [f64; 2], b: [f64; 2], p: [f64; 2]| (b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0]);
                side(a1.p, b1.p, a2.p) * side(a1.p, b1.p, b2.p) < 0.0 && side(a2.p, b2.p, a1.p) * side(a2.p, b2.p, b1.p) < 0.0
            };
            let min = if crosses { 0.0 } else { [seg_dist(a1.p, a2.p, b2.p), seg_dist(b1.p, a2.p, b2.p), seg_dist(a2.p, a1.p, b1.p), seg_dist(b2.p, a1.p, b1.p)].into_iter().fold(f64::MAX, f64::min) };
            let mut rows = Vec::new();
            let parallel = sin.abs() < 1e-9;
            let perpendicular = cos.abs() < 1e-9;
            if parallel {
                let w = [a2.p[0] - a1.p[0], a2.p[1] - a1.p[1]];
                rows.push(("Parallel distance", (u[0] * w[1] - u[1] * w[0]).abs() / lu.max(1e-300)));
            } else if !perpendicular {
                // The angle between the rays from where the lines meet to each one's far end (live H: two isometric
                // edges from one corner, 120.0 deg on the sheet).
                let w = [a2.p[0] - a1.p[0], a2.p[1] - a1.p[1]];
                let t = (w[0] * v[1] - w[1] * v[0]) / (u[0] * v[1] - u[1] * v[0]);
                let x = [a1.p[0] + t * u[0], a1.p[1] + t * u[1]];
                let far = |a: [f64; 2], b: [f64; 2]| if d2(x, a) > d2(x, b) { [a[0] - x[0], a[1] - x[1]] } else { [b[0] - x[0], b[1] - x[1]] };
                let (r1, r2) = (far(a1.p, b1.p), far(a2.p, b2.p));
                let ang = (r1[0] * r2[1] - r1[1] * r2[0]).abs().atan2(r1[0] * r2[0] + r1[1] * r2[1]).to_degrees();
                rows.push(("Angle", ang));
            }
            rows.push(("Min distance", min));
            rows.push(("Length", lu + lv));
            let relation = if parallel { "Parallel lines" } else if perpendicular { "Perpendicular lines" } else { "Line-Line" };
            (rows, Some(relation))
        }
        [MeasurePick::Line(a, b), MeasurePick::Circle { center, radius, .. }] => {
            let c = seg_dist(*center, a.p, b.p);
            let max = d2(*center, a.p).max(d2(*center, b.p)) + radius;
            (vec![("Min distance", (c - radius).max(0.0)), ("Max distance", max), ("Center distance", c), ("Length", d2(a.p, b.p) + circ(*radius))], Some("Line-Circle"))
        }
        [MeasurePick::Circle { center: c1, radius: r1, .. }, MeasurePick::Circle { center: c2, radius: r2, .. }] => {
            let c = d2(*c1, *c2);
            (vec![("Min distance", (c - r1 - r2).max(0.0)), ("Center distance", c), ("Length", circ(*r1) + circ(*r2))], Some("Circle-Circle"))
        }
        _ => (Vec::new(), None),
    }
}

/// The picks as the Measure panel's selection box lists them ("Line 1": live2 wake-1118), numbered per kind.
pub fn measure_pick_names(picks: &[MeasurePick]) -> Vec<String> {
    let mut count = [0usize; 3];
    picks.iter().map(|m| {
        let (k, name) = match m {
            MeasurePick::Point(_) => (0, "Point"),
            MeasurePick::Line(..) => (1, "Line"),
            MeasurePick::Circle { .. } => (2, "Circle"),
        };
        count[k] += 1;
        format!("{name} {}", count[k])
    }).collect()
}

impl SheetPick<'_> {
    /// What a Measure click at `c` picks: a snap point, else a straight edge, else a face-on circle.
    pub fn measure_pick(&self, c: [f64; 2]) -> Option<MeasurePick> {
        if let Some(p) = self.snap_at(c) {
            return Some(MeasurePick::Point(p));
        }
        if let Some(Pick::Line { a, b }) = self.line_pick() {
            return Some(MeasurePick::Line(a, b));
        }
        match self.circle_pick(c) {
            Some(Pick::Circle { view, center, radius, .. }) => Some(MeasurePick::Circle { view, center, radius }),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    /// Live measure-1119 on a 100 x 60 mm plate with a Ø6 hole (view coordinates in meters): each pick set's rows,
    /// values and relation.
    #[test]
    fn measure_rows_are_lives_for_each_pick_set() {
        use super::{MeasurePick, Measured, measure_rows};
        use crate::drawing_tools::PickedPoint;
        use noshape_model::drawing::ViewId;
        let pt = |x: f64, y: f64| PickedPoint { view: ViewId(1), p: [x * 1e-3, y * 1e-3], r: None };
        let point = |x, y| MeasurePick::Point(pt(x, y));
        let line = |a: (f64, f64), b: (f64, f64)| MeasurePick::Line(pt(a.0, a.1), pt(b.0, b.1));
        let hole = MeasurePick::Circle { view: ViewId(1), center: [50e-3, 30e-3], radius: 3e-3 };
        let shown = |picks: &[MeasurePick]| {
            let (rows, rel) = measure_rows(picks);
            (rows.into_iter().map(|(k, m)| match m {
                Measured::Length(m) => format!("{k}: {:.2}", m * 1e3),
                Measured::Angle(a) => format!("{k}: {a:.1} deg"),
            }).collect::<Vec<_>>(), rel)
        };
        let (top, bottom, left) = (line((0., 60.), (100., 60.)), line((0., 0.), (100., 0.)), line((0., 0.), (0., 60.)));
        assert_eq!(shown(&[point(0., 60.)]), (vec![], None));
        assert_eq!(shown(std::slice::from_ref(&hole)), (vec!["Diameter: 6.00".into(), "Length: 18.85".into()], Some("Circle")));
        assert_eq!(shown(&[point(0., 60.), point(100., 60.)]), (vec!["Min distance: 100.00".into(), "dx: 100.00".into(), "dy: 0.00".into()], Some("Point-Point")));
        assert_eq!(shown(&[point(0., 60.), bottom.clone()]), (vec!["Min distance: 60.00".into(), "Length: 100.00".into()], Some("Point-Line")));
        assert_eq!(shown(&[bottom.clone(), point(0., 60.)]).1, Some("Point-Line"), "kinds in a fixed order, not the pick order");
        assert_eq!(shown(&[top.clone(), bottom]), (vec!["Parallel distance: 60.00".into(), "Min distance: 60.00".into(), "Length: 200.00".into()], Some("Parallel lines")));
        assert_eq!(shown(&[top, left]), (vec!["Min distance: 0.00".into(), "Length: 160.00".into()], Some("Perpendicular lines")));
        let (rows, rel) = shown(&[point(80., 60.), hole.clone()]);
        assert_eq!((rows[0].as_str(), rows[3].as_str(), rel), ("Min distance: 39.43", "Length: 18.85", Some("Point-Circle")));
        // Round 2: the hole 30 mm above the bottom edge; a second hole 30 mm to its right.
        let hole2 = MeasurePick::Circle { view: ViewId(1), center: [80e-3, 30e-3], radius: 3e-3 };
        let bottom = line((0., 0.), (100., 0.));
        assert_eq!(shown(&[bottom.clone(), hole.clone()]), (vec!["Min distance: 27.00".into(), "Max distance: 61.31".into(), "Center distance: 30.00".into(), "Length: 118.85".into()], Some("Line-Circle")));
        assert_eq!(shown(&[hole, hole2]), (vec!["Min distance: 24.00".into(), "Center distance: 30.00".into(), "Length: 37.70".into()], Some("Circle-Circle")));
        let (c, s) = (30f64.to_radians().cos(), 30f64.to_radians().sin());
        let iso = [line((0., 0.), (-48.99 * c, -48.99 * s)), line((0., 0.), (81.65 * c, -81.65 * s))];
        assert_eq!(shown(&iso), (vec!["Angle: 120.0 deg".into(), "Min distance: 0.00".into(), "Length: 130.64".into()], Some("Line-Line")));
        assert_eq!(shown(&[point(0., 60.), point(100., 60.), bottom]), (vec!["Length: 100.00".into()], None), "three picks: the Length only");
    }
}
