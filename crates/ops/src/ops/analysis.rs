//! Part Studio analysis tools (Analysis tools menu): curvature, deviation,
//! connection (continuity), dihedral angle, interference detection. Each
//! runs the tool's computation (`crate::analysis_tools`) on the selection
//! and reports what its dialog shows, in workspace units.

use super::{doc_field, strings, studio_target, tab_field};
use crate::analysis_tools::{AnalysisKind, AnalysisOptions, AnalysisResult, analyze};
use crate::error::OpError;
use crate::ops::query::{len_scale, pt, units_json};
use crate::registry::{Args, Op, OpOutput, opt, req};
use crate::{Ctx, refs};
use noshape_model::analysis::Continuity;
use noshape_model::reference::Reference;
use serde_json::{Value, json};

fn op(verb: &'static str, summary: &'static str, description: &'static str, fields: Vec<crate::registry::Field>, run: crate::registry::Handler) -> Op {
    let mut all = vec![doc_field(), tab_field()];
    all.extend(fields);
    Op { group: "analysis", verb, summary, description, fields: all, mutates: false, destructive: false, run }
}

pub fn ops() -> Vec<Op> {
    vec![
        op(
            "curvature_map",
            "Curvature color map: the parts' surface curvature",
            "The Curvature color map (Analysis tools menu) as numbers: each part's surface sampled at its mesh vertices with the Curvature menu's `kind` — gaussian (1/length², default), mean (1/length), max_radius or min_radius (length) — giving each part's min / max over its curved surface, the share that is flat, and the `points` most extreme spots (largest curvature, smallest radius).",
            vec![
                opt("parts", strings(), "Part ids (`body:…`) or names; default all shown parts."),
                opt("kind", json!({ "type": "string", "enum": ["gaussian", "mean", "max_radius", "min_radius"], "default": "gaussian" }), "Curvature."),
                opt("points", json!({ "type": "integer", "minimum": 0, "default": 10 }), "Extreme spots to list."),
            ],
            curvature_map,
        ),
        op(
            "thickness",
            "Thickness analysis: the parts' wall thickness",
            "The Thickness analysis tool (Analysis tools menu): each part's surface sampled and measured with the color bar's Method — rolling_ball (default: the largest ball touching both walls), ray (straight through the part), or their _gradient forms (the rate of change, %). Returns each part's min / max / mean over its samples and, with `below`, the share of the surface thinner than that and up to `points` of the thinnest spots (location, thickness).",
            vec![
                opt("parts", strings(), "Part ids (`body:…`) or names; default all shown parts."),
                opt("method", json!({ "type": "string", "enum": ["rolling_ball", "rolling_ball_gradient", "ray", "ray_gradient"], "default": "rolling_ball" }), "Method."),
                opt("below", json!({ "type": ["number", "string"] }), "Report surface thinner than this (a length, or % for gradients)."),
                opt("points", json!({ "type": "integer", "minimum": 0, "default": 10 }), "Thinnest spots to list."),
            ],
            thickness,
        ),
        op(
            "curvature",
            "Curvature of edges, sketch curves and faces (Curve/surface analysis)",
            "The minimum radius of curvature over the selection and where it is, the inflection points of sketch curves, and the number of curvature combs (one per curve; U and V isoparametric curves per face).",
            vec![
                req("entities", strings(), "Edges, faces (`edge:…`, `face:…`) or sketch curves (`entity:…`)."),
                opt("extend", json!({ "type": "boolean" }), "Extend across face: include the tangent-connected faces."),
                opt("u_curves", json!({ "type": "integer", "minimum": 1, "maximum": 50 }), "Faces: U curves (default 8)."),
                opt("v_curves", json!({ "type": "integer", "minimum": 1, "maximum": 50 }), "Faces: V curves (default 8)."),
            ],
            curvature,
        ),
        op(
            "deviation",
            "Deviation between two curve chains or two surfaces (Deviation analysis)",
            "Minimum and maximum distance from the first selection to the second, with the points on each.",
            vec![req("first", strings(), "Edges / sketch curves (or faces with `surface`)."), req("second", strings(), "The chain / faces to measure to."), opt("surface", json!({ "type": "boolean" }), "Surface deviation (faces) instead of curves.")],
            deviation,
        ),
        op(
            "connection",
            "Continuity where curves and faces meet (Connection analysis)",
            "Each joint between the curves (G0 position, G1 tangent, G2 curvature, G3, or none) and each shared edge segment between the faces, with Onshape's tolerances (G1 0.3°, G2 2.5 %, G3 5 % unless given).",
            vec![
                req("entities", strings(), "Edges, sketch curves and faces."),
                opt("g1_deg", json!({ "type": "number" }), "G1 angle tolerance (degrees)."),
                opt("g2", json!({ "type": "number" }), "G2 relative tolerance."),
                opt("g3", json!({ "type": "number" }), "G3 relative tolerance."),
            ],
            connection,
        ),
        op("dihedral", "Dihedral angle along edges (Dihedral analysis)", "The minimum and maximum angle between the faces along the edges (degrees).", vec![req("entities", strings(), "Edges.")], dihedral),
        op(
            "interference",
            "Interference detection between parts",
            "Every pair of the parts whose material overlaps, with the overlap volume and its bounding box.",
            vec![opt("parts", strings(), "Parts (`body:…`). Default: every part.")],
            interference,
        ),
        op(
            "draft",
            "Draft analysis: each face's draft angle toward a pull direction",
            "The Draft analysis tool (Analysis tools menu): with mold split direction `pull` (a plane, planar face, sketch, mate connector or straight edge; `flip` reverses it), minimum draft `angle` (default 3 deg) and Show undercut regions (`undercut`, default on), each face of the checked `parts` (default all shown) is classed as Onshape's legend colors it — steep (under the minimum), undercut, side_1 (toward the pull) or side_2 — with its draft angle range (degrees). Faces mixing classes report the share of each.",
            vec![
                req("pull", json!({ "type": "string" }), "Mold split direction: a plane, planar face, sketch, mate connector or straight edge id."),
                opt("flip", json!({ "type": "boolean", "default": false }), "Reverse the pull direction."),
                opt("angle", json!({ "type": ["number", "string"], "default": "3 deg" }), "Minimum draft angle (0–90 deg)."),
                opt("undercut", json!({ "type": "boolean", "default": true }), "Show undercut regions."),
                opt("parts", strings(), "Entities to check: part ids (`body:…`) or names; default all shown parts."),
            ],
            draft,
        ),
        op(
            "flatten",
            "Flatten surfaces: a planar sheet of faces with its distortion, optionally exported",
            "The Flatten surfaces tool (Analysis tools menu): `faces` (contiguous) opened along `rips` (edges; closed faces open along their seams) flattened with minimum strain. Returns the 3D and flat areas, the strain range (negative: compressed, positive: stretched), the flat size and the boundary loops. `export` writes the sheet: DXF or SVG (`inner_edges`: the faces' shared edges too), STL or PARASOLID placed on the XY plane (best fit to the faces' footprint, outer side up) or at mate connector `position` (`angle`, `offset`).",
            vec![
                req("faces", strings(), "Faces to flatten."),
                opt("rips", strings(), "Rip edges."),
                opt("export", json!({ "type": "object" }), "{\"format\": \"DXF\" | \"SVG\" | \"STL\" | \"PARASOLID\", \"file\": path, \"inner_edges\": bool, \"position\": mate connector id, \"angle\": degrees, \"offset\": length, \"origin\": vertex id}"),
            ],
            flatten,
        ),
    ]
}

