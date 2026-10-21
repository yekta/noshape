//! Import features (Derived, Derived mirror, Import of CAD files) and the
//! not-yet-supported Decal (Wrap lives in `surfacing_c`).

use super::bshared::new_face_names;
use crate::display::BodyKind;
use crate::regen::eval::{EvalCtx, FResult, FeatureError, NamedShape, err};
use super::sheetmetal_form::frame_matrix;
use glam::{DMat4, DVec3};
use noshape_kernel as k;
use noshape_kernel::ShapeType;

// TODO(app): Derived needs another Part Studio's regenerated context. The app
// should resolve `partStudio` and hand the imported bodies to regeneration.
/// Prefix of a Derived feature's source configuration parameters.
pub const DERIVED_CONFIG_PREFIX: &str = "configuration:";

/// A Derived feature's source configuration (`configuration:<id>` texts).
pub fn derived_configuration(params: &crate::params::ParamValues) -> crate::configuration::ConfigValues {
    let mut v = crate::configuration::ConfigValues::default();
    for (k, p) in params.0.iter() {
        if let (Some(id), crate::params::ParamValue::Text(t) | crate::params::ParamValue::Enum(t)) = (k.strip_prefix(DERIVED_CONFIG_PREFIX), p) {
            v.0.insert(id.to_string(), t.clone());
        }
    }
    v
}

pub fn import_derived(ev: &mut EvalCtx) -> FResult<()> {
    let tab = ev.params.text("partStudio").trim().to_string();
    if tab.is_empty() {
        return err("Select a Part Studio to derive from");
    }
    let Some(provider) = ev.derived.clone() else {
        return err("Derived parts are not available in this context");
    };
    // A configured source: its configuration in `configuration:<input id>`
    // parameters (the dialog's inputs, or propagated from this studio's own
    // inputs by Input propagation).
    let config = derived_configuration(ev.params);
    // Live (psb 10-20 l): a second Derived of the same source in the same
    // configuration fails (configurations compared with the source's
    // defaults filled in).
    let effective = |c: &crate::configuration::ConfigValues| provider.configuration(&tab).map(|sc| sc.effective(c)).unwrap_or_else(|| c.clone());
    let mine = effective(&config);
    let earlier = ev.features.iter().take_while(|f| f.id != ev.feature);
    if earlier.filter(|f| f.type_id == "importDerived" && !f.suppressed && f.params.text("partStudio").trim() == tab).any(|f| effective(&derived_configuration(&f.params)) == mine) {
        return err("Creating multiple Derived features from the same source and configuration is not allowed. Consider editing the previous Derived feature.");
    }
    let src = if config.is_empty() { provider.context(&tab) } else { provider.context_configured(&tab, &config) };
    let Some(src) = src else {
        return err("The source Part Studio could not be regenerated (deleted, or a circular reference)");
    };
    // Selected source parts (BodyIds of the source studio); empty = all parts.
    let wanted: Vec<crate::reference::BodyId> = ev
        .query("parts")
        .iter()
        .filter_map(|r| match r {
            crate::reference::Reference::Body(b) => Some(b.clone()),
            _ => None,
        })
        .collect();
    // Placement: each location (mate connector or point) receives an
    // instance with the source origin, or the base mate connector, on it.
    let mut frames: Vec<DMat4> = Vec::new();
    for r in ev.query("location") {
        let f = match ev.resolve_connector(r) {
            Some(c) => frame_matrix(c.origin, c.x, c.z),
            None => match ev.resolve_vertex_point(r) {
                Some(p) => DMat4::from_translation(p),
                None => return err("A selected location no longer exists"),
            },
        };
        frames.push(f);
    }
    if frames.is_empty() {
        frames.push(DMat4::IDENTITY);
    }
    let base = if ev.is("placement", "AT_MATE_CONNECTOR") {
        let id = ev.params.text("mateConnectorId").trim().to_string();
        let found = if id.is_empty() {
            let i = ev.integer("mateConnectorIndex").unwrap_or(0).max(0) as usize;
            src.connectors.get(i)
        } else {
            src.connectors.iter().find(|c| c.name == id)
        };
        match found {
            Some(c) => frame_matrix(c.origin, c.x, c.z),
            None if src.connectors.is_empty() => return err("The source Part Studio has no mate connectors to place by"),
            None => return err(format!("The source Part Studio has no mate connector named \"{id}\"")),
        }
    } else {
        DMat4::IDENTITY
    };
    let inv = base.inverse();
    let mut n = 0u32;
    for (inst, f) in frames.iter().enumerate() {
        let m = *f * inv;
        let moved = !m.abs_diff_eq(DMat4::IDENTITY, 1e-15);
        for b in &src.bodies {
            if !wanted.is_empty() && !wanted.contains(&b.id) {
                continue;
            }
            // Keep the source's persistent names so references into derived
            // geometry follow the source's edits (further instances are copies).
            let id = crate::reference::BodyId { feature: ev.feature, index: n };
            n += 1;
            let color = ev.ctx.alloc_color();
            let shape = if moved { b.shape.transformed(&k::Transform::Matrix(m))? } else { b.shape.clone() };
            let faces = if inst == 0 {
                b.faces.clone()
            } else {
                b.faces.iter().map(|f| std::sync::Arc::new(crate::reference::TopoName::face(ev.feature, crate::reference::Role::Copy { instance: inst as u32, of: f.clone() }))).collect()
            };
            let name = if inst == 0 { b.name.clone() } else { format!("{} ({})", b.name, inst + 1) };
            let body = crate::regen::context::Body::new(id, name, b.kind, shape, faces, color, ev.feature);
            ev.ctx.bodies.push(std::sync::Arc::new(body));
        }
        if ev.bool("includeMateConnectors") {
            for c in &src.connectors {
                let mut c = c.clone();
                if let crate::reference::Reference::MateConnector { index, .. } = c.reference {
                    c.reference = crate::reference::Reference::MateConnector { feature: ev.feature, index: index + inst as u32 * 10_000 };
                }
                c.origin = m.transform_point3(c.origin);
                c.x = m.transform_vector3(c.x).normalize_or_zero();
                c.z = m.transform_vector3(c.z).normalize_or_zero();
                ev.ctx.connectors.push(c);
            }
        }
    }
    if n == 0 {
        return err("The selected parts no longer exist in the source Part Studio");
    }
    Ok(())
}

