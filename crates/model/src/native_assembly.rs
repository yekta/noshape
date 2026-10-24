//! Import of a native CAD assembly as Onshape does it (live G38, G38f): a
//! flat zip named like the root assembly, holding it and the part files it
//! references by name, becomes a Part Studio "<zip> parts" with one Import
//! feature (every part once, at its own origin) and an Assembly whose
//! instances ("<part> <1>") sit where the file places them, not fixed,
//! without mates. Names and order depend on the CAD system:
//! - SOLIDWORKS (G41): Assembly "<root>"; parts named after their
//!   component's first instance ("Shaft-1"), in the reverse of their first
//!   use, each with its thread curves first as a wire of the same name;
//!   instances grouped by part in that order.
//! - Inventor (G42): Assembly "<root>.iam"; parts "<ipt>__Solid1"; the
//!   occurrences in the reverse of the assembly's browser order, numbered per
//!   part in that order, parts in their first use; one .iam in the zip may
//!   have any zip name; an assembly of a single occurrence becomes only a
//!   Part Studio "<zip>".
//! - Solid Edge (G43): Assembly "<root>.asm"; parts "<file>.par"; the
//!   occurrences in the reverse of the assembly's order, as Inventor.
//!
//! "Combine to a single Part Studio" instead gives one Part Studio "<root>"
//! with every instance's bodies at their assembly positions (live G41 (5)).

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
/// A zip of STEP files: Onshape uploads it but doesn't translate it (live
/// qa5 pass 7: a BLOB tab, notification "<zip> was not translated. …").
pub const STEP_IN_ZIP: &str = "Translation is not supported for STEP (.stp) files.";

/// A native assembly as Onshape imports it (live G38): a flat zip named like
/// its root assembly, with the part files the assembly references by name.
pub struct AssemblyZip {
    /// The root assembly's name without its extension ("Wrench").
    pub root: String,
    /// The root assembly's file name ("Wrench.iam").
    pub root_file: String,
    /// The zip's name without ".zip".
    pub zip: String,
    pub system: NativeSystem,
    pub components: Vec<k::NativeComponent>,
    pub files: Vec<PartFile>,
}

/// The CAD system a native assembly comes from (Onshape names and orders
/// the import per system).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeSystem {
    Solidworks,
    Inventor,
    SolidEdge,
}

impl NativeSystem {
    /// By the root assembly's extension.
    pub fn of(file: &str) -> Option<NativeSystem> {
        let ext = file.rsplit_once('.')?.1.to_ascii_lowercase();
        match ext.as_str() {
            "sldasm" => Some(NativeSystem::Solidworks),
            "iam" => Some(NativeSystem::Inventor),
            "asm" => Some(NativeSystem::SolidEdge),
            _ => None,
        }
    }
}

/// The tabs a native assembly import added.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeTabs {
    /// The Assembly and its parts' Part Studio.
    Assembly { studio: TabId, assembly: TabId },
    /// Only a Part Studio (an Inventor assembly of one occurrence, live G42).
    Studio(TabId),
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
        // The assembly named like the zip; Inventor: also the only .iam,
        // whatever the zip's name (live G42).
        let assemblies: Vec<&String> = names.iter().filter(|n| NativeSystem::of(n).is_some()).collect();
        let iams: Vec<&&String> = assemblies.iter().filter(|n| NativeSystem::of(n) == Some(NativeSystem::Inventor)).collect();
        let found = assemblies.iter().find(|n| split(n).0.eq_ignore_ascii_case(&stem)).or(if iams.len() == 1 { iams.first().copied() } else { None });
        let Some(root) = found.copied() else {
            let step = |n: &String| matches!(split(n).1.as_str(), "step" | "stp");
            return Err(if names.iter().any(step) { STEP_IN_ZIP } else { NO_ROOT }.to_string());
        };
        let system = NativeSystem::of(root).ok_or(NO_ROOT)?;
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
        Ok(AssemblyZip { root: split(root).0, root_file: root.clone(), zip: stem, system, components, files })
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