fn refs_arg(a: &Args, name: &str) -> Result<Vec<Reference>, OpError> {
    a.array(name)?.iter().enumerate().map(|(i, v)| refs::parse(v).map_err(|e| e.with_param(format!("{name}[{i}]")))).collect()
}

/// Run analysis `kind` and pass the result and the length scale on.
fn run(ctx: &mut Ctx, a: &Args, kind: AnalysisKind, sel: Vec<Reference>, sel2: Vec<Reference>, o: AnalysisOptions) -> Result<(AnalysisResult, f64, Value), OpError> {
    let (od, tab) = studio_target(ctx, a)?;
    let units = od.doc().settings.units;
    let r = od.regenerate(tab, None)?;
    Ok((analyze(&r.context, &r.display, kind, &sel, &sel2, &o), len_scale(&units), units_json(&units)))
}

fn continuity(c: Continuity) -> &'static str {
    match c {
        Continuity::None => "none",
        Continuity::G0 => "G0",
        Continuity::G1 => "G1",
        Continuity::G2 => "G2",
        Continuity::G3 => "G3",
    }
}

fn curvature(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let sel = refs_arg(a, "entities")?;
    let mut o = AnalysisOptions { extend: a.bool_or("extend", false)?, ..Default::default() };
    if let Some(n) = a.opt_u64("u_curves")? {
        o.u_curves = n.clamp(1, 50) as usize;
    }
    if let Some(n) = a.opt_u64("v_curves")? {
        o.v_curves = n.clamp(1, 50) as usize;
    }
    let (r, k, units) = run(ctx, a, AnalysisKind::CurveSurface, sel, vec![], o)?;
    if r.combs.is_empty() {
        return Err(OpError::param("entities", "nothing to analyze: give edges, faces or sketch curves"));
    }
    let mut v = json!({ "units": units, "combs": r.combs.len(), "inflections": r.inflections.iter().map(|p| pt(*p, k)).collect::<Vec<_>>() });
    if let Some((radius, at)) = r.min_radius {
        v["min_radius"] = json!({ "radius": radius * k, "at": pt(at, k) });
    }
    Ok(v.into())
}

