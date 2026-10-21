//! Import of a native CAD assembly as Onshape does it (live G38, G38f): a
//! flat zip named like the root assembly, holding it and the part files it
//! references by name, becomes a Part Studio "<root> parts" with one Import
//! feature (every part once, at its own origin, named after its component's
//! first instance: "Shaft-1") and an Assembly "<root>" whose instances
//! ("Shaft-1 <1>") sit where the file places them, not fixed, without mates.
//! Parts come in the reverse of their first use in the assembly, each with
//! its thread curves first as a wire of the same name (live G41); instances
//! are grouped by part in that order. "Combine to a single Part Studio"
//! instead gives one Part Studio "<root>" with every instance's bodies at
//! their assembly positions (live G41 (5)).

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
/// Onshape's message when the zip has no assembly named like it, or folders.
pub const NO_ROOT: &str = "Root assembly file not found. Ensure zip file name matches root assembly name and the archive does not have folders";

/// A native assembly as Onshape imports it (live G38): a flat zip named like
/// its root assembly, with the part files the assembly references by name.
pub struct AssemblyZip {
    pub root: String,
    pub components: Vec<k::NativeComponent>,
    pub files: Vec<PartFile>,
}

/// Read an assembly zip: its root assembly's tree and every part file it
/// uses, once per configuration used, in zip order.
pub fn read_assembly_zip(path: &std::path::Path) -> Result<AssemblyZip, String> {
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let dir = std::env::temp_dir().join(format!("noshape-zip-{}-{stamp}", std::process::id()));
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let result = (|| {
        let names = k::extract_zip(path, &dir).map_err(|e| e.to_string())?;
        let stem = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        if names.iter().any(|n| n.contains('/') || n.contains('\\')) {
            return Err(NO_ROOT.to_string());
        }
        let split = |n: &str| n.rsplit_once('.').map(|(a, b)| (a.to_string(), b.to_ascii_lowercase())).unwrap_or((n.to_string(), String::new()));
        let root = names
            .iter()
            .find(|n| {
                let (base, ext) = split(n);
                // SOLIDWORKS, or Solid Edge (live G43 pending).
                (ext == "sldasm" || ext == "asm") && base.eq_ignore_ascii_case(&stem)
            })
            .ok_or(NO_ROOT)?;
        let components = k::read_native_assembly(&dir.join(root)).map_err(|e| e.to_string())?;
        let mut files = Vec::new();
        for n in &names {
            let mut configs: Vec<Option<i32>> = Vec::new();
            for c in components.iter().filter(|c| !c.assembly && c.file.eq_ignore_ascii_case(n)) {
                if !configs.contains(&c.configuration_id) {
                    configs.push(c.configuration_id);
                }
            }
            for configuration in configs {
                let bodies = k::read_native_parts_configuration(&dir.join(n), k::LengthUnit::Meter, configuration).map_err(|e| e.to_string())?;
                files.push(PartFile { file: n.clone(), configuration, bodies });
            }
        }
        if files.is_empty() {
            return Err(NO_GEOMETRY.to_string());
        }
        Ok(AssemblyZip { root: split(root).0, components, files })
    })();
    let _ = std::fs::remove_dir_all(&dir);
    result
}

