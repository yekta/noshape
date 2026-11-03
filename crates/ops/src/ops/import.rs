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
        description: "The Import dialog: each file becomes a Part Studio with an Import feature (or all of them one Part Studio with `combine`); a native assembly zip becomes an Assembly and its Part Studio. The files are kept as file tabs in a \"CAD Imports\" folder unless `keep_files` is false. One edit (\"Import <files>\"). Any other file (an image, PDF, text, Markdown, CSV, DXF / DWG…) becomes a file tab, one edit each (\"Import <file>\"), as the app's + › Import…. Returns the new tabs and the imported parts. The window then shows the first new tab (`first`), and tools without `tab` act on it.",
        fields: vec![
            doc_field(),
            req("files", json!({ "type": "array", "items": { "type": "string" }, "minItems": 1 }), "Paths of the files."),
            opt("combine", json!({ "type": "boolean" }), "Combine to a single Part Studio (default: one Part Studio per file)."),
            opt("appearances", json!({ "type": "boolean", "default": true }), "Import appearances."),
            opt("density", json!({ "type": "boolean" }), "Import material density: offered (and used) only when a file is one of Onshape's types for it (Parasolid x_t / x_b, SOLIDWORKS, Solid Edge, NX / Creo prt / asm, CATIA, zip); not STEP."),
            opt("y_up", json!({ "type": "boolean" }), "Orient imported models with Y axis up."),
            opt("mesh_unit", json!({ "type": "string", "enum": MESH_UNITS.iter().map(|u| u.0).collect::<Vec<_>>() }), "STL / OBJ units (default: the workspace unit; OBJ: Meter)."),
            opt("composite", json!({ "type": "boolean" }), "Create a composite part."),
            opt("join", json!({ "type": "boolean" }), "Join adjacent surfaces."),
            opt("keep_files", json!({ "type": "boolean", "default": true }), "Keep the files as file tabs in \"CAD Imports\"."),
            opt("after", s(), "Tab to put the new tabs after (default: the end)."),
        ],
        mutates: true,
        destructive: false,
        run: files,
    }]
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
        None => None,
    };
    // Other files (images, PDF, text, DXF / DWG…) become file tabs, one edit
    // each, as the app's "+ › Import…" (Onshape).
    let cad_file = |p: &PathBuf| p.extension().is_some_and(|e| import::IMPORT_EXTS.contains(&e.to_string_lossy().to_lowercase().as_str()));
    let (paths, other): (Vec<PathBuf>, Vec<PathBuf>) = paths.into_iter().partition(cad_file);
    let (mut stored, mut last) = (Vec::new(), None);
    for p in &other {
        let bytes = std::fs::read(p).map_err(|e| OpError::new("io", format!("reading {}: {e}", p.display())))?;
        let (_, file) = import::import_names(p);
        let id = od.session.edit_document(&format!("Import {file}"), |d| import::add_file_tab(d, noshape_model::file_tab::FileTab::new(&file, &bytes, noshape_model::file_tab::now()), None, after))?;
        stored.push(json!({ "id": id.to_string(), "name": file, "kind": "File" }));
        last = Some(id);
    }
    // Only other files: the window shows the last one (the app's).
    if paths.is_empty() {
        if let Some(t) = last {
            super::show(ctx, a.opt_str("doc")?, crate::UiRequest::ActivateTab(t))?;
        }
        return Ok(json!({ "tabs": stored, "parts": [], "first": last.map(|t| t.to_string()) }).into());
    }
    let mut c = ImportChoice::defaults(&paths, units.length);
    c.combine = a.bool_or("combine", false)?;
    c.appearances = a.bool_or("appearances", true)?;
    // As the dialog: only offered for Onshape's material density types.
    c.density = a.bool_or("density", false)? && import::offers_material_density(&paths);
    c.y_up = a.bool_or("y_up", false)?;
    c.composite = a.bool_or("composite", false)?;
    c.join = a.bool_or("join", false)?;
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
    let desc = import::description(&inputs.label);
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
        for b in &r.display.bodies {
            parts.push(json!({ "tab": name, "id": crate::refs::body_id(&b.id), "name": b.name }));
        }
    }
    let tabs: Vec<serde_json::Value> = stored.into_iter().chain(new_tabs).collect();
    let mut out = json!({ "tabs": tabs, "parts": parts, "first": first.map(|t| t.to_string()) });
    if !errors.is_empty() {
        out["errors"] = json!(errors);
    }
    // The window opens the import's first tab, as the app does: tools
    // without `tab` then work on it.
    if let Some(t) = first {
        super::show(ctx, a.opt_str("doc")?, crate::UiRequest::ActivateTab(t))?;
    }
    Ok(out.into())
}
