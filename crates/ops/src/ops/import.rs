//! Import CAD and mesh files into the document (the Import dialog).

use super::{doc_field, s};
use crate::error::OpError;
use crate::import::{self, ImportChoice, MESH_UNITS};
use crate::registry::{Args, Op, OpOutput, opt, req};
use crate::{Ctx, doc};
use serde_json::json;
use std::path::PathBuf;

pub fn ops() -> Vec<Op> {
    vec![Op {
        group: "import",
        verb: "files",
        summary: "Import CAD or mesh files (STEP, IGES, Parasolid, ACIS, JT, Rhino, SolidWorks/Inventor/NX/Solid Edge parts and assembly zips, STL, OBJ, 3MF, glTF); other files as file tabs",
        description: "The Import dialog: each file becomes a Part Studio with an Import feature (or all of them one Part Studio with `combine`); a native assembly zip becomes an Assembly and its Part Studio. The files are kept as file tabs in a \"CAD Imports\" folder unless `keep_files` is false. One edit (\"Import <files>\"). Any other file (an image, PDF, text, Markdown, CSV, DXF / DWG…) becomes a file tab, one edit each (\"Import <file>\"), as the app's + › Import…. Returns the new tabs and the imported parts (`first`: the first new tab). The window stays on its tab, as live's.",
        fields: vec![
            doc_field(),
            req("files", json!({ "type": "array", "items": { "type": "string" }, "minItems": 1 }), "Paths of the files."),
            opt("combine", json!({ "type": "boolean" }), "Combine to a single Part Studio (default: one Part Studio per file)."),
            opt("appearances", json!({ "type": "boolean", "default": true }), "Import appearances."),
            opt("density", json!({ "type": "boolean" }), "Import material density: offered (and used) only when a file is one of Onshape's types for it (Parasolid x_t / x_b, SOLIDWORKS, Solid Edge, NX / Creo prt / asm, CATIA, zip); not STEP."),
            opt("y_up", json!({ "type": "boolean" }), "Orient imported models with Y axis up."),
            opt("mesh_unit", json!({ "type": "string", "enum": MESH_UNITS.iter().map(|u| u.0).collect::<Vec<_>>() }), "STL / OBJ units (default: STL Millimeter, OBJ Meter)."),
            opt("composite", json!({ "type": "boolean" }), "Create a composite part."),
            opt("join", json!({ "type": "boolean" }), "Join adjacent surfaces."),
            opt("iges_postprocessing", json!({ "type": "boolean" }), "IGES postprocessing (IGES files): their straight edges read as lines."),
            opt("keep_files", json!({ "type": "boolean", "default": true }), "Keep the files as file tabs in \"CAD Imports\"."),
            opt("upload_only", json!({ "type": "boolean" }), "Upload only: every file kept as it is, as a file tab after the import's tab, nothing translated. A bare assembly file (.sldasm, .iam, .catproduct, .asm) needs it, as the dialog: Onshape needs a zip of the assembly and all of its references to translate it."),
            opt("after", s(), "Tab to put the new tabs after (default: the open tab, as the app)."),
        ],
        mutates: true,
        destructive: false,
        run: files,
    }, Op {
        group: "import",
        verb: "update",
        summary: "Update… a file tab with another file (CAD Imports › ‹file› › Update…): its imported Part Studios follow in place",
        description: "A file tab's context menu › Update…: the tab holds the new file (its name stays). A CAD file is translated again into every Import feature made from the old one, in place: the same Part Studios, so their parts follow (a whole-studio Derived of one takes the new part count; a part-picked Derived keeps its parts, with their new geometry, and leaves new ones out), as live. A CAD file imported nowhere is imported anew after the open tab, as the app. One edit (\"Update ‹file›\"). Returns the Part Studios updated and their parts.",
        fields: vec![
            doc_field(),
            req("tab", s(), "The file tab (name or id)."),
            req("file", s(), "Path of the new file."),
        ],
        mutates: true,
        destructive: false,
        run: update,
    }]
}

