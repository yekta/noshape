//! Part and assembly export (the Export modal's writers): every format of
//! Onshape's Export dialog, with its options. Shared by the app's Export
//! modal and the agent API.

use noshape_kernel as k;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PartFormat {
    Parasolid,
    Acis,
    Step,
    Iges,
    Pvz,
    Jt,
    Rhino,
    Gltf,
    Glb,
    Obj,
    ThreeMf,
    /// Assemblies only: a robot description with its mesh files (zip).
    Urdf,
    Stl,
}

impl PartFormat {
    /// Onshape's order (PARASOLID is the default).
    pub const ALL: [PartFormat; 12] = [Self::Parasolid, Self::Acis, Self::Step, Self::Iges, Self::Pvz, Self::Jt, Self::Rhino, Self::Gltf, Self::Glb, Self::Obj, Self::ThreeMf, Self::Stl];

    /// The Format list: Part Studios as `ALL`; assemblies without RHINO and
    /// with URDF before STL (live dom/dialog-export-partstudio-assembly.txt).
    /// RHINO only in builds that have it ([`k::RHINO_EXPORT`]; releases do).
    pub fn list(assembly: bool) -> Vec<PartFormat> {
        if assembly {
            vec![Self::Parasolid, Self::Acis, Self::Step, Self::Iges, Self::Pvz, Self::Jt, Self::Gltf, Self::Glb, Self::Obj, Self::ThreeMf, Self::Urdf, Self::Stl]
        } else {
            Self::ALL.into_iter().filter(|f| *f != Self::Rhino || k::RHINO_EXPORT).collect()
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Parasolid => "PARASOLID",
            Self::Acis => "ACIS",
            Self::Step => "STEP",
            Self::Iges => "IGES",
            Self::Pvz => "PVZ",
            Self::Jt => "JT",
            Self::Rhino => "RHINO",
            Self::Gltf => "GLTF",
            Self::Glb => "GLB",
            Self::Obj => "OBJ",
            Self::ThreeMf => "3MF",
            Self::Urdf => "URDF",
            Self::Stl => "STL",
        }
    }

    pub fn ext(self) -> &'static str {
        match self {
            Self::Parasolid => "x_t",
            Self::Acis => "sat",
            Self::Step => "step",
            Self::Iges => "igs",
            Self::Pvz => "pvz",
            Self::Jt => "jt",
            Self::Rhino => "3dm",
            Self::Gltf => "gltf",
            Self::Glb => "glb",
            Self::Obj => "obj",
            Self::ThreeMf => "3mf",
            Self::Urdf => "zip",
            Self::Stl => "stl",
        }
    }

    pub fn mesh(self) -> bool {
        matches!(self, Self::Gltf | Self::Glb | Self::Obj | Self::ThreeMf | Self::Stl)
    }
}

pub const PARASOLID_VERSIONS: [&str; 25] = ["38.1", "37.1", "37.0", "36.0", "35.1", "34.1", "34.0", "33.1", "33.0", "32.1", "32.0", "31.1", "31.0", "30.1", "30.0", "29.1", "29.0", "28.1", "28.0", "27.1", "27.0", "26.1", "26.0", "25.1", "25.0"];
pub const STEP_PREPROCESSING: [&str; 5] = ["None", "Automatic", "Remove small entities only (advanced)", "Remove small entities only (basic)", "Remove small entities only (advanced) and convert all surfaces to b-surfaces"];
pub const STEP_VERSIONS: [&str; 3] = ["AP242", "AP214", "AP203"];
pub const RHINO_VERSIONS: [&str; 7] = ["8", "7", "6", "5", "4", "3", "2"];
pub const RESOLUTIONS: [&str; 4] = ["Coarse", "Medium", "Fine", "Custom"];
pub const STL_FORMATS: [&str; 2] = ["Text", "Binary"];
/// Units and their size in meters.
pub const UNITS: [(&str, f64); 6] = [("Centimeter", 0.01), ("Foot", 0.3048), ("Inch", 0.0254), ("Meter", 1.0), ("Millimeter", 0.001), ("Yard", 0.9144)];

pub const OPTIONS: [&str; 3] = ["Download", "Download and store file in a new tab", "Store file in a new tab"];
/// URDF "Geometry format".
pub const URDF_GEOMETRY: [PartFormat; 4] = [PartFormat::Stl, PartFormat::Gltf, PartFormat::Glb, PartFormat::Obj];

