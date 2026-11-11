//! Export (Part Studio › Export…; the Export modal's writers).

use super::{doc_field, s, strings, studio_target, tab_field};
use crate::error::OpError;
use crate::export::{ExportSettings, PartFormat, SolidExportPrefs, UNITS};
use crate::registry::{Args, Op, OpOutput, opt, req};
use crate::{Ctx, refs};
use serde_json::json;

pub fn ops() -> Vec<Op> {
    vec![Op {
        group: "export",
        verb: "parts",
        summary: "Export a Part Studio's parts or an Assembly to a CAD or mesh file (STEP, Parasolid, IGES, ACIS, STL, OBJ, 3MF, glTF, GLB, JT, PVZ, Rhino)",
        description: "Onshape's Export dialog. Exports the visible parts (or `parts`) to `file`; with `tab` an Assembly, its visible instances (or `parts`: instance ids or names) — STEP, Parasolid and IGES keep the assembly (the root product / ASSEMBLY / subfigures, each part once, placed by its instances), the other formats get the placed parts; with `tab` a CAD file's tab (a STEP in CAD Imports, its menu's Export…), the file's parts converted to PARASOLID, ACIS, IGES, PVZ, JT, RHINO, GLTF, GLB, OBJ or 3MF, the stored file named after the file without its extension. Options as in the dialog: `y_up` (models oriented Y axis up), `version` (format version: STEP AP242/AP214/AP203, Parasolid 38.1..., Rhino 8...), `units` (mesh formats and STEP with custom units), `resolution` (coarse/medium/fine/custom for mesh formats; Coarse / Medium / Fine are chordal 0.24 / 0.12 / 0.06 mm at 12.5 / 6.25 / 2.5°; Custom's fields: STL `chordal_tolerance` / `angular_deviation` deg / `min_facet_width` in the STL's `units` (metres unless `units` says otherwise); an STL is facetted as Onshape's are at these values (its Coarse / Medium / Fine: the same triangle counts and smoothness), the other mesh formats `distance_tolerance` m / `angular_tolerance` deg / `max_chord_length` m; unset fields keep the last-used ones, first Medium's), `stl_text`, `individual` (one file per part, zipped), `include_hidden`, STEP `preprocessing` / `derived_tolerances` / `mbd_text_height`, IGES `iges_compatibility`, PVZ `group_composites`, glTF `compress`, URDF `geometry_format`. An option not given takes the value the app's Export dialog last exported with (per format, as Onshape remembers them; the first time Onshape's defaults: Medium, STL in Meter and Binary, the latest versions); `options` defaults to download.",
        fields: vec![
            doc_field(),
            tab_field(),
            req("format", json!({ "type": "string", "enum": PartFormat::ALL.iter().map(|f| f.label()).collect::<Vec<_>>() }), "File format."),
            opt("file", s(), "Output path; without an extension the format's is added (\"part\" -> \"part.x_t\"); individual files go in a .zip. Required unless `options` is \"store\" (then only its name is used; default \"<tab>.<ext>\")."),
            opt("options", json!({ "type": "string", "enum": ["download", "download_and_store", "store"], "default": "download" }), "Options: write `file` (Download), and / or keep the file as a file tab right after the tab (Store file in a new tab; one edit \"Store <file>\")."),
            opt("parts", strings(), "Part ids (`body:…`) or names; default all visible parts."),
            opt("y_up", json!({ "type": "boolean" }), "Export models oriented Y axis up."),
            opt("version", s(), "Format version (e.g. \"AP214\", \"35.1\", \"7\")."),
            opt("units", json!({ "type": "string", "enum": UNITS.iter().map(|u| u.0).collect::<Vec<_>>() }), "File units (STL, OBJ...; STEP: custom units)."),
            opt("resolution", json!({ "type": "string", "enum": ["Coarse", "Medium", "Fine", "Custom"] }), "Mesh resolution (default Medium); Custom takes the fields below."),
            opt("chordal_tolerance", json!({ "type": "number", "exclusiveMinimum": 0 }), "Custom, STL: Chordal tolerance (mm)."),
            opt("angular_deviation", json!({ "type": "number", "exclusiveMinimum": 0 }), "Custom, STL: Angular deviation (deg)."),
            opt("min_facet_width", json!({ "type": "number", "minimum": 0 }), "Custom, STL: Minimum facet width (mm; kept, not applied by our mesher)."),
            opt("distance_tolerance", json!({ "type": "number", "exclusiveMinimum": 0 }), "Custom, glTF / GLB / OBJ / 3MF: Distance tolerance (m)."),
            opt("angular_tolerance", json!({ "type": "number", "exclusiveMinimum": 0 }), "Custom, glTF / GLB / OBJ / 3MF: Angular tolerance (deg)."),
            opt("max_chord_length", json!({ "type": "number", "minimum": 0 }), "Custom, glTF / GLB / OBJ / 3MF: Maximum chord length (m; kept, not applied by our mesher)."),
            opt("stl_text", json!({ "type": "boolean" }), "STL as text instead of binary."),
            opt("individual", json!({ "type": "boolean" }), "Export unique parts as individual files (zipped)."),
            opt("include_hidden", json!({ "type": "boolean" }), "Include hidden parts."),
            opt("preprocessing", json!({ "type": "string", "enum": crate::export::STEP_PREPROCESSING }), "STEP: Preprocessing (default None)."),
            opt("derived_tolerances", json!({ "type": "boolean" }), "STEP: Include derived tolerances in export."),
            opt("mbd_text_height", json!({ "type": "number", "minimum": 0.0001, "maximum": 50 }), "STEP: Use custom annotation text height for MBD export: the annotations' text height in mm (default 4)."),
            opt("iges_compatibility", json!({ "type": "boolean" }), "IGES: Use compatibility mode for surfaces and curves."),
            opt("group_composites", json!({ "type": "boolean" }), "PVZ: Group composite parts into a single part file."),
            opt("compress", json!({ "type": "boolean" }), "glTF / GLB: Compress."),
            opt("geometry_format", json!({ "type": "string", "enum": crate::export::URDF_GEOMETRY.iter().map(|f| f.label()).collect::<Vec<_>>() }), "URDF: Geometry format (default STL)."),
        ],
        mutates: true,
        destructive: false,
        run: parts,
    }]
}

