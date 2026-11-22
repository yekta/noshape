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

/// Assembly files Onshape can't translate on their own (it needs a zip of the assembly and every file it references): the
/// Import dialog offers "Upload only" for them (live upload-only-1118).
pub const BARE_ASSEMBLIES: &[&str] = &["iam", "catproduct", "sldasm", "asm"];
/// The Import dialog's info box over "Upload only" (live upload-only-1118).
pub const BARE_ASSEMBLY_NOTE: &str = "It looks like you want to import an assembly. Onshape needs a zip file containing the assembly and all of its references to process the import.";

/// Whether any of `paths` is a bare assembly file ([`BARE_ASSEMBLIES`]): the dialog's "Upload only" variant.
pub fn has_bare_assembly(paths: &[PathBuf]) -> bool {
    paths.iter().any(|p| p.extension().is_some_and(|e| BARE_ASSEMBLIES.contains(&e.to_string_lossy().to_lowercase().as_str())))
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
    /// IGES files: "IGES postprocessing" (their straight edges read as lines).
    pub iges_post: bool,
    /// "Upload only" (a bare assembly's dialog): the files kept as they are, as file tabs, nothing translated.
    pub upload_only: bool,
}

impl ImportChoice {
    /// The dialog's starting choices for `paths`, the same every time (live2 feel/importexport: nothing is
    /// remembered): Import appearances on, the rest off; OBJ in Meter, STL in Millimeter whatever the document's units.
    pub fn defaults(paths: &[PathBuf]) -> Self {
        let obj = !paths.is_empty() && paths.iter().all(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("obj")));
        let default_unit = if obj { LengthUnit::Meter } else { LengthUnit::Millimeter };
        let unit = MESH_UNITS.iter().position(|(_, u)| *u == default_unit).unwrap_or(4);
        ImportChoice { appearances: true, unit, ..Default::default() }
    }
}

/// Whether the import dialog for `paths` shows "IGES postprocessing" (live2 feel/importexport: IGES files).
pub fn offers_iges_postprocessing(paths: &[PathBuf]) -> bool {
    paths.iter().any(|p| p.extension().is_some_and(|e| ["igs", "iges"].contains(&e.to_string_lossy().to_lowercase().as_str())))
}

/// Tab (and document) name of an imported file: the file name without its
/// extension; and the full file name.
pub fn import_names(path: &Path) -> (String, String) {
    let file = path.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default();
    let stem = path.file_stem().map(|f| f.to_string_lossy().to_string()).unwrap_or_else(|| file.clone());
    (stem, file)
}

/// Live's words for a file that didn't translate (its bubble, live errors-1118: "bad.step uploaded but failed to
/// translate."), never our reader's error or the file's path.
pub fn failed_to_translate(file: &str) -> String {
    format!("{file} uploaded but failed to translate.")
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
    read_foreign_with(path, units, false)
}

/// [`read_foreign`], an IGES file's straight edges read as lines with `iges_post` (Import › IGES postprocessing).
pub fn read_foreign_with(path: &Path, units: &UnitPrefs, iges_post: bool) -> anyhow::Result<Foreign> {
    let (shape, info) = read_shape(path, units, iges_post)?;
    foreign_of(&shape, &info)
}

fn foreign_of(shape: &noshape_kernel::Shape, info: &[noshape_model::features::importing::ImportPartInfo]) -> anyhow::Result<Foreign> {
    let t = std::time::Instant::now();
    let parts = if info.is_empty() { String::new() } else { serde_json::to_string(info)? };
    let brep = String::from_utf8_lossy(&shape.to_brep_bytes()?).to_string();
    if std::env::var_os("NOSHAPE_IMPORT_TIMING").is_some() {
        eprintln!("import stage brep text ({} MB): {:.1} ms", brep.len() / 1_000_000, t.elapsed().as_secs_f64() * 1000.0);
    }
    Ok(Foreign { brep, parts })
}

/// An assembly's parts read flat, one per placed instance: a product's second and later instances named "<name>_1",
/// "<name>_2"… (live exchange batch 3, Combine of an assembly STEP: "Ring", "Ring_1"…; two products of one name keep
/// it each).
fn instances_suffixed(mut parts: Vec<noshape_kernel::ImportedShape>) -> Vec<noshape_kernel::ImportedShape> {
    for i in 1..parts.len() {
        let (before, rest) = parts.split_at_mut(i);
        let p = &mut rest[0];
        let Some(name) = p.name.clone() else { continue };
        let n = before.iter().filter(|q| q.shape.is_partner(&p.shape) && q.name.as_deref().is_some_and(|m| m == name || m.strip_prefix(name.as_str()).is_some_and(|s| s.strip_prefix('_').is_some_and(|k| k.parse::<u32>().is_ok())))).count();
        if n > 0 {
            p.name = Some(format!("{name}_{n}"));
        }
    }
    parts
}

