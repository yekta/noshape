//! Materials and appearances of Part Studio parts, faces and features (the
//! Material panel and the Appearance editor), GUI-free. From
//! `crates/app/src/part_studio/material_panel.rs` and `appearance.rs`.

use noshape_model::partstudio::{Appearance, AppearanceTarget, FaceAppearance, PartStudio, TessQuality};
use noshape_model::reference::{BodyId, FeatureId, Reference};

/// A Material Library tab's Update… (also the history text).
pub const UPDATE_MATERIAL_LIBRARY: &str = "Update material library";

/// kg/m³ per lb/in³ (the Material panel shows lb/in³).
pub const LB_IN3: f64 = 27_679.904_710_191;

/// sRGB 0..255 to linear 0..1.
pub fn srgb_to_lin(c: u8) -> f32 {
    let c = c as f32 / 255.0;
    if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
}

/// Linear 0..1 to sRGB 0..255.
pub fn lin_to_srgb(c: f32) -> u8 {
    let c = c.clamp(0.0, 1.0);
    let s = if c <= 0.0031308 { c * 12.92 } else { 1.055 * c.powf(1.0 / 2.4) - 0.055 };
    (s * 255.0).round() as u8
}

/// Parse `#rrggbb` / `rrggbb` / `rgb`.
pub fn parse_hex(text: &str) -> Option<[u8; 3]> {
    let t = text.trim().trim_start_matches('#');
    let full: String = match t.len() {
        3 => t.chars().flat_map(|c| [c, c]).collect(),
        6 => t.to_string(),
        _ => return None,
    };
    let v = u32::from_str_radix(&full, 16).ok()?;
    Some([(v >> 16) as u8, (v >> 8) as u8, v as u8])
}

/// What the Appearance editor edits.
#[derive(Clone, Debug, PartialEq)]
pub enum AppearanceEditTarget {
    Parts(Vec<BodyId>),
    /// Faces (and the index of the appearance being edited, if any).
    Faces(Vec<Reference>, Option<usize>),
    /// A feature's faces or a sketch's curves.
    Feature(FeatureId, Option<usize>),
}

/// The Appearance editor's ✓ ("Edit appearance"): `rgb` sRGB, `transparency`
/// 0 = opaque … 1 = invisible; parts also take the tessellation quality.
pub fn apply_appearance(ps: &mut PartStudio, target: &AppearanceEditTarget, rgb: [u8; 3], transparency: f32, tess: TessQuality) {
    let color = [srgb_to_lin(rgb[0]), srgb_to_lin(rgb[1]), srgb_to_lin(rgb[2]), 1.0 - transparency];
    match target {
        AppearanceEditTarget::Parts(bodies) => {
            for b in bodies {
                let p = ps.part_properties.entry(b.clone()).or_default();
                p.appearance = Some(Appearance { color });
                p.tessellation = (tess != TessQuality::Auto).then_some(tess);
            }
        }
        AppearanceEditTarget::Faces(faces, index) => {
            let a = FaceAppearance { target: AppearanceTarget::Faces(faces.clone()), color };
            match index.and_then(|i| ps.appearances.get_mut(i)) {
                Some(slot) => *slot = a,
                None => ps.appearances.push(a),
            }
        }
        AppearanceEditTarget::Feature(f, index) => {
            let a = FaceAppearance { target: AppearanceTarget::Feature(*f), color };
            match index.or_else(|| ps.appearances.iter().position(|x| x.target == a.target)).and_then(|i| ps.appearances.get_mut(i)) {
                Some(slot) => *slot = a,
                None => ps.appearances.push(a),
            }
        }
    }
}

pub const EDIT_APPEARANCE: &str = "Edit appearance";
pub const REMOVE_APPEARANCE: &str = "Remove appearance";

/// Appearance panel › Remove appearance (a face / feature appearance).
pub fn remove_appearance(ps: &mut PartStudio, index: usize) {
    if index < ps.appearances.len() {
        ps.appearances.remove(index);
    }
}

/// Appearance panel › Remove appearance on a part ("Edit appearance"): its
/// own colour and tessellation go.
pub fn reset_part_appearance(ps: &mut PartStudio, b: &BodyId) {
    let p = ps.part_properties.entry(b.clone()).or_default();
    p.appearance = None;
    p.tessellation = None;
}

pub const ASSIGN_MATERIAL: &str = "Assign material";

/// The Material panel's ✓: the material (name, density kg/m³) of the parts.
pub fn assign_material(ps: &mut PartStudio, bodies: &[BodyId], name: &str, density: f64) {
    for b in bodies {
        let p = ps.part_properties.entry(b.clone()).or_default();
        p.material = Some(name.to_string());
        p.density = Some(density);
    }
}

/// A library material (the Material panel's rows).
#[derive(Clone, Debug)]
pub struct LibraryEntry {
    pub library: String,
    pub name: String,
    pub category: String,
    /// kg/m³.
    pub density: f64,
}

/// The materials of the Onshape Material Library and the document's
/// Material Library tabs, in the panel's order.
pub fn library_materials(doc: &noshape_model::Document) -> Vec<LibraryEntry> {
    let mut out: Vec<LibraryEntry> = crate::material_data::ONSHAPE_MATERIALS
        .iter()
        .map(|(n, c, d, _)| LibraryEntry { library: "Onshape Material Library".into(), name: n.to_string(), category: c.to_string(), density: d.parse::<f64>().unwrap_or(0.0) * LB_IN3 })
        .collect();
    for (_, lib_name, lib) in noshape_model::material_library::libraries(doc) {
        out.extend(lib.materials.iter().map(|m| LibraryEntry { library: lib_name.clone(), name: m.name.clone(), category: m.category.clone(), density: m.density }));
    }
    out
}
