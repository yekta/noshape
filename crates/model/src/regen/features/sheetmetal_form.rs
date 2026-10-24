//! Sheet metal Form (03c SM-08, FS `sheetMetalFormed`) and Corner (SM-12,
//! FS `sheetMetalCorner`).
//!
//! **Form.** The form tool comes from another Part Studio of the document
//! (through the Derived provider). Its Tag feature (purpose Form) names the
//! part to add, the part to remove and the form origin mate connector (an
//! untagged studio has no form tools, as std's derive finds none). The tool
//! is aligned with each location (mate connector or sketch point: origin on
//! the sheet face, Z = outward face normal, the sheet below Z = 0), the
//! remove part is subtracted from the wall and the add part united with it.
//! The flat pattern shows the form's footprint as an outline with a center
//! mark (the region itself stays flat), like Onshape's form sketch.
//!
//! **Corner.** Overrides the relief where two bends of one wall meet: a
//! round or square cutout centered where the two developed bend regions
//! overlap in the flat, cut through the folded part as a prism along the
//! wall normal and removed from the flat outline.

use super::sheetmetal_util::*;
use crate::naming::Name;
use crate::reference::{Reference, Role, Source, TopoName};
use crate::regen::eval::{EvalCtx, FResult, NamedShape, err, err_at};
use crate::sheetmetal::{FlatMark, FlatMarkKind};
use glam::{DMat4, DVec2, DVec3};
use noshape_kernel as k;
use noshape_kernel::Shape;
use std::sync::Arc;

/// Rigid frame (origin, x, z) as a matrix (local -> world).
pub fn frame_matrix(o: DVec3, x: DVec3, z: DVec3) -> DMat4 {
    let z = z.normalize();
    let x = (x - z * x.dot(z)).normalize();
    let y = z.cross(x);
    DMat4::from_cols(x.extend(0.0), y.extend(0.0), z.extend(0.0), o.extend(1.0))
}

/// Location frames (origin, x, z) of the selected mate connectors / sketch points.
fn locations(ev: &EvalCtx) -> FResult<Vec<(DVec3, DVec3, DVec3)>> {
    let mut out = Vec::new();
    for r in ev.query("locations") {
        match r {
            Reference::SketchEntity { sketch, .. } => {
                let Some(p) = ev.resolve_vertex_point(r) else { return err("A selected form location no longer exists") };
                let Some(s) = ev.ctx.sketch(*sketch) else { return err("A selected form location no longer exists") };
                out.push((p, s.frame.x, s.frame.normal));
            }
            _ => match ev.resolve_connector(r) {
                Some(c) => out.push((c.origin, c.x, c.z)),
                None => match ev.resolve_vertex_point(r) {
                    Some(p) => out.push((p, DVec3::X, DVec3::Z)),
                    None => return err("A selected form location no longer exists"),
                },
            },
        }
    }
    Ok(out)
}

