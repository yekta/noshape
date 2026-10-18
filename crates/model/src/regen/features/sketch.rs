//! Sketch feature.

use crate::params::ParamValue;
use crate::regen::context::SketchResult;
use crate::regen::eval::{EvalCtx, FResult, err};
use noshape_sketch::{ConstraintId, ConstraintKind};
use noshape_units::Dim;
use std::collections::BTreeMap;
use std::sync::Arc;

pub fn eval(ev: &mut EvalCtx) -> FResult<()> {
    let Some(plane_ref) = ev.query("sketchPlane").first() else {
        return err("Select a sketch plane");
    };
    let Some(frame) = ev.resolve_plane(plane_ref) else {
        return err("The sketch plane could not be found");
    };
    let data = match ev.params.get("sketch") {
        Some(ParamValue::Sketch(d)) => d.clone(),
        _ => Box::default(),
    };
    let mut sk = data.sketch.clone();
    // Re-project used ("Use") 3D edges so they follow upstream changes.
    let mut missing = 0;
    for (eid, r) in &data.external_refs {
        // Projected model vertices (snapped to while sketching).
        if matches!(r, crate::reference::Reference::Vertex(_)) {
            match ev.resolve_vertex_point(r) {
                Some(p) => {
                    let _ = sk.set_point(noshape_sketch::EntityId(*eid), frame.to_local(p));
                }
                None => missing += 1,
            }
            continue;
        }
        let edges = ev.resolve_edges(std::slice::from_ref(r));
        let Some(&(bi, ei)) = edges.first() else {
            missing += 1;
            continue;
        };
        let Ok(edge) = ev.edge_shape(bi, ei) else { continue };
        if let Some(c) = project_edge(&edge, &frame) {
            update_external(&mut sk, noshape_sketch::EntityId(*eid), &c);
        }
    }
    if missing > 0 {
        ev.warnings.push(format!("{missing} projected edge(s) no longer exist"));
    }
    // Re-section intersected faces so intersection curves follow the model.
    let mut lost = 0;
    for link in &data.intersections {
        let faces = ev.resolve_faces(std::slice::from_ref(&link.face));
        let curves = match faces.first() {
            Some(&(bi, fi)) => ev.face_shape(bi, fi).ok().and_then(|f| section_face(&f, &frame).ok()).unwrap_or_default(),
            None => Vec::new(),
        };
        lost += update_intersection(&mut sk, &link.entities, &curves);
    }
    if lost > 0 {
        ev.warnings.push(format!("{lost} intersection curve(s) could not be updated"));
    }
    // Expression-driven dimensions.
    let mut problems = Vec::new();
    for (cid, expr) in &data.dimension_exprs {
        let id = ConstraintId(*cid);
        let Some(c) = sk.constraint(id) else { continue };
        if !c.driving {
            continue;
        }
        let dim = match c.kind {
            ConstraintKind::Angle { .. } | ConstraintKind::ArcAngle { .. } => Dim::ANGLE,
            _ => Dim::LENGTH,
        };
        match ev.eval_text(&expr.text, dim) {
            Ok(q) => {
                let _ = sk.set_dimension_value(id, q.value);
            }
            Err(e) => problems.push(e.message),
        }
    }
    let report = sk.solve();
    // Text glyph outlines follow their (solved) text boxes.
    data.add_text_outlines(&mut sk);
    let regions = sk.regions();
    let wires = sk.wires();
    let mut dimension_text = BTreeMap::new();
    for c in sk.constraints() {
        if let Some(v) = c.kind.value() {
            let q = match c.kind {
                ConstraintKind::Angle { .. } | ConstraintKind::ArcAngle { .. } => noshape_units::Quantity::new(v, Dim::ANGLE),
                _ => noshape_units::Quantity::new(v, Dim::LENGTH),
            };
            let mut text = noshape_units::format_quantity(q, ev.units);
            if matches!(c.kind, ConstraintKind::Diameter { .. }) {
                text = format!("Ø{text}");
            } else if matches!(c.kind, ConstraintKind::Radius { .. }) {
                text = format!("R{text}");
            }
            if !c.driving {
                text = format!("({text})");
            }
            dimension_text.insert(c.id.0, text);
        }
    }
    if let noshape_sketch::SolveStatus::Overconstrained { .. } = report.status {
        ev.warnings.push("Sketch has conflicting constraints".into());
    }
    if !problems.is_empty() {
        ev.warnings.push(problems.join("; "));
    }
    let name = ev.name.to_string();
    ev.ctx.sketches.retain(|s| s.feature != ev.feature);
    ev.ctx.sketches.push(Arc::new(SketchResult { feature: ev.feature, name, frame, sketch: sk, report, regions, wires, dimension_text }));
    Ok(())
}

