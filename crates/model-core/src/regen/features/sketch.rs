//! Sketch feature.

use crate::params::ParamValue;
use crate::regen::context::SketchResult;
use crate::regen::eval::{EvalCtx, FResult, err};
use noshape_sketch::regions::Region;
use noshape_sketch::{ConstraintId, ConstraintKind};
use noshape_units::Dim;
use std::collections::BTreeMap;
use std::sync::Arc;

pub use noshape_model_data::sketch_feature::CONFIGURED_DIMENSION_PREFIX;

/// A sketch without a plane, or whose plane is gone (live: the row tooltip "Sketch 2 did not regenerate properly:
/// Select a sketch plane." with "1 missing selection"; a new sketch's blue bubble reads it without the period).
pub const NO_SKETCH_PLANE: &str = "Select a sketch plane.";

pub fn eval(ev: &mut EvalCtx) -> FResult<()> {
    let Some(plane_ref) = ev.query("sketchPlane").first() else {
        return crate::regen::eval::err_at(&["sketchPlane"], NO_SKETCH_PLANE);
    };
    let Some(frame) = ev.resolve_sketch_plane(plane_ref) else {
        return err("Select a sketch plane.");
    };
    if let Some((part, _)) = match plane_ref {
        crate::reference::Reference::Face(n) => crate::sheetmetal::flat_face_of(n),
        _ => None,
    } {
        ev.ctx.flat_sketches.insert(ev.feature, part);
    }
    let data = match ev.params.get("sketch") {
        Some(ParamValue::Sketch(d)) => d.clone(),
        _ => Default::default(),
    };
    let mut sk = data.sketch.clone();
    // Re-project used ("Use") 3D geometry so it follows upstream changes:
    // vertices, edges (exact or sampled projections, pierce points), plane
    // traces and silhouettes of curved faces.
    // A sketch that held before its used geometry moved (a feature above edited) follows it with the least change of
    // the whole sketch: c2-hunting-knife's Extrude 1 0.02 mm deeper slid Sketch 6's mirrored groove 2.3 mm along the
    // blade under the one-at-a-time placement, and Extrude 3 then cut away the corner Sketches 7, 8 and 11 use.
    let held = !(data.external_refs.is_empty() && data.intersections.is_empty()) && sk.holds();
    let mut lost_ids = Vec::new();
    let mut silhouettes: BTreeMap<String, Vec<noshape_sketch::Curve2>> = BTreeMap::new();
    for (eid, r) in &data.external_refs {
        let id = noshape_sketch::EntityId(*eid);
        // The Origin used as a point.
        if matches!(r, crate::reference::Reference::Origin) {
            let _ = sk.set_point(id, frame.to_local(glam::DVec3::ZERO));
            continue;
        }
        // An implicit point used (a circular edge's centre).
        if let crate::reference::Reference::Implicit { on, inference } = r {
            match crate::regen::implicit::implicit_connector(ev, on, *inference) {
                Some(c) => {
                    let _ = sk.set_point(id, frame.to_local(c.origin));
                }
                None => lost_ids.push(id),
            }
            continue;
        }
        // Projected model vertices (snapped to while sketching).
        if matches!(r, crate::reference::Reference::Vertex(_) | crate::reference::Reference::MeshPoint { .. }) {
            match ev.resolve_vertex_point(r) {
                Some(p) => {
                    let _ = sk.set_point(id, frame.to_local(p));
                }
                None => lost_ids.push(id),
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
                None => lost_ids.push(id),
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
            if cands.is_empty() || update_intersection(&mut sk, &[*eid], &cands) > 0 {
                lost_ids.push(id);
            }
            continue;
        }
        // Entities of other sketches are used like model edges / vertices.
        let edge = if let crate::reference::Reference::SketchEntity { .. } = r {
            let Some(shape) = crate::regen::measure::resolve_shapes(&ev.ctx, r).into_iter().next() else {
                lost_ids.push(id);
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
                lost_ids.push(id);
                continue;
            };
            let Ok(edge) = ev.edge_shape(bi, ei) else { continue };
            edge
        };
        // An end a Trim or Split cut off a used edge: kept at that end of its projection.
        if let Some(&at_end) = data.use_ends.get(eid) {
            match project_edge_with(&edge, &frame, true) {
                Some(SectionCurve::Exact(c)) => {
                    let (t0, t1) = c.domain();
                    let _ = sk.set_point(id, c.point(if at_end { t1 } else { t0 }));
                }
                _ => lost_ids.push(id),
            }
            continue;
        }
        if sk.kind(id) == Some(noshape_sketch::EntityKind::Point) {
            // Pierce point: where the edge crosses the sketch plane.
            match pierce_point(&edge, &frame) {
                Some(p) => {
                    let _ = sk.set_point(id, p);
                }
                None => lost_ids.push(id),
            }
            continue;
        }
        match project_edge_with(&edge, &frame, sk.kind(id) != Some(noshape_sketch::EntityKind::FitSpline)) {
            Some(SectionCurve::Exact(c)) => update_external(&mut sk, id, &c),
            Some(sampled) => {
                update_intersection(&mut sk, &[*eid], &[sampled]);
            }
            None => {}
        }
    }
    // Live (skasm plane-use-1125 (10)): a used entity whose source is gone stays where it was, as free geometry (drawn
    // blue); points it shares with geometry still used stay fixed.
    let missing = lost_ids.len();
    if missing > 0 {
        let lost: std::collections::HashSet<_> = lost_ids.iter().copied().collect();
        let kept: std::collections::HashSet<_> = sk.entities().filter(|e| e.external && !lost.contains(&e.id)).flat_map(|e| e.point_refs()).collect();
        for id in lost_ids {
            let points = sk.entity(id).map(|e| e.point_refs()).unwrap_or_default();
            for p in points.into_iter().filter(|p| !held.contains(p)).chain([id]) {
                if let Some(e) = sk.entity_mut(p) {
                    e.external = false;
                }
            }
        }
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
    // Live (clamp-guillem's Boceto 6): a WARNING, whatever was lost.
    if missing + lost > 0 {
        ev.warnings.push("Some external references are missing.".into());
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
        for t in std::sync::Arc::make_mut(&mut data).texts.iter_mut().filter(|t| t.expression) {
            match eval_text_expression(ev, &t.text) {
                Ok(v) if !v.trim().is_empty() => {
                    t.text = v;
                    let _ = t.fit_box(&mut sk, t.layout().width);
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
        [] if held => sk.solve_with(&noshape_sketch::SolveOptions { analyze: !sk.analysis_current(), ..Default::default() }),
        // One new value (a variable, a configuration): placed as live's
        // regeneration does (least change), not as an edit in the editor.
        [one] => {
            let opts = noshape_sketch::SolveOptions { constructive: true, ..sk.dimension_edit_options(*one) };
            sk.solve_with(&opts)
        }
        // The DOF analysis (fully / under defined) depends on the
        // constraints, not on where a drag moved the geometry: the sketch's
        // cached one is reused while nothing else changed.
        // New values place what they move as live does (constructive).
        _ if sk.analysis_current() => sk.solve_with(&noshape_sketch::SolveOptions { analyze: false, constructive: true, ..Default::default() }),
        _ => sk.solve_with(&noshape_sketch::SolveOptions { constructive: true, ..Default::default() }),
    };
    // Live (qa4 DIM-UNSOLVED-GEOM): a sketch that can't be solved keeps its
    // last solved shape, not the rest solved around the conflict.
    // (Nor one whose solve fails: live errors-1118's triangle with a side typed longer than the other two, the row red.)
    let unsolved = !report.is_solved();
    if unsolved {
        sk = last_solved;
    }
    // Text glyph outlines follow their (solved) text boxes.
    data.add_text_outlines(&mut sk);
    let (regions, imprinted, split) = imprint(ev, &frame, &sk, sk.regions());
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
    if unsolved {
        ev.warnings.push("Sketch could not be solved.".into());
    }
    // Live (spline-image-1125 (4)): a spline whose end lies on its own start without being closed is degenerate.
    let closes_on_itself = sk.entities().any(|e| match &e.geometry {
        noshape_sketch::Geometry::FitSpline { points, handles: Some(_) } => match (points.first(), points.last()) {
            (Some(a), Some(b)) if a != b => sk.point(*a).zip(sk.point(*b)).is_some_and(|(p, q)| (p - q).length() < 1e-9),
            _ => false,
        },
        _ => false,
    });
    if closes_on_itself {
        problems.push("Some of the geometry intersects itself or is degenerate.".into());
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
    ev.ctx.sketches.push(Arc::new(SketchResult { feature: ev.feature, name, frame, sketch: sk, report, regions, imprinted, split, wires, dimension_text, images, legacy_edge_directions: ev.ctx.legacy_edge_directions, bodies: None }));
    Ok(())
}

/// Entity ids of the face edges imprinted on a sketch (above any sketch entity's; region keys take one more bit).
pub const IMPRINT_ENTITY: u32 = 1 << 30;

/// Whether a sketch entity id is an imprinted face edge's (text boxes' glyph outlines lie above them).
pub fn is_imprint_entity(id: u32) -> bool {
    (IMPRINT_ENTITY..noshape_sketch::text::glyph_id_base(1)).contains(&id)
}

/// The sketch's regions with its plane's face imprinted, as live unless Disable imprinting: the face's edges split the
/// sketch's own regions, and the face's pieces outside them are regions too (sketch faces that qSketchRegion leaves
/// out). The regions, which of them are such pieces, and the sketch's own regions that were split.
fn imprint(ev: &EvalCtx, frame: &crate::display::PlaneFrame, sk: &noshape_sketch::Sketch, own: Vec<Region>) -> (Vec<Region>, Vec<bool>, Vec<Region>) {
    let plain = |own| (own, Vec::new(), Vec::new());
    use crate::reference::{Reference, Source};
    let Some(plane) = ev.query("sketchPlane").first().cloned() else { return plain(own) };
    // (Not in a studio saved before imprinting: its references name the regions it had then.)
    if ev.bool("disableImprinting") || noshape_kernel::legacy_topology() {
        return plain(own);
    }
    // A part's face, or another sketch's region (live 1747: "Face of Sketch 2").
    let face = match &plane {
        Reference::Face(_) => ev.resolve_faces(std::slice::from_ref(&plane)).first().and_then(|&(bi, fi)| ev.face_shape(bi, fi).ok()),
        Reference::SketchRegion { sketch, key } => ev.resolve_region(*sketch, key).and_then(|(s, r)| crate::regen::eval::region_profile(&s, &r, Source::Index(0)).ok()).map(|p| p.shape),
        _ => None,
    };
    let Some(face) = face else { return plain(own) };
    let mut extra = Vec::new();
    for (i, e) in face.edges().iter().enumerate() {
        let Some(c) = face_edge_curve(e, frame) else { return plain(own) };
        extra.push((noshape_sketch::EntityId(IMPRINT_ENTITY + i as u32), c));
    }
    let ids: std::collections::HashSet<&noshape_sketch::regions::RegionId> = own.iter().map(|o| &o.id).collect();
    // (A first filter only, `contains` decides: control-point boxes and polygons made once; exact spline boxes and
    // a tessellation per test were most of an imprint, 3-11 ms a sketch on a face.)
    let boxes: Vec<_> = own.iter().map(|o| o.hull_bbox()).collect();
    let testers: Vec<std::cell::OnceCell<noshape_sketch::regions::RegionTester>> = own.iter().map(|_| std::cell::OnceCell::new()).collect();
    // Every closed piece is a region: the face's, those the sketch's curves close with the face's edges off it (live:
    // c2-duct-multipart's Sketch 5 triangles between a dangling line, a diagonal and the face's edge), and a hole's
    // inside, closed by the face's edges alone (live: asm-robotic-arm's Sketch 3 on a frame's face: JPC inside JPG).
    let (mut regions, mut imprinted) = (Vec::new(), Vec::new());
    for r in sk.regions_with(&extra) {
        let inside = ids.contains(&r.id) || {
            let p = r.interior_point();
            own.iter().zip(&boxes).zip(&testers).any(|((o, b), t)| b.contains(p) && t.get_or_init(|| o.tester()).contains(p))
        };
        regions.push(r);
        imprinted.push(!inside);
    }
    let kept: std::collections::HashSet<noshape_sketch::regions::RegionId> = regions.iter().map(|r| r.id.clone()).collect();
    let split = own.into_iter().filter(|o| !kept.contains(&o.id)).collect();
    // The sketch's own regions first, as without imprinting.
    let (mut first, mut pieces): (Vec<_>, Vec<_>) = regions.into_iter().zip(imprinted).partition(|(_, p)| !p);
    first.append(&mut pieces);
    let (regions, imprinted) = first.into_iter().unzip();
    (regions, imprinted, split)
}

/// A planar face's edge in its sketch: lines and arcs as such, other curves as their exact B-spline.
fn face_edge_curve(edge: &noshape_kernel::Shape, frame: &crate::display::PlaneFrame) -> Option<noshape_sketch::Curve2> {
    if let Some(c) = project_edge(edge, frame) {
        return Some(c);
    }
    let b = edge.edge_bspline().ok()?;
    if b.periodic {
        return None;
    }
    let knots = b.knots.iter().zip(&b.mults).flat_map(|(&t, &m)| std::iter::repeat_n(t, m as usize)).collect();
    let ctrl = b.poles.iter().map(|&p| frame.to_local(p)).collect();
    Some(noshape_sketch::Curve2::Nurbs(noshape_sketch::Nurbs { degree: b.degree as usize, knots, ctrl, weights: b.weights }))
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

/// `name(argument)`'s argument when the whole of `text` is that one call.
fn call_argument<'a>(text: &'a str, name: &str) -> Option<&'a str> {
    let inner = text.strip_prefix(name)?.strip_prefix('(')?.strip_suffix(')')?;
    let mut depth = 0i32;
    for ch in inner.chars() {
        depth += match ch {
            '(' => 1,
            ')' => -1,
            _ => 0,
        };
        if depth < 0 {
            return None;
        }
    }
    (depth == 0).then_some(inner)
}

/// A number with up to `max` decimals, trailing zeros removed.
fn trim_number(v: f64, max: usize) -> String {
    let s = format!("{v:.max$}");
    if s.contains('.') { s.trim_end_matches('0').trim_end_matches('.').to_string() } else { s }
}

/// Evaluates a sketch text expression: string literals ("..."), numeric
/// expressions (variables, units, functions; unitless results print as
/// numbers, lengths and angles with their unit), `roundToPrecision(x, n)`,
/// `toString(x)` and `~` concatenation (Onshape's text expressions).
pub fn eval_text_expression(ev: &EvalCtx, text: &str) -> Result<String, String> {
    let mut out = String::new();
    for piece in split_top(text, '~') {
        let mut p = piece.trim();
        while let Some(inner) = call_argument(p, "toString") {
            p = inner.trim();
        }
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
        // An ellipse seen as a circle (a cylinder cut obliquely, seen along its axis: c2-hunting-knife's Sketch 16
        // offsets such an edge as a circle, live): its projected semi-axes equal and perpendicular.
        k::Curve::Ellipse { center, axis, x_dir, major_radius, minor_radius } => {
            let c = frame.to_local(center);
            let u = frame.to_local(center + x_dir.normalize() * major_radius) - c;
            let v = frame.to_local(center + axis.normalize().cross(x_dir.normalize()) * minor_radius) - c;
            let radius = u.length();
            if radius < 1e-12 || (v.length() - radius).abs() > 1e-7 * radius || u.dot(v).abs() > 1e-7 * radius * radius {
                return None;
            }
            if info.closed {
                return Some(Curve2::Circle { center: c, radius });
            }
            let ang = |p: glam::DVec3| {
                let d = frame.to_local(p) - c;
                d.y.atan2(d.x)
            };
            // The edge runs CCW from u to v; seen from the sketch normal it is CCW when u × v is positive.
            let (mut a0, mut a1) = (ang(info.start), ang(info.end));
            if u.perp_dot(v) < 0.0 {
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
    project_edge_with(edge, frame, true)
}

/// [`project_edge_any`]; with `exact_splines` off a B-spline edge is sampled (a Use made before it was exact: a fit
/// spline through its samples, which keeps following the model).
fn project_edge_with(edge: &noshape_kernel::Shape, frame: &crate::display::PlaneFrame, exact_splines: bool) -> Option<SectionCurve> {
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
    // A planar curve seen edge-on (a circle whose plane holds the sketch normal) projects onto a line: that line
    // between its extremes, as live (c2-rings-loft's Sketch 1: a point Coincident with such a circle lies on the line
    // past the circle's extent; sampled, it went back and forth on itself and held the point within it).
    let far = pl.iter().copied().max_by(|a, b| (*a - pl[0]).length().total_cmp(&(*b - pl[0]).length()))?;
    if let Some(d) = (far - pl[0]).try_normalize()
        && pl.iter().all(|p| (*p - pl[0]).perp_dot(d).abs() <= 1e-9 + 1e-7 * len)
    {
        let along = |p: &DVec2| (*p - pl[0]).dot(d);
        let (lo, hi) = pl.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), p| (lo.min(along(p)), hi.max(along(p))));
        // The extremes from the exact geometry (the tessellation's points are single precision: a Use joined to
        // another edge's exact end missed it by 3 nm, c2-fan-blade-sweep's Sketch 5): the edge's ends, a circle's
        // points farthest along the line.
        let mut exact: Vec<DVec2> = vec![frame.to_local(info.start), frame.to_local(info.end)];
        // How far the tessellation may stop short of an exact extreme: a circle's samples (0.05 rad apart) miss its
        // farthest point by up to r (1 - cos 0.05) (c2-valve-config's Sketch 3: an R 56 circle seen edge-on reached
        // 55.990 mm, so its Use missed live's line).
        let mut reach = 1e-6;
        if let k::Curve::Circle { center, axis, radius, .. } = info.curve {
            let d3 = frame.x * d.x + frame.y * d.y;
            let a = axis.normalize();
            if let Some(v) = (d3 - a * a.dot(d3)).try_normalize() {
                exact.extend([frame.to_local(center + v * radius), frame.to_local(center - v * radius)]);
            }
            reach += 1e-5 + radius * (1.0 - 0.05f64.cos());
        }
        let snap = |t: f64| exact.iter().map(along).filter(|e| (e - t).abs() <= reach).min_by(|x, y| (x - t).abs().total_cmp(&(y - t).abs())).unwrap_or(t);
        let foot = |t: f64| exact.iter().find(|p| (along(p) - t).abs() <= 1e-12).copied().unwrap_or(pl[0] + d * t);
        let (lo, hi) = (snap(lo), snap(hi));
        return Some(SectionCurve::Exact(noshape_sketch::Curve2::Line { a: foot(lo), b: foot(hi) }));
    }
    // A B-spline or Bézier edge projects exactly: its poles projected, its knots (on 0..1) and weights kept, as live's
    // Use stores it (c2-parting-line-draft-parting-line's Sketch 3: a fit through samples missed the part's parting edge
    // by 0.13 mm, and the Split by its extrusion left slivers).
    if exact_splines
        && matches!(info.curve, k::Curve::BSpline { periodic: false, .. } | k::Curve::Bezier { .. })
        && let Some(n) = projected_nurbs(edge, frame)
    {
        return Some(SectionCurve::Exact(noshape_sketch::Curve2::Nurbs(n)));
    }
    resample(&pl, SECTION_SAMPLES).map(SectionCurve::Sampled)
}

/// A non-periodic B-spline edge's orthogonal projection onto the sketch plane, exact (the projection is affine).
fn projected_nurbs(edge: &noshape_kernel::Shape, frame: &crate::display::PlaneFrame) -> Option<noshape_sketch::Nurbs> {
    let b = edge.edge_bspline().ok()?;
    let (k0, k1) = (*b.knots.first()?, *b.knots.last()?);
    if b.periodic || k1 <= k0 {
        return None;
    }
    let knots = b.knots.iter().zip(&b.mults).flat_map(|(k, m)| std::iter::repeat_n((k - k0) / (k1 - k0), *m as usize)).collect();
    let n = noshape_sketch::Nurbs { degree: b.degree as usize, knots, ctrl: b.poles.iter().map(|p| frame.to_local(*p)).collect(), weights: b.weights };
    n.is_valid().then_some(n)
}

/// Where an edge crosses the sketch plane (Pierce): bracketed on samples along the curve, then halved down on the curve
/// itself (exact on an arc, not on a chord of its tessellation).
pub fn pierce_point(edge: &noshape_kernel::Shape, frame: &crate::display::PlaneFrame) -> Option<glam::DVec2> {
    use glam::DVec3;
    const SAMPLES: usize = 64;
    let info = edge.edge_info().ok()?;
    let h = |p: DVec3| (p - frame.origin).dot(frame.normal);
    let at = |t: f64| edge.point_at(t).ok();
    let (t0, t1) = (info.first_param, info.last_param);
    let mut best: Option<(f64, DVec3, f64)> = None;
    let mut prev: Option<(f64, f64)> = None;
    for i in 0..=SAMPLES {
        let t = t0 + (t1 - t0) * i as f64 / SAMPLES as f64;
        let p = at(t)?;
        let v = h(p);
        if best.is_none_or(|b| v.abs() < b.0) {
            best = Some((v.abs(), p, t));
        }
        if let Some((ta, va)) = prev
            && va * v <= 0.0
            && (va - v).abs() > 1e-15
        {
            let (mut lo, mut hi, mut vlo) = (ta, t, va);
            for _ in 0..80 {
                let m = 0.5 * (lo + hi);
                let vm = h(at(m)?);
                if vlo * vm <= 0.0 {
                    hi = m;
                } else {
                    (lo, vlo) = (m, vm);
                }
            }
            return Some(frame.to_local(at(0.5 * (lo + hi))?));
        }
        prev = Some((t, v));
    }
    // Lines (straight edges) extended to the plane.
    if let noshape_kernel::Curve::Line { .. } = info.curve {
        let d = info.end - info.start;
        let den = d.dot(frame.normal);
        if den.abs() > 1e-12 {
            let s = (frame.origin - info.start).dot(frame.normal) / den;
            return Some(frame.to_local(info.start + d * s));
        }
    }
    // A curve that touches the plane without crossing it (a circle tangent to it, c2-loft-paths-guides' Sketch 5): the
    // touching point, the nearest sample refined.
    let (_, _, tb) = best?;
    let step = (t1 - t0) / SAMPLES as f64;
    let (mut lo, mut hi) = ((tb - step).max(t0), (tb + step).min(t1));
    let dist = |t: f64| at(t).map_or(f64::INFINITY, |p| h(p).abs());
    for _ in 0..100 {
        let (a, b) = (lo + (hi - lo) / 3.0, hi - (hi - lo) / 3.0);
        if dist(a) < dist(b) {
            hi = b;
        } else {
            lo = a;
        }
    }
    let t = 0.5 * (lo + hi);
    (dist(t) < 1e-7).then(|| at(t).map(|p| frame.to_local(p))).flatten()
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
    let external = |sk: &noshape_sketch::Sketch, p: noshape_sketch::EntityId| sk.entity(p).is_some_and(|e| e.external);
    use noshape_sketch::{Curve2, Geometry};
    let Some(e) = sk.entity(id).cloned() else { return };
    match (&e.geometry, c) {
        // An end a Trim or Split cut stays (it's no longer the edge's: not external); live keeps it where its other
        // constraints hold it.
        (Geometry::Line { start, end }, Curve2::Line { a, b }) => {
            // An edge turned round since the sketch was solved (a chamfer's edge senses) keeps the line's way: offsets
            // and distances measured from its side stay on it (qa's cf-surface-pattern: Sketch 7's square 9.144 mm).
            let was = sk.point(*end).zip(sk.point(*start)).map(|(e, s)| e - s);
            let (a, b) = if was.is_some_and(|w| w.dot(*b - *a) < 0.0) { (b, a) } else { (a, b) };
            for (p, q) in [(*start, *a), (*end, *b)] {
                if external(sk, p) {
                    let _ = sk.set_point(p, q);
                }
            }
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
            for (p, angle) in [(*start, *start_angle), (*end, *end_angle)] {
                if external(sk, p) {
                    let _ = sk.set_point(p, *c2 + glam::DVec2::from_angle(angle) * *radius);
                }
            }
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
                if let Some(ent) = sk.entity_mut(id)
                    && let Geometry::BSpline { degree, knots, weights, .. } = &mut ent.geometry
                {
                    (*degree, *knots, *weights) = (n.degree, n.knots.clone(), n.weights.clone());
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
    /// c2-loft-paths-guides' Sketch 5 (plane x = -50.8 mm): its PIERCE targets are circles of r 50.8 about x = 0 that
    /// only touch the plane: the touching point (it was none: no sample crossed it).
    #[test]
    fn a_circle_touching_the_plane_pierces_it_where_it_touches() {
        let frame = crate::display::PlaneFrame { origin: glam::DVec3::new(-0.0508, 0.0, 0.0), ..crate::display::PlaneFrame::RIGHT };
        // Turned off the samples: its touching point between two of them.
        for turn in [0.0f64, 0.0123] {
            let axis = glam::DVec3::new(0.0, (1.0 - turn * turn).sqrt(), 0.0) + glam::DVec3::new(0.0, 0.0, turn);
            let edge = noshape_kernel::make_circle(glam::DVec3::new(0.0, 0.254, 0.0), axis.normalize(), 0.0508).unwrap();
            let p = super::pierce_point(&edge, &frame).expect("where it touches");
            assert!((frame.to_world(p) - glam::DVec3::new(-0.0508, 0.254, 0.0)).length() < 1e-6, "{p}");
        }
    }

    use super::*;

    #[test]
    fn splits_concatenations_outside_quotes_and_calls() {
        assert_eq!(split_top(r#""a ~ b" ~ #x ~ f(1 ~ 2)"#, '~'), vec![r#""a ~ b" "#, " #x ", " f(1 ~ 2)"]);
        assert_eq!(split_top("roundToPrecision(#l / in, 3)", ','), vec!["roundToPrecision(#l / in, 3)"]);
        assert_eq!(trim_number(2.500, 6), "2.5");
        assert_eq!(trim_number(3.0, 6), "3");
        assert_eq!(call_argument("toString(roundToPrecision(#t / mm, 0))", "toString"), Some("roundToPrecision(#t / mm, 0)"));
        assert_eq!(call_argument("toString(#a) + toString(#b)", "toString"), None);
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
        f.params.set("sketch", ParamValue::Sketch(std::sync::Arc::new(data)));
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
        // (Both lengths and the Equal already over-define it, as in live.)
        assert!(!sk.solve().is_solved());
        sk.set_dimension_value(lb, 0.05).unwrap();
        let mut ps = PartStudio::new();
        let mut f = ps.new_feature("sketch", &units).unwrap();
        f.params.set("sketchPlane", ParamValue::Query(vec![Reference::Plane(DatumPlane::Top)]));
        f.params.set("sketch", ParamValue::Sketch(std::sync::Arc::new(SketchData { sketch: sk, ..Default::default() })));
        ps.features.push(f);
        let r = Regenerator::new().regenerate(&ps, &ConfigValues::default(), &units, &RegenOptions::default());
        let res = &r.context.sketches[0];
        assert!(res.report.conflicting().contains(&lb), "{:?}", res.report.status);
        let len = res.sketch.curve(b).unwrap().length();
        assert!((len - 0.032).abs() < 1e-12, "the last solved shape stays: {len}");
    }

    /// c2-hunting-knife's Sketch 6 (Extrude 3's groove, mirrored about a plane, its arc started on Extrude 1's top
    /// edge): the used edge 0.02 mm higher moves the groove 0.02 mm, not 2.3 mm along the edge (live knife-depth-1125:
    /// arc start (4, 2.02), line end (75, 1.3069), all 52 rows OK).
    #[test]
    fn a_mirrored_profile_follows_its_used_edge_by_the_least_change() {
        use crate::configuration::ConfigValues;
        use crate::partstudio::PartStudio;
        use crate::reference::{DatumPlane, Reference};
        use crate::regen::{RegenOptions, Regenerator};
        use crate::sketch_feature::SketchData;
        use noshape_sketch::{Curve2, DistanceMode, EntityId};
        let units = noshape_units::UnitPrefs::default();
        let m = |x: f64, y: f64| glam::DVec2::new(x * 1e-3, y * 1e-3);
        let mut sk = noshape_sketch::Sketch::new();
        let edge = *sk.use_external(&Curve2::Line { a: m(-5.0, 2.0), b: m(80.0, 2.0) }).unwrap().created.last().unwrap();
        let axis = *sk.use_external(&Curve2::Line { a: m(-5.0, 0.0), b: m(80.0, 0.0) }).unwrap().created.last().unwrap();
        let p0 = sk.add_point(m(4.742366115, 1.9));
        let p1 = sk.add_point(m(75.0, 1.286870918));
        let slope = sk.add_line_between(p0, p1).unwrap();
        let c = sk.add_point(m(4.768545721, 4.899885769));
        let p4 = sk.add_point(m(4.0, 2.0));
        let arc = sk.add_arc_between(c, p4, p0).unwrap();
        let p7 = sk.add_point(m(75.0, 2.0));
        let top = sk.add_line_between(p4, p7).unwrap();
        let end = sk.add_line_between(p7, p1).unwrap();
        let q7 = sk.add_point(m(75.0, -2.0));
        let q1 = sk.add_point(m(75.0, -1.286870918));
        let end2 = sk.add_line_between(q7, q1).unwrap();
        let qc = sk.add_point(m(4.768545721, -4.899885769));
        let q0 = sk.add_point(m(4.742366115, -1.9));
        let q4 = sk.add_point(m(4.0, -2.0));
        let arc2 = sk.add_arc_between(qc, q0, q4).unwrap();
        let slope2 = sk.add_line_between(q0, q1).unwrap();
        let top2 = sk.add_line_between(q4, q7).unwrap();
        for k in [
            ConstraintKind::Horizontal { a: top, b: None },
            ConstraintKind::Vertical { a: end2, b: None },
            ConstraintKind::Tangent { a: arc, b: slope },
            ConstraintKind::Coincident { a: edge, b: p4 },
            ConstraintKind::Vertical { a: p7, b: Some(p1) },
            ConstraintKind::Radius { curve: arc, value: 3e-3 },
            ConstraintKind::Distance { a: edge, b: p0, value: 0.1e-3, mode: DistanceMode::Center },
            ConstraintKind::Angle { a: edge, b: slope, value: 0.5f64.to_radians() },
        ] {
            sk.add_constraint(k).unwrap();
        }
        for (a, b) in [(end, end2), (p7, q7), (p1, q1), (arc, arc2), (p0, q0), (p4, q4), (slope, slope2), (top, top2)] {
            sk.add_constraint(ConstraintKind::Symmetric { a, b, axis }).unwrap();
        }
        assert!(sk.solve().is_solved());
        let mut ps = PartStudio::new();
        let mut plane = ps.new_feature("cPlane", &units).unwrap();
        plane.params.set("entities", ParamValue::Query(vec![Reference::Plane(DatumPlane::Top)]));
        plane.params.set("cplaneType", ParamValue::Enum("OFFSET".into()));
        plane.params.set("offset", ParamValue::Expr(noshape_units::ExprValue::new("2 mm")));
        let plane_id = plane.id;
        ps.features.push(plane);
        let mut f = ps.new_feature("sketch", &units).unwrap();
        f.params.set("sketchPlane", ParamValue::Query(vec![Reference::Plane(DatumPlane::Front)]));
        let external_refs = [(edge, Reference::FeaturePlane(plane_id)), (axis, Reference::Plane(DatumPlane::Top))].map(|(e, r): (EntityId, _)| (e.0, r));
        f.params.set("sketch", ParamValue::Sketch(std::sync::Arc::new(SketchData { sketch: sk, external_refs: external_refs.into(), ..Default::default() })));
        ps.features.push(f);
        let at = |ps: &PartStudio| {
            let r = Regenerator::new().regenerate(ps, &ConfigValues::default(), &units, &RegenOptions::default());
            let res = &r.context.sketches[0];
            assert!(res.report.is_solved(), "{:?}", res.report.status);
            [p4, p1].map(|p| res.sketch.point(p).unwrap() * 1e3)
        };
        let before = at(&ps);
        assert!((before[0] - glam::DVec2::new(4.0, 2.0)).length() < 1e-6, "{before:?}");
        ps.features[0].params.set("offset", ParamValue::Expr(noshape_units::ExprValue::new("2.02 mm")));
        let after = at(&ps);
        assert!((after[0] - glam::DVec2::new(4.0, 2.02)).length() < 1e-3, "the arc's start slid: {after:?}");
        assert!((after[1].y - 1.3069).abs() < 1e-3 && (after[1].x - 75.0).abs() < 0.01, "{after:?}");
    }

    /// Live errors-1118: a triangle with two 100 mm sides and the third typed 300 can't be solved: the row says so
    /// ("Sketch could not be solved.", ours said nothing) and the last shape stays.
    #[test]
    fn a_sketch_whose_solve_fails_says_so() {
        use crate::configuration::ConfigValues;
        use crate::partstudio::PartStudio;
        use crate::reference::{DatumPlane, Reference};
        use crate::regen::{RegenOptions, Regenerator};
        use crate::sketch_feature::SketchData;
        use glam::DVec2;
        let units = noshape_units::UnitPrefs::default();
        let mut sk = noshape_sketch::Sketch::new();
        let p: Vec<_> = [DVec2::new(0.0, 0.0), DVec2::new(0.1, 0.0), DVec2::new(0.05, 0.0866)].iter().map(|q| sk.add_point(*q)).collect();
        let a = sk.add_line_between(p[0], p[1]).unwrap();
        let b = sk.add_line_between(p[1], p[2]).unwrap();
        let c = sk.add_line_between(p[2], p[0]).unwrap();
        sk.add_constraint(ConstraintKind::Length { curve: a, value: 0.1 }).unwrap();
        sk.add_constraint(ConstraintKind::Length { curve: b, value: 0.1 }).unwrap();
        let lc = sk.add_constraint(ConstraintKind::Length { curve: c, value: 0.1 }).unwrap();
        assert!(sk.solve().is_solved());
        let before = sk.curve(c).unwrap().length();
        sk.set_dimension_value(lc, 0.3).unwrap();
        let mut ps = PartStudio::new();
        let mut f = ps.new_feature("sketch", &units).unwrap();
        let id = f.id;
        f.params.set("sketchPlane", ParamValue::Query(vec![Reference::Plane(DatumPlane::Top)]));
        f.params.set("sketch", ParamValue::Sketch(std::sync::Arc::new(SketchData { sketch: sk, ..Default::default() })));
        ps.features.push(f);
        let r = Regenerator::new().regenerate(&ps, &ConfigValues::default(), &units, &RegenOptions::default());
        let st = r.display.feature_state(id).unwrap();
        assert!(st.message.as_deref().is_some_and(|m| m.contains("Sketch could not be solved.")), "{st:?}");
        assert!((r.context.sketches[0].sketch.curve(c).unwrap().length() - before).abs() < 1e-9, "the last shape stays");
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