fn deviation(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (first, second) = (refs_arg(a, "first")?, refs_arg(a, "second")?);
    let o = AnalysisOptions { surface: a.bool_or("surface", false)?, ..Default::default() };
    let (r, k, units) = run(ctx, a, AnalysisKind::Deviation, first, second, o)?;
    let ((dmin, a1, b1), (dmax, a2, b2)) = r.deviation.ok_or_else(|| OpError::invalid("no deviation: the selections don't overlap (curves) or have no tessellation (faces)"))?;
    Ok(json!({ "units": units, "min": { "distance": dmin * k, "on_first": pt(a1, k), "on_second": pt(b1, k) }, "max": { "distance": dmax * k, "on_first": pt(a2, k), "on_second": pt(b2, k) } }).into())
}

fn connection(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let sel = refs_arg(a, "entities")?;
    let mut o = AnalysisOptions::default();
    if let Some(x) = a.opt_f64("g1_deg")? {
        o.tolerances.g1_deg = x;
    }
    if let Some(x) = a.opt_f64("g2")? {
        o.tolerances.g2 = x;
    }
    if let Some(x) = a.opt_f64("g3")? {
        o.tolerances.g3 = x;
    }
    let (r, k, units) = run(ctx, a, AnalysisKind::Connection, sel, vec![], o)?;
    Ok(json!({
        "units": units,
        "joints": r.joints.iter().map(|(p, c)| json!({ "at": pt(*p, k), "continuity": continuity(*c) })).collect::<Vec<_>>(),
        "face_segments": r.faces.segments.iter().map(|(s, c)| json!({ "a": pt(s[0], k), "b": pt(s[1], k), "continuity": continuity(*c) })).collect::<Vec<_>>(),
        "unconnected_boundaries": r.faces.unconnected.len(),
    })
    .into())
}

fn dihedral(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let sel = refs_arg(a, "entities")?;
    let (r, _, _) = run(ctx, a, AnalysisKind::Dihedral, sel, vec![], AnalysisOptions::default())?;
    let (lo, hi) = r.dihedral.ok_or_else(|| OpError::param("entities", "no dihedral angle: give edges between two faces"))?;
    Ok(json!({ "min_deg": lo, "max_deg": hi }).into())
}