/// A CAD file's parts as its tab's Export… writes them (live document/94): each with its name in the file ("Part
/// <n>" without one) and colour.
pub fn file_parts(path: &Path, units: &UnitPrefs) -> anyhow::Result<Vec<(String, noshape_kernel::Shape, [f32; 4])>> {
    let (shape, info) = read_shape(path, units, false)?;
    let shapes = if info.is_empty() { vec![shape] } else { shape.children() };
    Ok(shapes
        .into_iter()
        .enumerate()
        .map(|(i, sh)| {
            let p = info.get(i);
            let name = p.and_then(|p| p.name.clone()).filter(|n| !n.is_empty()).unwrap_or_else(|| format!("Part {}", i + 1));
            let [r, g, b] = p.and_then(|p| p.color).unwrap_or([0.7, 0.7, 0.7]);
            (name, sh, [r, g, b, p.and_then(|p| p.opacity).unwrap_or(1.0)])
        })
        .collect())
}

/// The unit a mesh file's coordinates are read in (the Import dialog's mesh unit).
fn mesh_unit_of(units: &UnitPrefs) -> noshape_kernel::LengthUnit {
    use noshape_kernel as k;
    match units.length {
        LengthUnit::Centimeter => k::LengthUnit::Centimeter,
        LengthUnit::Meter => k::LengthUnit::Meter,
        LengthUnit::Inch => k::LengthUnit::Inch,
        LengthUnit::Foot => k::LengthUnit::Foot,
        LengthUnit::Yard => k::LengthUnit::Yard,
        _ => k::LengthUnit::Millimeter,
    }
}

/// An OBJ export's zip (the .obj with its .mtl, as Onshape and ours write OBJ): each .obj's parts, one per group
/// (`g`), coloured by its material's diffuse colour (`Kd`). Onshape refuses such a zip ("Translation is not supported
/// for MTL (.mtl) files."), a refusal, not a target. None when the zip holds no .obj.
fn read_obj_zip(zip: &Path, units: &UnitPrefs) -> anyhow::Result<Option<Vec<(String, String, Foreign)>>> {
    use noshape_kernel as k;
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let dir = std::env::temp_dir().join(format!("noshape-obj-{}-{stamp}", std::process::id()));
    std::fs::create_dir_all(&dir)?;
    let result = (|| {
        let names = k::extract_zip(zip, &dir)?;
        let objs: Vec<&String> = names.iter().filter(|n| !n.contains('/') && n.to_ascii_lowercase().ends_with(".obj")).collect();
        if objs.is_empty() {
            return Ok(None);
        }
        let mut out = Vec::new();
        for n in objs {
            let obj = dir.join(n);
            let (shape, info) = read_obj_groups(&obj, &dir, mesh_unit_of(units))?;
            out.push((import_names(&obj).0, import_names(zip).1, foreign_of(&shape, &info)?));
        }
        Ok(Some(out))
    })();
    let _ = std::fs::remove_dir_all(&dir);
    result
}

/// A zip of Parasolid part files (Onshape's Parasolid export of a Part Studio with mesh bodies: the .x_t with the
/// .xmm_txt holding its meshes, live c2-hex-bookcase-derived): each part file's parts, its meshes read beside it.
fn read_xt_zip(zip: &Path, units: &UnitPrefs, iges_post: bool) -> anyhow::Result<Vec<(String, String, Foreign)>> {
    use noshape_kernel as k;
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let dir = std::env::temp_dir().join(format!("noshape-xt-{}-{stamp}", std::process::id()));
    std::fs::create_dir_all(&dir)?;
    let result = (|| {
        let names = k::extract_zip(zip, &dir)?;
        let part_file = |n: &String| !n.contains('/') && n.rsplit_once('.').is_some_and(|(_, e)| ["x_t", "x_b", "xmt_txt", "xmt_bin"].contains(&e.to_ascii_lowercase().as_str()));
        let mut out = Vec::new();
        for n in names.iter().filter(|n| part_file(n)) {
            let file = dir.join(n);
            out.push((import_names(&file).0, import_names(zip).1, read_foreign_with(&file, units, iges_post)?));
        }
        Ok(out)
    })();
    let _ = std::fs::remove_dir_all(&dir);
    result
}

