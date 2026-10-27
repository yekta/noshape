//! Importing CAD and mesh files (the Import dialog): each file translated
//! to an Import feature in a new Part Studio (or all in one with Combine),
//! native assembly zips to an Assembly and its Part Studio, the files kept
//! as file tabs in "CAD Imports". GUI-free; from
//! `crates/app/src/import_dialog.rs`, `workspace.rs` and `file_tab_view.rs`.

use noshape_model::document::{Document, TabContent, TabId};
use noshape_model::file_tab::FileTab;
use noshape_model::{ParamValue, PartStudio};
use noshape_units::{LengthUnit, UnitPrefs};
use std::path::{Path, PathBuf};

/// File types "Import files…" translates.
pub const IMPORT_EXTS: &[&str] = &["step", "stp", "iges", "igs", "brep", "stl", "obj", "pvz", "x_t", "x_b", "xmt_txt", "xmt_bin", "3dm", "sldprt", "par", "psm", "prt", "ipt", "sat", "sab", "3mf", "gltf", "glb", "jt", "zip"];
/// Native part files (their own dialog variant).
pub const NATIVE_PARTS: &[&str] = &["sldprt", "par", "psm", "prt", "ipt"];
/// Files whose import dialog offers "Import material density": Onshape's
/// own list (its import service's TYPES_THAT_SUPPORT_MATERIAL_DENSITY,
/// spelling included); shown when any of the files is one (STEP, SAT, PVZ,
/// Inventor and meshes never: live part-studio/2394, qa4 IMP-DENS).
pub const MATERIAL_DENSITY_TYPES: &[&str] = &["sldprt", "sldasm", "zip", "psm", "par", "x_t", "x_b", "xmm_bin", "xmm_txt", "catpart", "catproduct", "prt", "asm"];

/// Whether the import dialog for `paths` shows "Import material density".
pub fn offers_material_density(paths: &[std::path::PathBuf]) -> bool {
    paths.iter().any(|p| p.extension().is_some_and(|e| MATERIAL_DENSITY_TYPES.contains(&e.to_string_lossy().to_lowercase().as_str())))
}

/// Mesh files without units: Onshape asks for them (live G35, G35b).
pub const MESH_FILES: &[&str] = &["stl", "obj"];
/// The Units select, as Onshape lists units in its STL dialogs.
pub const MESH_UNITS: [(&str, LengthUnit); 6] = [
    ("Centimeter", LengthUnit::Centimeter),
    ("Foot", LengthUnit::Foot),
    ("Inch", LengthUnit::Inch),
    ("Meter", LengthUnit::Meter),
    ("Millimeter", LengthUnit::Millimeter),
    ("Yard", LengthUnit::Yard),
];

/// The Import dialog's choices.
#[derive(Clone, Copy, Debug, Default)]
pub struct ImportChoice {
    /// "Combine to a single Part Studio" (else "Import to this document").
    pub combine: bool,
    pub appearances: bool,
    pub density: bool,
    pub y_up: bool,
    /// Mesh files: index into [`MESH_UNITS`].
    pub unit: usize,
    pub composite: bool,
    pub join: bool,
}

impl ImportChoice {
    /// The dialog's starting choices for `paths` in a document of `doc_unit`
    /// (Import appearances on; OBJ in meters, other meshes in the document's unit).
    pub fn defaults(paths: &[PathBuf], doc_unit: LengthUnit) -> Self {
        let obj = !paths.is_empty() && paths.iter().all(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("obj")));
        let default_unit = if obj { LengthUnit::Meter } else { doc_unit };
        let unit = MESH_UNITS.iter().position(|(_, u)| *u == default_unit).unwrap_or(4);
        ImportChoice { appearances: true, unit, ..Default::default() }
    }
}

/// Tab (and document) name of an imported file: the file name without its
/// extension; and the full file name.
pub fn import_names(path: &Path) -> (String, String) {
    let file = path.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default();
    let stem = path.file_stem().map(|f| f.to_string_lossy().to_string()).unwrap_or_else(|| file.clone());
    (stem, file)
}