fn interference(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let sel = match a.get("parts") {
        Some(_) => refs_arg(a, "parts")?,
        None => {
            let (od, tab) = studio_target(ctx, a)?;
            let r = od.regenerate(tab, None)?;
            r.display.bodies.iter().map(|b| Reference::Body(b.id.clone())).collect()
        }
    };
    if let Some(i) = sel.iter().position(|r| !matches!(r, Reference::Body(_))) {
        return Err(OpError::param(&format!("parts[{i}]"), "a part id (`body:…`)"));
    }
    let (r, k, units) = run(ctx, a, AnalysisKind::Interference, sel, vec![], AnalysisOptions::default())?;
    Ok(json!({
        "units": units,
        "interferences": r.interferences.iter().map(|(i, na, nb)| json!({
            "a": { "id": refs::body_id(&r.interference_parts[i.a]), "name": na },
            "b": { "id": refs::body_id(&r.interference_parts[i.b]), "name": nb },
            "volume": i.volume * k * k * k,
            "bounds": { "min": pt(i.bounds.0, k), "max": pt(i.bounds.1, k) },
        })).collect::<Vec<_>>(),
    })
    .into())
}

fn thickness(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    use noshape_model::thickness::{ThicknessMethod as M, analyze};
    let method = match a.opt_str("method")?.unwrap_or("rolling_ball") {
        "rolling_ball" => M::RollingBall,
        "rolling_ball_gradient" => M::RollingBallGradient,
        "ray" => M::Ray,
        "ray_gradient" => M::RayGradient,
        _ => return Err(OpError::param("method", "rolling_ball, rolling_ball_gradient, ray or ray_gradient")),
    };
    let (od, tab) = studio_target(ctx, a)?;
    let units = od.doc().settings.units;
    let k = len_scale(&units);
    let r = od.regenerate(tab, None)?;
    let parts = shown_parts(&r.display, a)?;
    // Gradients are percentages; thicknesses lengths in workspace units.
    let scale = if method.is_gradient() { 1.0 } else { k };
    let below = match a.get("below") {
        None => None,
        Some(v) if method.is_gradient() => Some(v.as_f64().or_else(|| v.as_str().and_then(|t| t.trim().trim_end_matches('%').trim().parse().ok())).ok_or_else(|| OpError::param("below", "a percentage"))?),
        Some(v) => Some(crate::ops::sketch::length_value(v, &units).map_err(|e| e.with_param("below"))? * k),
    };
    let points = a.opt_u64("points")?.unwrap_or(10) as usize;
    let mut out = Vec::new();
    for b in parts {
        let (mesh, values) = analyze(&b.mesh, method, 25_000);
        let vals: Vec<(usize, f64)> = values.iter().enumerate().filter_map(|(i, v)| v.map(|x| (i, x * scale))).collect();
        let mut j = json!({ "id": refs::body_id(&b.id), "name": b.name, "samples": vals.len() });
        if !vals.is_empty() {
            let (min, max) = vals.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), (_, v)| (lo.min(*v), hi.max(*v)));
            j["min"] = json!(min);
            j["max"] = json!(max);
            j["mean"] = json!(vals.iter().map(|(_, v)| v).sum::<f64>() / vals.len() as f64);
            if let Some(t) = below {
                j["below_share"] = json!(vals.iter().filter(|(_, v)| *v < t).count() as f64 / vals.len() as f64);
            }
            let mut thin = vals.clone();
            thin.sort_by(|x, y| x.1.total_cmp(&y.1));
            j["thinnest"] = json!(thin.iter().filter(|(_, v)| below.is_none_or(|t| *v < t)).take(points).map(|(i, v)| json!({ "at": pt(mesh.positions[*i], k), "value": v })).collect::<Vec<_>>());
        }
        out.push(j);
    }
    Ok(json!({ "units": units_json(&units), "method": a.opt_str("method")?.unwrap_or("rolling_ball"), "values": if method.is_gradient() { "percent" } else { "length" }, "parts": out }).into())
}