/// Add the Part Studio and the Assembly of a native assembly to `doc`;
/// returns (Part Studio, Assembly) tab ids. `components`: the assembly's tree
/// (`k::read_native_assembly`); `files`: the part files in zip order.
pub fn add_native_assembly(doc: &mut Document, root: &str, components: &[k::NativeComponent], files: &[PartFile], import_appearances: bool, units: &UnitPrefs) -> Result<(TabId, TabId), String> {
    let world = world_placements(components);
    let same = |a: &str, b: &str| a.eq_ignore_ascii_case(b);
    // A component's part file, in the configuration it uses.
    let file_of = |c: &k::NativeComponent| files.iter().position(|f| same(&f.file, &c.file) && (f.configuration == c.configuration_id || f.configuration.is_none()));
    let used: Vec<(usize, &k::NativeComponent)> = components.iter().enumerate().filter(|(_, c)| !c.assembly && !c.suppressed).collect();
    let parts = parts_of(components, files, &used, &file_of);
    if parts.is_empty() {
        return Err(NO_GEOMETRY.into());
    }
    // The Part Studio: one Import feature with every part's bodies.
    let mut shapes = Vec::new();
    let mut info = Vec::new();
    let mut solid_of: Vec<Option<usize>> = Vec::new();
    for (fi, name, _) in &parts {
        let named = part_bodies(&files[*fi], name, None, &mut shapes, &mut info)?;
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
        let Some(body) = solid_of[pi].and_then(|_| regen.context.bodies.iter().find(|b| &b.name == name && b.kind != crate::display::BodyKind::Wire)).map(|b| b.id.clone()) else { continue };
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

/// "Combine to a single Part Studio" (live G41 (5)): one Part Studio
/// "<root>" with one Import feature holding every instance's bodies at its
/// assembly position, in the assembly's order; a part's first instance keeps
/// its name, the next ones get "_1", "_2", …; no Assembly. Returns the tab.
pub fn add_native_assembly_combined(doc: &mut Document, root: &str, components: &[k::NativeComponent], files: &[PartFile], import_appearances: bool, units: &UnitPrefs) -> Result<TabId, String> {
    let world = world_placements(components);
    let same = |a: &str, b: &str| a.eq_ignore_ascii_case(b);
    let file_of = |c: &k::NativeComponent| files.iter().position(|f| same(&f.file, &c.file) && (f.configuration == c.configuration_id || f.configuration.is_none()));
    let used: Vec<(usize, &k::NativeComponent)> = components.iter().enumerate().filter(|(_, c)| !c.assembly && !c.suppressed).collect();
    let parts = parts_of(components, files, &used, &file_of);
    let mut shapes = Vec::new();
    let mut info = Vec::new();
    let mut seen: Vec<(usize, usize)> = Vec::new();
    for (ci, c) in &used {
        let Some((fi, name, _)) = file_of(c).and_then(|fi| parts.iter().find(|p| p.0 == fi)) else { continue };
        let repeat = match seen.iter_mut().find(|(f, _)| f == fi) {
            Some((_, n)) => {
                *n += 1;
                *n
            }
            None => {
                seen.push((*fi, 0));
                0
            }
        };
        let name = if repeat == 0 { name.clone() } else { format!("{name}_{repeat}") };
        part_bodies(&files[*fi], &name, Some(&world[*ci]), &mut shapes, &mut info)?;
    }
    if shapes.is_empty() {
        return Err(NO_GEOMETRY.into());
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
    let name = unique_name(doc, root);
    Ok(doc.add_tab(&name, TabContent::PartStudio(ps)))
}

/// Each component's placement in the root: its parents' composed.
fn world_placements(components: &[k::NativeComponent]) -> Vec<glam::DMat4> {
    let mut world: Vec<glam::DMat4> = Vec::with_capacity(components.len());
    for c in components {
        let parent = c.parent.and_then(|p| world.get(p)).copied().unwrap_or(glam::DMat4::IDENTITY);
        world.push(parent * c.transform);
    }
    world
}

/// The parts (file, part name, the components using it): one per part file
/// and configuration used, in the reverse of their first use in the
/// assembly (live G41: the NIST box, used first, is the last part), named
/// "<component>-<n>" after the component's first instance.
fn parts_of(components: &[k::NativeComponent], files: &[PartFile], used: &[(usize, &k::NativeComponent)], file_of: &dyn Fn(&k::NativeComponent) -> Option<usize>) -> Vec<(usize, String, Vec<usize>)> {
    let mut order: Vec<usize> = Vec::new();
    for (_, c) in used {
        if let Some(fi) = file_of(c).filter(|fi| !order.contains(fi) && *fi < files.len()) {
            order.push(fi);
        }
    }
    order.reverse();
    order
        .into_iter()
        .filter_map(|fi| {
            let users: Vec<usize> = used.iter().filter(|(_, c)| file_of(c) == Some(fi)).map(|(i, _)| *i).collect();
            let first = users.iter().map(|&i| &components[i]).min_by_key(|c| c.number)?;
            Some((fi, format!("{}-{}", first.name, first.number), users))
        })
        .collect()
}

/// Add a part file's bodies (moved by `at`) for the Import feature: its
/// curves first as wires named like the part (live G41: a fastener's
/// cosmetic thread), then its solids, the first named like the part.
/// Returns the index of the named solid among `shapes`.
fn part_bodies(file: &PartFile, name: &str, at: Option<&glam::DMat4>, shapes: &mut Vec<k::Shape>, info: &mut Vec<ImportPartInfo>) -> Result<Option<usize>, String> {
    let place = |s: &k::Shape| match at {
        Some(m) => s.transformed(&k::Transform::Matrix(*m)).map_err(|e| e.to_string()),
        None => Ok(s.clone()),
    };
    let is_wire = |b: &k::ImportedShape| b.shape.count(k::ShapeType::Face) == 0;
    for b in file.bodies.iter().filter(|b| is_wire(b)) {
        info.push(ImportPartInfo { name: Some(name.to_string()), color: b.color, opacity: b.opacity, faces: Vec::new() });
        shapes.push(place(&b.shape)?);
    }
    let mut named = None;
    for b in file.bodies.iter().filter(|b| !is_wire(b)) {
        let first = named.is_none();
        if first {
            named = Some(shapes.len());
        }
        info.push(ImportPartInfo { name: first.then(|| name.to_string()), color: b.color, opacity: b.opacity, faces: b.face_colors.clone() });
        shapes.push(place(&b.shape)?);
    }
    Ok(named)
}

/// `base`, or Onshape's "<base> (1)", "(2)", … when a tab already has it
/// (live G41: a second import of an assembly).
fn unique_name(doc: &Document, base: &str) -> String {
    if !doc.tabs.iter().any(|t| t.name == base) {
        return base.to_string();
    }
    (1..).map(|n| format!("{base} ({n})")).find(|n| !doc.tabs.iter().any(|t| &t.name == n)).unwrap_or_else(|| base.to_string())
}