/// Project a 3D edge onto a sketch plane (orthogonal projection).
pub fn project_edge(edge: &noshape_kernel::Shape, frame: &crate::display::PlaneFrame) -> Option<noshape_sketch::Curve2> {
    use noshape_kernel as k;
    use noshape_sketch::Curve2;
    let info = edge.edge_info().ok()?;
    match info.curve {
        k::Curve::Line { .. } => Some(Curve2::Line { a: frame.to_local(info.start), b: frame.to_local(info.end) }),
        k::Curve::Circle { center, axis, radius, .. } if axis.normalize().dot(frame.normal).abs() > 1.0 - 1e-9 => {
            let c = frame.to_local(center);
            if info.closed {
                return Some(Curve2::Circle { center: c, radius });
            }
            let ang = |p: glam::DVec3| {
                let d = frame.to_local(p) - c;
                d.y.atan2(d.x)
            };
            // The edge runs CCW around its axis; seen from the sketch normal it is
            // CCW when the axis agrees with the normal.
            let (mut a0, mut a1) = (ang(info.start), ang(info.end));
            if axis.dot(frame.normal) < 0.0 {
                std::mem::swap(&mut a0, &mut a1);
            }
            let mut e = a1;
            while e <= a0 {
                e += std::f64::consts::TAU;
            }
            Some(Curve2::Arc { center: c, radius, start_angle: a0, end_angle: e })
        }
        _ => None,
    }
}

/// Update an external entity's geometry in place from a projected curve.
pub fn update_external(sk: &mut noshape_sketch::Sketch, id: noshape_sketch::EntityId, c: &noshape_sketch::Curve2) {
    use noshape_sketch::{Curve2, Geometry};
    let Some(e) = sk.entity(id).cloned() else { return };
    match (&e.geometry, c) {
        (Geometry::Line { start, end }, Curve2::Line { a, b }) => {
            let _ = sk.set_point(*start, *a);
            let _ = sk.set_point(*end, *b);
        }
        (Geometry::Circle { center, .. }, Curve2::Circle { center: c2, radius }) => {
            let _ = sk.set_point(*center, *c2);
            if let Some(ent) = sk.entity_mut(id)
                && let Geometry::Circle { radius: r, .. } = &mut ent.geometry
            {
                *r = *radius;
            }
        }
        (Geometry::Arc { center, start, end }, Curve2::Arc { center: c2, radius, start_angle, end_angle }) => {
            let _ = sk.set_point(*center, *c2);
            let _ = sk.set_point(*start, *c2 + glam::DVec2::from_angle(*start_angle) * *radius);
            let _ = sk.set_point(*end, *c2 + glam::DVec2::from_angle(*end_angle) * *radius);
        }
        (Geometry::BSpline { control: points, .. }, _) => {
            if let Curve2::Nurbs(n) = c
                && n.ctrl.len() == points.len()
            {
                for (p, q) in points.iter().zip(&n.ctrl) {
                    let _ = sk.set_point(*p, *q);
                }
            }
        }
        _ => {}
    }
}

/// Number of points of a sampled (free-form) intersection curve.
pub const SECTION_SAMPLES: usize = 13;

/// A section curve: exact (line/circle/arc in the sketch plane) or sampled
/// (free-form, interpolated by a fit spline through [`SECTION_SAMPLES`] points).
#[derive(Clone, Debug, PartialEq)]
pub enum SectionCurve {
    Exact(noshape_sketch::Curve2),
    Sampled(Vec<glam::DVec2>),
}

impl SectionCurve {
    fn mid(&self) -> glam::DVec2 {
        match self {
            SectionCurve::Exact(c) => {
                let (a, b) = c.domain();
                c.point((a + b) * 0.5)
            }
            SectionCurve::Sampled(p) => p[p.len() / 2],
        }
    }
}