/// A translated CAD file: its shapes as BREP text (a compound, one child per
/// part when `parts` is set) and what the file says about each part (JSON
/// list of `ImportPartInfo`, empty when the format names none).
#[derive(Clone)]
pub struct Foreign {
    pub brep: String,
    pub parts: String,
}

/// Read a CAD or mesh file as BREP text. Mesh files carry no unit: they are
/// read in `units`' length unit.
pub fn read_foreign(path: &Path, units: &UnitPrefs) -> anyhow::Result<Foreign> {
    use noshape_kernel as k;
    let mesh_unit = match units.length {
        LengthUnit::Centimeter => k::LengthUnit::Centimeter,
        LengthUnit::Meter => k::LengthUnit::Meter,
        LengthUnit::Inch => k::LengthUnit::Inch,
        LengthUnit::Foot => k::LengthUnit::Foot,
        LengthUnit::Yard => k::LengthUnit::Yard,
        _ => k::LengthUnit::Millimeter,
    };
    let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    // Parts with names and colours: one compound child each, described in `info`.
    let mut info: Vec<noshape_model::features::importing::ImportPartInfo> = Vec::new();
    let mut compound = |parts: Vec<k::ImportedShape>| {
        info = parts.iter().map(|p| noshape_model::features::importing::ImportPartInfo { name: p.name.clone(), color: p.color, opacity: p.opacity, faces: p.face_colors.clone() }).collect();
        k::Shape::compound(&parts.into_iter().map(|p| p.shape).collect::<Vec<_>>())
    };
    let shape = match ext.as_str() {
        "step" | "stp" => compound(k::read_step(path, k::LengthUnit::Meter)?),
        "iges" | "igs" => k::read_iges(path, k::LengthUnit::Meter)?,
        "brep" => k::read_brep(path)?,
        // Onshape imports STL, OBJ and PVZ as mesh parts (PVZ is in metres).
        "stl" | "obj" => k::read_mesh_bodies(path, mesh_unit)?,
        // 3MF and glTF carry their units (read in millimetres).
        "3mf" | "gltf" | "glb" => k::read_mesh_bodies(path, k::LengthUnit::Millimeter)?,
        // Every placed instance, named like it (a single instance, or Combine).
        "pvz" => {
            let parts = k::read_pvz_parts(path, k::LengthUnit::Meter)?;
            let (_, instances) = k::read_pvz_instances(path)?;
            let placed = instances
                .iter()
                .filter_map(|i| {
                    let p = parts.get(i.part)?;
                    Some(p.shape.transformed(&k::Transform::Matrix(i.transform)).map(|shape| k::ImportedShape { shape, name: Some(i.name.clone()), ..p.clone() }))
                })
                .collect::<Result<Vec<_>, _>>()?;
            compound(placed)
        }
        "x_t" | "x_b" | "xmt_txt" | "xmt_bin" => compound(k::read_parasolid_parts(path, k::LengthUnit::Meter)?),
        "3dm" => compound(k::read_rhino_parts(path, k::LengthUnit::Meter)?),
        // Native CAD parts: the geometry they embed (Parasolid: SolidWorks, Solid Edge, NX; ACIS: Inventor).
        "sat" | "sab" => compound(k::read_acis_parts(path, k::LengthUnit::Meter)?),
        "jt" => compound(k::read_jt_parts(path, k::LengthUnit::Meter)?),
        "sldprt" | "par" | "psm" | "prt" | "ipt" => compound(k::read_native_parts(path, k::LengthUnit::Meter)?),
        other => anyhow::bail!("Unsupported file type .{other}"),
    };
    let parts = if info.is_empty() { String::new() } else { serde_json::to_string(&info)? };
    Ok(Foreign { brep: String::from_utf8_lossy(&shape.to_brep_bytes()?).to_string(), parts })
}