#[derive(Clone, Debug)]
pub struct ExportSettings {
    pub format: PartFormat,
    pub y_up: bool,
    /// Version index (Parasolid, STEP, Rhino) and "Use latest version".
    pub version: usize,
    pub latest: bool,
    pub preprocessing: usize,
    pub custom_units: bool,
    pub derived_tolerances: bool,
    pub mbd_text_height: bool,
    pub iges_compat: bool,
    pub pvz_group: bool,
    pub resolution: usize,
    pub compress: bool,
    pub stl_text: bool,
    pub units: usize,
    pub individual: bool,
    pub hidden: bool,
    /// URDF: index into `URDF_GEOMETRY`.
    pub urdf_geometry: usize,
    /// Options: 0 Download, 1 Download and store file in a new tab, 2 Store
    /// file in a new tab (see [`store_file_tab`]).
    pub options: usize,
}

impl Default for ExportSettings {
    fn default() -> Self {
        Self {
            format: PartFormat::Parasolid,
            y_up: false,
            version: 0,
            latest: true,
            preprocessing: 0,
            custom_units: false,
            derived_tolerances: false,
            mbd_text_height: false,
            iges_compat: false,
            pvz_group: false,
            resolution: 1,
            compress: false,
            stl_text: false,
            units: 3,
            individual: false,
            hidden: false,
            urdf_geometry: 0,
            options: 0,
        }
    }
}

/// Write `parts` (meters, placed; linear RGBA): Y axis up, one file, or
/// with "Export unique parts as individual files" one file per part zipped
/// together (Onshape), or URDF. Returns the written file.
pub fn write_export(s: &ExportSettings, path: &std::path::Path, parts: Vec<(String, k::Shape, [f32; 4])>) -> anyhow::Result<std::path::PathBuf> {
    anyhow::ensure!(!parts.is_empty(), "There are no parts to export");
    if let Some(msg) = missing_writer(s.format) {
        anyhow::bail!(msg);
    }
    // Y axis up: the model's +Z becomes +Y.
    let parts: Vec<(String, k::Shape, [f32; 4])> = if s.y_up && s.format != PartFormat::Urdf {
        parts
            .into_iter()
            .map(|(n, sh, c)| Ok((n, sh.transformed(&k::Transform::Rotate { origin: glam::DVec3::ZERO, axis: glam::DVec3::X, angle: -std::f64::consts::FRAC_PI_2 })?, c)))
            .collect::<anyhow::Result<_>>()?
    } else {
        parts
    };
    let tmp = |i: usize, ext: &str| std::env::temp_dir().join(format!("noshape-export-{}-{i}.{ext}", std::process::id()));
    // Each part through the format's writer into a temporary file.
    let files_of = |s: &ExportSettings, ext: &str| -> anyhow::Result<Vec<(String, Vec<u8>)>> {
        let mut used = std::collections::HashSet::new();
        let mut out = Vec::new();
        for (i, p) in parts.iter().enumerate() {
            let mut name = sanitize(&p.0).replace(' ', "_");
            while !used.insert(name.clone()) {
                name.push('_');
            }
            let t = tmp(i, ext);
            write_parts(s, &t, std::slice::from_ref(p))?;
            out.push((format!("{name}.{ext}"), std::fs::read(&t)?));
            let _ = std::fs::remove_file(&t);
        }
        Ok(out)
    };
    if s.format == PartFormat::Urdf {
        let geo = URDF_GEOMETRY[s.urdf_geometry.min(URDF_GEOMETRY.len() - 1)];
        let ms = ExportSettings { format: geo, y_up: false, individual: false, ..s.clone() };
        let robot = path.file_stem().map(|x| x.to_string_lossy().to_string()).unwrap_or_else(|| "robot".into());
        let meshes = files_of(&ms, geo.ext())?;
        let mut urdf = format!("<?xml version=\"1.0\"?>\n<robot name=\"{}\">\n  <link name=\"base_link\"/>\n", xml_escape(&robot));
        for ((file, _), p) in meshes.iter().zip(&parts) {
            let link = file.rsplit_once('.').map(|x| x.0).unwrap_or(file);
            let c = p.2;
            urdf.push_str(&format!(
                "  <link name=\"{link}\">\n    <visual>\n      <geometry><mesh filename=\"package://{robot}/meshes/{file}\"/></geometry>\n      <material name=\"{link}_color\"><color rgba=\"{} {} {} 1\"/></material>\n    </visual>\n    <collision>\n      <geometry><mesh filename=\"package://{robot}/meshes/{file}\"/></geometry>\n    </collision>\n  </link>\n  <joint name=\"{link}_joint\" type=\"fixed\">\n    <parent link=\"base_link\"/>\n    <child link=\"{link}\"/>\n  </joint>\n",
                c[0].powf(1.0 / 2.2),
                c[1].powf(1.0 / 2.2),
                c[2].powf(1.0 / 2.2)
            ));
        }
        urdf.push_str("</robot>\n");
        let names: Vec<String> = meshes.iter().map(|(f, _)| format!("meshes/{f}")).collect();
        let urdf_name = format!("{robot}.urdf");
        let mut entries: Vec<(&str, &[u8])> = vec![(urdf_name.as_str(), urdf.as_bytes())];
        entries.extend(names.iter().zip(&meshes).map(|(n, (_, b))| (n.as_str(), b.as_slice())));
        let out = path.with_extension("zip");
        std::fs::write(&out, stored_zip(&entries))?;
        return Ok(out);
    }
    if s.individual && parts.len() > 1 {
        let files = files_of(s, s.format.ext())?;
        let entries: Vec<(&str, &[u8])> = files.iter().map(|(n, b)| (n.as_str(), b.as_slice())).collect();
        let out = path.with_extension("zip");
        std::fs::write(&out, stored_zip(&entries))?;
        return Ok(out);
    }
    write_parts(s, path, &parts)?;
    Ok(path.to_path_buf())
}

