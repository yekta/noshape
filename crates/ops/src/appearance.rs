//! Materials and appearances of Part Studio parts, faces and features (the
//! Material panel and the Appearance editor), GUI-free. From
//! `crates/app/src/part_studio/material_panel.rs` and `appearance.rs`.

use noshape_model::partstudio::{Appearance, AppearanceTarget, FaceAppearance, PartStudio, TessQuality};
use noshape_model::reference::{BodyId, FeatureId, Reference, Role, TopoName};

/// A Material Library tab's Update… (also the history text).
pub const UPDATE_MATERIAL_LIBRARY: &str = "Update material library";

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
        AppearanceEditTarget::Faces(faces, index) => set_face_appearance(ps, *index, AppearanceTarget::Faces(faces.clone()), color),
        AppearanceEditTarget::Feature(f, index) => {
            let target = AppearanceTarget::Feature(*f);
            let index = index.or_else(|| ps.appearances.iter().position(|x| x.target == target));
            set_face_appearance(ps, index, target, color);
        }
    }
}

/// A face / feature appearance's colour, at `index` (keeping its name and suppression) or a new one.
pub fn set_face_appearance(ps: &mut PartStudio, index: Option<usize>, target: AppearanceTarget, color: [f32; 4]) {
    match index.and_then(|i| ps.appearances.get_mut(i)) {
        Some(slot) => {
            slot.target = target;
            slot.color = color;
        }
        None => ps.appearances.push(FaceAppearance::new(target, color)),
    }
}

/// The Appearance panel's name of face / feature appearance `index` among `all`: its own, else "Face appearance N"
/// counting face appearances in order (live 2292).
pub fn face_appearance_name(all: &[FaceAppearance], index: usize) -> Option<String> {
    let a = all.get(index)?;
    if let Some(n) = &a.name {
        return Some(n.clone());
    }
    let n = all[..=index].iter().filter(|x| matches!(x.target, AppearanceTarget::Faces(_))).count();
    matches!(a.target, AppearanceTarget::Faces(_)).then(|| format!("Face appearance {n}"))
}

/// The face / feature appearance (index in `all`) each face of `body` shows (by face index): the last one covering
/// it; suppressed ones leave the part's colour (live 2292). `own` lists the studio's features: a feature's appearance
/// also covers the faces of the bodies it made that came from elsewhere (a Derived's, named after their source).
pub fn face_appearances_of(body: &noshape_model::display::BodyDisplay, all: &[FaceAppearance], own: &std::collections::HashSet<FeatureId>) -> Vec<Option<usize>> {
    let mut out = vec![None; body.face_refs.len()];
    for (k, a) in all.iter().enumerate().filter(|(_, a)| !a.suppressed) {
        for (fi, r) in body.face_refs.iter().enumerate() {
            let from = match r {
                Reference::Face(n) => lineage(n),
                _ => Vec::new(),
            };
            let hit = match &a.target {
                AppearanceTarget::Faces(faces) => faces.contains(r) || faces.iter().any(|f| matches!(f, Reference::Face(m) if from.contains(&m))),
                AppearanceTarget::Feature(f) => r.depends_on() == Some(*f) || from.iter().any(|m| m.creator() == Some(*f)) || (body.id.feature == *f && r.depends_on().is_none_or(|d| !own.contains(&d))),
            };
            if hit {
                out[fi] = Some(k);
            }
        }
    }
    out
}

/// The faces a face stands for: the faces it was split from and those a pattern, mirror or transform copied it from
/// (live appearance-1123 Q5: split pieces and copies keep their source's face and feature appearances).
fn lineage(n: &TopoName) -> Vec<&TopoName> {
    let mut out = Vec::new();
    let mut todo = vec![n];
    while let Some(n) = todo.pop() {
        out.push(n);
        match n {
            TopoName::Split { parent, .. } => todo.push(parent),
            TopoName::Face { role: Role::Copy { of, .. }, .. } => todo.push(of),
            _ => {}
        }
    }
    out
}

pub const RENAME_APPEARANCE: &str = "Rename appearance";
pub const SUPPRESS_APPEARANCE: &str = "Suppress appearance";
pub const UNSUPPRESS_APPEARANCE: &str = "Unsuppress appearance";

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
        .map(|(n, c, _, kg_m3, _)| LibraryEntry { library: "Onshape Material Library".into(), name: n.to_string(), category: c.to_string(), density: *kg_m3 })
        .collect();
    for (_, lib_name, lib) in noshape_model::material_library::libraries(doc) {
        out.extend(lib.materials.iter().map(|m| LibraryEntry { library: lib_name.clone(), name: m.name.clone(), category: m.category.clone(), density: m.density }));
    }
    out
}