/// Shown parts by id or name (`parts`), else all shown solid parts.
fn shown_parts<'a>(display: &'a noshape_model::display::StudioDisplay, a: &Args) -> Result<Vec<&'a noshape_model::display::BodyDisplay>, OpError> {
    let shown: Vec<&noshape_model::display::BodyDisplay> = display.bodies.iter().filter(|b| !b.hidden && b.kind == noshape_model::display::BodyKind::Solid).collect();
    match a.opt_array("parts")? {
        Some(list) => list
            .iter()
            .enumerate()
            .map(|(i, v)| {
                let key = v.as_str().unwrap_or_default();
                shown.iter().copied().find(|b| refs::body_id(&b.id) == key || b.name == key).ok_or_else(|| OpError::param(&format!("parts[{i}]"), "a shown solid part (id or name)"))
            })
            .collect(),
        None => Ok(shown),
    }
}

fn curvature_map(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    use noshape_model::analysis::{CurvatureKind as K, curvature_values};
    let kind = match a.opt_str("kind")?.unwrap_or("gaussian") {
        "gaussian" => K::Gaussian,
        "mean" => K::Mean,
        "max_radius" => K::MaxRadius,
        "min_radius" => K::MinRadius,
        _ => return Err(OpError::param("kind", "gaussian, mean, max_radius or min_radius")),
    };
    let (od, tab) = studio_target(ctx, a)?;
    let units = od.doc().settings.units;
    let k = len_scale(&units);
    let r = od.regenerate(tab, None)?;
    let points = a.opt_u64("points")?.unwrap_or(10) as usize;
    let mut out = Vec::new();
    for b in shown_parts(&r.display, a)? {
        let size = b.bounds.map(|(lo, hi)| (hi - lo).length()).unwrap_or(0.1);
        let values = curvature_values(&b.mesh, size, kind, 1.0 / k);
        let curved: Vec<(usize, f64)> = values.iter().enumerate().filter_map(|(i, v)| v.map(|x| (i, x))).collect();
        let mut j = json!({ "id": refs::body_id(&b.id), "name": b.name, "samples": values.len(), "flat_share": if values.is_empty() { 0.0 } else { 1.0 - curved.len() as f64 / values.len() as f64 } });
        if !curved.is_empty() {
            let (min, max) = curved.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), (_, v)| (lo.min(*v), hi.max(*v)));
            j["min"] = json!(min);
            j["max"] = json!(max);
            let mut ext = curved.clone();
            // The most curved: largest |curvature|, or smallest radius.
            match kind {
                K::MaxRadius | K::MinRadius => ext.sort_by(|x, y| x.1.total_cmp(&y.1)),
                _ => ext.sort_by(|x, y| y.1.abs().total_cmp(&x.1.abs())),
            }
            j["extremes"] = json!(ext.iter().take(points).map(|(i, v)| {
                let p = b.mesh.positions[*i];
                json!({ "at": pt(glam::DVec3::new(p[0] as f64, p[1] as f64, p[2] as f64), k), "value": v })
            }).collect::<Vec<_>>());
        }
        out.push(j);
    }
    Ok(json!({ "units": units_json(&units), "kind": a.opt_str("kind")?.unwrap_or("gaussian"), "parts": out }).into())
}

/// A pull / direction reference: a plane-like reference's normal, or a
/// straight edge's direction.
fn direction_of(r: &noshape_model::regen::RegenResult, rf: &Reference) -> Option<glam::DVec3> {
    if let Some(f) = crate::studio::frame_of_ref(&r.display, Some(&r.context), rf) {
        return Some(f.normal);
    }
    let pts = noshape_model::flatten::edge_polyline(&r.display.bodies, rf);
    let (a, b) = (*pts.first()?, *pts.last()?);
    let d = (b - a).try_normalize()?;
    pts.iter().all(|p| (*p - a).cross(d).length() <= 1e-6 + 1e-4 * a.distance(b)).then_some(d)
}