/// An Assembly to export: the root's name, each distinct part once in its
/// own coordinates (meters; linear RGBA), and the instances placing them.
pub struct AssemblyExport {
    pub root: String,
    pub parts: Vec<(String, k::Shape, [f32; 4])>,
    /// (part index, instance name, placement in meters).
    pub instances: Vec<(usize, String, glam::DMat4)>,
}

impl AssemblyExport {
    /// The solved assembly's part occurrences (`ids`: those instances, None:
    /// all; hidden ones only with "Include hidden instances in export").
    pub fn of(root: &str, result: &noshape_model::assembly::eval::AssemblyResult, ids: Option<&[noshape_model::assembly::InstanceId]>, include_hidden: bool) -> Self {
        let mut keys: Vec<(noshape_model::document::TabId, noshape_model::BodyId, noshape_model::configuration::ConfigValues)> = Vec::new();
        let mut out = AssemblyExport { root: root.to_string(), parts: Vec::new(), instances: Vec::new() };
        for o in result.occurrences.iter().filter(|o| (include_hidden || !o.hidden) && ids.is_none_or(|ids| ids.contains(&o.top()))) {
            let key = (o.studio, o.body.clone(), o.config.clone());
            let part = match keys.iter().position(|k| *k == key) {
                Some(i) => i,
                None => {
                    let Some(b) = o.context.bodies.iter().find(|b| b.id == o.body) else { continue };
                    let c = o.color;
                    out.parts.push((o.body_name.clone(), b.shape.clone(), [c[0], c[1], c[2], 1.0]));
                    keys.push(key);
                    keys.len() - 1
                }
            };
            out.instances.push((part, o.instance_name.clone(), o.transform));
        }
        out
    }

    /// Each instance as a placed part, named after it (formats without
    /// assemblies).
    pub fn placed(&self) -> anyhow::Result<Vec<(String, k::Shape, [f32; 4])>> {
        self.instances.iter().map(|(p, name, t)| Ok((name.clone(), self.parts[*p].1.transformed(&k::Transform::Matrix(*t))?, self.parts[*p].2))).collect()
    }
}

