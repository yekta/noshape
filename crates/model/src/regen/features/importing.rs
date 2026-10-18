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
pub fn import_derived(ev: &mut EvalCtx) -> FResult<()> {
    let tab = ev.params.text("partStudio").trim().to_string();
    if tab.is_empty() {
        return err("Select a Part Studio to derive from");
    }
    let Some(provider) = ev.derived.clone() else {
        return err("Derived parts are not available in this context");
    };
    let Some(src) = provider.context(&tab) else {
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
            other => return err(format!("Unsupported import format .{other}")),
        }
    };
    let up = ev.bool("yAxisIsUp");
    let mut tools = Vec::new();
    for s in shapes {
        let s = if up { s.transformed(&k::Transform::Rotate { origin: DVec3::ZERO, axis: DVec3::X, angle: std::f64::consts::FRAC_PI_2 })? } else { s };
        // One body per solid, else per shell / face set.
        let parts = if s.count(ShapeType::Solid) > 0 { s.solids() } else if s.count(ShapeType::Shell) > 0 { s.shells() } else { vec![s] };
        for p in parts {
            if p.count(ShapeType::Face) == 0 {
                continue;
            }
            tools.push(p);
        }
    }
    if tools.is_empty() {
        return err("The imported file contains no parts or surfaces");
    }
    for p in tools {
        let kind = if p.count(ShapeType::Solid) > 0 { BodyKind::Solid } else { BodyKind::Surface };
        let names = new_face_names(ev, &p);
        // Separate bodies even if they touch (imports are never merged).
        let before = ev.ctx.bodies.len();
        ev.apply_operation(vec![NamedShape::new(p, names)], "NEW", None, kind)?;
        if ev.ctx.bodies.len() == before {
            return Err(FeatureError::new("Import failed"));
        }
    }
    Ok(())
}

pub fn decal(_ev: &mut EvalCtx) -> FResult<()> {
    err("Decal is not supported yet")
}