pub fn derived_mirror(_ev: &mut EvalCtx) -> FResult<()> {
    err("Derived mirror is resolved by the application from the source Part Studio; it can't be evaluated here yet")
}

/// Import of a CAD file. `blobData` holds either inline BRep text or an
/// absolute path to a `.step/.stp/.iges/.igs/.brep` file stored with the document.
pub fn import_foreign(ev: &mut EvalCtx) -> FResult<()> {
    let data = ev.params.text("blobData").trim().to_string();
    if data.is_empty() {
        return err("The imported file's data is missing from this document");
    }
    let shapes: Vec<k::Shape> = if data.starts_with("DBRep_DrawableShape") || data.starts_with("CASCADE Topology") {
        vec![k::Shape::from_brep_bytes(data.as_bytes())?]
    } else {
        let path = std::path::Path::new(&data);
        if !path.is_file() {
            return err(format!("The imported file is not available: {data}"));
        }
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
        match ext.as_str() {
            "step" | "stp" => k::read_step(path, k::LengthUnit::Meter)?.into_iter().map(|s| s.shape).collect(),
            "iges" | "igs" => vec![k::read_iges(path, k::LengthUnit::Meter)?],
            "brep" | "brp" => vec![k::read_brep(path)?],
            "pvz" => vec![k::read_pvz(path, k::LengthUnit::Meter)?],
            other => return err(format!("Unsupported import format .{other}")),
        }
    };
    // The file's parts with what it says about each (names, colours).
    let info: Vec<crate::features::importing::ImportPartInfo> = serde_json::from_str(ev.params.text("partInfo")).unwrap_or_default();
    let shapes: Vec<(k::Shape, Option<crate::features::importing::ImportPartInfo>)> = match shapes.as_slice() {
        [s] if !info.is_empty() && s.shape_type() == ShapeType::Compound && s.children().len() == info.len() => s.children().into_iter().zip(info.into_iter().map(Some)).collect(),
        _ => shapes.into_iter().map(|s| (s, None)).collect(),
    };
    let appearances = ev.bool("importAppearances");
    let up = ev.bool("yAxisIsUp");
    let mut tools = Vec::new();
    // Per tool: its name, colour and faces' colours (sub-shapes of it).
    let mut looks: Vec<Look> = Vec::new();
    // Wire bodies, each with the number of tools before it in the file (a
    // native assembly's thread curves come before their part, live G41).
    let mut wires: Vec<(usize, k::Shape, Look)> = Vec::new();
    // Mesh imports (STL, OBJ, PVZ; Onshape G22/G27): per part, its closed
    // pieces make a mesh part; the open pieces of a part whose mesh doesn't
    // close become surfaces grouped in a closed composite part.
    let mut open_groups: Vec<Vec<k::Shape>> = Vec::new();
    for (s, part) in shapes {
        let s = if up { s.transformed(&k::Transform::Rotate { origin: DVec3::ZERO, axis: DVec3::X, angle: std::f64::consts::FRAC_PI_2 })? } else { s };
        let part = part.unwrap_or_default();
        let colored_faces: Vec<(k::Shape, [f32; 3])> = part.faces.iter().filter_map(|&(i, c)| Some((s.sub_shape(ShapeType::Face, i).ok()?, c))).collect();
        if s.is_mesh() {
            let children = s.children();
            let parts = if s.shape_type() == ShapeType::Compound && !children.is_empty() && children.iter().all(|c| c.shape_type() == ShapeType::Compound) { children } else { vec![s] };
            for part in parts {
                let solids = part.solids();
                looks.extend(solids.iter().map(|_| Look::default()));
                tools.extend(solids);
                let open: Vec<k::Shape> = part.children().into_iter().filter(|c| c.shape_type() == ShapeType::Shell).collect();
                if !open.is_empty() {
                    open_groups.push(open);
                }
            }
            continue;
        }
        // One body per solid, else per shell / face set.
        let parts = if s.count(ShapeType::Solid) > 0 { s.solids() } else if s.count(ShapeType::Shell) > 0 { s.shells() } else { vec![s] };
        for p in parts {
            if p.count(ShapeType::Face) == 0 {
                // Wire bodies come in as curves ("Curve 1", live G38/G38d).
                if p.count(ShapeType::Edge) > 0 {
                    wires.push((tools.len(), p, Look { name: part.name.clone(), color: part.color, opacity: part.opacity, faces: Vec::new() }));
                }
                continue;
            }
            let faces = colored_faces.iter().filter(|(f, _)| p.index_of(f, ShapeType::Face).is_some()).cloned().collect();
            looks.push(Look { name: part.name.clone(), color: part.color, opacity: part.opacity, faces });
            tools.push(p);
        }
    }
    if tools.is_empty() && open_groups.is_empty() && wires.is_empty() {
        return err("The imported file contains no parts or surfaces");
    }
    // "Join adjacent surfaces": sew the surface bodies along shared edges
    // (closed results become solids).
    if ev.bool("joinAdjacentSurfaces") {
        let (solids, surfaces): (Vec<Tool>, Vec<Tool>) = tools.into_iter().zip(std::mem::take(&mut looks)).partition(|t| t.0.count(ShapeType::Solid) > 0);
        (tools, looks) = solids.into_iter().unzip();
        let surfaces: Vec<k::Shape> = surfaces.into_iter().map(|t| t.0).collect();
        if surfaces.len() > 1 {
            let sewn = k::sew(&surfaces, 1e-6, true)?.shape;
            let joined = if sewn.count(ShapeType::Solid) > 0 { sewn.solids() } else if sewn.count(ShapeType::Shell) > 0 { sewn.shells() } else { vec![sewn] };
            looks.extend(joined.iter().map(|_| Look::default()));
            tools.extend(joined);
        } else {
            looks.extend(surfaces.iter().map(|_| Look::default()));
            tools.extend(surfaces);
        }
    }
    // Separate bodies even if they touch (imports are never merged), each
    // with its own body index.
    let mut next = 0u32;
    let mut add = |ev: &mut EvalCtx, p: k::Shape, kind: BodyKind| -> FResult<()> {
        let names = new_face_names(ev, &p);
        let before = ev.ctx.bodies.len();
        ev.add_new_bodies(NamedShape::new(p, names), kind, next)?;
        next += 1;
        if ev.ctx.bodies.len() == before {
            return Err(FeatureError::new("Import failed"));
        }
        Ok(())
    };
    // In the file's order: each wire before the tool that followed it.
    let mut wires = wires.into_iter().peekable();
    for (ti, (p, look)) in tools.into_iter().zip(looks).enumerate() {
        while let Some((_, w, wl)) = wires.next_if(|(at, _, _)| *at <= ti) {
            let before = ev.ctx.bodies.len();
            add(ev, w, BodyKind::Wire)?;
            for bi in before..ev.ctx.bodies.len() {
                apply_look(ev, bi, &wl, appearances);
            }
        }
        let kind = if p.count(ShapeType::Solid) > 0 { BodyKind::Solid } else { BodyKind::Surface };
        let before = ev.ctx.bodies.len();
        add(ev, p, kind)?;
        for bi in before..ev.ctx.bodies.len() {
            apply_look(ev, bi, &look, appearances);
        }
    }
    for (_, p, look) in wires {
        let before = ev.ctx.bodies.len();
        add(ev, p, BodyKind::Wire)?;
        for bi in before..ev.ctx.bodies.len() {
            apply_look(ev, bi, &look, appearances);
        }
    }
    for (gi, group) in open_groups.into_iter().enumerate() {
        let before = ev.ctx.bodies.len();
        for p in group {
            add(ev, p, BodyKind::Surface)?;
        }
        let idx: Vec<usize> = (before..ev.ctx.bodies.len()).collect();
        if idx.is_empty() {
            return Err(FeatureError::new("Import failed"));
        }
        super::refgeom::make_composite(ev, &idx, true, 30_000 + gi as u32)?;
    }
    Ok(())
}