/// Write an Assembly's export: STEP keeps the assembly (Onshape: the root
/// product with each part once, placed by its instances); the other formats
/// and individual files get the placed parts.
pub fn write_assembly_export(s: &ExportSettings, path: &std::path::Path, a: &AssemblyExport) -> anyhow::Result<std::path::PathBuf> {
    anyhow::ensure!(!a.instances.is_empty(), "There are no parts to export");
    if s.format != PartFormat::Step || s.individual {
        return write_export(s, path, a.placed()?);
    }
    // Y axis up: the model's +Z becomes +Y.
    let up = if s.y_up { glam::DMat4::from_rotation_x(-std::f64::consts::FRAC_PI_2) } else { glam::DMat4::IDENTITY };
    let shapes: Vec<k::ExportShape> = a.parts.iter().map(|(n, sh, c)| step_shape(n, sh, c)).collect();
    let instances: Vec<k::StepInstance> = a.instances.iter().map(|(p, _, t)| k::StepInstance { part: *p, transform: up * *t }).collect();
    k::write_step_assembly(path, &a.root, &shapes, &instances, k::LengthUnit::Meter, &step_options(s))?;
    Ok(path.to_path_buf())
}

/// A part as the STEP writer takes it: Onshape writes the colour's 8-bit sRGB values.
fn step_shape(n: &str, sh: &k::Shape, c: &[f32; 4]) -> k::ExportShape {
    let srgb = |v: f32| crate::appearance::lin_to_srgb(v) as f32 / 255.0;
    k::ExportShape { shape: sh.clone(), name: Some(n.to_string()), color: Some([srgb(c[0]), srgb(c[1]), srgb(c[2])]) }
}

/// The dialog's STEP version and units.
fn step_options(s: &ExportSettings) -> k::StepOptions {
    let schema = if s.latest { k::StepSchema::Ap242 } else { [k::StepSchema::Ap242, k::StepSchema::Ap214, k::StepSchema::Ap203].get(s.version).copied().unwrap_or(k::StepSchema::Ap242) };
    let file_unit = if s.custom_units {
        match UNITS[s.units].0 {
            "Centimeter" => Some(k::LengthUnit::Centimeter),
            "Foot" => Some(k::LengthUnit::Foot),
            "Inch" => Some(k::LengthUnit::Inch),
            "Meter" => Some(k::LengthUnit::Meter),
            "Yard" => Some(k::LengthUnit::Yard),
            _ => Some(k::LengthUnit::Millimeter),
        }
    } else {
        None
    };
    k::StepOptions { schema, file_unit }
}

/// The Part Studio Format list's row `i` ([`PartFormat::list`]).
pub fn set_part_format(s: &mut ExportSettings, i: usize) {
    s.format = PartFormat::list(false)[i];
    s.version = 0;
    s.latest = true;
}

pub fn set_asm_format(s: &mut ExportSettings, i: usize) {
    s.format = PartFormat::list(true)[i];
    s.version = 0;
    s.latest = true;
    // Onshape checks "Include hidden instances in export" for URDF.
    s.hidden = s.format == PartFormat::Urdf;
}

/// The formats whose writer is still to come (kern): Export shows Onshape's
/// flow, then this error.
pub fn missing_writer(f: PartFormat) -> Option<String> {
    let _ = f; // every format has its writer now (kern)
    None
}