pub fn form(ev: &mut EvalCtx) -> FResult<()> {
    let tab = ev.params.text("formPartStudio").trim().to_string();
    if tab.is_empty() {
        return err("Select the Part Studio of the form tool");
    }
    let Some(provider) = ev.derived.clone() else { return err("Form tools are not available in this context") };
    let Some(src) = provider.context(&tab) else { return err("The form Part Studio could not be regenerated (deleted, or a circular reference)") };
    let locs = locations(ev)?;
    if locs.is_empty() {
        return err("Select form locations (sketch points or mate connectors on a sheet metal face)");
    }
    // Tool parts and origin.
    let (adds, removes, origin) = match src.form_tools.last() {
        Some(tool) => {
            let get = |id: &Option<crate::reference::BodyId>| id.as_ref().and_then(|i| src.body(i)).map(|b| b.shape.clone());
            (get(&tool.add).into_iter().collect::<Vec<_>>(), get(&tool.remove).into_iter().collect::<Vec<_>>(), tool.origin)
        }
        // Std derives only Tag-marked tools (IMPORT_DERIVED_NO_PARTS, reported as FORMED_FAILED_TO_DERIVE).
        None => return err_at(&["formPartStudio"], "Failed to import form tools. Check configuration parameters and use Tag feature to mark the tool bodies."),
    };
    if adds.is_empty() && removes.is_empty() {
        return err_at(&["formPartStudio"], "Failed to import form tools. Check configuration parameters and use Tag feature to mark the tool bodies.");
    }
    let from = match origin {
        Some((o, x, z)) => frame_matrix(o, x, z),
        None => DMat4::IDENTITY,
    }
    .inverse();
    let feature = ev.feature;
    let flip = ev.bool("flipDirection");
    let mut placed = 0usize;
    let mut skipped = 0usize;
    for (li, &(o, x, z)) in locs.iter().enumerate() {
        let z = if flip { -z } else { z };
        // (Re-resolved per location: earlier forms renumber the faces.)
        // Candidate faces: the selection only (std: qContainsPoint(targetFaces), which the dialog's editing logic fills).
        let targets: Vec<(usize, usize)> = ev.resolve_faces(ev.query("targetFaces"));
        // Face containing the location, parallel to the location's Z.
        let mut hit: Option<(usize, usize, DVec3)> = None;
        for &(bi, fi) in &targets {
            let f = ev.face_shape(bi, fi)?;
            let Some((c, n)) = plane_of(&f) else { continue };
            if n.cross(z).length() > 1e-6 || (o - c).dot(n).abs() > 1e-6 {
                continue;
            }
            if f.project_point(o).map(|p| p.distance).unwrap_or(1.0) < 1e-7 {
                hit = Some((bi, fi, n));
                break;
            }
        }
        let Some((bi, fi, n)) = hit else {
            skipped += 1;
            continue;
        };
        let mut data = (*active_data(ev, bi)?).clone();
        let t = data.thickness;
        let body = ev.ctx.bodies[bi].clone();
        let Some((wall, on_ref)) = data.wall_of_face(&body.faces[fi]) else { return err("Form locations must lie on sheet metal wall faces") };
        // The form sits on the face whose outward normal is the location's Z.
        let o = if z.dot(n) < 0.0 { o + z * t } else { o };
        let m = frame_matrix(o, x, z) * from;
        let key = |kind: u32, j: usize| -> Name { Arc::new(TopoName::face(feature, Role::Side(Source::Index(li as u32 * 10_000 + kind * 5_000 + j as u32)))) };
        let place = |s: &Shape| -> FResult<Shape> { Ok(s.transformed(&k::Transform::Matrix(m))?) };
        let removes_w: Vec<Shape> = removes.iter().map(place).collect::<FResult<_>>()?;
        let adds_w: Vec<Shape> = adds.iter().map(place).collect::<FResult<_>>()?;
        let mut cur = body_named(&body);
        for (j, s) in removes_w.iter().enumerate() {
            let tool = tool_named(s.clone(), |i| key(0, j * 500 + i as usize));
            let (shape, names) = bool_named(ev, k::BooleanOp::Cut, &cur, &[tool])?;
            cur = NamedShape::new(shape, names);
        }
        for (j, s) in adds_w.iter().enumerate() {
            let tool = tool_named(s.clone(), |i| key(1, j * 500 + i as usize));
            let (shape, names) = bool_named(ev, k::BooleanOp::Fuse, &cur, &[tool])?;
            cur = NamedShape::new(shape, names);
        }
        if solid_count(&cur.shape) != 1 {
            return err("The form splits the part or does not touch it");
        }
        ev.replace_body(bi, cur.shape, cur.names.faces);
        // Footprint for the flat pattern, in the wall's reference plane.
        let all: Vec<Shape> = removes_w.iter().chain(adds_w.iter()).cloned().collect();
        let tools = if all.len() == 1 { all[0].clone() } else { Shape::compound(&all) };
        // Reference-side plane: the face the form sits on, or the opposite one.
        let on_form_face_ref = (z.dot(n) > 0.0) == on_ref;
        let ref_origin = if on_form_face_ref { o } else { o - z * t };
        let ref_n = z;
        let extent = |l: &Vec<DVec3>| {
            let (mut lo, mut hi) = (DVec3::splat(f64::INFINITY), DVec3::splat(f64::NEG_INFINITY));
            for p in l {
                lo = lo.min(*p);
                hi = hi.max(*p);
            }
            (hi - lo).length()
        };
        let to_ref = |p: DVec3| p - ref_n * (p - ref_origin).dot(ref_n);
        // Outline: the widest section of the tools near the sheet.
        let mut loops: Vec<Vec<DVec3>> = Vec::new();
        for h in [-0.5 * t, 0.25 * t, 0.75 * t, 1.5 * t] {
            let pl = k::Plane::new(o + z * h, z, x);
            let mut ls: Vec<Vec<DVec3>> = Vec::new();
            if let Ok(ws) = k::section_wires(&tools, &pl) {
                for w in ws {
                    let pts = crate::sheetmetal::flat::wire_points(&w);
                    if pts.len() >= 3 {
                        ls.push(pts.into_iter().map(to_ref).collect());
                    }
                }
            }
            ls.sort_by(|a, b| extent(b).total_cmp(&extent(a)));
            if ls.first().map(extent).unwrap_or(0.0) > loops.first().map(extent).unwrap_or(0.0) + 1e-9 {
                loops = ls;
            }
        }
        // Fill: what the tools cut out of the sheet (section at mid-sheet).
        let mid = k::Plane::new(o - z * (0.5 * t), z, x);
        let fill = k::section_caps(&tools, &mid)
            .ok()
            .filter(|f| f.count(noshape_kernel::ShapeType::Face) > 0)
            .and_then(|f| f.translated(-ref_n * (mid.origin - ref_origin).dot(ref_n)).ok());
        let center = o - ref_n * (o - ref_origin).dot(ref_n);
        data.marks.push(FlatMark { feature, wall, kind: FlatMarkKind::Form, loops, origin: Some(center), fill });
        put_data(ev, bi, data);
        placed += 1;
    }
    // Std FORMED_SELECT_LOCATION_ON_ACTIVE_FACE: an error when no location is placed, else a warning.
    if placed == 0 {
        return err_at(&["locations"], "Select location on selected active sheet metal faces.");
    }
    if skipped > 0 {
        ev.warnings.push("Select location on selected active sheet metal faces.".into());
    }
    Ok(())
}

