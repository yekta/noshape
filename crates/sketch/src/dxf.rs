//! Minimal DXF reader for "Insert DXF or DWG": lines, circles, arcs,
//! (lightweight) polylines with bulges, points, ellipses and splines from
//! the ENTITIES section, plus the drawing units ($INSUNITS).

use crate::geom::Curve2;
use crate::ids::EntityId;
use crate::nurbs::Nurbs;
use crate::sketch::{Result, Sketch, SketchError};
use glam::DVec2;
use std::f64::consts::TAU;

/// One drawing entity in file units.
#[derive(Clone, Debug, PartialEq)]
pub enum DxfEntity {
    Point(DVec2),
    Curve(Curve2),
}

/// A parsed DXF drawing.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DxfDrawing {
    pub entities: Vec<DxfEntity>,
    /// Meters per drawing unit from `$INSUNITS`, if the file states it.
    pub unit_scale: Option<f64>,
}

/// Meters per unit of a DXF `$INSUNITS` code.
fn insunits(code: i64) -> Option<f64> {
    Some(match code {
        1 => 0.0254,
        2 => 0.3048,
        4 => 0.001,
        5 => 0.01,
        6 => 1.0,
        8 => 0.0254e-6,
        9 => 0.0254e-3,
        10 => 0.9144,
        14 => 0.1,
        _ => return None,
    })
}

/// Group code / value pairs of a DXF text file.
fn pairs(text: &str) -> Vec<(i32, String)> {
    let lines: Vec<&str> = text.lines().collect();
    lines.chunks(2).filter(|c| c.len() == 2).filter_map(|c| Some((c[0].trim().parse::<i32>().ok()?, c[1].trim().to_string()))).collect()
}

/// Arc through two polyline vertices with a DXF bulge (tan of a quarter of the sweep).
fn bulge_arc(a: DVec2, b: DVec2, bulge: f64) -> Curve2 {
    let chord = b - a;
    let sweep = 4.0 * bulge.atan();
    let r = chord.length() / (2.0 * (sweep / 2.0).sin().abs());
    let mid = (a + b) * 0.5;
    let h = r * (sweep / 2.0).cos();
    let n = chord.normalize_or_zero().perp();
    let center = mid + n * h * sweep.signum();
    let (sa, ea) = ((a - center).to_angle(), (b - center).to_angle());
    // DXF arcs run counter-clockwise for positive bulges.
    let (s, e) = if bulge > 0.0 { (sa, ea) } else { (ea, sa) };
    let mut e = e;
    while e <= s {
        e += TAU;
    }
    Curve2::Arc { center, radius: r, start_angle: s, end_angle: e }
}

fn poly_curves(pts: &[(DVec2, f64)], closed: bool, out: &mut Vec<DxfEntity>) {
    let n = pts.len();
    let segs = if closed { n } else { n.saturating_sub(1) };
    for i in 0..segs {
        let (a, bulge) = pts[i];
        let b = pts[(i + 1) % n].0;
        if (b - a).length() < 1e-12 {
            continue;
        }
        out.push(DxfEntity::Curve(if bulge.abs() > 1e-12 { bulge_arc(a, b, bulge) } else { Curve2::Line { a, b } }));
    }
}