fn sanitize(name: &str) -> String {
    name.chars().map(|c| if matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') { '_' } else { c }).collect()
}

/// Chord tolerance (meters) of a mesh resolution, from the parts' size.
fn deflection(s: &ExportSettings, parts: &[(String, k::Shape, [f32; 4])]) -> f64 {
    let diag = parts.iter().filter_map(|p| p.1.bounding_box(false).ok()).map(|b| b.diagonal()).fold(0.0f64, f64::max).max(1e-3);
    let rel = match s.resolution {
        0 => 1e-3,
        2 | 3 => 5e-5,
        _ => 2e-4,
    };
    diag * rel
}

fn write_parts(s: &ExportSettings, path: &std::path::Path, parts: &[(String, k::Shape, [f32; 4])]) -> anyhow::Result<()> {
    match s.format {
        PartFormat::Step => {
            let shapes: Vec<k::ExportShape> = parts.iter().map(|(n, sh, c)| step_shape(n, sh, c)).collect();
            k::write_step_with(path, &shapes, k::LengthUnit::Meter, &step_options(s))?;
        }
        PartFormat::Iges => k::write_iges(path, &parts.iter().map(|p| p.1.clone()).collect::<Vec<_>>(), k::LengthUnit::Meter)?,
        // Text transmit file in the chosen version (live G36: the body in that
        // version's layouts; "Use latest version" = 38.1).
        PartFormat::Parasolid => {
            let version = if s.latest { PARASOLID_VERSIONS[0] } else { PARASOLID_VERSIONS.get(s.version).copied().unwrap_or(PARASOLID_VERSIONS[0]) };
            k::write_parasolid_version(path, &parts.iter().map(|p| p.1.clone()).collect::<Vec<_>>(), k::LengthUnit::Meter, false, version)?
        }
        PartFormat::Acis => k::write_acis(path, &parts.iter().map(|p| p.1.clone()).collect::<Vec<_>>(), k::LengthUnit::Meter)?,
        PartFormat::Pvz => {
            let shapes: Vec<k::ExportShape> = parts.iter().map(|(n, sh, _)| k::ExportShape { shape: sh.clone(), name: Some(n.clone()), color: None }).collect();
            k::write_pvz(path, &shapes, k::LengthUnit::Meter)?;
        }
        PartFormat::Jt => {
            let shapes: Vec<k::ExportShape> = parts.iter().map(|(n, sh, _)| k::ExportShape { shape: sh.clone(), name: Some(n.clone()), color: None }).collect();
            k::write_jt(path, &shapes, k::LengthUnit::Meter)?;
        }
        PartFormat::Rhino => {
            let shapes: Vec<k::ExportShape> = parts.iter().map(|(n, sh, _)| k::ExportShape { shape: sh.clone(), name: Some(n.clone()), color: None }).collect();
            let version = if s.latest { 8 } else { RHINO_VERSIONS.get(s.version).and_then(|v| v.parse().ok()).unwrap_or(8) };
            k::write_rhino(path, &shapes, k::LengthUnit::Meter, version)?;
        }
        f if f.mesh() => {
            let defl = deflection(s, parts);
            let meshes: Vec<(String, k::Tessellation, [f32; 4])> = parts.iter().map(|(n, sh, c)| Ok((n.clone(), k::tessellate(sh, defl, 0.35, true)?, *c))).collect::<anyhow::Result<_>>()?;
            match f {
                PartFormat::Stl | PartFormat::Obj => {
                    // STL: in the chosen unit; OBJ: meters.
                    let scale = if f == PartFormat::Stl { 1.0 / UNITS[s.units].1 } else { 1.0 } as f32;
                    let mut all = k::Tessellation::default();
                    for (_, m, _) in &meshes {
                        let base = all.positions.len() as u32;
                        let first = all.indices.len() as u32;
                        all.faces.extend(m.faces.iter().map(|r| k::FaceRange { first_index: r.first_index + first, index_count: r.index_count, first_vertex: r.first_vertex + base, vertex_count: r.vertex_count }));
                        all.positions.extend(m.positions.iter().map(|p| [p[0] * scale, p[1] * scale, p[2] * scale]));
                        all.normals.extend_from_slice(&m.normals);
                        all.indices.extend(m.indices.iter().map(|i| i + base));
                    }
                    if f == PartFormat::Stl {
                        k::write_stl_mesh(&all, path, if s.stl_text { k::StlFormat::Ascii } else { k::StlFormat::Binary })?;
                    } else {
                        k::write_obj_mesh(&all, path)?;
                    }
                }
                PartFormat::Gltf | PartFormat::Glb => std::fs::write(path, gltf(&meshes, f == PartFormat::Glb))?,
                _ => std::fs::write(path, three_mf(&meshes))?,
            }
        }
        f => anyhow::bail!("{} export is not available in noshape yet", f.label()),
    }
    Ok(())
}

/// glTF 2.0: one mesh node per part (positions, normals, indices) with its
/// colour as a PBR material; `.glb` packs JSON and buffer into one file,
/// `.gltf` embeds the buffer as a data URI.
fn gltf(meshes: &[(String, k::Tessellation, [f32; 4])], binary: bool) -> Vec<u8> {
    let mut buf: Vec<u8> = Vec::new();
    let (mut views, mut accessors, mut materials, mut gmeshes, mut nodes) = (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for (i, (name, m, c)) in meshes.iter().enumerate() {
        let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
        for p in &m.positions {
            for k in 0..3 {
                lo[k] = lo[k].min(p[k]);
                hi[k] = hi[k].max(p[k]);
            }
        }
        let mut push = |bytes: &[u8], target: u32| {
            while !buf.len().is_multiple_of(4) {
                buf.push(0);
            }
            let off = buf.len();
            buf.extend_from_slice(bytes);
            views.push(serde_json::json!({"buffer": 0, "byteOffset": off, "byteLength": bytes.len(), "target": target}));
            views.len() - 1
        };
        let pos: Vec<u8> = m.positions.iter().flatten().flat_map(|v| v.to_le_bytes()).collect();
        let nor: Vec<u8> = m.normals.iter().flatten().flat_map(|v| v.to_le_bytes()).collect();
        let idx: Vec<u8> = m.indices.iter().flat_map(|v| v.to_le_bytes()).collect();
        let (vp, vn, vi) = (push(&pos, 34962), push(&nor, 34962), push(&idx, 34963));
        let a0 = accessors.len();
        accessors.push(serde_json::json!({"bufferView": vp, "componentType": 5126, "count": m.positions.len(), "type": "VEC3", "min": lo, "max": hi}));
        accessors.push(serde_json::json!({"bufferView": vn, "componentType": 5126, "count": m.normals.len(), "type": "VEC3"}));
        accessors.push(serde_json::json!({"bufferView": vi, "componentType": 5125, "count": m.indices.len(), "type": "SCALAR"}));
        materials.push(serde_json::json!({"name": name, "pbrMetallicRoughness": {"baseColorFactor": c, "metallicFactor": 0.0, "roughnessFactor": 0.5}}));
        gmeshes.push(serde_json::json!({"name": name, "primitives": [{"attributes": {"POSITION": a0, "NORMAL": a0 + 1}, "indices": a0 + 2, "material": i}]}));
        nodes.push(serde_json::json!({"name": name, "mesh": i}));
    }
    while !buf.len().is_multiple_of(4) {
        buf.push(0);
    }
    let mut buffer = serde_json::json!({"byteLength": buf.len()});
    if !binary {
        buffer["uri"] = serde_json::Value::String(format!("data:application/octet-stream;base64,{}", base64(&buf)));
    }
    let doc = serde_json::json!({
        "asset": {"version": "2.0", "generator": "noshape"},
        "scene": 0,
        "scenes": [{"nodes": (0..nodes.len()).collect::<Vec<_>>()}],
        "nodes": nodes,
        "meshes": gmeshes,
        "materials": materials,
        "accessors": accessors,
        "bufferViews": views,
        "buffers": [buffer],
    });
    let mut json = serde_json::to_vec(&doc).unwrap_or_default();
    if !binary {
        return json;
    }
    while !json.len().is_multiple_of(4) {
        json.push(b' ');
    }
    let total = 12 + 8 + json.len() + 8 + buf.len();
    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(b"glTF");
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&(total as u32).to_le_bytes());
    out.extend_from_slice(&(json.len() as u32).to_le_bytes());
    out.extend_from_slice(b"JSON");
    out.extend_from_slice(&json);
    out.extend_from_slice(&(buf.len() as u32).to_le_bytes());
    out.extend_from_slice(b"BIN\0");
    out.extend_from_slice(&buf);
    out
}

/// 3MF: the core-spec package (a zip with the content types, the root
/// relationship and the model XML, in millimeters), stored uncompressed.
fn three_mf(meshes: &[(String, k::Tessellation, [f32; 4])]) -> Vec<u8> {
    use std::fmt::Write as _;
    let mut model = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<model unit=\"millimeter\" xml:lang=\"en-US\" xmlns=\"http://schemas.microsoft.com/3dmanufacturing/core/2015/02\">\n<resources>\n<basematerials id=\"1\">\n");
    for (name, _, c) in meshes {
        let srgb = |v: f32| (v.max(0.0).powf(1.0 / 2.2) * 255.0).round() as u8;
        let _ = writeln!(model, "<base name=\"{}\" displaycolor=\"#{:02X}{:02X}{:02X}\"/>", xml_escape(name), srgb(c[0]), srgb(c[1]), srgb(c[2]));
    }
    model.push_str("</basematerials>\n");
    for (i, (name, m, _)) in meshes.iter().enumerate() {
        let _ = writeln!(model, "<object id=\"{}\" name=\"{}\" type=\"model\" pid=\"1\" pindex=\"{i}\"><mesh><vertices>", i + 2, xml_escape(name));
        // 3MF Core: a manifold mesh by index (each edge in two triangles):
        // the faces' vertices welded.
        let (positions, remap) = m.welded_positions();
        for p in &positions {
            let _ = writeln!(model, "<vertex x=\"{}\" y=\"{}\" z=\"{}\"/>", p[0] * 1000.0, p[1] * 1000.0, p[2] * 1000.0);
        }
        model.push_str("</vertices><triangles>\n");
        for t in m.welded_triangles(&remap) {
            let _ = writeln!(model, "<triangle v1=\"{}\" v2=\"{}\" v3=\"{}\"/>", t[0], t[1], t[2]);
        }
        model.push_str("</triangles></mesh></object>\n");
    }
    model.push_str("</resources>\n<build>\n");
    for i in 0..meshes.len() {
        let _ = writeln!(model, "<item objectid=\"{}\"/>", i + 2);
    }
    model.push_str("</build>\n</model>\n");
    let types = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\"><Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/><Default Extension=\"model\" ContentType=\"application/vnd.ms-package.3dmanufacturing-3dmodel+xml\"/></Types>";
    let rels = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Target=\"/3D/3dmodel.model\" Id=\"rel0\" Type=\"http://schemas.microsoft.com/3dmanufacturing/2013/01/3dmodel\"/></Relationships>";
    stored_zip(&[("[Content_Types].xml", types.as_bytes()), ("_rels/.rels", rels.as_bytes()), ("3D/3dmodel.model", model.as_bytes())])
}

