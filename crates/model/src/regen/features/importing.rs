//! Import features (Derived, Derived mirror, Import of CAD files) and the
//! not-yet-supported Wrap / Decal.

use super::bshared::new_face_names;
use crate::display::BodyKind;
use crate::regen::eval::{EvalCtx, FResult, FeatureError, NamedShape, err};
use glam::DVec3;
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
    let mut n = 0u32;
    for b in &src.bodies {
        if !wanted.is_empty() && !wanted.contains(&b.id) {
            continue;
        }
        // Keep the source's persistent names so references into derived
        // geometry follow the source's edits.
        let id = crate::reference::BodyId { feature: ev.feature, index: n };
        n += 1;
        let color = ev.ctx.alloc_color();
        let body = crate::regen::context::Body::new(id, b.name.clone(), b.kind, b.shape.clone(), b.faces.clone(), color, ev.feature);
        ev.ctx.bodies.push(std::sync::Arc::new(body));
    }
    if ev.bool("includeMateConnectors") {
        for c in &src.connectors {
            let mut c = c.clone();
            if let crate::reference::Reference::MateConnector { index, .. } = c.reference {
                c.reference = crate::reference::Reference::MateConnector { feature: ev.feature, index };
            }
            ev.ctx.connectors.push(c);
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

pub fn wrap(_ev: &mut EvalCtx) -> FResult<()> {
    err("Wrap is not supported yet")
}

pub fn decal(_ev: &mut EvalCtx) -> FResult<()> {
    err("Decal is not supported yet")
}