/// Add a native assembly zip's tabs as Onshape imports it for its system
/// (see the module docs).
pub fn add_native_zip(doc: &mut Document, z: &AssemblyZip, import_appearances: bool, units: &UnitPrefs) -> Result<NativeTabs, String> {
    if z.system == NativeSystem::Solidworks {
        let (studio, assembly) = add_native_assembly(doc, &z.root, &z.components, &z.files, import_appearances, units)?;
        return Ok(NativeTabs::Assembly { studio, assembly });
    }
    let world = world_placements(&z.components);
    let files = &z.files;
    let file_of = |c: &k::NativeComponent| files.iter().position(|f| f.file.eq_ignore_ascii_case(&c.file) && (f.configuration == c.configuration_id || f.configuration.is_none()));
    // The occurrences in the reverse of the assembly's order, and the parts
    // in their first use there.
    let used: Vec<(usize, usize)> = z.components.iter().enumerate().rev().filter(|(_, c)| !c.assembly && !c.suppressed).filter_map(|(i, c)| Some((i, file_of(c)?))).collect();
    let mut order: Vec<usize> = Vec::new();
    for &(_, fi) in &used {
        if !order.contains(&fi) {
            order.push(fi);
        }
    }
    if order.is_empty() {
        return Err(NO_GEOMETRY.into());
    }
    // Part names: Inventor "<ipt>__Solid<n>" for every solid of the file,
    // Solid Edge the file's name.
    let base = |fi: usize| std::path::Path::new(&files[fi].file).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let solid_name = |fi: usize, n: usize| -> Option<String> {
        match z.system {
            NativeSystem::Inventor => Some(format!("{}__Solid{}", base(fi).rsplit_once('.').map(|x| x.0.to_string()).unwrap_or_else(|| base(fi)), n + 1)),
            _ => (n == 0).then(|| base(fi)),
        }
    };
    let mut shapes = Vec::new();
    let mut info = Vec::new();
    let mut part_names: Vec<Option<String>> = Vec::new();
    for &fi in &order {
        let first = solid_name(fi, 0).unwrap_or_default();
        let named = part_bodies_named(&files[fi], Some(&first), None, &mut shapes, &mut info, &|n| solid_name(fi, n))?;
        part_names.push(named.map(|_| first));
    }
    let brep = k::Shape::compound(&shapes).to_brep_bytes().map_err(|e| e.to_string())?;
    let mut ps = PartStudio::new();
    let mut f = ps.new_feature("importForeign", units).ok_or("no Import feature")?;
    f.params.set("allowFaultyParts", ParamValue::Bool(true));
    f.params.set("blobData", ParamValue::Text(String::from_utf8_lossy(&brep).to_string()));
    f.params.set("partInfo", ParamValue::Text(serde_json::to_string(&info).map_err(|e| e.to_string())?));
    f.params.set("foreignId", ParamValue::Text(format!("{}.zip", z.zip)));
    f.params.set("importAppearances", ParamValue::Bool(import_appearances));
    ps.insert_at_rollback(f);
    // An Inventor assembly of one occurrence: only a Part Studio "<zip>".
    if z.system == NativeSystem::Inventor && used.len() == 1 {
        let name = unique_name(doc, &z.zip);
        return Ok(NativeTabs::Studio(doc.add_tab(&name, TabContent::PartStudio(ps))));
    }
    let regen = Regenerator::new().regenerate(&ps, &ConfigValues::default(), units, &RegenOptions::default());
    let studio_name = unique_name(doc, &format!("{} parts", z.zip));
    let studio = doc.add_tab(&studio_name, TabContent::PartStudio(ps));
    let mut asm = Assembly::default();
    let mut counts = vec![0usize; order.len()];
    for &(ci, fi) in &used {
        let Some(pi) = order.iter().position(|&o| o == fi) else { continue };
        let Some(name) = &part_names[pi] else { continue };
        let Some(body) = regen.context.bodies.iter().find(|b| &b.name == name && b.kind != crate::display::BodyKind::Wire).map(|b| b.id.clone()) else { continue };
        counts[pi] += 1;
        let id = InstanceId(asm.next_id());
        let mut inst = Instance::new(id, format!("{name} <{}>", counts[pi]), InstanceSource::Part { studio, body, config: ConfigValues::default() }, Placement::from_dmat4(&world[ci]));
        inst.hidden = z.components[ci].hidden;
        asm.instances.push(inst);
    }
    let asm_name = unique_name(doc, &z.root_file);
    let assembly = doc.add_tab(&asm_name, TabContent::Assembly(Box::new(asm)));
    let order = &mut doc.settings.tab_order;
    if let (Some(a), Some(s)) = (order.iter().position(|t| *t == assembly), order.iter().position(|t| *t == studio)) {
        let id = order.remove(a);
        order.insert(s, id);
    }
    Ok(NativeTabs::Assembly { studio, assembly })
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
    part_bodies_named(file, Some(name), at, shapes, info, &|n| (n == 0).then(|| name.to_string()))
}

/// [`part_bodies`] with the wires named `wire_name` and the `n`-th solid
/// named `solid_name(n)`.
fn part_bodies_named(file: &PartFile, wire_name: Option<&str>, at: Option<&glam::DMat4>, shapes: &mut Vec<k::Shape>, info: &mut Vec<ImportPartInfo>, solid_name: &dyn Fn(usize) -> Option<String>) -> Result<Option<usize>, String> {
    let place = |s: &k::Shape| match at {
        Some(m) => s.transformed(&k::Transform::Matrix(*m)).map_err(|e| e.to_string()),
        None => Ok(s.clone()),
    };
    let is_wire = |b: &k::ImportedShape| b.shape.count(k::ShapeType::Face) == 0;
    for b in file.bodies.iter().filter(|b| is_wire(b)) {
        info.push(ImportPartInfo { name: wire_name.map(str::to_string), color: b.color, opacity: b.opacity, faces: Vec::new() });
        shapes.push(place(&b.shape)?);
    }
    let mut named = None;
    for (n, b) in file.bodies.iter().filter(|b| !is_wire(b)).enumerate() {
        if n == 0 {
            named = Some(shapes.len());
        }
        info.push(ImportPartInfo { name: solid_name(n), color: b.color, opacity: b.opacity, faces: b.face_colors.clone() });
        shapes.push(place(&b.shape)?);
    }
    Ok(named)
}