fn base64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for c in data.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        for (i, shift) in [18, 12, 6, 0].into_iter().enumerate() {
            out.push(if i <= c.len() { T[(n >> shift & 63) as usize] as char } else { '=' });
        }
    }
    out
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// A zip archive with the files stored (no compression).
fn stored_zip(files: &[(&str, &[u8])]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut central = Vec::new();
    for (name, data) in files {
        let crc = crc32(data);
        let off = out.len() as u32;
        let header = |sig: u32, central_dir: bool| {
            let mut h = Vec::new();
            h.extend_from_slice(&sig.to_le_bytes());
            if central_dir {
                h.extend_from_slice(&20u16.to_le_bytes());
            }
            h.extend_from_slice(&20u16.to_le_bytes()); // version needed
            h.extend_from_slice(&0u16.to_le_bytes()); // flags
            h.extend_from_slice(&0u16.to_le_bytes()); // stored
            h.extend_from_slice(&0u32.to_le_bytes()); // time, date
            h.extend_from_slice(&crc.to_le_bytes());
            h.extend_from_slice(&(data.len() as u32).to_le_bytes());
            h.extend_from_slice(&(data.len() as u32).to_le_bytes());
            h.extend_from_slice(&(name.len() as u16).to_le_bytes());
            h.extend_from_slice(&0u16.to_le_bytes()); // extra
            if central_dir {
                h.extend_from_slice(&[0u8; 6]); // comment length, disk, internal attributes
                h.extend_from_slice(&0u32.to_le_bytes()); // external attributes
                h.extend_from_slice(&off.to_le_bytes());
            }
            h.extend_from_slice(name.as_bytes());
            h
        };
        out.extend(header(0x0403_4b50, false));
        out.extend_from_slice(data);
        central.extend(header(0x0201_4b50, true));
    }
    let cd_off = out.len() as u32;
    out.extend_from_slice(&central);
    out.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
    out.extend_from_slice(&[0u8; 4]);
    out.extend_from_slice(&(files.len() as u16).to_le_bytes());
    out.extend_from_slice(&(files.len() as u16).to_le_bytes());
    out.extend_from_slice(&(central.len() as u32).to_le_bytes());
    out.extend_from_slice(&cd_off.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xffff_ffffu32;
    for &b in data {
        crc ^= b as u32;
        for _ in 0..8 {
            crc = if crc & 1 != 0 { (crc >> 1) ^ 0xedb8_8320 } else { crc >> 1 };
        }
    }
    !crc
}

/// Export Options › "Store file in a new tab" (and "Download and store"):
/// the exported file becomes a file tab directly after the exported tab
/// (live: "Part 1 Drawing 1.dwt" right after the drawing; the exported tab
/// stays active). One edit, [`store_description`].
pub fn store_file_tab(d: &mut noshape_model::document::Document, file_name: &str, bytes: &[u8], after: noshape_model::document::TabId) -> noshape_model::document::TabId {
    crate::import::add_file_tab(d, noshape_model::file_tab::FileTab::new(file_name, bytes, noshape_model::file_tab::now()), None, Some(after))
}

/// The history entry of [`store_file_tab`]: "Store <file>".
pub fn store_description(file_name: &str) -> String {
    format!("Store {file_name}")
}

#[cfg(test)]
mod tests {
    #[test]
    fn crc32_matches_reference() {
        assert_eq!(super::crc32(b"123456789"), 0xcbf4_3926);
    }

    #[test]
    fn base64_pads() {
        assert_eq!(super::base64(b"ab"), "YWI=");
        assert_eq!(super::base64(b"abc"), "YWJj");
        assert_eq!(super::base64(b"a"), "YQ==");
    }

    /// qa EXPORT-MESH-SHARED: a 3MF body is one closed mesh by index (every
    /// edge in exactly two triangles, once each way), so slicers find no
    /// open edges; the faces' shared points are one vertex.
    #[test]
    fn three_mf_bodies_are_manifold_by_index() {
        use noshape_kernel as k;
        use std::collections::HashMap;
        let block = k::make_box(&k::Plane::xy(), 0.04, 0.03, 0.01).unwrap();
        let pin = k::make_cylinder(&k::Plane::new(glam::DVec3::new(0.02, 0.015, -0.01), glam::DVec3::Z, glam::DVec3::X), 0.005, 0.03, std::f64::consts::TAU).unwrap();
        let part = k::cut(&block, &pin).unwrap().shape;
        let m = k::tessellate(&part, 0.0001, 0.35, true).unwrap();
        let xml = String::from_utf8_lossy(&super::three_mf(&[("Part 1".into(), m.clone(), [0.5, 0.5, 0.5, 1.0])])).to_string();
        let attr = |l: &str, k: &str| -> u32 { l.split(&format!("{k}=\"")).nth(1).unwrap().split('"').next().unwrap().parse().unwrap() };
        let tris: Vec<[u32; 3]> = xml.lines().filter(|l| l.starts_with("<triangle ")).map(|l| [attr(l, "v1"), attr(l, "v2"), attr(l, "v3")]).collect();
        let verts = xml.lines().filter(|l| l.starts_with("<vertex ")).count();
        assert!(verts < m.positions.len(), "welded: {verts} of {}", m.positions.len());
        let mut directed: HashMap<(u32, u32), u32> = HashMap::new();
        for t in &tris {
            for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
                *directed.entry((a, b)).or_default() += 1;
            }
        }
        let bad: Vec<_> = directed.iter().filter(|((a, b), n)| **n != 1 || directed.get(&(*b, *a)) != Some(&1)).collect();
        assert!(bad.is_empty(), "{} of {} directed edges unmatched", bad.len(), directed.len());
    }

    /// qa RHINO-GREEN: RHINO is offered only by a build that can write it;
    /// the dialog's rows and the formats stay aligned either way.
    #[test]
    fn rhino_is_listed_only_when_built_in() {
        use super::PartFormat;
        let list = PartFormat::list(false);
        assert_eq!(list.contains(&PartFormat::Rhino), noshape_kernel::RHINO_EXPORT);
        for (i, f) in list.iter().enumerate() {
            let mut s = super::ExportSettings::default();
            super::set_part_format(&mut s, i);
            assert_eq!(s.format, *f);
        }
    }

    #[test]
    fn stored_zip_has_end_record() {
        let z = super::stored_zip(&[("a.txt", b"hello")]);
        assert_eq!(&z[..4], &[0x50, 0x4b, 0x03, 0x04]);
        assert_eq!(&z[z.len() - 22..z.len() - 18], &[0x50, 0x4b, 0x05, 0x06]);
        // Local header 30 + name + data, central entry 46 + name, end record 22.
        assert_eq!(z.len(), 30 + 5 + 5 + 46 + 5 + 22);
    }
}
