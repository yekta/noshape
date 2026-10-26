//! Import features (Derived, Derived mirror, Import of CAD files) and the
//! not-yet-supported Decal (Wrap lives in `surfacing_c`).

use super::bshared::new_face_names;
use crate::display::BodyKind;
use crate::regen::eval::{EvalCtx, FResult, FeatureError, NamedShape, err};
use super::sheetmetal_form::frame_matrix;
use glam::{DMat4, DVec3};
use noshape_kernel as k;
use noshape_kernel::ShapeType;
use crate::reference::FeatureId;

/// std ErrorStringEnum.DERIVED_NO_INSTANCING (live text, ui 10-22 b).
pub const DERIVED_NO_INSTANCING: &str = "Using derived feature to make instances is not recommended. Consider using an Assembly instead.";

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
        // Live ("qa5 derived" (7)): a deleted source reads "Error regenerating.".
        if provider.configuration(&tab).is_none() {
            return err("Error regenerating.");
        }
        return err("The source Part Studio could not be regenerated (a circular reference)");
    };
    // Selected source parts and sketches (BodyIds of the source studio, a
    // sketch as `sketch_body_id`); empty = everything.
    let wanted: Vec<crate::reference::BodyId> = ev
        .query("parts")
        .iter()
        .filter_map(|r| match r {
            crate::reference::Reference::Body(b) => Some(b.clone()),
            _ => None,
        })
        .collect();
    let picked_sketches: Vec<FeatureId> = wanted.iter().filter(|b| b.index == u32::MAX).map(|b| b.feature).collect();
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
    // std importDerived reports DERIVED_NO_INSTANCING for 2+ locations; live
    // (ui 10-22 b) shows it as the info bubble while the dialog is open.
    if frames.len() > 1 {
        ev.infos.push(DERIVED_NO_INSTANCING.into());
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
            let base_name = provider.part_name(&tab, &b.id).unwrap_or_else(|| b.name.clone());
            // Live (ui 10-22 b): every instance keeps the source name ("Part 1"
            // twice, no suffix).
            ev.ctx.own_names.insert(id.clone());
            let body = crate::regen::context::Body::new(id, base_name, b.kind, shape, faces, color, ev.feature);
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
    // A whole studio with no parts left (its Extrude suppressed) still
    // brings its sketches and stays OK (live "qa5 derived"); only picked
    // parts that are gone fail.
    // Live ("psb (10-26 a)" D3): only a sketch picked brings no parts.
    if n == 0 && wanted.iter().any(|b| b.index != u32::MAX) {
        return err("The selected parts no longer exist in the source Part Studio");
    }
    // A whole studio brings its sketches too (live "qa5 derived" (1)/(4): the
    // Derived row's ▸ lists them; deriving chosen parts brings none). They
    // keep their source ids, so they can be picked and referenced here. Each
    // location gets a copy (live ui 10-22 b: drawn at both, the ▸ lists
    // "Sketch 1" twice); picks and references resolve to the first copy.
    let already: Vec<FeatureId> = ev.ctx.sketches.iter().map(|x| x.feature).collect();
    for frame in frames.iter().filter(|_| wanted.is_empty() || !picked_sketches.is_empty()) {
        let m = *frame * inv;
        let moved = !m.abs_diff_eq(DMat4::IDENTITY, 1e-15);
        for s in &src.sketches {
            if already.contains(&s.feature) || (!wanted.is_empty() && !picked_sketches.contains(&s.feature)) {
                continue;
            }
            let s = if moved {
                let mut c = (**s).clone();
                let f = &mut c.frame;
                f.origin = m.transform_point3(f.origin);
                f.x = m.transform_vector3(f.x).normalize_or_zero();
                f.y = m.transform_vector3(f.y).normalize_or_zero();
                f.normal = m.transform_vector3(f.normal).normalize_or_zero();
                std::sync::Arc::new(c)
            } else {
                s.clone()
            };
            ev.ctx.derived_sketches.push((ev.feature, s.feature));
            ev.ctx.sketches.push(s);
        }
    }
    Ok(())
}