/// An imported part's shape with what the file says about it.
type Tool = (k::Shape, Look);

/// What an imported file says about one of its parts.
#[derive(Clone, Default)]
struct Look {
    name: Option<String>,
    color: Option<[f32; 3]>,
    opacity: Option<f32>,
    faces: Vec<(k::Shape, [f32; 3])>,
}

/// sRGB (0..1) as linear RGBA.
fn linear(c: [f32; 3]) -> [f32; 4] {
    let l = |v: f32| if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) };
    [l(c[0]), l(c[1]), l(c[2]), 1.0]
}

/// Onshape's generated colour for an imported mesh part without a colour of
/// its own (live G35: STL "Part 1" rgb(248,135,1), isGenerated).
const MESH_PART_COLOR: [f32; 3] = [248.0 / 255.0, 135.0 / 255.0, 1.0 / 255.0];

/// Give an imported part the file's name, and with "Import appearances"
/// its colours (the part's and its faces' own).
fn apply_look(ev: &mut EvalCtx, bi: usize, look: &Look, appearances: bool) {
    if let Some(name) = look.name.as_ref().filter(|n| !n.trim().is_empty()) {
        ev.ctx.bodies[bi] = std::sync::Arc::new(ev.ctx.bodies[bi].renamed(name.clone()));
    }
    let body = ev.ctx.bodies[bi].clone();
    if !(appearances && look.color.is_some()) && body.shape.is_mesh() {
        ev.ctx.part_appearance.insert(body.id.clone(), linear(MESH_PART_COLOR));
    }
    if !appearances {
        return;
    }
    if let Some(c) = look.color {
        let mut c = linear(c);
        c[3] = look.opacity.unwrap_or(1.0);
        ev.ctx.part_appearance.insert(body.id.clone(), c);
    }
    for (face, c) in &look.faces {
        if let Some(fi) = body.shape.index_of(face, ShapeType::Face) {
            ev.ctx.face_appearance.push((body.id.clone(), body.faces[fi].clone(), linear(*c)));
        }
    }
}

pub fn decal(_ev: &mut EvalCtx) -> FResult<()> {
    err("Decal is not supported yet")
}
