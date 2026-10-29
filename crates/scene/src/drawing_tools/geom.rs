//! Sheet-plane vector helpers and dimension placement math of the drawing
//! tools (from `crates/app/src/drawing_tools.rs`).

use noshape_model::drawing::{AnnotationKind, DimOrientation};

pub fn sub(a: [f64; 2], b: [f64; 2]) -> [f64; 2] {
    [a[0] - b[0], a[1] - b[1]]
}
pub fn dot(a: [f64; 2], b: [f64; 2]) -> f64 {
    a[0] * b[0] + a[1] * b[1]
}
pub fn len(a: [f64; 2]) -> f64 {
    dot(a, a).sqrt()
}
pub fn add2(a: [f64; 2], b: [f64; 2]) -> [f64; 2] {
    [a[0] + b[0], a[1] + b[1]]
}
pub fn mul(a: [f64; 2], k: f64) -> [f64; 2] {
    [a[0] * k, a[1] * k]
}
pub fn norm(a: [f64; 2]) -> [f64; 2] {
    let l = len(a).max(1e-12);
    [a[0] / l, a[1] / l]
}

/// Dimension offset that puts the dimension line through the cursor
/// (mirrors `SheetGeometry::linear_dimension`).
/// Whether `c` lies in the angle swept counterclockwise from leg point `a`
/// to leg point `b` around `v`.
pub fn in_sweep(v: [f64; 2], a: [f64; 2], b: [f64; 2], c: [f64; 2]) -> bool {
    let ang = |p: [f64; 2]| (p[1] - v[1]).atan2(p[0] - v[0]);
    let norm = |x: f64| x.rem_euclid(std::f64::consts::TAU);
    norm(ang(c) - ang(a)) <= norm(ang(b) - ang(a))
}

/// Line to line angular: make the dimension measure the sector of the two
/// extended lines that sheet point `at` is in, as live (behaviours D11):
/// between the edges the inner angle, beside them 180° minus it (a leg
/// turned through the vertex), opposite them the reflex 360° minus it.
/// True when legs `a` and `b` swapped (their references must too).
pub fn angular_to_sector(kind: &mut AnnotationKind, pl: &noshape_model::drawing::build::ViewPlacement, at: [f64; 2]) -> bool {
    let AnnotationKind::AngularDimension { vertex, a, b, flip_a, flip_b, three_point: false, .. } = kind else { return false };
    let v = pl.to_sheet(*vertex);
    let ang = |p: [f64; 2]| {
        let s = pl.to_sheet(p);
        (s[1] - v[1]).atan2(s[0] - v[0])
    };
    let mirror = |p: [f64; 2]| [2.0 * vertex[0] - p[0], 2.0 * vertex[1] - p[1]];
    // The four rays: (stored leg b?, mirrored?), in CCW order from at's angle.
    let tau = std::f64::consts::TAU;
    let c = (at[1] - v[1]).atan2(at[0] - v[0]);
    let rays = [(false, false, ang(*a)), (true, false, ang(*b)), (false, true, ang(mirror(*a))), (true, true, ang(mirror(*b)))];
    let since = |t: f64| (c - t).rem_euclid(tau);
    let until = |t: f64| (t - c).rem_euclid(tau);
    // The sector's first leg is the ray just clockwise of `at`, its second
    // the ray just counterclockwise.
    let first = rays.iter().min_by(|x, y| since(x.2).total_cmp(&since(y.2))).copied().expect("four rays");
    let second = rays.iter().min_by(|x, y| until(x.2).total_cmp(&until(y.2))).copied().expect("four rays");
    if first.0 == second.0 {
        return false;
    }
    // Opposite the edges: the reflex angle, from the second edge round to
    // the first (never the vertically opposite angle).
    let (first, second) = if first.1 && second.1 { ((second.0, false, 0.0), (first.0, false, 0.0)) } else { (first, second) };
    let swap = first.0;
    if swap {
        std::mem::swap(a, b);
    }
    (*flip_a, *flip_b) = (first.1, second.1);
    swap
}

