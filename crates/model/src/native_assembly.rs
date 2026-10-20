//! Import of a native CAD assembly as Onshape does it (live G38, G38f): a
//! flat zip named like the root assembly, holding it and the part files it
//! references by name, becomes a Part Studio "<root> parts" with one Import
//! feature (every part once, at its own origin, named after its component's
//! first instance: "Shaft-1") and an Assembly "<root>" whose instances
//! ("Shaft-1 <1>") sit where the file places them, not fixed, without mates.
//! Parts follow the order of their files in the zip.

use crate::assembly::{Assembly, Instance, InstanceId, InstanceSource, Placement};
use crate::configuration::ConfigValues;
use crate::document::{Document, TabContent, TabId};
use crate::features::importing::ImportPartInfo;
use crate::params::ParamValue;
use crate::partstudio::PartStudio;
use crate::regen::{RegenOptions, Regenerator};
use noshape_kernel as k;
use noshape_units::UnitPrefs;

/// One part file read for an assembly: its name as the assembly references
/// it, the configuration read, and its bodies.
pub struct PartFile {
    pub file: String,
    pub configuration: Option<i32>,
    pub bodies: Vec<k::ImportedShape>,
}

/// Onshape's message when no referenced part file came with the assembly.
pub const NO_GEOMETRY: &str = "Imported file contains no translatable geometry";

/// Add the Part Studio and the Assembly of a native assembly to `doc`;
/// returns (Part Studio, Assembly) tab ids. `components`: the assembly's tree
/// (`k::read_native_assembly`); `files`: the part files in zip order.
pub fn add_native_assembly(doc: &mut Document, root: &str, components: &[k::NativeComponent], files: &[PartFile], import_appearances: bool, units: &UnitPrefs) -> Result<(TabId, TabId), String> {
    // Each component's placement in the root: its parents' composed.
    let mut world: Vec<glam::DMat4> = Vec::with_capacity(components.len());
    for c in components {
        let parent = c.parent.and_then(|p| world.get(p)).copied().unwrap_or(glam::DMat4::IDENTITY);
        world.push(parent * c.transform);
    }
    let same = |a: &str, b: &str| a.eq_ignore_ascii_case(b);
    // A component's part file, in the configuration it uses.
    let file_of = |c: &k::NativeComponent| files.iter().position(|f| same(&f.file, &c.file) && (f.configuration == c.configuration_id || f.configuration.is_none()));
    let used: Vec<(usize, &k::NativeComponent)> = components.iter().enumerate().filter(|(_, c)| !c.assembly && !c.suppressed).collect();
    // The parts, in zip order: (file, part name, the components using it).
    let mut parts: Vec<(usize, String, Vec<usize>)> = Vec::new();
    for (fi, _) in files.iter().enumerate() {
        let users: Vec<usize> = used.iter().filter(|(_, c)| file_of(c) == Some(fi)).map(|(i, _)| *i).collect();
        let Some(first) = users.iter().map(|&i| &components[i]).min_by_key(|c| c.number) else { continue };
        parts.push((fi, format!("{}-{}", first.name, first.number), users));
    }
    if parts.is_empty() {
        return Err(NO_GEOMETRY.into());
    }
    // The Part Studio: one Import feature with every part's bodies, the
    // first solid of each named after its component.
    let mut shapes = Vec::new();
    let mut info = Vec::new();
    let mut solid_of: Vec<Option<usize>> = Vec::new();
    for (fi, name, _) in &parts {
        let mut named = None;
        for b in &files[*fi].bodies {
            let solid = named.is_none() && b.shape.count(k::ShapeType::Face) > 0;
            if solid {
                named = Some(shapes.len());
            }
            info.push(ImportPartInfo { name: if solid { Some(name.clone()) } else { None }, color: b.color, opacity: b.opacity, faces: b.face_colors.clone() });
            shapes.push(b.shape.clone());
        }
        solid_of.push(named);
    }
    let brep = k::Shape::compound(&shapes).to_brep_bytes().map_err(|e| e.to_string())?;
    let mut ps = PartStudio::new();
    let mut f = ps.new_feature("importForeign", units).ok_or("no Import feature")?;
    f.params.set("allowFaultyParts", ParamValue::Bool(true));
    f.params.set("blobData", ParamValue::Text(String::from_utf8_lossy(&brep).to_string()));
    f.params.set("partInfo", ParamValue::Text(serde_json::to_string(&info).map_err(|e| e.to_string())?));
    f.params.set("foreignId", ParamValue::Text(format!("{root}.zip")));
    f.params.set("importAppearances", ParamValue::Bool(import_appearances));
    ps.insert_at_rollback(f);
    let regen = Regenerator::new().regenerate(&ps, &ConfigValues::default(), units, &RegenOptions::default());
    let studio_name = unique_name(doc, &format!("{root} parts"));
    let studio = doc.add_tab(&studio_name, TabContent::PartStudio(ps));
    // The Assembly: each part's instances in the assembly's order.
    let mut asm = Assembly::default();
    for (pi, (_, name, users)) in parts.iter().enumerate() {
        let Some(body) = solid_of[pi].and_then(|_| regen.context.bodies.iter().find(|b| &b.name == name)).map(|b| b.id.clone()) else { continue };
        for (k, &ci) in users.iter().enumerate() {
            let id = InstanceId(asm.next_id());
            let mut inst = Instance::new(id, format!("{name} <{}>", k + 1), InstanceSource::Part { studio, body: body.clone(), config: ConfigValues::default() }, Placement::from_dmat4(&world[ci]));
            inst.hidden = components[ci].hidden;
            asm.instances.push(inst);
        }
    }
    let asm_name = unique_name(doc, root);
    let assembly = doc.add_tab(&asm_name, TabContent::Assembly(Box::new(asm)));
    // The Assembly tab comes first, then its parts' Part Studio (live G38).
    let order = &mut doc.settings.tab_order;
    if let (Some(a), Some(s)) = (order.iter().position(|t| *t == assembly), order.iter().position(|t| *t == studio)) {
        let id = order.remove(a);
        order.insert(s, id);
    }
    Ok((studio, assembly))
}

/// `base`, or Onshape's next free variant of it when a tab already has it.
fn unique_name(doc: &Document, base: &str) -> String {
    if doc.tabs.iter().any(|t| t.name == base) { doc.next_tab_name(base) } else { base.to_string() }
}
