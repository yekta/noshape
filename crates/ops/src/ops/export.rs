//! Export (Part Studio › Export…; the Export modal's writers).

use super::{doc_field, s, strings, studio_target, tab_field};
use crate::error::OpError;
use crate::export::{ExportSettings, PartFormat, UNITS};
use crate::registry::{Args, Op, OpOutput, opt, req};
use crate::{Ctx, refs};
use serde_json::json;

pub fn ops() -> Vec<Op> {
    vec![Op {
        group: "export",
        verb: "parts",
        summary: "Export a Part Studio's parts to a CAD or mesh file (STEP, Parasolid, IGES, ACIS, STL, OBJ, 3MF, glTF, GLB, JT, PVZ, Rhino)",
        description: "Onshape's Export dialog. Exports the visible parts (or `parts`) to `file`. Options as in the dialog: `y_up` (models oriented Y axis up), `version` (format version: STEP AP242/AP214/AP203, Parasolid 38.1..., Rhino 8...), `units` (mesh formats and STEP with custom units), `resolution` (coarse/medium/fine for mesh formats), `stl_text`, `individual` (one file per part, zipped), `include_hidden`.",
        fields: vec![
            doc_field(),
            tab_field(),
            req("format", json!({ "type": "string", "enum": PartFormat::ALL.iter().map(|f| f.label()).collect::<Vec<_>>() }), "File format."),
            req("file", s(), "Output path; without an extension the format's is added (\"part\" -> \"part.x_t\"); individual files go in a .zip."),
            opt("parts", strings(), "Part ids (`body:…`) or names; default all visible parts."),
            opt("y_up", json!({ "type": "boolean" }), "Export models oriented Y axis up."),
            opt("version", s(), "Format version (e.g. \"AP214\", \"35.1\", \"7\")."),
            opt("units", json!({ "type": "string", "enum": UNITS.iter().map(|u| u.0).collect::<Vec<_>>() }), "File units (STL, OBJ...; STEP: custom units)."),
            opt("resolution", json!({ "type": "string", "enum": ["Coarse", "Medium", "Fine"] }), "Mesh resolution (default Medium)."),
            opt("stl_text", json!({ "type": "boolean" }), "STL as text instead of binary."),
            opt("individual", json!({ "type": "boolean" }), "Export unique parts as individual files (zipped)."),
            opt("include_hidden", json!({ "type": "boolean" }), "Include hidden parts."),
        ],
        mutates: false,
        destructive: false,
        run: parts,
    }]
}

fn parts(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let label = a.str("format")?;
    let format = PartFormat::ALL.into_iter().find(|f| f.label().eq_ignore_ascii_case(label) || f.ext().eq_ignore_ascii_case(label)).ok_or_else(|| OpError::param("format", "unknown format"))?;
    let mut s = ExportSettings::default();
    crate::export::set_part_format(&mut s, PartFormat::ALL.iter().position(|f| *f == format).expect("listed"));
    s.y_up = a.bool_or("y_up", false)?;
    s.stl_text = a.bool_or("stl_text", false)?;
    s.individual = a.bool_or("individual", false)?;
    s.hidden = a.bool_or("include_hidden", false)?;
    if let Some(u) = a.opt_str("units")? {
        s.units = UNITS.iter().position(|x| x.0.eq_ignore_ascii_case(u)).ok_or_else(|| OpError::param("units", "unknown unit"))?;
        s.custom_units = true;
    }
    if let Some(r) = a.opt_str("resolution")? {
        s.resolution = ["coarse", "medium", "fine"].iter().position(|x| x.eq_ignore_ascii_case(r)).ok_or_else(|| OpError::param("resolution", "Coarse, Medium or Fine"))?;
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
    let n = list.len();
    let mut file = std::path::PathBuf::from(a.str("file")?);
    if file.extension().is_none() {
        file.set_extension(format.ext());
    }
    if let Some(dir) = file.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir).map_err(|e| OpError::new("io", format!("{}: {e}", dir.display())).with_param("file"))?;
    }
    let out = crate::export::write_export(&s, &file, list).map_err(|e| OpError::new("export_failed", format!("{e:#}")))?;
    let bytes = std::fs::metadata(&out).map(|m| m.len()).unwrap_or(0);
    Ok(json!({ "file": out.display().to_string(), "format": format.label(), "parts": n, "bytes": bytes }).into())
}