/// Append an Import feature ("Import N") holding `brep` to `ps`.
pub fn add_import_feature(ps: &mut PartStudio, file: &str, foreign: Foreign, units: &UnitPrefs) {
    if let Some(mut f) = ps.new_feature("importForeign", units) {
        // Live (documents/64-import-feature-dialog.png): the feature is
        // numbered like any other ("Import 1") and allows faulty parts.
        f.params.set("allowFaultyParts", ParamValue::Bool(true));
        f.params.set("blobData", ParamValue::Text(foreign.brep));
        f.params.set("partInfo", ParamValue::Text(foreign.parts));
        f.params.set("foreignId", ParamValue::Text(file.to_string()));
        ps.insert_at_rollback(f);
    }
}

/// Add a file tab named after the file, in `folder` when given. A second
/// file of the same name keeps it (live: two "g25-b.step" tabs).
pub fn add_file_tab(d: &mut Document, file: FileTab, folder: Option<TabId>, after: Option<TabId>) -> TabId {
    let name = file.file_name.clone();
    let id = d.add_tab(&name, TabContent::File(file));
    crate::docedit::place_after(d, id, after);
    if let Some(f) = folder {
        d.set_parent(id, Some(f));
    }
    id
}

/// The "CAD Imports" folder (Onshape keeps imported CAD files there), made if needed.
pub fn cad_imports_folder(d: &mut Document) -> TabId {
    match d.settings.folders.iter().find(|f| f.name == "CAD Imports") {
        Some(f) => f.id,
        None => d.add_folder("CAD Imports", None),
    }
}

/// A .pvz file of several part instances: a Part Studio "<name> parts" and
/// an Assembly (see `noshape_model::native_assembly::add_pvz_assembly`).
struct PvzAssembly {
    name: String,
    file: String,
    root: String,
    /// The distinct parts in their own coordinates.
    foreign: Foreign,
    names: Vec<Option<String>>,
    instances: Vec<noshape_kernel::PvzInstance>,
    /// From a STEP assembly (else a .pvz).
    step: bool,
}

/// Read a .pvz of several instances for [`PvzAssembly`]; `None` for one
/// instance (an ordinary Part Studio).
fn read_pvz_assembly(path: &Path) -> anyhow::Result<Option<PvzAssembly>> {
    use noshape_kernel as k;
    let (root, instances) = k::read_pvz_instances(path)?;
    if instances.len() < 2 {
        return Ok(None);
    }
    let parts = k::read_pvz_parts(path, k::LengthUnit::Meter)?;
    let info: Vec<noshape_model::features::importing::ImportPartInfo> = parts.iter().map(|p| noshape_model::features::importing::ImportPartInfo { name: p.name.clone(), ..Default::default() }).collect();
    let names = parts.iter().map(|p| p.name.clone()).collect();
    let shape = k::Shape::compound(&parts.into_iter().map(|p| p.shape).collect::<Vec<_>>());
    let foreign = Foreign { brep: String::from_utf8_lossy(&shape.to_brep_bytes()?).to_string(), parts: serde_json::to_string(&info)? };
    let (name, file) = import_names(path);
    Ok(Some(PvzAssembly { name, file, root: if root.is_empty() { "RootAssembly".into() } else { root }, foreign, names, instances, step: false }))
}

/// A STEP file whose root product is an assembly: an Assembly named after
/// the root and a Part Studio "<name> parts" with the distinct parts in
/// their own coordinates (live qa4 r4 s5); `None` for other STEP files.
/// Live ("api (10-25) STEP assembly part order") walks the occurrences in
/// the reverse of the file's order: each product becomes one part at its
/// first occurrence there (NAUOs D, A, B, C give parts C, B, A, D; B, D, C,
/// A, B give B, A, C, D), and the instances are listed and numbered in that
/// order (`add_pvz_assembly` reverses them).
fn read_step_assembly(path: &Path) -> anyhow::Result<Option<PvzAssembly>> {
    use noshape_kernel as k;
    // Only files with assembly usages are read for their product tree (the
    // others are read once, as parts).
    const NAUO: &[u8] = b"NEXT_ASSEMBLY_USAGE_OCCURRENCE";
    if !std::fs::read(path)?.windows(NAUO.len()).any(|w| w.eq_ignore_ascii_case(NAUO)) {
        return Ok(None);
    }
    let Some(a) = k::read_step_assembly(path, k::LengthUnit::Meter)? else { return Ok(None) };
    pvz_of_tree(path, a).map(Some)
}