pub fn dim_offset(p: [f64; 2], q: [f64; 2], o: DimOrientation, c: [f64; 2]) -> f64 {
    let nrm = match o {
        DimOrientation::Horizontal => [0.0, 1.0],
        DimOrientation::Vertical => [1.0, 0.0],
        DimOrientation::Aligned => {
            let d = norm(sub(q, p));
            [-d[1], d[0]]
        }
    };
    dot(c, nrm) - dot(p, nrm).max(dot(q, nrm))
}

/// Orientation of a smart linear dimension from the cursor position.
pub fn auto_orientation(p: [f64; 2], q: [f64; 2], c: [f64; 2]) -> DimOrientation {
    let (x0, x1) = (p[0].min(q[0]), p[0].max(q[0]));
    let (y0, y1) = (p[1].min(q[1]), p[1].max(q[1]));
    let ex = if c[0] < x0 { x0 - c[0] } else if c[0] > x1 { c[0] - x1 } else { 0.0 };
    let ey = if c[1] < y0 { y0 - c[1] } else if c[1] > y1 { c[1] - y1 } else { 0.0 };
    if (x1 - x0) < 1e-6 {
        return DimOrientation::Vertical;
    }
    if (y1 - y0) < 1e-6 {
        return DimOrientation::Horizontal;
    }
    if ex > ey && ex > 0.0 {
        DimOrientation::Vertical
    } else if ey > 0.0 {
        DimOrientation::Horizontal
    } else {
        DimOrientation::Aligned
    }
}

/// Intersection of two infinite 2D lines.
pub fn intersect(a0: [f64; 2], a1: [f64; 2], b0: [f64; 2], b1: [f64; 2]) -> Option<[f64; 2]> {
    let r = sub(a1, a0);
    let s = sub(b1, b0);
    let den = r[0] * s[1] - r[1] * s[0];
    if den.abs() < 1e-14 * (len(r) * len(s)).max(1e-30) {
        return None;
    }
    let t = ((b0[0] - a0[0]) * s[1] - (b0[1] - a0[1]) * s[0]) / den;
    Some([a0[0] + r[0] * t, a0[1] + r[1] * t])
}

#[cfg(test)]
mod angular_tests {
    use noshape_model::drawing::build::ViewPlacement;
    use noshape_model::drawing::{AnnotationKind, ViewId};

    /// Edges at 0° and 60° meeting at the origin: the pointer's sector
    /// gives 60° (between them), 120° (beside) or 300° (opposite: reflex).
    #[test]
    fn line_to_line_angular_measures_the_pointer_sector() {
        let pl = ViewPlacement::new(ViewId(1), [0.0, 0.0], [100.0, 100.0], 1.0);
        let r = 0.01;
        let (c60, s60) = (60f64.to_radians().cos(), 60f64.to_radians().sin());
        let v = pl.to_sheet([0.0, 0.0]);
        let at = |deg: f64| [v[0] + 20.0 * deg.to_radians().cos(), v[1] + 20.0 * deg.to_radians().sin()];
        for (deg, want) in [(30.0, 60.0), (120.0, 120.0), (210.0, 300.0), (300.0, 120.0), (30.0, 60.0)] {
            let mut k = AnnotationKind::AngularDimension { view: ViewId(1), vertex: [0.0, 0.0], a: [r, 0.0], b: [r * c60, r * s60], radius: 20.0, three_point: false, flip_a: false, flip_b: false, text_angle: None };
            super::angular_to_sector(&mut k, &pl, at(deg));
            let (o, p, q) = k.angular_legs().unwrap();
            let (o, p, q) = (pl.to_sheet(o), pl.to_sheet(p), pl.to_sheet(q));
            let ang = |x: [f64; 2]| (x[1] - o[1]).atan2(x[0] - o[0]);
            let sweep = (ang(q) - ang(p)).rem_euclid(std::f64::consts::TAU).to_degrees();
            assert!((sweep - want).abs() < 1e-6, "at {deg}°: {sweep}");
        }
    }
}