/// Live's "Derived mirror" (the one feature of an assembly Mirror's
/// "<Part Studio>-Mirrored" Part Studio, behaviours "skb (10-23 15:30)"): every
/// part of the source mirrored, named "<part>-Mirrored". The mirror plane is
/// the source's YZ plane (x -> -x; live's isn't captured, UNCONFIRMED); the
/// parts keep the source's ids so the assembly maps a part to its mirror.
pub fn derived_mirror(ev: &mut EvalCtx) -> FResult<()> {
    let tab = ev.params.text("partStudio").trim().to_string();
    let Some(provider) = ev.derived.clone() else {
        return err("Derived parts are not available in this context");
    };
    let Some(src) = provider.context(&tab) else {
        return err("Error regenerating.");
    };
    let mirror = k::Transform::Mirror(k::Plane::yz());
    for b in src.bodies.iter().filter(|b| b.kind == BodyKind::Solid) {
        let shape = b.shape.transformed(&mirror)?;
        let name = format!("{}-Mirrored", provider.part_name(&tab, &b.id).unwrap_or_else(|| b.name.clone()));
        let color = ev.ctx.alloc_color();
        ev.ctx.own_names.insert(b.id.clone());
        let body = crate::regen::context::Body::new(b.id.clone(), name, b.kind, shape, b.faces.clone(), color, ev.feature);
        ev.ctx.bodies.push(std::sync::Arc::new(body));
    }
    Ok(())
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
            let pieces = if s.shape_type() == ShapeType::Compound && !children.is_empty() && children.iter().all(|c| c.shape_type() == ShapeType::Compound) { children } else { vec![s] };
            for piece in pieces {
                // The file's part name (PVZ) goes to the part's solid.
                let solids = piece.solids();
                looks.extend(solids.iter().map(|_| Look { name: part.name.clone(), ..Look::default() }));
                tools.extend(solids);
                let open: Vec<k::Shape> = piece.children().into_iter().filter(|c| c.shape_type() == ShapeType::Shell).collect();
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
    // Face names run on across the bodies (Onshape: every face of an import
    // has its own id), so two parts never share a face name (qa4 IMP1).
    let mut face_base = 0u32;
    let mut add = |ev: &mut EvalCtx, p: k::Shape, kind: BodyKind| -> FResult<()> {
        let names = new_face_names(ev, &p).into_iter().map(|n| offset_new_name(n, face_base)).collect();
        face_base += p.count(ShapeType::Face) as u32;
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
        let id = ev.ctx.bodies[bi].id.clone();
        ev.ctx.own_names.insert(id);
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

/// Decal (std decal.fs): the image's placement on a planar or cylindrical
/// face, recorded for display (`Context::decals`); the part is unchanged.
pub fn decal(ev: &mut EvalCtx) -> FResult<()> {
    use crate::regen::context::{DecalMapping, DecalRecord};
    use crate::regen::eval::err_at;
    let Some(&(bi, fi)) = ev.resolve_faces(ev.query("face")).first() else { return err_at(&["face"], "Select face for decal") };
    let tab = ev.params.text("image").to_string();
    let file = if tab.is_empty() {
        None
    } else if tab.starts_with("ext:") || tab.contains('/') {
        ev.derived.as_ref().and_then(|d| d.file(&tab))
    } else {
        ev.custom.clone().unwrap_or_else(crate::custom::fallback_provider).file(&tab)
    };
    let size = file.and_then(|f| f.bytes()).map(|b| crate::custom::reference::image_size(&b));
    let Some((Some(iw), Some(ih), _)) = size else { return err_at(&["image"], "Select image for decal") };
    if iw > 4096 || ih > 4096 {
        return err_at(&["image"], "Inserted image exceeds 4k x 4k size limit");
    }
    let angle = ev.angle("angle")? * if ev.bool("angleOppositeDirection") { -1.0 } else { 1.0 };
    let (mut w, mut h) = (ev.length("width")?, ev.length("height")?);
    let aspect = iw as f64 / ih.max(1) as f64;
    if ev.bool("maintainAspectRatio") {
        if ev.is("aspectRatioConstraint", "HEIGHT_DRIVING") {
            w = h * aspect;
        } else {
            h = w / aspect;
        }
    }
    let (u_shift, v_shift) = (ev.length("uShift")?, ev.length("vShift")?);
    let realign = ev.bool("realign");
    let origin_mc = if realign { ev.query("originConnector").first().and_then(|r| ev.resolve_connector(r)) } else { None };
    let href = if realign { ev.query("horizontalReference").first().and_then(|r| ev.resolve_direction(r)) } else { None };
    let body = ev.ctx.bodies[bi].clone();
    let face = body.shape.sub_shape(ShapeType::Face, fi)?;
    let bad_reference = || err_at(&["horizontalReference"], "A valid reference direction could not be created from selection.");
    let parallel = |a: DVec3, b: DVec3| a.normalize().cross(b.normalize()).length() < 1e-9;
    // TransformUV: scale by 1/size after rotating; the origin at the image's
    // centre, or at the chosen one of its 3 × 3 points with a placement origin.
    let uv = |turn: f64| -> ([[f64; 2]; 2], [f64; 2]) {
        let (c, s) = (turn.cos(), turn.sin());
        let lin = [[c / w, -s / w], [s / h, c / h]];
        let i = if origin_mc.is_some() { ev.integer("imageOriginIndex").unwrap_or(4).clamp(0, 8) } else { 4 };
        (lin, [0.5 * (i % 3) as f64, 0.5 * (i / 3) as f64])
    };
    // Points of the face at its parameter box's corners and centre (std
    // evFaceTangentPlane at (0,0) … (1,1)).
    let [u0, u1, v0, v1] = face.uv_bounds()?;
    let at = |a: f64, b: f64| face.point_at_uv(u0 + (u1 - u0) * a, v0 + (v1 - v0) * b);
    let record = match face.face_surface()? {
        k::Surface::Plane(_) => {
            let z = face.normal_at_point(face.centroid()?)?.normalize();
            let x = match href {
                Some(d) => {
                    if parallel(z, d) {
                        return bad_reference();
                    }
                    let x = z.cross(d).normalize().cross(z);
                    if x.x < 0.0 { -x } else { x }
                }
                None => in_plane_perpendicular_to_z(z),
            };
            let y = z.cross(x);
            let origin = origin_mc.as_ref().map(|c| c.origin).unwrap_or(face.centroid()?) + x * u_shift + y * v_shift;
            let (uv_linear, uv_translation) = uv(angle);
            DecalRecord { feature: ev.feature, body: body.id.clone(), face: body.faces[fi].clone(), image: tab, mapping: DecalMapping::Planar { origin, x, y, z }, uv_linear, uv_translation }
        }
        k::Surface::Cylinder { origin, axis, x_dir, radius } => {
            let (mut z, mut x) = (axis.normalize(), x_dir.normalize());
            // The axis toward world up.
            if z.z < 0.0 {
                (z, x) = (-z, -x);
            }
            let mut o = origin;
            let y_of = |z: DVec3, x: DVec3| z.cross(x);
            let angle_of = |o: DVec3, x: DVec3, z: DVec3, p: DVec3| {
                let d = p - o;
                let (a, b) = (d.dot(x), d.dot(y_of(z, x)));
                if a * a + b * b < 1e-16 {
                    return 0.0;
                }
                let t = b.atan2(a);
                if t < 0.0 { t + std::f64::consts::TAU } else { t }
            };
            // getFaceCylinderProjectionInformation.
            let (pmin, pmid, pmax) = (at(0.0, 0.0)?, at(0.5, 0.5)?, at(1.0, 1.0)?);
            let (mut zmin, mut zmax) = ((pmin - o).dot(z), (pmax - o).dot(z));
            if zmin > zmax {
                std::mem::swap(&mut zmin, &mut zmax);
            }
            let (mut amin, amid, mut amax) = (angle_of(o, x, z, pmin), angle_of(o, x, z, pmid), angle_of(o, x, z, pmax));
            let (amin, amax) = if (amax - amin).abs() < 1e-12 {
                (-std::f64::consts::PI, std::f64::consts::PI)
            } else {
                if amin > amax {
                    std::mem::swap(&mut amin, &mut amax);
                }
                if (amin - amid) * (amid - amax) < 0.0 { (amax - std::f64::consts::TAU, amin) } else { (amin, amax) }
            };
            let (mut zc, mut ac) = ((zmax + zmin) / 2.0, (amax + amin) / 2.0);
            if let Some(c) = &origin_mc {
                ac = angle_of(o, x, z, c.origin);
                zc = (c.origin - o).dot(z);
            }
            let t = ac + u_shift / radius;
            let on = o + z * (zc + v_shift) + (x * t.cos() + y_of(z, x) * t.sin()) * radius;
            let to = on - o;
            if !parallel(to, z) {
                o += z * to.dot(z);
                x = (on - o).normalize();
            }
            let y = y_of(z, x);
            let mut href_angle = 0.0;
            if let Some(d) = href {
                if parallel(x, d) {
                    return bad_reference();
                }
                let (py, pz) = (d.dot(y), d.dot(z));
                let n = (py * py + pz * pz).sqrt().max(1e-300);
                href_angle = -(pz / n).atan2(py / n);
            }
            let (uv_linear, uv_translation) = uv(href_angle + angle);
            DecalRecord { feature: ev.feature, body: body.id.clone(), face: body.faces[fi].clone(), image: tab, mapping: DecalMapping::Cylindrical { origin: o, x, y, z, radius }, uv_linear, uv_translation }
        }
        _ => return err_at(&["face"], "Select face for decal"),
    };
    // isDecalNearFace.
    let near = match &record.mapping {
        DecalMapping::Planar { .. } => {
            let (mut lo, mut hi) = ([f64::INFINITY; 2], [f64::NEG_INFINITY; 2]);
            for (a, b) in [(0.0, 0.0), (0.0, 1.0), (1.0, 0.0), (1.0, 1.0)] {
                let q = record.uv_at(at(a, b)?);
                for i in 0..2 {
                    lo[i] = lo[i].min(q[i]);
                    hi[i] = hi[i].max(q[i]);
                }
            }
            lo[0] < 1.0 && hi[0] > 0.0 && lo[1] < 1.0 && hi[1] > 0.0
        }
        DecalMapping::Cylindrical { origin, z, .. } => {
            // The image's corners (0,0) and (1,1) on the cylinder, against the
            // face's height range.
            let (pmin, pmax) = (at(0.0, 0.0)?, at(1.0, 1.0)?);
            let (fz0, fz1) = ((pmin - *origin).dot(*z), (pmax - *origin).dot(*z));
            let (fz0, fz1) = (fz0.min(fz1), fz0.max(fz1));
            // The height of an image point: inverse TransformUV's second row.
            let world_z = |uvp: [f64; 2]| -> f64 {
                let l = &record.uv_linear;
                let (a, b) = (uvp[0] - record.uv_translation[0], uvp[1] - record.uv_translation[1]);
                (-l[1][0] * a + l[0][0] * b) / (l[0][0] * l[1][1] - l[0][1] * l[1][0])
            };
            let (bz, tz) = (world_z([0.0, 0.0]), world_z([1.0, 1.0]));
            (bz < tz && bz < fz1 && tz > fz0) || (bz >= tz && tz < fz1 && bz > fz0)
        }
    };
    ev.ctx.decals.push(record);
    if !near {
        return err("Decal is not on target face.");
    }
    Ok(())
}

/// std decal.fs onDecalFeatureChange: once the dialog has both a face and an
/// image, the decal's width and height fit the face (getDefaultDecalSize:
/// the face's extent in the decal's plane, or round and along the
/// cylinder) with the image's aspect. `image`: the image file's bytes;
/// `href`: Realign's horizontal reference direction, if any.
pub fn decal_default_size(ctx: &crate::regen::context::Context, params: &crate::params::ParamValues, href: Option<DVec3>, image: &[u8]) -> Option<(f64, f64)> {
    let (Some(iw), Some(ih), _) = crate::custom::reference::image_size(image) else { return None };
    let aspect = iw as f64 / ih.max(1) as f64;
    let face = crate::regen::measure::resolve_shapes(ctx, params.query("face").first()?).into_iter().next()?;
    let (w, h) = match face.face_surface().ok()? {
        k::Surface::Plane(_) => {
            let z = face.normal_at_point(face.centroid().ok()?).ok()?.normalize();
            let x = match href.filter(|_| params.bool("realign")) {
                Some(d) if d.normalize().cross(z).length() > 1e-9 => {
                    let x = z.cross(d).normalize().cross(z);
                    if x.x < 0.0 { -x } else { x }
                }
                _ => in_plane_perpendicular_to_z(z),
            };
            let y = z.cross(x);
            // projectBox3d of the face's tight box.
            let b = face.bounding_box(true).ok()?;
            let (mut lo, mut hi) = ([f64::INFINITY; 2], [f64::NEG_INFINITY; 2]);
            for i in 0..8 {
                let c = DVec3::new([b.min.x, b.max.x][i & 1], [b.min.y, b.max.y][(i >> 1) & 1], [b.min.z, b.max.z][i >> 2]);
                for (k, a) in [x, y].into_iter().enumerate() {
                    lo[k] = lo[k].min(c.dot(a));
                    hi[k] = hi[k].max(c.dot(a));
                }
            }
            (hi[0] - lo[0], hi[1] - lo[1])
        }
        k::Surface::Cylinder { origin, axis, x_dir, radius } => {
            let (mut z, mut x) = (axis.normalize(), x_dir.normalize());
            if z.z < 0.0 {
                (z, x) = (-z, -x);
            }
            let y = z.cross(x);
            let [u0, u1, v0, v1] = face.uv_bounds().ok()?;
            let at = |a: f64, b: f64| face.point_at_uv(u0 + (u1 - u0) * a, v0 + (v1 - v0) * b).ok();
            let angle = |p: DVec3| {
                let d = p - origin;
                let t = d.dot(y).atan2(d.dot(x));
                if t < 0.0 { t + std::f64::consts::TAU } else { t }
            };
            let (pmin, pmid, pmax) = (at(0.0, 0.0)?, at(0.5, 0.5)?, at(1.0, 1.0)?);
            let (mut a0, am, mut a1) = (angle(pmin), angle(pmid), angle(pmax));
            let span = if (a1 - a0).abs() < 1e-12 {
                std::f64::consts::TAU
            } else {
                if a0 > a1 {
                    std::mem::swap(&mut a0, &mut a1);
                }
                if (a0 - am) * (am - a1) < 0.0 { std::f64::consts::TAU - (a1 - a0) } else { a1 - a0 }
            };
            (span * radius, ((pmax - origin).dot(z) - (pmin - origin).dot(z)).abs())
        }
        _ => return None,
    };
    Some(if w / aspect > h { (h * aspect, h) } else { (w, w / aspect) })
}

/// std decal.fs vectorInPlanePerpendicularToZ: the plane's x direction,
/// level with world Z (world X on a horizontal plane).
fn in_plane_perpendicular_to_z(n: DVec3) -> DVec3 {
    let z = DVec3::Z;
    if n.normalize().cross(z).length() > 1e-9 {
        let projected = (z - n * n.dot(z) / n.length_squared()).normalize();
        projected.cross(n.normalize())
    } else {
        DVec3::X
    }
}

/// A `Role::New` face name moved on by `base` (other names as they are).
fn offset_new_name(n: crate::naming::Name, base: u32) -> crate::naming::Name {
    match &*n {
        crate::reference::TopoName::Face { feature, role: crate::reference::Role::New(i) } if base > 0 => std::sync::Arc::new(crate::reference::TopoName::Face { feature: *feature, role: crate::reference::Role::New(i + base) }),
        _ => n,
    }
}