fn update(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let od = ctx.doc(a.opt_str("doc")?)?;
    let units = od.doc().settings.units;
    let tab = doc::find_tab(od.doc(), a.str("tab")?).map_err(|e| e.with_param("tab"))?;
    if !matches!(od.doc().tab(tab).map(|t| &t.content), Some(noshape_model::document::TabContent::File(_))) {
        return Err(OpError::param("tab", "a file tab (Update… is on file tabs)"));
    }
    let path = PathBuf::from(a.str("file")?);
    if !path.is_file() {
        return Err(OpError::param("file", &format!("no such file: {}", path.display())));
    }
    let (file, foreign) = import::read_update(&path, &units).ok_or_else(|| OpError::param("file", &format!("could not read {}", path.display())))?;
    let (name, cad) = (file.file_name.clone(), file.kind() == noshape_model::file_tab::FileKind::Cad);
    if cad && foreign.is_none() {
        return Err(OpError::param("file", &import::failed_to_translate(&name)));
    }
    let mut updated = od.session.edit_document(&format!("Update {name}"), |d| import::update_file(d, tab, file, foreign.as_ref()))?;
    // Imported nowhere: imported anew, as the app's Update….
    if cad && updated.is_empty() {
        let before: Vec<noshape_model::document::TabId> = od.doc().tabs.iter().map(|t| t.id).collect();
        let c = ImportChoice::defaults(std::slice::from_ref(&path));
        let (inputs, _) = import::read_inputs(std::slice::from_ref(&path), &c, false, &units);
        let after = od.window.active_tab.or(od.session.active_tab);
        od.session.edit_document(&import::description(), |d| import::apply(d, inputs, &c, after, &units))?;
        updated = od.doc().tabs.iter().filter(|t| !before.contains(&t.id) && doc::is_part_studio(&t.content)).map(|t| t.id).collect();
    }
    let mut studios = Vec::new();
    for t in updated {
        let r = od.regenerate(t, None)?;
        let parts: Vec<serde_json::Value> = r.display.bodies.iter().map(|b| json!({ "id": crate::refs::body_id(&b.id), "name": b.name })).collect();
        studios.push(json!({ "tab": od.doc().tab(t).map(|x| x.name.clone()).unwrap_or_default(), "parts": parts }));
    }
    Ok(json!({ "file": name, "updated": studios }).into())
}