/// An .obj's groups as parts (each read as the mesh reader reads a file), named by the group and coloured by its
/// material's `Kd` from the .mtl in `dir` (sRGB 0..1) when there is one, in metres when its header says so; the whole
/// file as one mesh when it has no groups. (Live meshparts-1119 / meshcheck-1119: a bare .obj's groups are a part each,
/// never joined, one inside another too.)
fn read_obj_groups(obj: &Path, dir: &Path, unit: noshape_kernel::LengthUnit) -> anyhow::Result<(noshape_kernel::Shape, Vec<noshape_model::features::importing::ImportPartInfo>)> {
    use noshape_kernel as k;
    let text = std::fs::read_to_string(obj)?;
    // (Onshape's and ours say their unit in the header: "# OBJ generated by … Units = meters".)
    let unit = if text.lines().take(3).any(|l| l.starts_with('#') && l.contains("Units = meters")) { k::LengthUnit::Meter } else { unit };
    let mut shared = String::new();
    let mut groups: Vec<(String, Option<String>, String)> = Vec::new();
    let mut mtllib = None;
    for line in text.lines() {
        let mut words = line.split_whitespace();
        match words.next() {
            Some("v" | "vn" | "vt") => {
                shared.push_str(line);
                shared.push('\n');
            }
            Some("mtllib") => mtllib = words.next().map(str::to_string),
            Some("g") => groups.push((words.collect::<Vec<_>>().join(" "), None, String::new())),
            Some("usemtl") => {
                if let Some(g) = groups.last_mut() {
                    g.1 = words.next().map(str::to_string);
                }
            }
            Some("f") => {
                if let Some(g) = groups.last_mut() {
                    g.2.push_str(line);
                    g.2.push('\n');
                }
            }
            _ => {}
        }
    }
    let mtl = mtllib.and_then(|m| std::fs::read_to_string(dir.join(m)).ok());
    groups.retain(|g| !g.2.is_empty());
    if groups.is_empty() {
        return Ok((k::read_mesh_bodies(obj, unit)?, Vec::new()));
    }
    let mut kd: std::collections::HashMap<String, [f32; 3]> = std::collections::HashMap::new();
    let mut current = None;
    for line in mtl.as_deref().unwrap_or("").lines() {
        let w: Vec<&str> = line.split_whitespace().collect();
        match w.as_slice() {
            ["newmtl", name, ..] => current = Some(name.to_string()),
            ["Kd", r, g, b, ..] => {
                if let (Some(m), Ok(r), Ok(g), Ok(b)) = (&current, r.parse(), g.parse(), b.parse()) {
                    kd.insert(m.clone(), [r, g, b]);
                }
            }
            _ => {}
        }
    }
    let groups = groups.into_iter().map(|(name, material, faces)| (name, material.as_ref().and_then(|m| kd.get(m).copied()), format!("{shared}{faces}"))).collect();
    mesh_groups(groups, unit)
}

/// Mesh parts one per group (name, colour, the group's OBJ text, read in `unit`): a part each, never joined nor made
/// another's hollow (live meshparts-1119); empty groups dropped.
fn mesh_groups(groups: Vec<(String, Option<[f32; 3]>, String)>, unit: noshape_kernel::LengthUnit) -> anyhow::Result<(noshape_kernel::Shape, Vec<noshape_model::features::importing::ImportPartInfo>)> {
    use noshape_kernel as k;
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let tmp = std::env::temp_dir().join(format!("noshape-obj-groups-{}-{stamp}", std::process::id()));
    std::fs::create_dir_all(&tmp)?;
    let parts = (|| {
        let mut parts = Vec::new();
        for (i, (_, _, text)) in groups.iter().enumerate() {
            let part = tmp.join(format!("group-{i}.obj"));
            std::fs::write(&part, text)?;
            parts.push(k::read_mesh_bodies(&part, unit)?);
        }
        anyhow::Ok(parts)
    })();
    let _ = std::fs::remove_dir_all(&tmp);
    let parts = k::mesh_group_parts(&parts?)?;
    let (mut shapes, mut info) = (Vec::new(), Vec::new());
    for ((name, color, _), part) in groups.into_iter().zip(parts) {
        if part.count(k::ShapeType::Face) == 0 {
            continue;
        }
        shapes.push(part);
        info.push(noshape_model::features::importing::ImportPartInfo { name: Some(name), color, opacity: None, faces: Vec::new() });
    }
    Ok((k::Shape::compound(&shapes), info))
}

