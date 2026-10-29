//! Standard content parts: regeneration of a spec's Part Studio. The specs and the catalogue are in
//! `noshape_model_data::assembly::standard` (re-exported here).

pub use noshape_model_data::assembly::standard::*;
use crate::configuration::ConfigValues;
use crate::custom::{CustomFeatureProvider, DocumentScripts};
use crate::document::{Document, TabContent};
use crate::params::ParamValue;
use crate::partstudio::PartStudio;
use crate::reference::{BodyId, FeatureId};
use crate::regen::{RegenOptions, RegenResult, Regenerator};
use noshape_units::{ExprValue, UnitPrefs};
use std::sync::{Arc, LazyLock};

/// The part of a regenerated standard part (its only body).
pub fn part_body(r: &RegenResult) -> Option<BodyId> {
    match r.context.bodies.as_slice() {
        [b] => Some(b.id.clone()),
        _ => None,
    }
}

/// The built-in Feature Studio, compiled once (registers the feature types).
pub fn scripts() -> Arc<DocumentScripts> {
    static SCRIPTS: LazyLock<Arc<DocumentScripts>> = LazyLock::new(|| {
        let mut doc = Document::new();
        doc.tabs.clear();
        doc.settings.tab_order.clear();
        let source = test_source().unwrap_or_else(|| SOURCE.to_string());
        let id = doc.add_tab("Standard content", TabContent::FeatureStudio(crate::custom::FeatureStudio::new(&source)));
        if let Some(t) = doc.tab_mut(id) {
            t.id = STANDARD_TAB;
        }
        DocumentScripts::from_document(&doc)
    });
    SCRIPTS.clone()
}

/// Tests only: the script from the file `NOSHAPE_STD_FS` names, so
/// fasteners.fs can be iterated on without rebuilding.
#[cfg(test)]
fn test_source() -> Option<String> {
    std::env::var("NOSHAPE_STD_FS").ok().and_then(|p| std::fs::read_to_string(p).ok())
}

#[cfg(not(test))]
fn test_source() -> Option<String> {
    None
}

/// Regenerate a standard part (the error says why it failed).
pub fn regenerate(spec: &StandardSpec) -> Result<RegenResult, String> {
    let scripts = scripts();
    if let Some(e) = scripts.studios().find_map(|(_, _, c)| c.first_error().map(|d| d.to_string())) {
        return Err(format!("Standard content script: {e}"));
    }
    let ps = part_studio(spec).ok_or("Unknown standard content item")?;
    let provider: Arc<dyn CustomFeatureProvider> = scripts;
    let opts = RegenOptions { custom_features: Some(provider), ..Default::default() };
    let r = Regenerator::new().regenerate(&ps, &ConfigValues::default(), &UnitPrefs::default(), &opts);
    if part_body(&r).is_none() {
        let why: Vec<String> = r.display.features.iter().filter_map(|f| f.message.clone()).collect();
        let bodies: Vec<String> = r.context.bodies.iter().map(|b| format!("{:?}", b.id)).collect();
        let notices: Vec<String> = crate::custom::run::outputs().values().flat_map(|o| o.notices.iter().map(|n| format!("{} {}: {}", n.module, n.pos, n.message))).collect();
        return Err(format!("{} was not generated: {} (bodies: {}; notices: {})", spec.part_name(), why.join("; "), bodies.join(", "), notices.join(" | ")));
    }
    Ok(r)
}

/// Script generating the standard parts. Every part has its mate connector
/// origin at the Part Studio origin with the head (or the part) above it.
pub const SOURCE: &str = include_str!("fasteners.fs");

/// Whether a picked entity is a planar face (the Insert closest / furthest
/// buttons want planar faces with holes in them).
pub fn planar_face(ctx: &Arc<crate::regen::context::Context>, body: Option<&BodyId>, on: &crate::reference::Reference) -> bool {
    use noshape_kernel::{ShapeType, Surface};
    let crate::reference::Reference::Face(n) = on else { return false };
    ctx.bodies.iter().filter(|b| body.is_none_or(|id| &b.id == id)).any(|b| {
        b.find_faces(n).first().is_some_and(|&fi| matches!(b.shape.sub_shape(ShapeType::Face, fi).ok().and_then(|f| f.face_surface().ok()), Some(Surface::Plane(_))))
    })
}