fn files(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let od = ctx.doc(a.opt_str("doc")?)?;
    let units = od.doc().settings.units;
    let paths: Vec<PathBuf> = a.array("files")?.iter().enumerate().map(|(i, v)| v.as_str().map(PathBuf::from).ok_or_else(|| OpError::param(&format!("files[{i}]"), "a path"))).collect::<Result<_, _>>()?;
    for (i, p) in paths.iter().enumerate() {
        if !p.is_file() {
            return Err(OpError::param(&format!("files[{i}]"), &format!("no such file: {}", p.display())));
        }
    }
    let after = match a.opt_str("after")? {
        Some(t) => Some(doc::find_tab(od.doc(), t).map_err(|e| e.with_param("after"))?),
        // Live import-fidelity-1123: right after the open tab (a new document's first), never at the end.
        None => od.window.active_tab.or(od.session.active_tab).filter(|t| od.doc().tab(*t).is_some()).or_else(|| od.doc().ordered_tabs().first().map(|t| t.id)),
    };
    // A bare assembly file: the dialog's Import stays disabled until Upload only is ticked (live upload-only-1118).
    let upload_only = a.bool_or("upload_only", false)?;
    if import::has_bare_assembly(&paths) && !upload_only {
        return Err(OpError::param("upload_only", &format!("{} Pass upload_only to keep the file as it is.", import::BARE_ASSEMBLY_NOTE)));
    }
    // Other files (images, PDF, text, DXF / DWG…) become file tabs, one edit
    // each, as the app's "+ › Import…" (Onshape).
    let cad_file = |p: &PathBuf| upload_only || p.extension().is_some_and(|e| import::IMPORT_EXTS.contains(&e.to_string_lossy().to_lowercase().as_str()));
    let (paths, other): (Vec<PathBuf>, Vec<PathBuf>) = paths.into_iter().partition(cad_file);
    let (mut stored, mut last) = (Vec::new(), None);
    for p in &other {
        let bytes = std::fs::read(p).map_err(|e| OpError::new("io", format!("reading {}: {e}", p.display())))?;
        let (_, file) = import::import_names(p);
        let id = od.session.edit_document(&format!("Import {file}"), |d| import::add_file_tab(d, noshape_model::file_tab::FileTab::new(&file, &bytes, noshape_model::file_tab::now()), None, after))?;
        stored.push(json!({ "id": id.to_string(), "name": file, "kind": "File" }));
        last = Some(id);
    }
    // Only other files (live doc-plus-import-png: the open tab stays).
    if paths.is_empty() {
        return Ok(json!({ "tabs": stored, "parts": [], "first": last.map(|t| t.to_string()) }).into());
    }
    let mut c = ImportChoice::defaults(&paths);
    c.combine = a.bool_or("combine", false)?;
    c.appearances = a.bool_or("appearances", true)?;
    // As the dialog: only offered for Onshape's material density types.
    c.density = a.bool_or("density", false)? && import::offers_material_density(&paths);
    c.y_up = a.bool_or("y_up", false)?;
    // Combine hides the composite box (live import-fidelity-1123).
    c.composite = a.bool_or("composite", false)? && !c.combine;
    c.join = a.bool_or("join", false)?;
    // As the dialog: only offered for IGES files.
    c.iges_post = a.bool_or("iges_postprocessing", false)? && import::offers_iges_postprocessing(&paths);
    c.upload_only = upload_only;
    if let Some(u) = a.opt_str("mesh_unit")? {
        c.unit = MESH_UNITS.iter().position(|x| x.0.eq_ignore_ascii_case(u)).ok_or_else(|| OpError::param("mesh_unit", "one of the listed units"))?;
    }
    // NOSHAPE_IMPORT_TIMING=1: each stage's time on stderr (dx_perf big imports).
    let timing = std::env::var_os("NOSHAPE_IMPORT_TIMING").is_some();
    let t0 = std::time::Instant::now();
    let stage = |what: &str, t: std::time::Instant| {
        if timing {
            eprintln!("import stage {what}: {:.1} ms", t.elapsed().as_secs_f64() * 1000.0);
        }
    };
    let (inputs, mut errors) = import::read_inputs(&paths, &c, a.bool_or("keep_files", true)?, &units);
    stage("read", t0);
    if inputs.is_empty() {
        return Err(OpError::param("files", &errors.join("; ")));
    }
    let before: Vec<noshape_model::document::TabId> = od.doc().tabs.iter().map(|t| t.id).collect();
    let desc = import::description();
    let t1 = std::time::Instant::now();
    let (first, apply_errors) = od.session.edit_document(&desc, |d| import::apply(d, inputs, &c, after, &units))?;
    stage("apply + save", t1);
    errors.extend(apply_errors);
    let new_tabs: Vec<serde_json::Value> = od.doc().tabs.iter().filter(|t| !before.contains(&t.id)).map(|t| json!({ "id": t.id.to_string(), "name": t.name, "kind": t.content.kind_name() })).collect();
    // The imported parts of each new Part Studio.
    let studios: Vec<noshape_model::document::TabId> = od.doc().tabs.iter().filter(|t| !before.contains(&t.id) && doc::is_part_studio(&t.content)).map(|t| t.id).collect();
    let mut parts = Vec::new();
    let t2 = std::time::Instant::now();
    for t in studios {
        let r = od.regenerate(t, None)?;
        stage("regenerate", t2);
        if timing {
            let features: f32 = r.display.features.iter().map(|s| s.time_ms).sum();
            eprintln!("import stage regenerate: features {features:.1} ms, regen total {:.1} ms", r.display.regen_ms);
        }
        let name = od.doc().tab(t).map(|x| x.name.clone()).unwrap_or_default();
        // A feature that failed (an Import whose parts didn't read as valid bodies) says so: no silent empty import.
        if let Ok(ps) = doc::studio(od.doc(), t) {
            for f in r.display.features.iter().filter(|f| matches!(f.status, noshape_model::display::FeatureStatus::Error)) {
                let feature = ps.features.iter().find(|x| x.id == f.id).map_or_else(|| f.id.to_string(), |x| x.name.clone());
                errors.push(format!("{name}: {feature}: {}", f.message.as_deref().unwrap_or("Error regenerating.")));
            }
        }
        for b in &r.display.bodies {
            parts.push(json!({ "tab": name, "id": crate::refs::body_id(&b.id), "name": b.name }));
        }
    }
    let tabs: Vec<serde_json::Value> = stored.into_iter().chain(new_tabs).collect();
    let mut out = json!({ "tabs": tabs, "parts": parts, "first": first.map(|t| t.to_string()) });
    if !errors.is_empty() {
        out["errors"] = json!(errors);
    }
    // The window stays on its tab, as live's Import (studios + dx 11-18 import feel: the new tabs go after it).
    Ok(out.into())
}