/// A Parasolid file holding an assembly: imported as a STEP assembly is
/// (live "dx speed: … NIST import": an Assembly named after the top
/// assembly with its 36 instances and a Part Studio "<file> parts" of the
/// 8 distinct parts); `None` for files of bodies only.
fn read_xt_assembly(path: &Path) -> anyhow::Result<Option<PvzAssembly>> {
    use noshape_kernel as k;
    let Some(a) = k::read_parasolid_assembly(path, k::LengthUnit::Meter)? else { return Ok(None) };
    pvz_of_tree(path, a).map(Some)
}

/// An assembly file's tree (STEP, Parasolid) as the Assembly + Part Studio import.
fn pvz_of_tree(path: &Path, (root, parts, mut instances): noshape_kernel::StepAssembly) -> anyhow::Result<PvzAssembly> {
    use noshape_kernel as k;
    let mut order: Vec<usize> = Vec::new();
    for i in instances.iter().rev() {
        if i.part < parts.len() && !order.contains(&i.part) {
            order.push(i.part);
        }
    }
    let unused: Vec<usize> = (0..parts.len()).rev().filter(|p| !order.contains(p)).collect();
    order.extend(unused);
    let mut slots: Vec<Option<k::ImportedShape>> = parts.into_iter().map(Some).collect();
    let parts: Vec<k::ImportedShape> = order.iter().filter_map(|&p| slots[p].take()).collect();
    for i in &mut instances {
        i.part = order.iter().position(|&p| p == i.part).unwrap_or(usize::MAX);
    }
    let info: Vec<noshape_model::features::importing::ImportPartInfo> = parts.iter().map(|p| noshape_model::features::importing::ImportPartInfo { name: p.name.clone(), color: p.color, opacity: p.opacity, faces: p.face_colors.clone() }).collect();
    let names = parts.iter().map(|p| p.name.clone()).collect();
    let shape = k::Shape::compound(&parts.into_iter().map(|p| p.shape).collect::<Vec<_>>());
    let foreign = Foreign { brep: String::from_utf8_lossy(&shape.to_brep_bytes()?).to_string(), parts: serde_json::to_string(&info)? };
    let (name, file) = import_names(path);
    Ok(PvzAssembly { name, file, root, foreign, names, instances, step: true })
}

/// Files read for an import (before the document edit).
pub struct ImportInputs {
    read: Vec<(String, String, Foreign)>,
    pvz: Vec<PvzAssembly>,
    assemblies: Vec<noshape_model::native_assembly::AssemblyZip>,
    files: Vec<FileTab>,
    /// "a.step, b.zip": the history entry is "Import <label>".
    pub label: String,
}

impl ImportInputs {
    /// Nothing to add: no translation and no file tab.
    pub fn is_empty(&self) -> bool {
        self.read.is_empty() && self.pvz.is_empty() && self.assemblies.is_empty() && self.files.is_empty()
    }
}