fn parts(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let label = a.str("format")?;
    let format = PartFormat::ALL.into_iter().find(|f| f.label().eq_ignore_ascii_case(label) || f.ext().eq_ignore_ascii_case(label)).ok_or_else(|| {
        let names: Vec<&str> = PartFormat::ALL.iter().map(|f| f.label()).collect();
        OpError::param("format", &format!("`{label}` is not an export format{}; formats: {}", crate::error::did_you_mean(label, names.iter().copied()), names.join(", ")))
    })?;
    // The options not given are the Export dialog's remembered ones.
    let mut s = SolidExportPrefs::load().settings(false);
    let Some(row) = PartFormat::list(false).iter().position(|f| *f == format) else {
        return Err(OpError::param("format", "RHINO export is not in this build (it needs openNURBS; releases have it)"));
    };
    crate::export::set_part_format(&mut s, row);
    s.y_up = a.bool_or("y_up", s.y_up)?;
    s.stl_text = a.bool_or("stl_text", s.stl_text)?;
    s.individual = a.bool_or("individual", s.individual)?;
    s.hidden = a.bool_or("include_hidden", s.hidden || format == PartFormat::Urdf)?;
    s.derived_tolerances = a.bool_or("derived_tolerances", s.derived_tolerances)?;
    if let Some(h) = a.opt_f64("mbd_text_height")? {
        if !(0.0001..=50.0).contains(&h) {
            return Err(OpError::param("mbd_text_height", "Valid annotations text height in range from 0.0001 to 50 is required."));
        }
        s.mbd_text_height = true;
        s.mbd_text_mm = h;
    }
    s.iges_compat = a.bool_or("iges_compatibility", s.iges_compat)?;
    s.pvz_group = a.bool_or("group_composites", s.pvz_group)?;
    s.compress = a.bool_or("compress", s.compress)?;
    if let Some(p) = a.opt_str("preprocessing")? {
        s.preprocessing = crate::export::STEP_PREPROCESSING.iter().position(|x| x.eq_ignore_ascii_case(p)).ok_or_else(|| OpError::param("preprocessing", "one of the listed choices"))?;
    }
    if let Some(g) = a.opt_str("geometry_format")? {
        s.urdf_geometry = crate::export::URDF_GEOMETRY.iter().position(|f| f.label().eq_ignore_ascii_case(g)).ok_or_else(|| OpError::param("geometry_format", "STL, GLTF, GLB or OBJ"))?;
    }
    if let Some(u) = a.opt_str("units")? {
        s.units = UNITS.iter().position(|x| x.0.eq_ignore_ascii_case(u)).ok_or_else(|| OpError::param("units", "unknown unit"))?;
        s.custom_units = true;
    }
    if let Some(r) = a.opt_str("resolution")? {
        s.resolution = ["coarse", "medium", "fine", "custom"].iter().position(|x| x.eq_ignore_ascii_case(r)).ok_or_else(|| OpError::param("resolution", "Coarse, Medium, Fine or Custom"))?;
    }
    // Custom's fields; STL's lengths in its units (live 1745), the others' in metres.
    let stl_mm = UNITS[s.units].1 * 1000.0;
    let c = &mut s.custom;
    for (k, v, scale) in [("chordal_tolerance", &mut c.chordal_mm, stl_mm), ("angular_deviation", &mut c.angular_deg, 1.0), ("min_facet_width", &mut c.min_facet_mm, stl_mm), ("distance_tolerance", &mut c.distance_m, 1.0), ("angular_tolerance", &mut c.angular_tol_deg, 1.0), ("max_chord_length", &mut c.max_chord_m, 1.0)] {
        if let Some(x) = a.opt_f64(k)? {
            if !x.is_finite() || x <= 0.0 {
                return Err(OpError::param(k, "a positive number"));
            }
            *v = x * scale;
        }
    }
    if let Some(v) = a.opt_str("version")? {
        let list: &[&str] = match format {
            PartFormat::Step => &crate::export::STEP_VERSIONS,
            PartFormat::Parasolid => &crate::export::PARASOLID_VERSIONS,
            PartFormat::Rhino => &crate::export::RHINO_VERSIONS,
            _ => return Err(OpError::param("version", "this format has no version choice")),
        };
        s.version = list.iter().position(|x| x.eq_ignore_ascii_case(v)).ok_or_else(|| OpError::param("version", &format!("one of {}", list.join(", "))))?;
        s.latest = false;
    }
    if is_cad_file(ctx, a)? {
        return cad_file(ctx, a, format, s);
    }
    if super::configuration::asm::is_assembly(ctx, a)? {
        return assembly(ctx, a, format, s);
    }
    let (od, tab) = studio_target(ctx, a)?;
    let r = od.regenerate(tab, None)?;
    let selected: Vec<noshape_model::BodyId> = match a.opt_array("parts")? {
        Some(list) => list
            .iter()
            .enumerate()
            .map(|(i, v)| {
                let key = v.as_str().unwrap_or_default();
                r.display
                    .bodies
                    .iter()
                    .find(|b| matches!(refs::parse_str(key), Some(noshape_model::Reference::Body(id)) if id == b.id) || b.name == key)
                    .map(|b| b.id.clone())
                    .ok_or_else(|| OpError::param(&format!("parts[{i}]"), "no such part"))
            })
            .collect::<Result<_, _>>()?,
        None => Vec::new(),
    };
    // What the Part Studio's Export writes (write_part_export).
    let list: Vec<(String, noshape_kernel::Shape, [f32; 4])> = r
        .context
        .bodies
        .iter()
        .filter(|b| selected.is_empty() || selected.contains(&b.id))
        .filter_map(|b| {
            let d = r.display.body(&b.id);
            if d.is_some_and(|d| d.hidden) && !s.hidden && selected.is_empty() {
                return None;
            }
            let name = d.map(|d| d.name.clone()).unwrap_or_else(|| b.name.clone());
            let color = d.map(|d| d.color).unwrap_or([0.7, 0.7, 0.7, 1.0]);
            Some((name, b.shape.clone(), color))
        })
        .collect();
    // The exported bodies' MBD (STEP AP242 semantic PMI).
    let ids: Vec<noshape_model::BodyId> = r
        .context
        .bodies
        .iter()
        .filter(|b| selected.is_empty() || selected.contains(&b.id))
        .filter(|b| !(r.display.body(&b.id).is_some_and(|d| d.hidden) && !s.hidden && selected.is_empty()))
        .map(|b| b.id.clone())
        .collect();
    if let Some(noshape_model::TabContent::PartStudio(ps)) = od.doc().tab(tab).map(|t| &t.content) {
        s.pmi = crate::export::studio_pmi(ps, &r.context, &ids, &od.doc().settings.units, s.mbd_text());
    }
    let n = list.len();
    write_out(od, tab, a, format, n, |file| crate::export::write_export(&s, file, list))
}