fn draft(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    use noshape_model::analysis::{DraftClass, draft_classes};
    let (od, tab) = studio_target(ctx, a)?;
    let units = od.doc().settings.units;
    let r = od.regenerate(tab, None)?;
    let pull_ref = refs::parse(a.get("pull").expect("required")).map_err(|e| e.with_param("pull"))?;
    let mut pull = direction_of(&r, &pull_ref).ok_or_else(|| OpError::param("pull", "Select a plane, a planar face or a straight edge"))?;
    if a.bool_or("flip", false)? {
        pull = -pull;
    }
    let angle = match a.get("angle") {
        None => 3f64.to_radians(),
        Some(v) => crate::ops::sketch::angle_value(v, &units).map_err(|e| e.with_param("angle"))?,
    };
    if !(0.0..=std::f64::consts::FRAC_PI_2).contains(&angle) {
        return Err(OpError::param("angle", "0 to 90 deg"));
    }
    let undercut = a.bool_or("undercut", true)?;
    let names = [(DraftClass::Steep, "steep"), (DraftClass::Undercut, "undercut"), (DraftClass::Side1, "side_1"), (DraftClass::Side2, "side_2")];
    let mut parts = Vec::new();
    for b in shown_parts(&r.display, a)? {
        let size = b.bounds.map(|(lo, hi)| (hi - lo).length()).unwrap_or(0.1);
        let v = draft_classes(&b.mesh, size, pull, angle, undercut);
        let mut faces = Vec::new();
        for (fi, fr) in b.face_refs.iter().enumerate() {
            let Some(&(first, count)) = b.mesh.face_ranges.get(fi) else { continue };
            let idx = b.mesh.indices.get(first as usize..(first + count) as usize).unwrap_or(&[]);
            if idx.is_empty() {
                continue;
            }
            let mut counts = [0usize; 4];
            let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
            for &i in idx {
                let Some(&(ang, c)) = v.get(i as usize) else { continue };
                counts[names.iter().position(|n| n.0 == c).expect("class")] += 1;
                lo = lo.min(ang);
                hi = hi.max(ang);
            }
            let total = counts.iter().sum::<usize>().max(1) as f64;
            let main = (0..4).max_by_key(|k| counts[*k]).expect("four classes");
            let mut j = json!({ "face": refs::to_id(fr), "class": names[main].1, "draft_deg": [ (lo.to_degrees() * 1e3).round() / 1e3, (hi.to_degrees() * 1e3).round() / 1e3 ] });
            if counts.iter().filter(|c| **c > 0).count() > 1 {
                j["shares"] = names.iter().zip(counts).filter(|(_, c)| *c > 0).map(|(n, c)| (n.1.to_string(), json!(((c as f64 / total) * 1e3).round() / 1e3))).collect::<serde_json::Map<String, Value>>().into();
            }
            faces.push(j);
        }
        parts.push(json!({ "id": refs::body_id(&b.id), "name": b.name, "faces": faces }));
    }
    Ok(json!({ "pull": [pull.x, pull.y, pull.z], "parts": parts }).into())
}