/// Read and translate the files (mesh files in the chosen unit); files
/// that fail give "Import failed: <path>: <why>".
pub fn read_inputs(paths: &[PathBuf], c: &ImportChoice, keep_files: bool, units: &UnitPrefs) -> (ImportInputs, Vec<String>) {
    let mut errors = Vec::new();
    let mut read = Vec::new();
    // Native assemblies (a zip named like the root assembly; live G38).
    let mut assemblies = Vec::new();
    let mut pvz = Vec::new();
    for path in paths {
        if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("zip")) {
            match noshape_model::native_assembly::read_assembly_zip(path) {
                Ok(a) => assemblies.push(a),
                // Onshape keeps the zip as a file tab and reports it.
                Err(e) if e == noshape_model::native_assembly::STEP_IN_ZIP => errors.push(format!("{} was not translated. {e}", import_names(path).1)),
                Err(e) => errors.push(format!("Import failed: {}: {e:#}", path.display())),
            }
            continue;
        }
        // A .pvz of several instances: Part Studio + Assembly (not when combining).
        if !c.combine && path.extension().is_some_and(|e| e.eq_ignore_ascii_case("pvz")) {
            match read_pvz_assembly(path) {
                Ok(Some(a)) => {
                    pvz.push(a);
                    continue;
                }
                Ok(None) => {}
                Err(e) => {
                    errors.push(format!("Import failed: {}: {e:#}", path.display()));
                    continue;
                }
            }
        }
        // A STEP or Parasolid assembly: Assembly + Part Studio (not when combining).
        let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
        if !c.combine && matches!(ext.as_str(), "step" | "stp" | "x_t" | "x_b") {
            let tree = if ext.starts_with('x') { read_xt_assembly(path) } else { read_step_assembly(path) };
            match tree {
                Ok(Some(a)) => {
                    pvz.push(a);
                    continue;
                }
                Ok(None) => {}
                Err(e) => {
                    errors.push(format!("Import failed: {}: {e:#}", path.display()));
                    continue;
                }
            }
        }
        // Mesh files in the unit chosen in the dialog.
        let units = if MESH_FILES.iter().any(|e| path.extension().is_some_and(|x| x.eq_ignore_ascii_case(e))) { UnitPrefs { length: MESH_UNITS[c.unit.min(MESH_UNITS.len() - 1)].1, ..*units } } else { *units };
        match read_foreign(path, &units) {
            Ok(foreign) => {
                let (name, file) = import_names(path);
                read.push((name, file, foreign));
            }
            Err(e) => errors.push(format!("Import failed: {}: {e:#}", path.display())),
        }
    }
    let mut label = read.iter().map(|r| r.1.clone()).chain(pvz.iter().map(|p| p.file.clone())).chain(assemblies.iter().map(|a| format!("{}.zip", a.zip))).collect::<Vec<_>>().join(", ");
    if label.is_empty() {
        label = paths.iter().map(|p| import_names(p).1).collect::<Vec<_>>().join(", ");
    }
    let files = if keep_files { paths.iter().filter_map(|p| Some(FileTab::new(&import_names(p).1, &std::fs::read(p).ok()?, noshape_model::file_tab::now()))).collect() } else { Vec::new() };
    (ImportInputs { read, pvz, assemblies, files, label }, errors)
}