/// `base`, or Onshape's "<base> (1)", "(2)", … when a tab already has it
/// (live G41: a second import of an assembly; "api (10-23) import parity":
/// a second import of a file's Part Studio).
pub fn unique_name(doc: &Document, base: &str) -> String {
    if !doc.tabs.iter().any(|t| t.name == base) {
        return base.to_string();
    }
    (1..).map(|n| format!("{base} ({n})")).find(|n| !doc.tabs.iter().any(|t| &t.name == n)).unwrap_or_else(|| base.to_string())
}

/// A .pvz file read for [`add_pvz_assembly`].
pub struct PvzImport {
    /// The file name without extension, and the product tree's root name.
    pub stem: String,
    pub root: String,
    /// The Import feature's data: the parts in their own coordinates as BREP
    /// text and their `ImportPartInfo` JSON.
    pub brep: String,
    pub info: String,
    /// The file name (the feature's foreignId).
    pub foreign_id: String,
    pub instances: Vec<k::PvzInstance>,
    /// The parts' names (index = `PvzInstance::part`).
    pub names: Vec<Option<String>>,
}

/// A PVZ (Creo View) file of several part instances, as Onshape imports it
/// (live qa5 zip import, G22): a Part Studio "<file> parts" with one Import
/// feature holding each distinct part once as mesh bodies, named as in the
/// file (open meshes become a composite part), and an Assembly named like
/// the product tree's root ("RootAssembly") whose instances ("Cylinder <1>")
/// sit where the file places them; composite parts aren't instanced. The
/// Assembly tab comes first. `brep` / `info` are the Import feature's data
/// (the parts in their own coordinates, see `noshape_kernel::read_pvz_parts`).
pub fn add_pvz_assembly(doc: &mut Document, pvz: &PvzImport, units: &UnitPrefs) -> Result<(TabId, TabId), String> {
    let PvzImport { stem, root, brep, info, foreign_id, instances, names } = pvz;
    let (brep, info) = (brep.clone(), info.clone());
    let mut ps = PartStudio::new();
    let mut f = ps.new_feature("importForeign", units).ok_or("no Import feature")?;
    f.params.set("allowFaultyParts", ParamValue::Bool(true));
    f.params.set("blobData", ParamValue::Text(brep));
    f.params.set("partInfo", ParamValue::Text(info));
    f.params.set("foreignId", ParamValue::Text(foreign_id.clone()));
    ps.insert_at_rollback(f);
    let regen = Regenerator::new().regenerate(&ps, &ConfigValues::default(), units, &RegenOptions::default());
    let studio_name = unique_name(doc, &format!("{stem} parts"));
    let studio = doc.add_tab(&studio_name, TabContent::PartStudio(ps));
    let mut asm = Assembly::default();
    let mut count: Vec<u32> = vec![0; names.len()];
    // The occurrences in the reverse of the file's order (live: pvz-rot-asm's
    // <1> is its second, turned occurrence), as native assemblies.
    for inst in instances.iter().rev() {
        let Some(Some(name)) = names.get(inst.part) else { continue };
        // The part's solid (mesh parts whose mesh is open are composites: not instanced).
        let Some(body) = regen.context.bodies.iter().find(|b| &b.name == name && b.kind == crate::display::BodyKind::Solid).map(|b| b.id.clone()) else { continue };
        count[inst.part] += 1;
        let id = InstanceId(asm.next_id());
        asm.instances.push(Instance::new(id, format!("{} <{}>", inst.name, count[inst.part]), InstanceSource::Part { studio, body, config: ConfigValues::default() }, Placement::from_dmat4(&inst.transform)));
    }
    let assembly = doc.add_tab(&pvz_assembly_name(doc, root), TabContent::Assembly(Box::new(asm)));
    let order = &mut doc.settings.tab_order;
    if let (Some(a), Some(s)) = (order.iter().position(|t| *t == assembly), order.iter().position(|t| *t == studio)) {
        let id = order.remove(a);
        order.insert(s, id);
    }
    Ok((studio, assembly))
}

/// The Assembly's name from the product tree's root name (live: "pvz rot
/// asm_54" comes in as "pvz rot asm", or as "pvz rot asm_54" when that name
/// is taken; a root without the suffix as "RootAssembly (1)").
fn pvz_assembly_name(doc: &Document, root: &str) -> String {
    let taken = |n: &str| doc.tabs.iter().any(|t| t.name == n);
    let plain = match root.rsplit_once('_') {
        Some((head, tail)) if !head.is_empty() && !tail.is_empty() && tail.bytes().all(|b| b.is_ascii_digit()) => head,
        _ => root,
    };
    if !taken(plain) {
        plain.to_string()
    } else if plain != root && !taken(root) {
        root.to_string()
    } else {
        unique_name(doc, plain)
    }
}