/// Parses the ENTITIES section of an ASCII DXF file.
pub fn parse_dxf(text: &str) -> Result<DxfDrawing> {
    let p = pairs(text);
    if p.is_empty() {
        return Err(SketchError::OperationFailed("not a DXF file".into()));
    }
    let mut out = DxfDrawing::default();
    // header units
    for (i, (c, v)) in p.iter().enumerate() {
        if *c == 9 && v == "$INSUNITS"
            && let Some((70, code)) = p.get(i + 1).map(|x| (x.0, x.1.clone()))
        {
            out.unit_scale = code.parse::<i64>().ok().and_then(insunits);
        }
    }
    let start = p.iter().position(|(c, v)| *c == 2 && v == "ENTITIES").ok_or_else(|| SketchError::OperationFailed("the DXF file has no ENTITIES section".into()))?;
    let mut i = start + 1;
    // Polyline (old style) being collected.
    let mut poly: Option<(Vec<(DVec2, f64)>, bool)> = None;
    while i < p.len() {
        let (c, v) = &p[i];
        if *c != 0 {
            i += 1;
            continue;
        }
        if v == "ENDSEC" {
            break;
        }
        let kind = v.clone();
        let mut j = i + 1;
        let mut f: Vec<(i32, String)> = Vec::new();
        while j < p.len() && p[j].0 != 0 {
            f.push(p[j].clone());
            j += 1;
        }
        let num = |code: i32| f.iter().find(|x| x.0 == code).and_then(|x| x.1.parse::<f64>().ok());
        let nums = |code: i32| f.iter().filter(|x| x.0 == code).filter_map(|x| x.1.parse::<f64>().ok()).collect::<Vec<f64>>();
        match kind.as_str() {
            "LINE" => {
                if let (Some(x0), Some(y0), Some(x1), Some(y1)) = (num(10), num(20), num(11), num(21)) {
                    out.entities.push(DxfEntity::Curve(Curve2::Line { a: DVec2::new(x0, y0), b: DVec2::new(x1, y1) }));
                }
            }
            "CIRCLE" => {
                if let (Some(x), Some(y), Some(r)) = (num(10), num(20), num(40)) {
                    out.entities.push(DxfEntity::Curve(Curve2::Circle { center: DVec2::new(x, y), radius: r }));
                }
            }
            "ARC" => {
                if let (Some(x), Some(y), Some(r), Some(a0), Some(a1)) = (num(10), num(20), num(40), num(50), num(51)) {
                    let (s, mut e) = (a0.to_radians(), a1.to_radians());
                    while e <= s {
                        e += TAU;
                    }
                    out.entities.push(DxfEntity::Curve(Curve2::Arc { center: DVec2::new(x, y), radius: r, start_angle: s, end_angle: e }));
                }
            }
            "POINT" => {
                if let (Some(x), Some(y)) = (num(10), num(20)) {
                    out.entities.push(DxfEntity::Point(DVec2::new(x, y)));
                }
            }
            "LWPOLYLINE" => {
                let closed = num(70).is_some_and(|fl| (fl as i64) & 1 == 1);
                let mut pts: Vec<(DVec2, f64)> = Vec::new();
                let mut x = None;
                for (code, val) in &f {
                    let Ok(n) = val.parse::<f64>() else { continue };
                    match code {
                        10 => x = Some(n),
                        20 => {
                            if let Some(xx) = x.take() {
                                pts.push((DVec2::new(xx, n), 0.0));
                            }
                        }
                        42 => {
                            if let Some(last) = pts.last_mut() {
                                last.1 = n;
                            }
                        }
                        _ => {}
                    }
                }
                poly_curves(&pts, closed, &mut out.entities);
            }
            "POLYLINE" => poly = Some((Vec::new(), num(70).is_some_and(|fl| (fl as i64) & 1 == 1))),
            "VERTEX" => {
                if let (Some((pts, _)), Some(x), Some(y)) = (poly.as_mut(), num(10), num(20)) {
                    pts.push((DVec2::new(x, y), num(42).unwrap_or(0.0)));
                }
            }
            "SEQEND" => {
                if let Some((pts, closed)) = poly.take() {
                    poly_curves(&pts, closed, &mut out.entities);
                }
            }
            "ELLIPSE" => {
                if let (Some(x), Some(y), Some(mx), Some(my), Some(ratio)) = (num(10), num(20), num(11), num(21), num(40)) {
                    let center = DVec2::new(x, y);
                    let major = DVec2::new(mx, my);
                    let minor = major.length() * ratio;
                    let (t0, t1) = (num(41).unwrap_or(0.0), num(42).unwrap_or(TAU));
                    if (t1 - t0 - TAU).abs() < 1e-9 || (t1 - t0).abs() < 1e-12 {
                        out.entities.push(DxfEntity::Curve(Curve2::Ellipse { center, major, minor }));
                    } else {
                        let mut e = t1;
                        while e <= t0 {
                            e += TAU;
                        }
                        out.entities.push(DxfEntity::Curve(Curve2::EllipticalArc { center, major, minor, start_param: t0, end_param: e }));
                    }
                }
            }
            "SPLINE" => {
                let degree = num(71).unwrap_or(3.0) as usize;
                let (xs, ys) = (nums(10), nums(20));
                let knots = nums(40);
                let weights = nums(41);
                let ctrl: Vec<DVec2> = xs.iter().zip(&ys).map(|(x, y)| DVec2::new(*x, *y)).collect();
                if ctrl.len() > degree && knots.len() == ctrl.len() + degree + 1 {
                    let w = (weights.len() == ctrl.len()).then_some(weights);
                    let n = Nurbs { degree, knots, ctrl, weights: w };
                    if n.is_valid() {
                        out.entities.push(DxfEntity::Curve(Curve2::Nurbs(n)));
                    }
                } else {
                    // fit points only: interpolate
                    let fx = nums(11);
                    let fy = nums(21);
                    let fit: Vec<DVec2> = fx.iter().zip(&fy).map(|(x, y)| DVec2::new(*x, *y)).collect();
                    if let Some(n) = Nurbs::interpolate(&fit) {
                        out.entities.push(DxfEntity::Curve(Curve2::Nurbs(n)));
                    }
                }
            }
            _ => {}
        }
        i = j;
    }
    if out.entities.is_empty() {
        return Err(SketchError::OperationFailed("the DXF file contains no supported 2D geometry".into()));
    }
    Ok(out)
}