/// The import's tabs added to `d` after `after`: returns the first new tab
/// (to open) and the errors of assemblies that failed.
pub fn apply(d: &mut Document, inputs: ImportInputs, c: &ImportChoice, after: Option<TabId>, units: &UnitPrefs) -> (Option<TabId>, Vec<String>) {
    let mut first = None;
    let mut errors = Vec::new();
    // Onshape keeps the imported files in a "CAD Imports" folder.
    if !inputs.files.is_empty() {
        let folder = cad_imports_folder(d);
        for f in inputs.files {
            add_file_tab(d, f, Some(folder), None);
        }
    }
    let mut at = after;
    for a in &inputs.assemblies {
        // Combine: one Part Studio, no Assembly (live G41 (5)).
        if c.combine {
            match noshape_model::native_assembly::add_native_assembly_combined(d, &a.root, &a.components, &a.files, c.appearances, units) {
                Ok(studio) => {
                    crate::docedit::place_after(d, studio, at);
                    at = Some(studio);
                    first.get_or_insert(studio);
                }
                Err(e) => errors.push(format!("Import failed: {}: {e}", a.root)),
            }
            continue;
        }
        match noshape_model::native_assembly::add_native_zip(d, a, c.appearances, units) {
            Ok(noshape_model::native_assembly::NativeTabs::Assembly { studio, assembly }) => {
                crate::docedit::place_after(d, assembly, at);
                crate::docedit::place_after(d, studio, Some(assembly));
                at = Some(studio);
                first.get_or_insert(assembly);
            }
            Ok(noshape_model::native_assembly::NativeTabs::Studio(studio)) => {
                crate::docedit::place_after(d, studio, at);
                at = Some(studio);
                first.get_or_insert(studio);
            }
            Err(e) => errors.push(format!("Import failed: {}: {e}", a.root)),
        }
    }
    for p in &inputs.pvz {
        let mut options = vec![("importAppearances", c.appearances), ("yAxisIsUp", c.y_up), ("createComposite", c.composite)];
        if p.step {
            options.push(("joinAdjacentSurfaces", c.join));
        }
        let pvz = noshape_model::native_assembly::PvzImport { stem: p.name.clone(), root: p.root.clone(), brep: p.foreign.brep.clone(), info: p.foreign.parts.clone(), foreign_id: p.file.clone(), instances: p.instances.clone(), names: p.names.clone(), step: p.step, options };
        match noshape_model::native_assembly::add_pvz_assembly(d, &pvz, units) {
            Ok((studio, assembly)) => {
                crate::docedit::place_after(d, assembly, at);
                crate::docedit::place_after(d, studio, Some(assembly));
                at = Some(studio);
                first.get_or_insert(assembly);
            }
            Err(e) => errors.push(format!("Import failed: {}: {e}", p.file)),
        }
    }
    let groups: Vec<Vec<&(String, String, Foreign)>> = if c.combine { vec![inputs.read.iter().collect()] } else { inputs.read.iter().map(|r| vec![r]).collect() };
    for g in groups.into_iter().filter(|g| !g.is_empty()) {
        let mut ps = PartStudio::new();
        for (_, file, foreign) in &g {
            add_import_feature(&mut ps, file, foreign.clone(), units);
            if let Some(f) = ps.features.last_mut() {
                f.params.set("importAppearances", ParamValue::Bool(c.appearances));
                f.params.set("specifyMaterialData", ParamValue::Bool(c.density));
                f.params.set("importMaterialDensity", ParamValue::Bool(c.density));
                f.params.set("yAxisIsUp", ParamValue::Bool(c.y_up));
                f.params.set("createComposite", ParamValue::Bool(c.composite));
                f.params.set("joinAdjacentSurfaces", ParamValue::Bool(c.join));
            }
        }
        // A name already used: "<name> (1)", "(2)", … (live).
        let name = noshape_model::native_assembly::unique_name(d, &g[0].0);
        let id = d.add_tab(&name, TabContent::PartStudio(ps));
        crate::docedit::place_after(d, id, at);
        at = Some(id);
        first.get_or_insert(id);
    }
    (first, errors)
}

/// "Import <label>".
pub fn description(label: &str) -> String {
    format!("Import {label}")
}

#[cfg(test)]
mod tests {
    /// "Import material density" as Onshape offers it (qa4 IMP-DENS): not
    /// for STEP, SAT, PVZ or meshes; any one file of its types is enough.
    #[test]
    fn material_density_follows_onshapes_types() {
        let offers = |names: &[&str]| super::offers_material_density(&names.iter().map(std::path::PathBuf::from).collect::<Vec<_>>());
        for yes in [&["a.x_t"][..], &["a.X_B"], &["a.sldprt"], &["a.zip"], &["a.step", "b.par"]] {
            assert!(offers(yes), "{yes:?}");
        }
        for no in [&["a.step"][..], &["a.stp"], &["a.sat"], &["a.pvz"], &["a.stl"], &["a.ipt"], &["a.xmt_txt"]] {
            assert!(!offers(no), "{no:?}");
        }
    }
}