/// Whether `tab` is a CAD file's tab (a STEP in CAD Imports).
fn is_cad_file(ctx: &mut Ctx, a: &Args) -> Result<bool, OpError> {
    let Some(key) = a.opt_str("tab")? else { return Ok(false) };
    let od = ctx.doc(a.opt_str("doc")?)?;
    let Ok(tab) = crate::doc::find_tab(od.doc(), key) else { return Ok(false) };
    Ok(matches!(od.doc().tab(tab).map(|t| &t.content), Some(noshape_model::TabContent::File(f)) if f.kind() == noshape_model::file_tab::FileKind::Cad))
}

/// Export of a CAD file tab (its menu's Export…, live document/94): the file's parts in another format, named after
/// the file without its extension.
fn cad_file(ctx: &mut Ctx, a: &Args, format: PartFormat, s: ExportSettings) -> Result<OpOutput, OpError> {
    if !crate::export::FILE_FORMATS.contains(&format) {
        let names: Vec<&str> = crate::export::FILE_FORMATS.iter().map(|f| f.label()).collect();
        return Err(OpError::param("format", &format!("a CAD file exports to {}", names.join(", "))));
    }
    let od = ctx.doc(a.opt_str("doc")?)?;
    let tab = crate::doc::find_tab(od.doc(), a.str("tab")?)?;
    let Some(noshape_model::TabContent::File(f)) = od.doc().tab(tab).map(|t| t.content.clone()) else { return Err(OpError::param("tab", "not a file tab")) };
    let parts = crate::export::cad_file_parts(&f, &od.doc().settings.units).map_err(|e| OpError::new("export_failed", format!("{e:#}")).with_param("tab"))?;
    let n = parts.len();
    write_out_named(od, tab, &crate::export::file_stem(&f.file_name), a, format, n, |file| crate::export::write_export(&s, file, parts))
}

