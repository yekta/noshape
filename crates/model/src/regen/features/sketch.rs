//! Sketch feature.

use crate::params::ParamValue;
use crate::regen::context::SketchResult;
use crate::regen::eval::{EvalCtx, FResult, err};
use noshape_sketch::{ConstraintId, ConstraintKind};
use noshape_units::Dim;
use std::collections::BTreeMap;
use std::sync::Arc;

pub use noshape_model_data::sketch_feature::CONFIGURED_DIMENSION_PREFIX;

pub fn eval(ev: &mut EvalCtx) -> FResult<()> {
    let Some(plane_ref) = ev.query("sketchPlane").first() else {
        return crate::regen::eval::err_at(&["sketchPlane"], "Select a sketch plane");
    };
    let Some(frame) = ev.resolve_plane(plane_ref) else {
        return err("The sketch plane could not be found");
    };
    let data = match ev.params.get("sketch") {
        Some(ParamValue::Sketch(d)) => d.clone(),
        _ => Box::default(),
    };
    let mut sk = data.sketch.clone();
    // Re-project used ("Use") 3D geometry so it follows upstream changes:
    // vertices, edges (exact or sampled projections, pierce points), plane
    // traces and silhouettes of curved faces.
    let mut missing = 0;
    let mut silhouettes: BTreeMap<String, Vec<noshape_sketch::Curve2>> = BTreeMap::new();
    for (eid, r) in &data.external_refs {
        let id = noshape_sketch::EntityId(*eid);
        // Projected model vertices (snapped to while sketching).
        if matches!(r, crate::reference::Reference::Vertex(_)) {
            match ev.resolve_vertex_point(r) {
                Some(p) => {
                    let _ = sk.set_point(id, frame.to_local(p));
                }
                None => missing += 1,
            }
            continue;
        }
        // Planes (and planar faces) appear as their trace line.
        let curved_face = matches!(r, crate::reference::Reference::Face(_)) && {
            let f = ev.resolve_faces(std::slice::from_ref(r));
            f.first().and_then(|&(bi, fi)| ev.face_shape(bi, fi).ok()).is_some_and(|f| !f.is_planar())
        };
        if !curved_face && let Some(pl) = (!matches!(r, crate::reference::Reference::Edge(_))).then(|| ev.resolve_plane(r)).flatten() {
            match plane_trace(&pl, &frame) {
                Some((p, d)) => update_trace(&mut sk, id, p, d),
                None => missing += 1,
            }
            continue;
        }
        if curved_face {
            let key = format!("{r:?}");
            if !silhouettes.contains_key(&key) {
                let f = ev.resolve_faces(std::slice::from_ref(r));
                let sil = f.first().and_then(|&(bi, fi)| ev.face_shape(bi, fi).ok()).map(|f| face_silhouettes(&f, &frame)).unwrap_or_default();
                silhouettes.insert(key.clone(), sil);
            }
            let cands: Vec<SectionCurve> = silhouettes[&key].iter().cloned().map(SectionCurve::Exact).collect();
            if cands.is_empty() {
                missing += 1;
            } else {
                missing += update_intersection(&mut sk, &[*eid], &cands);
            }
            continue;
        }
        // Entities of other sketches are used like model edges / vertices.
        let edge = if let crate::reference::Reference::SketchEntity { .. } = r {
            let Some(shape) = crate::regen::measure::resolve_shapes(&ev.ctx, r).into_iter().next() else {
                missing += 1;
                continue;
            };
            if let Ok(p) = shape.vertex_point() {
                let _ = sk.set_point(id, frame.to_local(p));
                continue;
            }
            shape
        } else {
            let edges = ev.resolve_edges(std::slice::from_ref(r));
            let Some(&(bi, ei)) = edges.first() else {
                missing += 1;
                continue;
            };
            let Ok(edge) = ev.edge_shape(bi, ei) else { continue };
            edge
        };
        if sk.kind(id) == Some(noshape_sketch::EntityKind::Point) {
            // Pierce point: where the edge crosses the sketch plane.
            match pierce_point(&edge, &frame) {
                Some(p) => {
                    let _ = sk.set_point(id, p);
                }
                None => missing += 1,
            }
            continue;
        }
        match project_edge_any(&edge, &frame) {
            Some(SectionCurve::Exact(c)) => update_external(&mut sk, id, &c),
            Some(sampled) => {
                update_intersection(&mut sk, &[*eid], &[sampled]);
            }
            None => {}
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
    // Expression-driven dimensions. The ones whose value changes are kept:
    // a single changed dimension solves like an edit in the sketch (only its
    // geometry moves, live SK5), not as a fresh solve of everything.
    let mut problems = Vec::new();
    let mut changed: Vec<ConstraintId> = Vec::new();
    // An unchanged value isn't set again: that would count as an edit and
    // drop the sketch's cached DOF analysis (re-done every drag frame).
    let mut set_value = |sk: &mut noshape_sketch::Sketch, id: ConstraintId, v: f64| {
        let old = sk.constraint(id).and_then(|c| c.kind.value());
        if old.is_some_and(|old| (old - v).abs() > 1e-12 * old.abs().max(1.0)) {
            changed.push(id);
        }
        if old != Some(v) {
            let _ = sk.set_dimension_value(id, v);
        }
    };
    for (cid, expr) in &data.dimension_exprs {
        let id = ConstraintId(*cid);
        let Some(c) = sk.constraint(id) else { continue };
        if !c.driving {
            continue;
        }
        let dim = match c.kind {
            ConstraintKind::Angle { .. } | ConstraintKind::ArcAngle { .. } => Dim::ANGLE,
            ConstraintKind::Rho { .. } => Dim::UNITLESS,
            _ => Dim::LENGTH,
        };
        // Ellipse axes are typed as diameters but stored as radii.
        let scale = c.kind.display_scale();
        match ev.eval_text(&expr.text, dim) {
            Ok(q) => set_value(&mut sk, id, q.value / scale),
            Err(e) => problems.push(e.message),
        }
    }
    // Configured dimensions ("Configure dimension"): the configuration's
    // value for this dimension overrides its expression.
    for (key, v) in ev.params.0.iter() {
        let (Some(cid), crate::params::ParamValue::Expr(expr)) = (key.strip_prefix(CONFIGURED_DIMENSION_PREFIX).and_then(|c| c.parse::<u32>().ok()), v) else { continue };
        let id = ConstraintId(cid);
        let Some(c) = sk.constraint(id) else { continue };
        let dim = match c.kind {
            ConstraintKind::Angle { .. } | ConstraintKind::ArcAngle { .. } => Dim::ANGLE,
            ConstraintKind::Rho { .. } => Dim::UNITLESS,
            _ => Dim::LENGTH,
        };
        // Ellipse axes are typed as diameters but stored as radii.
        let scale = c.kind.display_scale();
        match ev.eval_text(&expr.text, dim) {
            Ok(q) => set_value(&mut sk, id, q.value / scale),
            Err(e) => problems.push(e.message),
        }
    }
    // Text expressions (Convert to expression): evaluate, and size the box
    // to the evaluated text before solving.
    let mut data = data;
    if data.texts.iter().any(|t| t.expression) {
        for t in data.texts.iter_mut().filter(|t| t.expression) {
            match eval_text_expression(ev, &t.text) {
                Ok(v) if !v.trim().is_empty() => {
                    t.text = v;
                    let width = noshape_sketch::text::layout_text(&t.text, &t.font, t.bold, t.italic).width;
                    if let Some(rc) = t.ratio_constraint {
                        let _ = sk.set_length_ratio(ConstraintId(rc), width.max(1e-6));
                    }
                }
                Ok(_) => problems.push("The text expression is empty".into()),
                Err(e) => problems.push(e),
            }
        }
    }
    changed.sort();
    changed.dedup();
    let last_solved = sk.clone();
    let report = match changed.as_slice() {
        [one] => {
            let opts = sk.dimension_edit_options(*one);
            sk.solve_with(&opts)
        }
        // The DOF analysis (fully / under defined) depends on the
        // constraints, not on where a drag moved the geometry: the sketch's
        // cached one is reused while nothing else changed.
        _ if sk.analysis_current() => sk.solve_with(&noshape_sketch::SolveOptions { analyze: false, ..Default::default() }),
        _ => sk.solve(),
    };
    // Live (qa4 DIM-UNSOLVED-GEOM): a sketch that can't be solved keeps its
    // last solved shape, not the rest solved around the conflict.
    if matches!(report.status, noshape_sketch::SolveStatus::Overconstrained { .. }) {
        sk = last_solved;
    }
    // Text glyph outlines follow their (solved) text boxes.
    data.add_text_outlines(&mut sk);
    let regions = sk.regions();
    let wires = sk.wires();
    let mut dimension_text = BTreeMap::new();
    for c in sk.constraints() {
        if let Some(v) = c.kind.value() {
            let q = match c.kind {
                ConstraintKind::Angle { .. } | ConstraintKind::ArcAngle { .. } => noshape_units::Quantity::new(v, Dim::ANGLE),
                ConstraintKind::Rho { .. } => noshape_units::Quantity::new(v, Dim::UNITLESS),
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
    let images = data
        .images
        .iter()
        .map(|i| crate::regen::context::SketchResultImage { id: i.id, box_points: i.box_points.clone(), data_base64: i.data_base64.clone() })
        .collect();
    ev.ctx.sketches.push(Arc::new(SketchResult { feature: ev.feature, name, frame, sketch: sk, report, regions, wires, dimension_text, images }));
    Ok(())
}

/// Splits `text` at top-level occurrences of `sep` (outside quotes and parentheses).
fn split_top(text: &str, sep: char) -> Vec<String> {
    let (mut out, mut cur, mut depth, mut quoted) = (Vec::new(), String::new(), 0i32, false);
    for ch in text.chars() {
        match ch {
            '"' => quoted = !quoted,
            '(' | '[' if !quoted => depth += 1,
            ')' | ']' if !quoted => depth -= 1,
            c if c == sep && !quoted && depth == 0 => {
                out.push(std::mem::take(&mut cur));
                continue;
            }
            _ => {}
        }
        cur.push(ch);
    }
    out.push(cur);
    out
}

/// A number with up to `max` decimals, trailing zeros removed.
fn trim_number(v: f64, max: usize) -> String {
    let s = format!("{v:.max$}");
    if s.contains('.') { s.trim_end_matches('0').trim_end_matches('.').to_string() } else { s }
}

/// Evaluates a sketch text expression: string literals ("..."), numeric
/// expressions (variables, units, functions; unitless results print as
/// numbers, lengths and angles with their unit), `roundToPrecision(x, n)`
/// and `~` concatenation (Onshape's text expressions).
pub fn eval_text_expression(ev: &EvalCtx, text: &str) -> Result<String, String> {
    let mut out = String::new();
    for piece in split_top(text, '~') {
        let p = piece.trim();
        if p.is_empty() {
            continue;
        }
        if p.len() >= 2 && p.starts_with('"') && p.ends_with('"') {
            out.push_str(&p[1..p.len() - 1].replace("\\\"", "\""));
            continue;
        }
        if let Some(inner) = p.strip_prefix("roundToPrecision(").and_then(|r| r.strip_suffix(')')) {
            let args = split_top(inner, ',');
            if args.len() == 2 {
                let x = ev.eval_text(args[0].trim(), Dim::UNITLESS).map_err(|e| e.message)?.value;
                let n = ev.eval_text(args[1].trim(), Dim::UNITLESS).map_err(|e| e.message)?.value.round().clamp(0.0, 12.0) as usize;
                out.push_str(&format!("{x:.n$}"));
                continue;
            }
        }
        let v = ev
            .eval_text(p, Dim::UNITLESS)
            .map(|q| trim_number(q.value, 6))
            .or_else(|_| ev.eval_text(p, Dim::LENGTH).map(|q| noshape_units::format_quantity(q, ev.units)))
            .or_else(|_| ev.eval_text(p, Dim::ANGLE).map(|q| noshape_units::format_quantity(q, ev.units)))
            .map_err(|e| e.message)?;
        out.push_str(&v);
    }
    Ok(out)
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

/// Projection of any edge onto the sketch plane: exact for lines, circles
/// in planes parallel to the sketch and circles seen obliquely (ellipses),
/// sampled (fit spline) otherwise.
pub fn project_edge_any(edge: &noshape_kernel::Shape, frame: &crate::display::PlaneFrame) -> Option<SectionCurve> {
    use glam::{DVec2, DVec3};
    use noshape_kernel as k;
    if let Some(c) = project_edge(edge, frame) {
        return Some(SectionCurve::Exact(c));
    }
    let info = edge.edge_info().ok()?;
    if let k::Curve::Circle { center, axis, x_dir, radius } = info.curve {
        let a = axis.normalize();
        let cos = a.dot(frame.normal).abs();
        if cos > 1e-6 {
            // A circle seen obliquely is an ellipse: the axis direction in
            // the plane foreshortens by cos, the perpendicular stays.
            let c2 = frame.to_local(center);
            let in_plane = frame.normal.cross(a);
            let major3 = if in_plane.length() < 1e-12 { x_dir.normalize() } else { in_plane.normalize() };
            let m2 = frame.to_local(center + major3 * radius) - c2;
            if info.closed {
                return Some(SectionCurve::Exact(noshape_sketch::Curve2::Ellipse { center: c2, major: m2, minor: radius * cos }));
            }
        }
    }
    let t = k::tessellate(edge, 1e-5, 0.05, false).ok()?;
    if t.edges.is_empty() {
        return None;
    }
    let pl: Vec<DVec2> = t.edge_polyline(0).iter().map(|v| frame.to_local(DVec3::new(v[0] as f64, v[1] as f64, v[2] as f64))).collect();
    // An edge seen end-on (normal to the sketch) projects to a point.
    let len: f64 = pl.windows(2).map(|w| (w[1] - w[0]).length()).sum();
    if len < 1e-9 {
        return None;
    }
    resample(&pl, SECTION_SAMPLES).map(SectionCurve::Sampled)
}

/// Where an edge crosses the sketch plane (Pierce).
pub fn pierce_point(edge: &noshape_kernel::Shape, frame: &crate::display::PlaneFrame) -> Option<glam::DVec2> {
    use glam::DVec3;
    let t = noshape_kernel::tessellate(edge, 1e-6, 0.02, false).ok()?;
    if t.edges.is_empty() {
        return None;
    }
    let pts: Vec<DVec3> = t.edge_polyline(0).iter().map(|v| DVec3::new(v[0] as f64, v[1] as f64, v[2] as f64)).collect();
    let h = |p: DVec3| (p - frame.origin).dot(frame.normal);
    let mut best: Option<(f64, DVec3)> = None;
    for w in pts.windows(2) {
        let (a, b) = (h(w[0]), h(w[1]));
        if a * b <= 0.0 && (a - b).abs() > 1e-15 {
            let p = w[0] + (w[1] - w[0]) * (a / (a - b));
            return Some(frame.to_local(p));
        }
        let d = a.abs().min(b.abs());
        if best.is_none_or(|bb| d < bb.0) {
            best = Some((d, if a.abs() < b.abs() { w[0] } else { w[1] }));
        }
    }
    // Lines (straight edges) extended to the plane.
    if let Ok(info) = edge.edge_info()
        && let noshape_kernel::Curve::Line { .. } = info.curve
    {
        let d = info.end - info.start;
        let den = d.dot(frame.normal);
        if den.abs() > 1e-12 {
            let s = (frame.origin - info.start).dot(frame.normal) / den;
            return Some(frame.to_local(info.start + d * s));
        }
    }
    best.filter(|b| b.0 < 1e-7).map(|b| frame.to_local(b.1))
}

/// Trace of a plane in the sketch plane: a point on it and its direction
/// (sketch coordinates). `None` when the planes are parallel.
pub fn plane_trace(plane: &crate::display::PlaneFrame, frame: &crate::display::PlaneFrame) -> Option<(glam::DVec2, glam::DVec2)> {
    let d3 = frame.normal.cross(plane.normal);
    if d3.length() < 1e-9 {
        return None;
    }
    let d3 = d3.normalize();
    // In-plane direction perpendicular to the trace, towards the other plane.
    let w = frame.normal.cross(d3);
    let den = w.dot(plane.normal);
    if den.abs() < 1e-15 {
        return None;
    }
    let s = (plane.origin - frame.origin).dot(plane.normal) / den;
    let p3 = frame.origin + w * s;
    let p = frame.to_local(p3);
    let d = frame.to_local(p3 + d3) - p;
    Some((p, d.normalize()))
}

/// Move a trace line (external line entity) onto a new trace, keeping its
/// length and centering it at the projection of its old midpoint.
pub fn update_trace(sk: &mut noshape_sketch::Sketch, id: noshape_sketch::EntityId, p: glam::DVec2, d: glam::DVec2) {
    let Some((s, e)) = sk.endpoints(id) else { return };
    let (Some(a), Some(b)) = (sk.point(s), sk.point(e)) else { return };
    let half = ((b - a).length() * 0.5).max(1e-6);
    let m = (a + b) * 0.5;
    let c = p + d * (m - p).dot(d);
    let dir = if (b - a).dot(d) < 0.0 { -d } else { d };
    let _ = sk.set_point(s, c - dir * half);
    let _ = sk.set_point(e, c + dir * half);
}

/// Silhouette curves of a curved face seen normal to the sketch plane
/// (Use silhouette edges): cylinders, cones, spheres and tori whose
/// silhouettes are trackable analytic curves.
pub fn face_silhouettes(face: &noshape_kernel::Shape, frame: &crate::display::PlaneFrame) -> Vec<noshape_sketch::Curve2> {
    use glam::DVec3;
    use noshape_kernel::Surface;
    use noshape_sketch::Curve2;
    let Ok(surf) = face.face_surface() else { return Vec::new() };
    let n = frame.normal;
    // Extent of the face along a 3D direction (from its bounding box corners).
    let extent = |o: DVec3, a: DVec3| -> Option<(f64, f64)> {
        let bb = face.bounding_box(true).ok()?;
        let (lo, hi) = (bb.min, bb.max);
        let mut r = (f64::INFINITY, f64::NEG_INFINITY);
        for i in 0..8 {
            let c = DVec3::new(if i & 1 == 0 { lo.x } else { hi.x }, if i & 2 == 0 { lo.y } else { hi.y }, if i & 4 == 0 { lo.z } else { hi.z });
            let t = (c - o).dot(a);
            r = (r.0.min(t), r.1.max(t));
        }
        Some(r)
    };
    let line = |p: DVec3, q: DVec3| Curve2::Line { a: frame.to_local(p), b: frame.to_local(q) };
    match surf {
        Surface::Cylinder { origin, axis, radius, .. } => {
            let a = axis.normalize();
            let side = n.cross(a);
            if side.length() < 1e-9 {
                // Axis normal to the sketch: the silhouette is the circle.
                return vec![Curve2::Circle { center: frame.to_local(origin), radius }];
            }
            let side = side.normalize() * radius;
            let Some((t0, t1)) = extent(origin, a) else { return Vec::new() };
            [side, -side].iter().map(|s| line(origin + *s + a * t0, origin + *s + a * t1)).collect()
        }
        Surface::Sphere { center, radius, .. } => vec![Curve2::Circle { center: frame.to_local(center), radius }],
        Surface::Torus { center, axis, major_radius, minor_radius } if axis.normalize().dot(n).abs() > 1.0 - 1e-9 => {
            let c = frame.to_local(center);
            vec![Curve2::Circle { center: c, radius: major_radius + minor_radius }, Curve2::Circle { center: c, radius: (major_radius - minor_radius).abs() }]
        }
        Surface::Cone { axis, half_angle, apex, .. } if axis.normalize().dot(n).abs() < 1e-9 => {
            // Axis in a plane parallel to the sketch: two generators from the apex.
            let a = axis.normalize();
            let side = n.cross(a).normalize();
            let Some((t0, t1)) = extent(apex, a) else { return Vec::new() };
            let (c, s) = (half_angle.cos(), half_angle.sin());
            [1.0, -1.0]
                .iter()
                .map(|k| {
                    let g = a * c + side * (s * *k);
                    let (u0, u1) = (t0.max(0.0) / c, t1 / c);
                    line(apex + g * u0, apex + g * u1)
                })
                .filter(|l| l.length() > 1e-9)
                .collect()
        }
        _ => Vec::new(),
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
        (Geometry::Ellipse { center, .. }, Curve2::Ellipse { center: c2, major, minor }) => {
            let _ = sk.set_point(*center, *c2);
            if let Some(ent) = sk.entity_mut(id)
                && let Geometry::Ellipse { major: m, minor: n, .. } = &mut ent.geometry
            {
                *m = *major;
                *n = *minor;
            }
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

#[cfg(test)]
mod text_expression_tests {
    use super::*;

    #[test]
    fn splits_concatenations_outside_quotes_and_calls() {
        assert_eq!(split_top(r#""a ~ b" ~ #x ~ f(1 ~ 2)"#, '~'), vec![r#""a ~ b" "#, " #x ", " f(1 ~ 2)"]);
        assert_eq!(split_top("roundToPrecision(#l / in, 3)", ','), vec!["roundToPrecision(#l / in, 3)"]);
        assert_eq!(trim_number(2.500, 6), "2.5");
        assert_eq!(trim_number(3.0, 6), "3");
    }

    #[test]
    fn ellipse_diameter_expressions_regenerate_as_radii() {
        use crate::configuration::ConfigValues;
        use crate::partstudio::PartStudio;
        use crate::reference::{DatumPlane, Reference};
        use crate::regen::{RegenOptions, Regenerator};
        use crate::sketch_feature::SketchData;
        let units = noshape_units::UnitPrefs::default();
        let mut sk = noshape_sketch::Sketch::new();
        let e = sk.add_ellipse(glam::DVec2::ZERO, glam::DVec2::new(0.01, 0.0), 0.005);
        let cid = sk.add_constraint(ConstraintKind::EllipseRadius { ellipse: e, major: true, value: 0.01 }).unwrap();
        let mut data = SketchData { sketch: sk, ..Default::default() };
        // Typed as a diameter (Onshape shows ellipse axes as diameters).
        data.dimension_exprs.insert(cid.0, noshape_units::ExprValue::new("2 * 15 mm"));
        let mut ps = PartStudio::new();
        let mut f = ps.new_feature("sketch", &units).unwrap();
        f.params.set("sketchPlane", ParamValue::Query(vec![Reference::Plane(DatumPlane::Top)]));
        f.params.set("sketch", ParamValue::Sketch(Box::new(data)));
        ps.features.push(f);
        let r = Regenerator::new().regenerate(&ps, &ConfigValues::default(), &units, &RegenOptions::default());
        let solved = &r.context.sketches[0].sketch;
        let v = solved.constraint(cid).unwrap().kind.value().unwrap();
        assert!((v - 0.015).abs() < 1e-12, "stored radius = typed diameter / 2, got {v}");
    }

    /// Live (qa4 DIM-UNSOLVED-GEOM): a dimension set against an Equal keeps
    /// the last solved shape and is listed with the conflict (red).
    #[test]
    fn unsolvable_sketch_keeps_its_last_solved_shape() {
        use crate::configuration::ConfigValues;
        use crate::partstudio::PartStudio;
        use crate::reference::{DatumPlane, Reference};
        use crate::regen::{RegenOptions, Regenerator};
        use crate::sketch_feature::SketchData;
        let units = noshape_units::UnitPrefs::default();
        let mut sk = noshape_sketch::Sketch::new();
        let a = sk.add_line(glam::DVec2::new(0.0, 0.0), glam::DVec2::new(0.0, 0.032));
        let b = sk.add_line(glam::DVec2::new(0.02, 0.0), glam::DVec2::new(0.02, 0.032));
        let lb = sk.add_constraint(ConstraintKind::Length { curve: b, value: 0.032 }).unwrap();
        sk.add_constraint(ConstraintKind::Vertical { a, b: None }).unwrap();
        sk.add_constraint(ConstraintKind::Vertical { a: b, b: None }).unwrap();
        sk.add_constraint(ConstraintKind::Length { curve: a, value: 0.032 }).unwrap();
        sk.add_constraint(ConstraintKind::Equal { a, b }).unwrap();
        assert!(sk.solve().is_solved());
        sk.set_dimension_value(lb, 0.05).unwrap();
        let mut ps = PartStudio::new();
        let mut f = ps.new_feature("sketch", &units).unwrap();
        f.params.set("sketchPlane", ParamValue::Query(vec![Reference::Plane(DatumPlane::Top)]));
        f.params.set("sketch", ParamValue::Sketch(Box::new(SketchData { sketch: sk, ..Default::default() })));
        ps.features.push(f);
        let r = Regenerator::new().regenerate(&ps, &ConfigValues::default(), &units, &RegenOptions::default());
        let res = &r.context.sketches[0];
        assert!(res.report.conflicting().contains(&lb), "{:?}", res.report.status);
        let len = res.sketch.curve(b).unwrap().length();
        assert!((len - 0.032).abs() < 1e-12, "the last solved shape stays: {len}");
    }

    #[test]
    fn plane_trace_of_perpendicular_planes() {
        use crate::display::PlaneFrame;
        // The Front plane seen from a Top sketch: the sketch x axis.
        let (p, d) = plane_trace(&PlaneFrame::FRONT, &PlaneFrame::TOP).unwrap();
        assert!(p.y.abs() < 1e-12 && d.y.abs() < 1e-12 && (d.x.abs() - 1.0).abs() < 1e-12);
        assert!(plane_trace(&PlaneFrame::TOP, &PlaneFrame::TOP).is_none());
        let mut sk = noshape_sketch::Sketch::new();
        let l = sk.add_line(glam::DVec2::new(-1.0, 3.0), glam::DVec2::new(1.0, 3.0));
        update_trace(&mut sk, l, p, d);
        let c = sk.curve(l).unwrap();
        assert!(c.start_point().y.abs() < 1e-12 && c.end_point().y.abs() < 1e-12 && (c.length() - 2.0).abs() < 1e-12);
    }
}