impl Sketch {
    /// Adds the drawing's entities (scaled to meters by `scale` and moved by
    /// `offset`). Curve ends that meet share one point. Returns the new
    /// curves and points.
    pub fn insert_drawing(&mut self, drawing: &DxfDrawing, scale: f64, offset: DVec2) -> Vec<EntityId> {
        let map = |p: DVec2| p * scale + offset;
        let mut created = Vec::new();
        for e in &drawing.entities {
            match e {
                DxfEntity::Point(p) => created.push(self.add_point(map(*p))),
                DxfEntity::Curve(c) => {
                    let c2 = match c {
                        Curve2::Line { a, b } => Curve2::Line { a: map(*a), b: map(*b) },
                        Curve2::Circle { center, radius } => Curve2::Circle { center: map(*center), radius: radius * scale },
                        Curve2::Arc { center, radius, start_angle, end_angle } => Curve2::Arc { center: map(*center), radius: radius * scale, start_angle: *start_angle, end_angle: *end_angle },
                        Curve2::Ellipse { center, major, minor } => Curve2::Ellipse { center: map(*center), major: *major * scale, minor: minor * scale },
                        Curve2::EllipticalArc { center, major, minor, start_param, end_param } => Curve2::EllipticalArc { center: map(*center), major: *major * scale, minor: minor * scale, start_param: *start_param, end_param: *end_param },
                        Curve2::Nurbs(n) => Curve2::Nurbs(Nurbs { ctrl: n.ctrl.iter().map(|p| map(*p)).collect(), ..n.clone() }),
                    };
                    if let Ok(r) = self.use_external(&c2)
                        && let Some(&id) = r.created.last()
                    {
                        // Inserted geometry is regular (not fixed) sketch geometry.
                        for e in r.created.iter() {
                            let _ = self.set_external(*e, false);
                        }
                        created.push(id);
                    }
                }
            }
        }
        // Join coincident curve ends.
        let tol = 1e-9 * self.bbox().size().max_element().max(1e-3);
        let ends: Vec<EntityId> = created.iter().filter_map(|c| self.endpoints(*c)).flat_map(|(a, b)| [a, b]).collect();
        let mut merged: Vec<EntityId> = Vec::new();
        for (i, a) in ends.iter().enumerate() {
            if merged.contains(a) || self.entity(*a).is_none() {
                continue;
            }
            for b in ends.iter().skip(i + 1) {
                if merged.contains(b) || a == b || self.entity(*b).is_none() {
                    continue;
                }
                if let (Some(pa), Some(pb)) = (self.point(*a), self.point(*b))
                    && (pa - pb).length() <= tol
                    && self.merge_points(*b, *a).is_ok()
                {
                    merged.push(*b);
                }
            }
        }
        created
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "0\nSECTION\n2\nHEADER\n9\n$INSUNITS\n70\n4\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n0\nLINE\n8\n0\n10\n0\n20\n0\n11\n10\n21\n0\n0\nLINE\n8\n0\n10\n10\n20\n0\n11\n10\n21\n5\n0\nCIRCLE\n8\n0\n10\n5\n20\n5\n40\n2\n0\nARC\n10\n0\n20\n0\n40\n3\n50\n0\n51\n90\n0\nLWPOLYLINE\n90\n3\n70\n1\n10\n20\n20\n0\n10\n30\n20\n0\n42\n1\n10\n30\n20\n10\n0\nENDSEC\n0\nEOF\n";

    #[test]
    fn parses_entities_and_units() {
        let d = parse_dxf(SAMPLE).unwrap();
        assert_eq!(d.unit_scale, Some(0.001));
        // 2 lines, circle, arc, closed polyline of 3 segments (one bulged)
        assert_eq!(d.entities.len(), 7);
        assert!(d.entities.iter().any(|e| matches!(e, DxfEntity::Curve(Curve2::Arc { radius, .. }) if (radius - 3.0).abs() < 1e-12)));
        let mut sk = Sketch::new();
        let ids = sk.insert_drawing(&d, 0.001, DVec2::ZERO);
        assert_eq!(ids.len(), 7);
        // the two lines share their corner point
        let lines: Vec<EntityId> = ids.iter().copied().filter(|e| sk.kind(*e) == Some(crate::entities::EntityKind::Line)).collect();
        let (_, e0) = sk.endpoints(lines[0]).unwrap();
        let (s1, _) = sk.endpoints(lines[1]).unwrap();
        assert_eq!(e0, s1);
        assert!(sk.solve().is_solved());
        assert!(!sk.entity(lines[0]).unwrap().external);
    }

    #[test]
    fn rejects_non_dxf() {
        assert!(parse_dxf("hello").is_err());
    }
}