/// Holes to fasten standard content to, from a picked entity (studio
/// coordinates): a circular edge or cylindrical face gives one hole (the
/// end whose face points most along `toward`, the viewer), a planar face
/// every circular hole in it. Each hole: its connector (center of the
/// circle, primary axis out of the face around it) and diameter.
pub fn hole_connectors(ctx: &Arc<crate::regen::context::Context>, body: Option<&BodyId>, on: &crate::reference::Reference, toward: glam::DVec3, units: &UnitPrefs) -> Vec<(super::eval::InferencePoint, f64)> {
    use crate::reference::{Inference, Reference};
    use noshape_kernel::{Curve, ShapeType, Surface};
    let Some(b) = ctx.bodies.iter().find(|b| {
        body.is_none_or(|id| &b.id == id)
            && match on {
                Reference::Face(n) => !b.find_faces(n).is_empty(),
                Reference::Edge(n) => !b.find_edges(n).is_empty(),
                _ => false,
            }
    }) else {
        return Vec::new();
    };
    let Ok(edge_faces) = b.shape.edge_faces() else { return Vec::new() };
    let surface = |fi: u32| b.shape.sub_shape(ShapeType::Face, fi as usize).ok().and_then(|f| f.face_surface().ok());
    let planar = |fi: u32| matches!(surface(fi), Some(Surface::Plane(_)));
    let circle = |ei: usize| match b.shape.sub_shape(ShapeType::Edge, ei).ok().and_then(|e| e.edge_info().ok()).map(|i| i.curve) {
        Some(Curve::Circle { radius, .. }) => Some(radius),
        _ => None,
    };
    let cylinder = |fi: u32| matches!(surface(fi), Some(Surface::Cylinder { .. }));
    let circles_of = |fi: u32| edge_faces.iter().enumerate().filter(move |(_, f)| f.contains(&fi)).map(|(e, _)| e).filter(|e| circle(*e).is_some()).collect::<Vec<_>>();
    let other = |ei: usize, fi: u32| edge_faces[ei].iter().copied().find(|f| *f != fi);
    // A hole's end at circular edge `ei` of cylinder `cyl`: the edge and the
    // planar face around it, through a fillet or chamfer ring if there is one.
    let end = |ei: usize, cyl: u32| -> Option<(usize, u32)> {
        let g = other(ei, cyl)?;
        if planar(g) {
            return Some((ei, g));
        }
        circles_of(g).into_iter().filter(|e| *e != ei).find_map(|e| other(e, g).filter(|h| planar(*h)).map(|h| (e, h)))
    };
    // The cylinder behind a ring face (fillet or chamfer around a hole).
    let ring_cylinder = |g: u32| circles_of(g).into_iter().find_map(|e| other(e, g).filter(|c| cylinder(*c)).map(|c| (e, c)));
    // (edge, planar face around it, diameter)
    let mut holes: Vec<(usize, u32, f64)> = Vec::new();
    // Planar face: every hole in it; edge or cylinder: one end of one hole.
    let mut single = true;
    let cyl_ends = |cyl: u32, holes: &mut Vec<(usize, u32, f64)>| {
        for e in circles_of(cyl) {
            if let (Some((edge, face)), Some(r)) = (end(e, cyl), circle(e)) {
                holes.push((edge, face, 2.0 * r));
            }
        }
    };
    match on {
        Reference::Edge(n) => {
            if let Some(&ei) = b.find_edges(n).first()
                && circle(ei).is_some()
            {
                let faces = &edge_faces[ei];
                if let Some(&c) = faces.iter().find(|f| cylinder(**f)) {
                    cyl_ends(c, &mut holes);
                } else if let Some((_, c)) = faces.iter().find_map(|f| ring_cylinder(*f)) {
                    cyl_ends(c, &mut holes);
                }
            }
        }
        Reference::Face(n) => {
            let Some(&fi) = b.find_faces(n).first() else { return Vec::new() };
            let fi = fi as u32;
            if cylinder(fi) {
                cyl_ends(fi, &mut holes);
            } else if planar(fi) {
                single = false;
                for e in circles_of(fi) {
                    let Some(g) = other(e, fi) else { continue };
                    let d = if cylinder(g) { circle(e) } else { ring_cylinder(g).and_then(|(ce, _)| circle(ce)) };
                    if let Some(r) = d {
                        holes.push((e, fi, 2.0 * r));
                    }
                }
            } else if let Some((_, c)) = ring_cylinder(fi) {
                cyl_ends(c, &mut holes);
            }
        }
        _ => {}
    }
    let mut out: Vec<(super::eval::InferencePoint, f64)> = Vec::new();
    for (ei, fi, d) in holes {
        let (Some(en), Some(fname)) = (b.edges.get(ei), b.faces.get(fi as usize)) else { continue };
        let reference = Reference::Implicit { on: Box::new(Reference::Edge((**en).clone())), inference: Inference::Center };
        let align = Reference::Face((**fname).clone());
        if let Some(frame) = super::eval::resolve_connector_aligned(ctx, body, &reference, Some(&align), units) {
            out.push((super::eval::InferencePoint { reference, align: Some(align), frame }, d));
        }
    }
    if single && out.len() > 1 {
        let best = out.iter().enumerate().max_by(|a, c| a.1.0.frame.z().dot(toward).total_cmp(&c.1.0.frame.z().dot(toward))).map(|(i, _)| i).unwrap_or(0);
        out = vec![out.swap_remove(best)];
    }
    out
}

