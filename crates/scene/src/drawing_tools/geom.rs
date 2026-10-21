//! Sheet-plane vector helpers and dimension placement math of the drawing
//! tools (from `crates/app/src/drawing_tools.rs`).

use noshape_model::drawing::DimOrientation;

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