/// Export of an Assembly tab: its instances (or `parts`: instance ids or
/// names); STEP and Parasolid keep the assembly, as Onshape's.
fn assembly(ctx: &mut Ctx, a: &Args, format: PartFormat, s: ExportSettings) -> Result<OpOutput, OpError> {
    use noshape_model::document::TabContent;
    let od = ctx.doc(a.opt_str("doc")?)?;
    let tab = od.tab_of_kind(a.opt_str("tab")?, "Assembly", |c| matches!(c, TabContent::Assembly(_)))?;
    let Some(TabContent::Assembly(asm)) = od.doc().tab(tab).map(|t| &t.content) else { unreachable!("checked") };
    let asm = asm.clone();
    let ids: Option<Vec<noshape_model::assembly::InstanceId>> = match a.opt_array("parts")? {
        Some(list) => Some(list.iter().enumerate().map(|(i, v)| super::assembly::find_instance(&asm, v.as_str().unwrap_or_default(), &format!("parts[{i}]"))).collect::<Result<_, _>>()?),
        None => None,
    };
    let result = super::assembly::solve_assembly(od, tab, &asm, &Default::default());
    let root = od.doc().tab(tab).map(|t| t.name.clone()).unwrap_or_default();
    let export = crate::export::AssemblyExport::of(&root, &result, ids.as_deref(), s.hidden);
    let n = export.instances.len();
    write_out(od, tab, a, format, n, |file| crate::export::write_assembly_export(&s, file, &export))
}

