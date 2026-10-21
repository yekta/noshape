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