fn flatten(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    use noshape_model::flatten::{SheetPlacement, flat_sheet, flatten_faces, placed_sheet};
    let (od, tab) = studio_target(ctx, a)?;
    let units = od.doc().settings.units;
    let k = len_scale(&units);
    let r = od.regenerate(tab, None)?;
    let faces = refs_arg(a, "faces")?;
    for (i, f) in faces.iter().enumerate() {
        if !matches!(f, Reference::Face(_)) || !r.display.bodies.iter().any(|b| b.face_refs.contains(f)) {
            return Err(OpError::param(&format!("faces[{i}]"), "a face of a shown part"));
        }
    }
    if faces.is_empty() {
        return Err(OpError::param("faces", "Select faces to flatten"));
    }
    let rips = match a.get("rips") {
        Some(_) => refs_arg(a, "rips")?,
        None => Vec::new(),
    };
    for (i, e) in rips.iter().enumerate() {
        if noshape_model::flatten::edge_polyline(&r.display.bodies, e).is_empty() {
            return Err(OpError::param(&format!("rips[{i}]"), "an edge of a shown part"));
        }
    }
    let f = flatten_faces(&r.display.bodies, &faces, &rips, Some(&r.context));
    let (a3, a2) = f.areas();
    let (smin, smax) = f.strain.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), s| (lo.min(*s), hi.max(*s)));
    let (lo, hi) = f.flat.iter().fold((glam::DVec2::splat(f64::INFINITY), glam::DVec2::splat(f64::NEG_INFINITY)), |(lo, hi), q| (lo.min(*q), hi.max(*q)));
    let mut out = json!({
        "units": units_json(&units),
        "area_3d": a3 * k * k,
        "area_flat": a2 * k * k,
        "strain": [if smin.is_finite() { smin } else { 0.0 }, if smax.is_finite() { smax } else { 0.0 }],
        "flat_size": [(hi.x - lo.x) * k, (hi.y - lo.y) * k],
        "loops": f.loops(false).iter().map(|(_, l)| l.len()).sum::<usize>(),
    });
    if let Some(e) = a.object("export")? {
        let err = |k: &str, m: &str| OpError::param(&format!("export.{k}"), m);
        let format = e.get("format").and_then(Value::as_str).unwrap_or("DXF").to_ascii_uppercase();
        let file = e.get("file").and_then(Value::as_str).ok_or_else(|| err("file", "a path"))?;
        let inner = e.get("inner_edges").and_then(Value::as_bool).unwrap_or(false);
        let connector = match e.get("position").and_then(Value::as_str) {
            Some(id) => {
                let rf = refs::parse_str(id).ok_or_else(|| err("position", "a mate connector id"))?;
                Some(r.display.connectors.iter().find(|c| c.reference == rf).map(|c| (c.origin, c.x, c.z)).ok_or_else(|| err("position", "a mate connector of this Part Studio"))?)
            }
            None => None,
        };
        let origin_vertex = match e.get("origin").and_then(Value::as_str) {
            Some(id) => {
                let rf = refs::parse_str(id).ok_or_else(|| err("origin", "a vertex id"))?;
                Some(r.display.bodies.iter().find_map(|b| b.vertex_refs.iter().position(|x| *x == rf).and_then(|i| b.mesh.vertices.get(i)).map(|p| glam::DVec3::new(p[0] as f64, p[1] as f64, p[2] as f64))).ok_or_else(|| err("origin", "a vertex of a shown part"))?)
            }
            None => None,
        };
        let angle = match e.get("angle") {
            Some(v) => v.as_f64().ok_or_else(|| err("angle", "degrees"))?.to_radians(),
            None => 0.0,
        };
        let offset = match e.get("offset") {
            Some(v) => crate::ops::sketch::length_value(v, &units).map_err(|x| x.with_param("export.offset"))?,
            None => 0.0,
        };
        let placement = SheetPlacement { connector, angle, offset, origin_vertex };
        let io = |r: std::io::Result<()>| r.map_err(|x| OpError::new("io", format!("{file}: {x}")).with_param("export.file"));
        match format.as_str() {
            "DXF" => io(std::fs::write(file, f.to_dxf(inner)))?,
            "SVG" => io(std::fs::write(file, f.to_svg(inner)))?,
            "STL" => io(std::fs::write(file, f.to_stl(&placed_sheet(&f, &placement))))?,
            "PARASOLID" => {
                let sheet = flat_sheet(&f, &placed_sheet(&f, &placement), inner).map_err(|x| OpError::new("export_failed", x))?;
                noshape_kernel::write_parasolid(std::path::Path::new(file), &[sheet], noshape_kernel::LengthUnit::Meter, false).map_err(|x| OpError::new("export_failed", x.to_string()))?;
            }
            _ => return Err(err("format", "DXF, SVG, STL or PARASOLID")),
        }
        out["file"] = json!(file);
    }
    Ok(out.into())
}
