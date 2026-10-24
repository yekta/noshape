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