// ------------------------------------------------------------------ corner

pub fn corner(ev: &mut EvalCtx) -> FResult<()> {
    let Some(r) = ev.query("corner").first().cloned() else { return err("Select a vertex or edge of a corner where two bends meet") };
    let Some((bi, p)) = super::sheetmetal_ops::point_of(ev, &r) else { return err("The selected corner no longer exists") };
    let mut data = (*active_data(ev, bi)?).clone();
    let t = data.thickness;
    // Bend ends near the selection.
    let mut ends: Vec<(usize, f64, f64)> = Vec::new();
    for (i, b) in data.bends.iter().enumerate() {
        for s in [b.s0, b.s1] {
            let d = (p - (b.origin + b.axis * s)).length();
            if d < (b.radius + t) * 2.0 + t {
                ends.push((i, s, d));
            }
        }
    }
    ends.sort_by(|a, b| a.2.total_cmp(&b.2));
    let mut pair = None;
    'outer: for (ia, a) in ends.iter().enumerate() {
        for b in &ends[ia + 1..] {
            if a.0 != b.0 && data.bends[a.0].parent == data.bends[b.0].parent && data.bends[a.0].axis.cross(data.bends[b.0].axis).length() > 1e-6 {
                pair = Some((*a, *b));
                break 'outer;
            }
        }
    }
    let Some((ea, eb)) = pair else {
        return err("The selection is not a corner between two bends of one wall (use Bend relief at a single bend end)");
    };
    let (b1, b2) = (data.bends[ea.0].clone(), data.bends[eb.0].clone());
    // Corner of the two tangent lines on the parent's reference side.
    let (p1, d1) = (b1.tangent_point(t, ea.1), b1.axis);
    let (p2, d2) = (b2.tangent_point(t, eb.1), b2.axis);
    let w = p1 - p2;
    let (a, bb, c, d, e) = (d1.dot(d1), d1.dot(d2), d2.dot(d2), d1.dot(w), d2.dot(w));
    let den = a * c - bb * bb;
    if den.abs() < 1e-12 {
        return err("The bends of the corner are parallel");
    }
    let s1 = (bb * e - c * d) / den;
    let s2 = (a * e - bb * d) / den;
    let corner_pt = ((p1 + d1 * s1) + (p2 + d2 * s2)) * 0.5;
    let (ba1, ba2) = (b1.allowance(t), b2.allowance(t));
    let center = corner_pt - b1.dir_a * ba1 * 0.5 - b2.dir_a * ba2 * 0.5;
    let n = b1.dir_a.cross(b1.axis).normalize();
    // Reference-face normal of the parent: radial_a points from the axis to it.
    let n = if n.dot(-b1.radial_a) < 0.0 { -n } else { n };
    let style = ev.enum_("cornerStyle").to_string();
    let ex = (-b1.dir_a).normalize();
    let ey = n.cross(ex);
    let seg_circle = |r: f64| vec![k::Segment2d::Circle { center: DVec2::ZERO, radius: r }];
    let seg_square = |w: f64| {
        let h = w * 0.5;
        let p = [DVec2::new(-h, -h), DVec2::new(h, -h), DVec2::new(h, h), DVec2::new(-h, h)];
        (0..4).map(|i| k::Segment2d::Line { a: p[i], b: p[(i + 1) % 4] }).collect::<Vec<_>>()
    };
    let half_diag = (ba1 * ba1 + ba2 * ba2).sqrt() * 0.5;
    let (segs, outline): (Vec<k::Segment2d>, Vec<DVec2>) = match style.as_str() {
        "SIMPLE" => {
            ev.warnings.push("Simple corners have no relief; nothing was cut".into());
            put_data(ev, bi, data);
            return Ok(());
        }
        "CLOSED" => return err("Closed corners are not supported yet"),
        "SIZED_ROUND" | "ROUND" => {
            let r = if style == "ROUND" { ev.real("cornerReliefScale")? * half_diag } else { ev.length("roundReliefDiameter")? * 0.5 };
            if r <= 0.0 {
                return err("The corner relief size must be positive");
            }
            (seg_circle(r), (0..64).map(|i| DVec2::from_angle(i as f64 / 64.0 * std::f64::consts::TAU) * r).collect())
        }
        _ => {
            let w = if style == "RECTANGLE" { ev.real("cornerReliefScale")? * ba1.max(ba2) } else { ev.length("squareReliefWidth")? };
            if w <= 0.0 {
                return err("The corner relief size must be positive");
            }
            let h = w * 0.5;
            (seg_square(w), vec![DVec2::new(-h, -h), DVec2::new(h, -h), DVec2::new(h, h), DVec2::new(-h, h)])
        }
    };
    let reach = (b1.radius.max(b2.radius) + t) * 2.0 + t;
    let plane = k::Plane::new(center - n * reach, n, ex);
    let face = k::make_face_2d(&plane, &segs, &[])?;
    let prism = k::extrude(&face, n * (2.0 * reach))?.shape;
    let feature = ev.feature;
    let key = b1.inner_faces.first().cloned().unwrap_or_else(|| Arc::new(TopoName::face(feature, Role::New(ea.0 as u32))));
    let tool = tool_named(prism, |i| side_name(feature, &key, 500 + i as u16));
    let body = ev.ctx.bodies[bi].clone();
    let (shape, names) = bool_named(ev, k::BooleanOp::Cut, &body_named(&body), &[tool])?;
    if solid_count(&shape) != 1 {
        return err("The corner relief would split the part");
    }
    ev.replace_body(bi, shape, names);
    let loop3: Vec<DVec3> = outline.iter().map(|q| center + ex * q.x + ey * q.y).collect();
    let cut = k::make_face_2d(&k::Plane::new(center, n, ex), &segs, &[]).ok();
    data.marks.push(FlatMark { feature, wall: b1.parent, kind: FlatMarkKind::Cut, loops: vec![loop3], origin: Some(center), fill: cut });
    put_data(ev, bi, data);
    Ok(())
}