fn mm(v: f64) -> ParamValue {
    ParamValue::Expr(ExprValue::new(format!("{} mm", (v * 1e6).round() / 1e6)))
}

/// The one-feature Part Studio generating a standard part (plus its mate
/// connectors). `None` for an unknown spec.
pub fn part_studio(spec: &StandardSpec) -> Option<PartStudio> {
    let c = component(spec)?;
    let s = spec.size_row()?;
    let u = c.unit();
    let units = UnitPrefs::default();
    scripts();
    let mut ps = PartStudio::new();
    let mut f = ps.new_feature(&crate::custom::type_id(STANDARD_TAB, c.shape.feature()), &units)?;
    f.id = PART_FEATURE;
    f.name = spec.part_name();
    // The forms (integers and reals), then what the chosen options change.
    let mut form = c.form.clone();
    for p in &c.params {
        if let Some(over) = p.forms.get(spec.param(&p.label)) {
            form.extend(over.iter().map(|(k, v)| (k.clone(), *v)));
        }
    }
    for (k, v) in &form {
        f.params.set(k, ParamValue::Expr(ExprValue::new(format!("{v}"))));
    }
    if !s.dims.contains_key("d") && matches!(c.shape, Shape::Screw | Shape::Nut | Shape::Pin) {
        f.params.set("d", mm(s.d * u));
    }
    // Plain dimensions, then those of the chosen options ("sd@Head
    // type=Type I": the recess of a Type I head), which take precedence.
    for (k, v) in s.dims.iter().filter(|(k, _)| !k.contains('@')) {
        f.params.set(k, mm(v * u));
    }
    let mut chosen = s.dims.clone();
    for (k, v) in &s.dims {
        if let Some((name, cond)) = k.split_once('@')
            && let Some((param, option)) = cond.split_once('=')
            && spec.param(param) == option
        {
            f.params.set(name, mm(v * u));
            chosen.insert(name.to_string(), *v);
        }
    }
    for (k, v) in &s.counts {
        f.params.set(k, ParamValue::Expr(ExprValue::new(format!("{v}"))));
    }
    // Dimensions of the chosen length (a blind standoff's hole depth).
    if let Some(over) = spec.length.as_ref().and_then(|l| s.length_dims.get(l)) {
        for (k, v) in over {
            f.params.set(k, mm(v * u));
            chosen.insert(k.clone(), *v);
        }
    }
    // Hex sockets' drill point (live: ISO 140, inch 120 degrees; a form's
    // own angle wins: ASME socket countersunk 140).
    if c.shape == Shape::Screw && !form.contains_key("drill") {
        f.params.set("drill", ParamValue::Expr(ExprValue::new(if c.inch { "120" } else { "140" })));
    }
    if c.shape.has_length()
        && let Some(len) = spec.length.as_deref().and_then(|l| c.length_m(l))
    {
        f.params.set("l", mm(len * 1e3));
        // The chosen Thread length (where the shank face splits; live's
        // askew head bolt).
        if c.shape == Shape::Screw
            && let Some(lt) = spec.length.as_deref().map(|l| c.thread_lengths(s, l)).and_then(|t| spec.params.get(THREAD_LENGTH).filter(|v| t.contains(*v)).cloned().or_else(|| t.first().cloned()))
            && let Some(lt) = c.length_m(&lt)
        {
            f.params.set("lt", mm(lt * 1e3));
        }
    }
    ps.features.push(f);
    // Explicit mate connectors with the primary axis out of the contact
    // face (mates oppose the Z axes): under the head pointing down the shank
    // (countersunk: flush with the top), nuts and washers on both faces.
    let mut tops = vec![0.0];
    // Retaining rings (extruded symmetric about z = 0): live's one "Near"
    // connector, mid-plane.
    if !c.shape.has_length() && c.shape != Shape::RetainingRing && c.near_only.is_none() {
        // The chosen option's thickness (a washer's Outside diameter).
        tops.push(c.shape.height_key().iter().find_map(|k| chosen.get(*k).copied()).unwrap_or(0.0) * u);
    }
    for (i, z) in tops.into_iter().enumerate() {
        let mut m = ps.new_feature("mateConnector", &units)?;
        m.id = FeatureId(PART_FEATURE.0 + 1 + i as u64);
        m.params.set("originQuery", ParamValue::Query(vec![crate::reference::Reference::Origin]));
        if i == 0 && c.near_only != Some(false) {
            m.params.set("flipPrimary", ParamValue::Bool(true));
        }
        if z != 0.0 {
            m.params.set("transform", ParamValue::Bool(true));
            m.params.set("translationZ", mm(z));
        }
        ps.features.push(m);
    }
    Some(ps)
}