/// A glTF / GLB file's meshes as live imports them (exchange batch 2, 11-20): a part per node with a mesh, named by
/// the node, its triangles placed by the node's transforms; positions in metres as the file holds them (no Y-up turn:
/// that is the dialog's option).
fn read_gltf_groups(path: &Path) -> anyhow::Result<(noshape_kernel::Shape, Vec<noshape_model::features::importing::ImportPartInfo>)> {
    use glam::{DMat4, DQuat, DVec3};
    let bytes = std::fs::read(path)?;
    let u32_at = |b: &[u8], i: usize| b.get(i..i + 4).map(|x| u32::from_le_bytes([x[0], x[1], x[2], x[3]]) as usize);
    // GLB: a 12-byte header, then a JSON chunk and a BIN chunk.
    let (json_bytes, bin) = if bytes.starts_with(b"glTF") {
        let n = u32_at(&bytes, 12).ok_or_else(|| anyhow::anyhow!("a GLB without its JSON"))?;
        let json = bytes.get(20..20 + n).ok_or_else(|| anyhow::anyhow!("a GLB cut short"))?;
        let at = 20 + n;
        let bin = u32_at(&bytes, at).and_then(|m| bytes.get(at + 8..at + 8 + m)).map(<[u8]>::to_vec);
        (json.to_vec(), bin)
    } else {
        (bytes, None)
    };
    let j: serde_json::Value = serde_json::from_slice(&json_bytes)?;
    let dir = path.parent().unwrap_or(Path::new("."));
    let buffers: Vec<Vec<u8>> = j["buffers"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|b| match b["uri"].as_str() {
            None => bin.clone().ok_or_else(|| anyhow::anyhow!("a buffer without its data")),
            Some(u) if u.starts_with("data:") => u.split_once(',').map(|(_, d)| base64_decode(d)).ok_or_else(|| anyhow::anyhow!("a bad data URI")),
            Some(u) => Ok(std::fs::read(dir.join(u))?),
        })
        .collect::<anyhow::Result<_>>()?;
    // A buffer view's bytes.
    let view = |v: usize| -> Option<&[u8]> {
        let bv = &j["bufferViews"][v];
        let start = bv["byteOffset"].as_u64().unwrap_or(0) as usize;
        buffers.get(bv["buffer"].as_u64()? as usize)?.get(start..start + bv["byteLength"].as_u64()? as usize)
    };
    // An accessor's elements as numbers (`n` per element).
    let read = |a: usize, n: usize| -> Option<Vec<f64>> {
        let acc = &j["accessors"][a];
        let bv = &j["bufferViews"][acc["bufferView"].as_u64()? as usize];
        let buf = buffers.get(bv["buffer"].as_u64()? as usize)?;
        let ct = acc["componentType"].as_u64()?;
        let size = match ct {
            5120 | 5121 => 1,
            5122 | 5123 => 2,
            _ => 4,
        };
        let stride = bv["byteStride"].as_u64().map_or(size * n, |s| s as usize);
        let start = bv["byteOffset"].as_u64().unwrap_or(0) as usize + acc["byteOffset"].as_u64().unwrap_or(0) as usize;
        let mut out = Vec::new();
        for e in 0..acc["count"].as_u64()? as usize {
            for c in 0..n {
                let at = start + e * stride + c * size;
                let x = buf.get(at..at + size)?;
                out.push(match ct {
                    5126 => f64::from(f32::from_le_bytes([x[0], x[1], x[2], x[3]])),
                    5125 => f64::from(u32::from_le_bytes([x[0], x[1], x[2], x[3]])),
                    5123 => f64::from(u16::from_le_bytes([x[0], x[1]])),
                    5122 => f64::from(i16::from_le_bytes([x[0], x[1]])),
                    5121 => f64::from(x[0]),
                    _ => f64::from(x[0] as i8),
                });
            }
        }
        Some(out)
    };
    let local = |n: &serde_json::Value| -> DMat4 {
        let f = |k: &str| n[k].as_array().map(|a| a.iter().filter_map(serde_json::Value::as_f64).collect::<Vec<f64>>());
        if let Some(m) = f("matrix").filter(|m| m.len() == 16) {
            return DMat4::from_cols_slice(&m);
        }
        let t = f("translation").filter(|v| v.len() == 3).map_or(DVec3::ZERO, |v| DVec3::new(v[0], v[1], v[2]));
        let r = f("rotation").filter(|v| v.len() == 4).map_or(DQuat::IDENTITY, |v| DQuat::from_xyzw(v[0], v[1], v[2], v[3]));
        let s = f("scale").filter(|v| v.len() == 3).map_or(DVec3::ONE, |v| DVec3::new(v[0], v[1], v[2]));
        DMat4::from_scale_rotation_translation(s, r, t)
    };
    let scene = j["scene"].as_u64().unwrap_or(0) as usize;
    let mut stack: Vec<(usize, DMat4)> = j["scenes"][scene]["nodes"].as_array().into_iter().flatten().filter_map(|n| n.as_u64()).map(|n| (n as usize, DMat4::IDENTITY)).rev().collect();
    let mut groups = Vec::new();
    while let Some((i, parent)) = stack.pop() {
        let node = &j["nodes"][i];
        let at = parent * local(node);
        for c in node["children"].as_array().into_iter().flatten().rev().filter_map(|c| c.as_u64()) {
            stack.push((c as usize, at));
        }
        let Some(m) = node["mesh"].as_u64() else { continue };
        let mesh = &j["meshes"][m as usize];
        let (mut verts, mut faces) = (String::new(), String::new());
        let mut base = 0;
        for p in mesh["primitives"].as_array().into_iter().flatten().filter(|p| p["mode"].as_u64().unwrap_or(4) == 4) {
            // Compressed (live's Compress): the positions and triangles in the extension's Draco buffer view.
            let draco = &p["extensions"]["KHR_draco_mesh_compression"];
            let (pos, index) = if draco.is_object() {
                let data = draco["bufferView"].as_u64().and_then(|v| view(v as usize)).ok_or_else(|| anyhow::anyhow!("a compressed mesh without its data"))?;
                let id = draco["attributes"]["POSITION"].as_u64().ok_or_else(|| anyhow::anyhow!("a compressed mesh without positions"))?;
                draco_triangles(data, id as u32)?
            } else {
                let Some(pos) = p["attributes"]["POSITION"].as_u64().and_then(|a| read(a as usize, 3)) else { continue };
                let n = pos.len() / 3;
                let index = p["indices"].as_u64().and_then(|a| read(a as usize, 1)).map_or_else(|| (0..n).collect(), |v| v.into_iter().map(|x| x as usize).collect());
                (pos, index)
            };
            let n = pos.len() / 3;
            for v in pos.chunks(3) {
                let w = at.transform_point3(DVec3::new(v[0], v[1], v[2]));
                verts.push_str(&format!("v {} {} {}\n", w.x, w.y, w.z));
            }
            for t in index.as_chunks::<3>().0.iter().filter(|t| t.iter().all(|x| *x < n)) {
                faces.push_str(&format!("f {} {} {}\n", base + t[0] + 1, base + t[1] + 1, base + t[2] + 1));
            }
            base += n;
        }
        if faces.is_empty() {
            continue;
        }
        let name = node["name"].as_str().or(mesh["name"].as_str()).map_or_else(|| format!("Part {}", groups.len() + 1), str::to_string);
        groups.push((name, None, format!("{verts}{faces}")));
    }
    anyhow::ensure!(!groups.is_empty(), "no meshes in the file");
    mesh_groups(groups, noshape_kernel::LengthUnit::Meter)
}