/// Write the export to `file` (or store it), as the dialog's Options say.
fn write_out(od: &mut crate::OpenDoc, tab: noshape_model::document::TabId, a: &Args, format: PartFormat, n: usize, write: impl FnOnce(&std::path::Path) -> anyhow::Result<std::path::PathBuf>) -> Result<OpOutput, OpError> {
    let tab_name = od.doc().tab(tab).map(|t| t.name.clone()).unwrap_or_default();
    write_out_named(od, tab, &tab_name, a, format, n, write)
}

/// [`write_out`], a stored file named `tab_name` (with the format's extension) unless `file` names it.
fn write_out_named(od: &mut crate::OpenDoc, tab: noshape_model::document::TabId, tab_name: &str, a: &Args, format: PartFormat, n: usize, write: impl FnOnce(&std::path::Path) -> anyhow::Result<std::path::PathBuf>) -> Result<OpOutput, OpError> {
    let (download, store) = options_arg(a)?;
    let mut file = match a.opt_str("file")? {
        Some(f) => std::path::PathBuf::from(f),
        None if !download => store_dir()?.join(tab_name),
        None => return Err(OpError::param("file", "required (or options \"store\")")),
    };
    if !download && a.opt_str("file")?.is_some() {
        file = store_dir()?.join(file.file_name().unwrap_or_default());
    }
    if file.extension().is_none() {
        file.set_extension(format.ext());
    }
    if let Some(dir) = file.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir).map_err(|e| OpError::new("io", format!("{}: {e}", dir.display())).with_param("file"))?;
    }
    let out = write(&file).map_err(|e| OpError::new("export_failed", format!("{e:#}")))?;
    let bytes = std::fs::metadata(&out).map(|m| m.len()).unwrap_or(0);
    let mut reply = json!({ "format": format.label(), "parts": n, "bytes": bytes });
    if store {
        reply["stored_tab"] = json!(store_tab(od, tab, &out)?);
    }
    if download {
        reply["file"] = json!(out.display().to_string());
    } else {
        let _ = std::fs::remove_file(&out);
    }
    Ok(reply.into())
}

/// The export `options` field: (download, store).
pub(super) fn options_arg(a: &Args) -> Result<(bool, bool), OpError> {
    match a.opt_str("options")?.unwrap_or("download") {
        "download" => Ok((true, false)),
        "download_and_store" => Ok((true, true)),
        "store" => Ok((false, true)),
        _ => Err(OpError::param("options", "download, download_and_store or store")),
    }
}

/// Where a stored-only export is written before it becomes a file tab.
pub(super) fn store_dir() -> Result<std::path::PathBuf, OpError> {
    let d = std::env::temp_dir().join(format!("noshape-store-{}", std::process::id()));
    std::fs::create_dir_all(&d).map_err(|e| OpError::new("io", format!("{}: {e}", d.display())))?;
    Ok(d)
}

/// Keep the exported file `path` as a file tab right after `tab` ("Store <file>").
pub(super) fn store_tab(od: &mut crate::OpenDoc, tab: noshape_model::document::TabId, path: &std::path::Path) -> Result<String, OpError> {
    let name = path.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default();
    let bytes = std::fs::read(path).map_err(|e| OpError::new("io", format!("{}: {e}", path.display())))?;
    let id = od.session.edit_document(&crate::export::store_description(&name), |d| crate::export::store_file_tab(d, &name, &bytes, tab))?;
    Ok(id.to_string())
}