/// Intersection of a model face with the sketch plane, as sketch curves.
pub fn section_face(face: &noshape_kernel::Shape, frame: &crate::display::PlaneFrame) -> Result<Vec<SectionCurve>, String> {
    use glam::{DVec2, DVec3};
    let plane = noshape_kernel::Plane::new(frame.origin, frame.normal, frame.x);
    let sec = noshape_kernel::section_plane(face, &plane).map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for e in sec.edges() {
        if let Some(c) = project_edge(&e, frame) {
            out.push(SectionCurve::Exact(c));
            continue;
        }
        let Ok(t) = noshape_kernel::tessellate(&e, 1e-5, 0.05, false) else { continue };
        if t.edges.is_empty() {
            continue;
        }
        let pl: Vec<DVec2> = t.edge_polyline(0).iter().map(|v| frame.to_local(DVec3::new(v[0] as f64, v[1] as f64, v[2] as f64))).collect();
        if let Some(pts) = resample(&pl, SECTION_SAMPLES) {
            out.push(SectionCurve::Sampled(pts));
        }
    }
    Ok(out)
}

/// `n` points equally spaced by arc length along a polyline.
fn resample(pl: &[glam::DVec2], n: usize) -> Option<Vec<glam::DVec2>> {
    if pl.len() < 2 || n < 2 {
        return None;
    }
    let mut acc = vec![0.0];
    for w in pl.windows(2) {
        acc.push(acc.last().unwrap() + (w[1] - w[0]).length());
    }
    let total = *acc.last().unwrap();
    if total < 1e-12 {
        return None;
    }
    let mut out = Vec::with_capacity(n);
    let mut j = 0;
    for i in 0..n {
        let s = total * i as f64 / (n - 1) as f64;
        while j + 2 < acc.len() && acc[j + 1] < s {
            j += 1;
        }
        let seg = (acc[j + 1] - acc[j]).max(1e-300);
        out.push(pl[j].lerp(pl[j + 1], ((s - acc[j]) / seg).clamp(0.0, 1.0)));
    }
    Some(out)
}

/// Adds section curves as fixed external sketch geometry. Returns the
/// created curve entities (in section order).
pub fn add_section_curves(sk: &mut noshape_sketch::Sketch, curves: &[SectionCurve]) -> Vec<noshape_sketch::EntityId> {
    let mut out = Vec::new();
    for c in curves {
        match c {
            SectionCurve::Exact(c) => {
                if let Ok(r) = sk.use_external(c)
                    && let Some(&id) = r.created.last()
                {
                    out.push(id);
                }
            }
            SectionCurve::Sampled(pts) => {
                if let Ok(id) = sk.add_fit_spline(pts) {
                    let _ = sk.set_external(id, true);
                    out.push(id);
                }
            }
        }
    }
    out
}

/// Updates intersection entities from freshly computed section curves,
/// matching each entity to the nearest compatible new curve. Returns the
/// number of entities that found no match.
pub fn update_intersection(sk: &mut noshape_sketch::Sketch, entities: &[u32], curves: &[SectionCurve]) -> usize {
    use noshape_sketch::{Curve2, EntityId, EntityKind};
    let mut used = vec![false; curves.len()];
    let mut lost = 0;
    for &e in entities {
        let id = EntityId(e);
        let (Some(kind), Some(old)) = (sk.kind(id), sk.curve(id)) else { continue };
        let (a, b) = old.domain();
        let old_mid = old.point((a + b) * 0.5);
        let compatible = |c: &SectionCurve| match (kind, c) {
            (EntityKind::Line, SectionCurve::Exact(Curve2::Line { .. })) => true,
            (EntityKind::Circle, SectionCurve::Exact(Curve2::Circle { .. })) => true,
            (EntityKind::Arc, SectionCurve::Exact(Curve2::Arc { .. })) => true,
            (EntityKind::FitSpline, SectionCurve::Sampled(p)) => sk.entity(id).is_some_and(|en| en.point_refs().len() == p.len()),
            _ => false,
        };
        let best = curves
            .iter()
            .enumerate()
            .filter(|(i, c)| !used[*i] && compatible(c))
            .min_by(|x, y| (x.1.mid() - old_mid).length().total_cmp(&(y.1.mid() - old_mid).length()));
        let Some((i, c)) = best else {
            lost += 1;
            continue;
        };
        used[i] = true;
        match c {
            SectionCurve::Exact(c) => update_external(sk, id, c),
            SectionCurve::Sampled(pts) => {
                let refs = sk.entity(id).map(|en| en.point_refs()).unwrap_or_default();
                for (p, q) in refs.iter().zip(pts) {
                    let _ = sk.set_point(*p, *q);
                }
            }
        }
    }
    lost
}