/// A Draco mesh's positions (the attribute of unique id `position`) and triangles' point indices.
fn draco_triangles(data: &[u8], position: u32) -> anyhow::Result<(Vec<f64>, Vec<usize>)> {
    let mut mesh = draco_core::Mesh::new();
    draco_core::MeshDecoder::new().decode(&mut draco_core::DecoderBuffer::new(data), &mut mesh).map_err(|e| anyhow::anyhow!("a compressed mesh that doesn't decode: {e}"))?;
    let att = mesh.attribute_by_unique_id(position).ok_or_else(|| anyhow::anyhow!("a compressed mesh without positions"))?;
    let pos = att.read_f32s(mesh.num_points(), 3).into_iter().map(f64::from).collect();
    let index = (0..mesh.num_faces() as u32).flat_map(|f| mesh.face(draco_core::FaceIndex(f))).map(|p| p.0 as usize).collect();
    Ok((pos, index))
}

/// Standard base64 (a glTF data URI's buffer); other characters skipped.
fn base64_decode(text: &str) -> Vec<u8> {
    let value = |c: u8| match c {
        b'A'..=b'Z' => Some(c - b'A'),
        b'a'..=b'z' => Some(c - b'a' + 26),
        b'0'..=b'9' => Some(c - b'0' + 52),
        b'+' | b'-' => Some(62),
        b'/' | b'_' => Some(63),
        _ => None,
    };
    let (mut out, mut acc, mut bits) = (Vec::with_capacity(text.len() * 3 / 4), 0u32, 0);
    for v in text.bytes().filter_map(value) {
        acc = (acc << 6) | u32::from(v);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    out
}

/// The file read as the Import reads it: one shape (its named and coloured parts a compound's children, described in
/// the infos, empty when the file has none).
fn read_shape(path: &Path, units: &UnitPrefs, iges_post: bool) -> anyhow::Result<(noshape_kernel::Shape, Vec<noshape_model::features::importing::ImportPartInfo>)> {
    use noshape_kernel as k;
    let mesh_unit = mesh_unit_of(units);
    let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    // Parts with names and colours: one compound child each, described in `info`.
    let mut info: Vec<noshape_model::features::importing::ImportPartInfo> = Vec::new();
    let mut compound = |parts: Vec<k::ImportedShape>| {
        let parts = k::split_connected_pieces(parts);
        info = parts.iter().map(|p| noshape_model::features::importing::ImportPartInfo { name: p.name.clone(), color: p.color, opacity: p.opacity, faces: p.face_colors.clone() }).collect();
        k::Shape::compound(&parts.into_iter().map(|p| p.shape).collect::<Vec<_>>())
    };
    let shape = match ext.as_str() {
        "step" | "stp" => compound(instances_suffixed(k::read_step(path, k::LengthUnit::Meter)?)),
        // Named and coloured (XDE), each placed instance a part.
        // IGES postprocessing: straight edges as lines (read in metres: straight within a micrometre; the parts and
        // their faces keep their order, so `info` still describes them).
        "iges" | "igs" if iges_post => k::straighten_edges(&compound(k::read_iges_parts(path, k::LengthUnit::Meter)?), 1e-6)?,
        "iges" | "igs" => compound(k::read_iges_parts(path, k::LengthUnit::Meter)?),
        "brep" => k::read_brep(path)?,
        // Onshape imports STL, OBJ and PVZ as mesh parts (PVZ is in metres); an OBJ a part per group, named by it.
        "stl" => k::read_mesh_bodies(path, mesh_unit)?,
        "obj" => {
            let (shape, parts) = read_obj_groups(path, path.parent().unwrap_or(Path::new(".")), mesh_unit)?;
            info = parts;
            shape
        }
        // 3MF a part per build item, named by its object (its unit in the file).
        "3mf" => {
            let parts = k::read_3mf_parts(path)?;
            info = parts.iter().map(|p| noshape_model::features::importing::ImportPartInfo { name: p.name.clone(), color: None, opacity: None, faces: Vec::new() }).collect();
            k::Shape::compound(&parts.into_iter().map(|p| p.shape).collect::<Vec<_>>())
        }
        // glTF a part per node with a mesh, named by it (in metres, as the file holds it).
        "gltf" | "glb" => {
            let (shape, parts) = read_gltf_groups(path)?;
            info = parts;
            shape
        }
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
    Ok((shape, info))
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
    /// A STEP assembly's subassemblies (live request 1742).
    nesting: Option<noshape_kernel::StepNesting>,
    /// The file's part index of each part here (`pvz_of_tree`'s order).
    order: Vec<usize>,
    /// Each part's (solids, shells, faces, edges), STEP / Parasolid.
    part_shapes: Option<Vec<[usize; 4]>>,
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
    Ok(Some(PvzAssembly { name, file, root: if root.is_empty() { "RootAssembly".into() } else { root }, foreign, names, instances, step: false, nesting: None, order: Vec::new(), part_shapes: None }))
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
    // An assembly of one instance is read as parts too: live imports it as a
    // Part Studio only, the part where the instance was (live/dx/one-part-asm-1122).
    const NAUO: &[u8] = b"NEXT_ASSEMBLY_USAGE_OCCURRENCE";
    if std::fs::read(path)?.windows(NAUO.len()).filter(|w| w.eq_ignore_ascii_case(NAUO)).count() < 2 {
        return Ok(None);
    }
    let Some((a, nesting)) = k::read_step_assembly_nested(path, k::LengthUnit::Meter)? else { return Ok(None) };
    let mut p = pvz_of_tree(path, a)?;
    // Subassemblies keep their own Assembly tabs (live request 1742 (A)),
    // their parts numbered as the Part Studio's.
    if !nesting.subs.is_empty() {
        let mut nesting = nesting;
        for (_, c) in &mut nesting.children {
            c.part = c.part.and_then(|f| p.order.iter().position(|&o| o == f));
        }
        p.nesting = Some(nesting);
    }
    Ok(Some(p))
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

/// An IGES file holding placed instances of shared shapes (subfigures: how
/// Onshape exports an assembly to IGES; live 1755 re-imports it as an
/// Assembly of 2000 instances and a "<file> parts" Part Studio); `None` for
/// files of separate shapes.
fn read_iges_assembly(path: &Path) -> anyhow::Result<Option<PvzAssembly>> {
    use noshape_kernel as k;
    let Some(a) = k::read_iges_assembly(path, k::LengthUnit::Meter)? else { return Ok(None) };
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
    let part_shapes = parts.iter().map(|p| [k::ShapeType::Solid, k::ShapeType::Shell, k::ShapeType::Face, k::ShapeType::Edge].map(|t| p.shape.count(t))).collect();
    let t = std::time::Instant::now();
    let shape = k::Shape::compound(&parts.into_iter().map(|p| p.shape).collect::<Vec<_>>());
    let foreign = Foreign { brep: String::from_utf8_lossy(&shape.to_brep_bytes()?).to_string(), parts: serde_json::to_string(&info)? };
    if std::env::var_os("NOSHAPE_IMPORT_TIMING").is_some() {
        eprintln!("import stage brep text ({} MB): {:.1} ms", foreign.brep.len() / 1_000_000, t.elapsed().as_secs_f64() * 1000.0);
    }
    let (name, file) = import_names(path);
    Ok(PvzAssembly { name, file, root, foreign, names, instances, step: true, nesting: None, order, part_shapes: Some(part_shapes) })
}

/// Files read for an import (before the document edit).
pub struct ImportInputs {
    read: Vec<(String, String, Foreign)>,
    pvz: Vec<PvzAssembly>,
    assemblies: Vec<noshape_model::native_assembly::AssemblyZip>,
    files: Vec<FileTab>,
    /// Upload only: the files as file tabs, after the import's tab (live upload-only-1118).
    uploads: Vec<FileTab>,
    /// "a.step, b.zip": the history entry is "Import <label>".
    pub label: String,
}

impl ImportInputs {
    /// Nothing to add: no translation and no file tab.
    pub fn is_empty(&self) -> bool {
        self.read.is_empty() && self.pvz.is_empty() && self.assemblies.is_empty() && self.files.is_empty() && self.uploads.is_empty()
    }
}

/// Read and translate the files (mesh files in the chosen unit); a file that doesn't translate gives live's
/// "<file> uploaded but failed to translate." (one live refuses gives its own "<file> was not translated. …").
pub fn read_inputs(paths: &[PathBuf], c: &ImportChoice, keep_files: bool, units: &UnitPrefs) -> (ImportInputs, Vec<String>) {
    let mut errors = Vec::new();
    if c.upload_only {
        let mut uploads = Vec::new();
        for path in paths {
            match std::fs::read(path) {
                Ok(bytes) => uploads.push(FileTab::new(&import_names(path).1, &bytes, noshape_model::file_tab::now())),
                Err(e) => errors.push(format!("Import failed: {}: {e}", path.display())),
            }
        }
        let label = paths.iter().map(|p| import_names(p).1).collect::<Vec<_>>().join(", ");
        return (ImportInputs { read: Vec::new(), pvz: Vec::new(), assemblies: Vec::new(), files: Vec::new(), uploads, label }, errors);
    }
    let mut read = Vec::new();
    // Native assemblies (a zip named like the root assembly; live G38).
    let mut assemblies = Vec::new();
    let mut pvz = Vec::new();
    for path in paths {
        if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("zip")) {
            match noshape_model::native_assembly::read_assembly_zip(path) {
                Ok(a) => assemblies.push(a),
                // An OBJ export's zip: its .obj (Onshape refuses it; only an .mtl without one is its message).
                Err(e) if e == noshape_model::native_assembly::MTL_IN_ZIP => {
                    let mesh = UnitPrefs { length: MESH_UNITS[c.unit.min(MESH_UNITS.len() - 1)].1, ..*units };
                    match read_obj_zip(path, &mesh) {
                        Ok(Some(objs)) => read.extend(objs),
                        Ok(None) => errors.push(format!("{} was not translated. {e}", import_names(path).1)),
                        Err(_) => errors.push(failed_to_translate(&import_names(path).1)),
                    }
                }
                Err(e) if e == noshape_model::native_assembly::XT_IN_ZIP => match read_xt_zip(path, units, c.iges_post) {
                    Ok(parts) => read.extend(parts),
                    Err(_) => errors.push(failed_to_translate(&import_names(path).1)),
                },
                // Onshape keeps the zip as a file tab and reports it.
                Err(e) if e == noshape_model::native_assembly::STEP_IN_ZIP => errors.push(format!("{} was not translated. {e}", import_names(path).1)),
                Err(_) => errors.push(failed_to_translate(&import_names(path).1)),
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
                Err(_) => {
                    errors.push(failed_to_translate(&import_names(path).1));
                    continue;
                }
            }
        }
        // A STEP or Parasolid assembly: Assembly + Part Studio (not when combining).
        let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
        if !c.combine && matches!(ext.as_str(), "step" | "stp" | "x_t" | "x_b" | "igs" | "iges") {
            let tree = match ext.as_str() {
                "x_t" | "x_b" => read_xt_assembly(path),
                "igs" | "iges" => read_iges_assembly(path),
                _ => read_step_assembly(path),
            };
            match tree {
                Ok(Some(a)) => {
                    pvz.push(a);
                    continue;
                }
                Ok(None) => {}
                Err(_) => {
                    errors.push(failed_to_translate(&import_names(path).1));
                    continue;
                }
            }
        }
        // Mesh files in the unit chosen in the dialog.
        let units = if MESH_FILES.iter().any(|e| path.extension().is_some_and(|x| x.eq_ignore_ascii_case(e))) { UnitPrefs { length: MESH_UNITS[c.unit.min(MESH_UNITS.len() - 1)].1, ..*units } } else { *units };
        match read_foreign_with(path, &units, c.iges_post) {
            Ok(foreign) => {
                let (name, file) = import_names(path);
                read.push((name, file, foreign));
            }
            Err(_) => errors.push(failed_to_translate(&import_names(path).1)),
        }
    }
    let mut label = read.iter().map(|r| r.1.clone()).chain(pvz.iter().map(|p| p.file.clone())).chain(assemblies.iter().map(|a| format!("{}.zip", a.zip))).collect::<Vec<_>>().join(", ");
    if label.is_empty() {
        label = paths.iter().map(|p| import_names(p).1).collect::<Vec<_>>().join(", ");
    }
    let files = if keep_files { paths.iter().filter_map(|p| Some(FileTab::new(&import_names(p).1, &std::fs::read(p).ok()?, noshape_model::file_tab::now()))).collect() } else { Vec::new() };
    (ImportInputs { read, pvz, assemblies, files, uploads: Vec::new(), label }, errors)
}

/// The import's tabs added to `d` after `after`: returns the first new tab
/// (to open) and the errors of assemblies that failed.
pub fn apply(d: &mut Document, inputs: ImportInputs, c: &ImportChoice, after: Option<TabId>, units: &UnitPrefs) -> (Option<TabId>, Vec<String>) {
    let mut first = None;
    let mut errors = Vec::new();
    // Upload only: each file as it is, right after the import's tab (live upload-only-1118).
    let mut at = after;
    for f in inputs.uploads {
        let id = add_file_tab(d, f, None, at);
        first.get_or_insert(id);
        at = Some(id);
    }
    // Onshape keeps the imported files in a "CAD Imports" folder.
    if !inputs.files.is_empty() {
        let folder = cad_imports_folder(d);
        for f in inputs.files {
            add_file_tab(d, f, Some(folder), None);
        }
    }
    for a in &inputs.assemblies {
        // Combine: one Part Studio, no Assembly (live G41 (5)).
        if c.combine {
            match noshape_model::native_assembly::add_native_assembly_combined(d, &a.root, &a.components, &a.files, c.appearances, units) {
                Ok(studio) => {
                    crate::docedit::place_after(d, studio, at);
                    at = Some(studio);
                    first.get_or_insert(studio);
                }
                Err(_) => errors.push(failed_to_translate(&format!("{}.zip", a.zip))),
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
            Err(_) => errors.push(failed_to_translate(&format!("{}.zip", a.zip))),
        }
    }
    for p in &inputs.pvz {
        let mut options = vec![("importAppearances", c.appearances), ("yAxisIsUp", c.y_up), ("createComposite", c.composite)];
        if p.step {
            options.push(("joinAdjacentSurfaces", c.join));
        }
        let pvz = noshape_model::native_assembly::PvzImport { stem: p.name.clone(), root: p.root.clone(), brep: p.foreign.brep.clone(), info: p.foreign.parts.clone(), foreign_id: p.file.clone(), instances: p.instances.clone(), names: p.names.clone(), step: p.step, options, nesting: p.nesting.clone(), part_shapes: p.part_shapes.clone() };
        match noshape_model::native_assembly::add_pvz_assembly(d, &pvz, units) {
            Ok((studio, assembly)) => {
                crate::docedit::place_after(d, assembly, at);
                // Its subassemblies' Assemblies next, as they are used (live
                // request 1742: Assembly 1, Sub, the parts' Part Studio).
                let mut last = assembly;
                let mut queue = vec![assembly];
                let mut seen = vec![assembly];
                while let Some(t) = queue.pop() {
                    let Some(noshape_model::TabContent::Assembly(a)) = d.tab(t).map(|t| &t.content) else { continue };
                    let subs: Vec<TabId> = a.instances.iter().filter_map(|i| if let noshape_model::assembly::InstanceSource::Assembly { assembly, .. } = &i.source { Some(*assembly) } else { None }).collect();
                    for s in subs.into_iter().rev() {
                        if !seen.contains(&s) {
                            seen.push(s);
                            crate::docedit::place_after(d, s, Some(last));
                            last = s;
                            queue.insert(0, s);
                        }
                    }
                }
                crate::docedit::place_after(d, studio, Some(last));
                at = Some(studio);
                first.get_or_insert(assembly);
            }
            Err(_) => errors.push(failed_to_translate(&p.file)),
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
    /// A glTF's meshes as live imports them (exchange batch 2, 11-20): a part per node, named by it, even where two
    /// touch (c2-duct-multipart's four; ours read them as two).
    #[test]
    fn gltf_nodes_are_parts() {
        // Two unit tetrahedra sharing a vertex, one buffer (positions, then u16 indices), one node each.
        let mut bin: Vec<u8> = Vec::new();
        for p in [[0.0f32, 0.0, 0.0], [0.01, 0.0, 0.0], [0.0, 0.01, 0.0], [0.0, 0.0, 0.01]] {
            p.iter().for_each(|x| bin.extend(x.to_le_bytes()));
        }
        for i in [0u16, 2, 1, 0, 1, 3, 1, 2, 3, 0, 3, 2] {
            bin.extend(i.to_le_bytes());
        }
        let uri = format!("data:application/octet-stream;base64,{}", crate::export::base64(&bin));
        let gltf = serde_json::json!({
            "asset": { "version": "2.0" },
            "scene": 0,
            "scenes": [{ "nodes": [0, 1] }],
            "nodes": [{ "name": "Part 4", "mesh": 0 }, { "name": "Part 2", "mesh": 0, "translation": [-0.01, 0.0, 0.0] }],
            "meshes": [{ "primitives": [{ "attributes": { "POSITION": 0 }, "indices": 1 }] }],
            "buffers": [{ "byteLength": bin.len(), "uri": uri }],
            "bufferViews": [{ "buffer": 0, "byteLength": 48 }, { "buffer": 0, "byteOffset": 48, "byteLength": 24 }],
            "accessors": [
                { "bufferView": 0, "componentType": 5126, "count": 4, "type": "VEC3" },
                { "bufferView": 1, "componentType": 5123, "count": 12, "type": "SCALAR" }
            ]
        });
        let dir = std::env::temp_dir().join(format!("noshape-gltf-nodes-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("two.gltf");
        std::fs::write(&path, gltf.to_string()).unwrap();
        let read = super::read_gltf_groups(&path);
        let _ = std::fs::remove_dir_all(&dir);
        let (shape, info) = read.unwrap();
        let names: Vec<Option<String>> = info.into_iter().map(|p| p.name).collect();
        assert_eq!(names, [Some("Part 4".to_string()), Some("Part 2".to_string())]);
        assert_eq!(shape.children().len(), 2);
    }

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

    /// Onshape's Parasolid export of a Part Studio with a mesh part comes zipped, the .x_t with the .xmm_txt holding
    /// its meshes (live c2-hex-bookcase-derived; live's import of the zip: its 2 parts): both parts come in, named.
    #[test]
    fn a_parasolid_zip_imports_its_parts_and_meshes() {
        let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../kernel/tests/data/xt-parts");
        let read = |f: &str| std::fs::read(data.join(f)).unwrap();
        let (xt, xmm) = (read("hex-bookcase-mesh.x_t"), read("hex-bookcase-mesh.xmm_txt"));
        let dir = std::env::temp_dir().join(format!("noshape-xt-zip-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let zip = dir.join("Part Studio 1.zip");
        std::fs::write(&zip, crate::export::stored_zip(&[("Part Studio 1.x_t", &xt), ("Part Studio 1.xmm_txt", &xmm)])).unwrap();
        let (inputs, errors) = super::read_inputs(std::slice::from_ref(&zip), &super::ImportChoice::default(), false, &noshape_units::UnitPrefs::default());
        let _ = std::fs::remove_dir_all(&dir);
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(inputs.read.iter().map(|r| (r.0.as_str(), r.1.as_str())).collect::<Vec<_>>(), [("Part Studio 1", "Part Studio 1.zip")]);
        let parts = &inputs.read[0].2.parts;
        assert!(parts.contains("\"shrunk\"") && parts.contains("\"imported\""), "{parts}");
    }
}
